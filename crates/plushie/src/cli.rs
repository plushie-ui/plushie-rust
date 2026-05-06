//! Zero-config CLI entry point for plushie apps.
//!
//! There are two ways to wire a plushie app's `main`:
//!
//! 1. **Easy path.** Call [`run`] from your `main` and get the full
//!    `--plushie-*` reserved-flag surface for free: mode selection,
//!    socket attach, automation script / replay, tree inspection, and
//!    `--plushie-help`. Perfect for apps that don't need a custom CLI.
//!
//! 2. **Curated path.** Build your own CLI (clap, lexopt, whatever)
//!    and dispatch directly to the public primitives: [`crate::run`],
//!    `run_connect` and `run_spawn` (both wire-feature only), and
//!    the [`crate::automation::cli`] helpers (`script`, `replay`,
//!    `inspect`). The easy path is a thin wrapper over those.
//!
//! The easy path parses only `--plushie-*` prefixed flags. Any flag
//! with that prefix that isn't recognised is a hard error pointing
//! at `--plushie-help`; everything else is left alone so the user's
//! own argument parser (if they have one) still sees its args.
//!
//! ```ignore
//! fn main() -> plushie::Result {
//!     plushie::cli::run::<MyApp>()
//! }
//! ```
//!
//! # Reserved flags
//!
//! | Flag                      | Effect                                       |
//! |---------------------------|----------------------------------------------|
//! | `--plushie-mode=<mode>`   | Force `direct` or `wire` (see [`crate::run`]).|
//! | `--plushie-socket <path>` | Attach to a listen-mode renderer over socket.|
//! | `--plushie-script <path>` | Run a `.plushie` script via [`script`][1].    |
//! | `--plushie-replay <path>` | Run a `.plushie` script against windowed render. |
//! | `--plushie-inspect`       | Emit pretty-JSON tree snapshot and exit.     |
//! | `--plushie-help`          | Print the reserved-flag summary and exit.    |
//!
//! `--plushie-mode` and `--plushie-socket` are already honored by
//! [`crate::run`]; this module re-parses them only so they appear in
//! the help summary.
//!
//! [1]: crate::automation::cli::script

use crate::{App, Error, Result};

/// Zero-config entry point.
///
/// Parses `std::env::args()` for the `--plushie-*` reserved prefix,
/// dispatches to the matching primitive, and falls through to
/// [`crate::run`] when no reserved flag is present.
///
/// See the [module docs](self) for the full flag list.
///
/// # Errors
///
/// Returns [`Error::InvalidSettings`] when an unknown `--plushie-*`
/// flag is passed or when a recognised flag is missing its value.
/// Otherwise the errors are whatever the dispatched primitive
/// returns (see [`crate::run`], [`crate::automation::cli`]).
pub fn run<A: App>() -> Result {
    let args: Vec<String> = std::env::args().collect();
    match parse(&args)? {
        Action::Help => {
            print_help();
            Ok(())
        }
        Action::Script(path) => crate::automation::cli::script::<A>(&path),
        Action::Replay(path) => crate::automation::cli::replay::<A>(&path),
        Action::Inspect => {
            let snapshot = crate::automation::cli::inspect::<A>()?;
            println!("{snapshot}");
            Ok(())
        }
        Action::Fallthrough => crate::run::<A>(),
    }
}

/// Decoded action from the reserved-flag parser.
#[derive(Debug)]
enum Action {
    /// No `--plushie-*` flag found (other than `--plushie-mode` /
    /// `--plushie-socket`, which [`crate::run`] handles itself).
    Fallthrough,
    /// `--plushie-help` was present.
    Help,
    /// `--plushie-script <path>` or `--plushie-script=<path>`.
    Script(String),
    /// `--plushie-replay <path>` or `--plushie-replay=<path>`.
    Replay(String),
    /// `--plushie-inspect`.
    Inspect,
}

/// Recognise a flag as "known to the easy path".
///
/// `--plushie-mode` and `--plushie-socket` are pass-through: they're
/// consumed by [`crate::run`]'s dispatch, but we still recognise them
/// so we don't accidentally reject them.
fn known_plushie_flag(flag: &str) -> bool {
    matches!(
        flag,
        "--plushie-mode"
            | "--plushie-socket"
            | "--plushie-token"
            | "--plushie-script"
            | "--plushie-replay"
            | "--plushie-inspect"
            | "--plushie-help"
    )
}

fn parse(args: &[String]) -> std::result::Result<Action, Error> {
    let mut i = 1;
    let mut action = Action::Fallthrough;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--plushie-") {
            i += 1;
            continue;
        }

        // Split on `=` once so both `--flag value` and `--flag=value` work.
        let (flag, inline_value) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v.to_string())),
            None => (arg.as_str(), None),
        };

        if !known_plushie_flag(flag) {
            return Err(Error::InvalidSettings(format!(
                "unknown flag `{flag}`; run with --plushie-help for the \
                 reserved-flag list"
            )));
        }

        match flag {
            "--plushie-help" => {
                action = Action::Help;
                break;
            }
            "--plushie-inspect" => {
                action = Action::Inspect;
                break;
            }
            "--plushie-script" => {
                let value = take_value(flag, inline_value, args, &mut i)?;
                action = Action::Script(value);
                break;
            }
            "--plushie-replay" => {
                let value = take_value(flag, inline_value, args, &mut i)?;
                action = Action::Replay(value);
                break;
            }
            // Pass-through: consumed by `plushie::run` itself.
            "--plushie-mode" | "--plushie-socket" | "--plushie-token" => {
                // Advance past the value too so the outer loop doesn't
                // misparse `--plushie-mode wire` as two separate args.
                if inline_value.is_none() {
                    i += 1; // skip the value
                }
            }
            _ => unreachable!("known_plushie_flag lies"),
        }
        i += 1;
    }
    Ok(action)
}

fn take_value(
    flag: &str,
    inline: Option<String>,
    args: &[String],
    i: &mut usize,
) -> std::result::Result<String, Error> {
    if let Some(v) = inline {
        Ok(v)
    } else {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| {
            Error::InvalidSettings(format!("{flag} requires a value (e.g. `{flag} <path>`)"))
        })
    }
}

fn print_help() {
    println!(
        "plushie reserved CLI flags:

  --plushie-help              Print this help text.
  --plushie-mode=<mode>       Force `direct` or `wire` runner selection.
  --plushie-socket <path>     Attach to a listen-mode renderer socket.
  --plushie-token <token>     Token presented during socket handshake.
  --plushie-script <path>     Execute a .plushie automation script.
  --plushie-replay <path>     Replay a .plushie script against a real renderer.
  --plushie-inspect           Print a pretty-JSON snapshot of the initial view tree.

Flags without the `--plushie-` prefix are ignored by this entry point,
so you can still build your own CLI on top (or skip this module entirely
and call plushie::run / plushie::automation::cli directly)."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> std::result::Result<Action, Error> {
        let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        parse(&owned)
    }

    #[test]
    fn no_flags_falls_through() {
        let action = parse_args(&["app"]).unwrap();
        assert!(matches!(action, Action::Fallthrough));
    }

    #[test]
    fn help_flag_recognised() {
        let action = parse_args(&["app", "--plushie-help"]).unwrap();
        assert!(matches!(action, Action::Help));
    }

    #[test]
    fn inspect_flag_recognised() {
        let action = parse_args(&["app", "--plushie-inspect"]).unwrap();
        assert!(matches!(action, Action::Inspect));
    }

    #[test]
    fn script_with_separate_value() {
        let action = parse_args(&["app", "--plushie-script", "foo.plushie"]).unwrap();
        assert!(matches!(action, Action::Script(ref p) if p == "foo.plushie"));
    }

    #[test]
    fn script_with_equals_value() {
        let action = parse_args(&["app", "--plushie-script=bar.plushie"]).unwrap();
        assert!(matches!(action, Action::Script(ref p) if p == "bar.plushie"));
    }

    #[test]
    fn replay_with_separate_value() {
        let action = parse_args(&["app", "--plushie-replay", "x.plushie"]).unwrap();
        assert!(matches!(action, Action::Replay(ref p) if p == "x.plushie"));
    }

    #[test]
    fn unknown_plushie_flag_errors() {
        let err = parse_args(&["app", "--plushie-wombat"]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unknown flag"), "got: {msg}");
        assert!(msg.contains("--plushie-help"), "got: {msg}");
    }

    #[test]
    fn script_missing_value_errors() {
        let err = parse_args(&["app", "--plushie-script"]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("requires a value"), "got: {msg}");
    }

    #[test]
    fn non_plushie_flags_ignored() {
        // User-owned CLI flags should not trip the parser.
        let action = parse_args(&["app", "--verbose", "--count=3"]).unwrap();
        assert!(matches!(action, Action::Fallthrough));
    }

    #[test]
    fn mode_flag_passthrough_with_separate_value() {
        // --plushie-mode is honored by plushie::run, not by this
        // module; we just need to avoid rejecting it.
        let action = parse_args(&["app", "--plushie-mode", "direct"]).unwrap();
        assert!(matches!(action, Action::Fallthrough));
    }

    #[test]
    fn mode_flag_passthrough_with_equals_value() {
        let action = parse_args(&["app", "--plushie-mode=wire"]).unwrap();
        assert!(matches!(action, Action::Fallthrough));
    }

    #[test]
    fn socket_flag_passthrough() {
        let action = parse_args(&["app", "--plushie-socket", "/tmp/sock"]).unwrap();
        assert!(matches!(action, Action::Fallthrough));
    }

    #[test]
    fn inspect_wins_over_later_flags() {
        // Once a terminal action is chosen, stop scanning.
        let action = parse_args(&["app", "--plushie-inspect", "--plushie-wombat"]).unwrap();
        assert!(matches!(action, Action::Inspect));
    }
}
