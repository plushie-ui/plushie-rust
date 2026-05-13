//! Renderer entry point. Parses CLI flags, reads the initial Settings
//! message, spawns the stdin reader, and starts the iced daemon.

use iced::{Subscription, Task};
use parking_lot::Mutex;

use plushie_renderer_engine::Codec;
use plushie_widget_sdk::protocol::IncomingMessage;
use plushie_widget_sdk::runtime::{Message, StdinEvent};

use plushie_renderer_lib::App;
use plushie_renderer_lib::emitters::emit_hello;

use super::stdin::{STDIN_RX, spawn_stdin_reader};

fn log_hello_error(err: &std::io::Error) {
    if err.kind() != std::io::ErrorKind::BrokenPipe {
        log::error!("failed to emit hello: {err}");
    }
}

pub(crate) fn run(builder: plushie_widget_sdk::app::PlushieAppBuilder) -> iced::Result {
    let args: Vec<String> = std::env::args().collect();

    // Levelled logging via RUST_LOG. Default: warn (quiet). Use
    // RUST_LOG=plushie_renderer=debug (or =info, =trace) for more output.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    run_inner(builder, args)
}

fn run_inner(
    mut builder: plushie_widget_sdk::app::PlushieAppBuilder,
    args: Vec<String>,
) -> iced::Result {
    let options = match parse_cli(&args) {
        Ok(options) => options,
        Err(e) => {
            log::error!("invalid renderer arguments: {e}");
            return Ok(());
        }
    };

    // Create transport based on flags.
    let transport = if let Some(addr_arg) = options.listen_arg.as_ref() {
        // --listen mode: socket transport.
        let addr = match crate::transport::ListenAddr::parse(addr_arg.as_deref()) {
            Ok(a) => a,
            Err(e) => {
                log::error!("invalid --listen address: {e}");
                return Ok(());
            }
        };
        match crate::transport::Transport::listen(
            &addr,
            options.exec_command.as_ref(),
            &options.extra_exec_env,
        ) {
            Ok(t) => t,
            Err(e) => {
                log::error!("failed to start listen transport: {e}");
                return Ok(());
            }
        }
    } else if let Some(cmd) = &options.exec_command {
        // Exec without --listen: piped stdin/stdout.
        match crate::transport::Transport::exec(cmd, &options.extra_exec_env) {
            Ok(t) => t,
            Err(e) => {
                log::error!("failed to start exec transport: {e}");
                return Ok(());
            }
        }
    } else {
        #[cfg(windows)]
        set_binary_mode();
        crate::transport::Transport::stdio()
    };

    let transport_name = transport.name();
    let expected_token = transport.expected_token.clone();
    let (reader, mut writer_opt, _transport_guard, _token) = {
        let (r, w, g, t) = transport.into_parts();
        (r, Some(w), g, t)
    };

    // Collect custom type names before building the dispatcher so the
    // hello message can report which widget types are available.
    let ext_keys = builder
        .custom_type_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    // Extract the optional session-factory closure before the
    // builder is consumed. Multiplex sessions need it to rebuild
    // a fresh registry per session; single-session and windowed
    // modes ignore it.
    let session_factory = builder.take_session_factory();

    // Headless/mock modes handle their own sink initialization
    // after codec detection. They receive the writer directly.
    if options.mock_mode {
        // Invariant: writer_opt is Some on entry; only one of the
        // --mock / --headless / windowed branches takes it.
        let writer = writer_opt.take().expect("writer available on mock path");
        crate::headless::run(
            options.forced_codec,
            crate::headless::Mode::Mock,
            options.max_sessions,
            &ext_keys,
            transport_name,
            reader,
            writer,
            expected_token.as_deref(),
            session_factory,
        );
        return Ok(());
    }
    if options.headless_mode {
        // Invariant: writer_opt is Some on entry; only one of the
        // --mock / --headless / windowed branches takes it.
        let writer = writer_opt
            .take()
            .expect("writer available on headless path");
        crate::headless::run(
            options.forced_codec,
            crate::headless::Mode::Headless,
            options.max_sessions,
            &ext_keys,
            transport_name,
            reader,
            writer,
            expected_token.as_deref(),
            session_factory,
        );
        return Ok(());
    }

    // Startup handshake: detect codec, send Hello, then read Settings.
    let mut reader = reader;
    // Codec-detection errors have no codec to encode with; log and
    // return so RAII runs (transport sockets, spawned children).
    let codec = match crate::startup::detect_codec(options.forced_codec, &mut reader) {
        Ok(c) => c,
        Err(e) => {
            log::error!("{e}");
            return Ok(());
        }
    };

    // Initialize the global sink for windowed mode now that we know
    // the codec. Use a channel writer to avoid blocking the event loop.
    let writer = writer_opt.take().expect("writer consumed by headless path");
    let channel_writer = crate::output::spawn_writer_thread(writer);
    let sink = plushie_renderer_lib::WriterSink::new(Box::new(channel_writer), codec);
    plushie_renderer_lib::emitters::init_sink(Box::new(sink));

    // Install the renderer panic hook now that the sink is wired.
    // A panic in an iced subscription, window handler, or effect
    // handler emits session_error + session_closed on the wire
    // before the default abort runs, so hosts see a structured
    // signal when the renderer dies unexpectedly.
    plushie_renderer_lib::emitters::install_panic_hook();

    let ext_key_refs: Vec<&str> = ext_keys.iter().map(|s| s.as_str()).collect();
    if let Err(e) = emit_hello("windowed", "wgpu", &ext_key_refs, &["iced"], transport_name) {
        log_hello_error(&e);
        return Ok(());
    }

    let initial = match crate::startup::read_required_settings(&codec, &mut reader) {
        Ok(v) => v,
        Err(e) => {
            crate::startup::emit_startup_error(&codec, &e);
            return Ok(());
        }
    };
    if let Err(e) = crate::startup::validate_settings(
        &initial.settings,
        expected_token.as_deref(),
        &ext_key_refs,
    ) {
        crate::startup::emit_startup_error(&codec, &e);
        return Ok(());
    }
    let iced_settings = plushie_renderer_lib::settings::parse_iced_settings(&initial.settings);
    plushie_renderer_lib::settings::apply_validate_props(&initial.settings);
    let font_bytes = crate::startup::collect_font_bytes(&initial.settings);

    // Spawn stdin reader thread with tokio channel.
    let (tx, rx) = tokio::sync::mpsc::channel::<StdinEvent>(64);
    spawn_stdin_reader(codec, tx, reader);
    // parking_lot::Mutex doesn't poison; a panic in a previous holder
    // leaves the slot intact for the next caller.
    *STDIN_RX.lock() = Some(rx);

    let settings_slot: Mutex<Option<(serde_json::Value, Vec<Vec<u8>>)>> =
        Mutex::new(Some((initial.settings, font_bytes)));
    let builder_slot: Mutex<Option<plushie_widget_sdk::app::PlushieAppBuilder>> =
        Mutex::new(Some(builder));

    iced::daemon(
        move || {
            let (settings, fonts) = settings_slot.lock().take().unwrap_or_default();

            let builder = builder_slot
                .lock()
                .take()
                .expect("daemon init closure called more than once")
                .widget_set(&plushie_widget_sdk::runtime::iced_widget_set());
            let registry = builder.build();

            let effect_handler = Box::new(plushie_renderer_lib::NativeEffectHandler);
            let sink = plushie_renderer_lib::emitters::sink_arc();
            let mut app = App::new(registry, effect_handler, sink);
            app.set_codec(codec);

            // Extract scale_factor before applying settings to Core
            app.scale_factor = plushie_renderer_lib::app::validate_scale_factor(
                settings
                    .get("scale_factor")
                    .and_then(|v| v.as_f64())
                    .map(plushie_widget_sdk::prop_helpers::f64_to_f32)
                    .unwrap_or(1.0),
            );

            // Apply initial settings to Core.
            let effects = app.core.apply(IncomingMessage::Settings { settings });
            for effect in effects {
                use plushie_renderer_engine::{CoreEffect, StateChange};
                match effect {
                    CoreEffect::StateChange(StateChange::WidgetConfig(config)) => {
                        let ctx = plushie_widget_sdk::registry::InitCtx {
                            config: &config,
                            theme: &app.theme,
                            default_text_size: app.core.default_text_size,
                            default_font: app.core.default_font,
                        };
                        app.registry.init_all(&ctx);
                    }
                    other => {
                        log::warn!("unexpected effect from initial Settings: {other:?}");
                    }
                }
            }

            // Build font load tasks
            let font_tasks: Vec<Task<Message>> = fonts
                .into_iter()
                .map(|bytes| {
                    iced::font::load(bytes).map(|result| {
                        if let Err(e) = result {
                            log::error!("font load error: {e:?}");
                        }
                        Message::NoOp
                    })
                })
                .collect();

            let task = if font_tasks.is_empty() {
                Task::none()
            } else {
                Task::batch(font_tasks)
            };

            (app, task)
        },
        App::update,
        App::view_window,
    )
    .title(App::title_for_window)
    .subscription(|app: &App| {
        Subscription::batch([
            app.renderer_subscriptions(),
            Subscription::run(super::stdin::stdin_subscription).map(Message::Stdin),
        ])
    })
    .theme(App::theme_for_window)
    .scale_factor(App::scale_factor_for_window)
    .settings(iced_settings)
    .run()
}

#[derive(Debug, PartialEq, Eq)]
struct CliOptions {
    forced_codec: Option<Codec>,
    max_sessions: usize,
    exec_command: Option<crate::transport::ExecCommand>,
    extra_exec_env: Vec<String>,
    listen_arg: Option<Option<String>>,
    mock_mode: bool,
    headless_mode: bool,
}

fn parse_cli(args: &[String]) -> Result<CliOptions, String> {
    let mut forced_codec = None;
    let mut max_sessions = 1;
    let mut exec_bin = None;
    let mut exec_args = Vec::new();
    let mut extra_exec_env = Vec::new();
    let mut listen_arg = None;
    let mut mock_mode = false;
    let mut headless_mode = false;

    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "--msgpack" => {
                forced_codec = Some(Codec::MsgPack);
                idx += 1;
            }
            "--json" => {
                forced_codec = Some(Codec::Json);
                idx += 1;
            }
            "--mock" => {
                mock_mode = true;
                idx += 1;
            }
            "--headless" => {
                headless_mode = true;
                idx += 1;
            }
            "--max-sessions" => {
                let value = required_arg(args, idx, "--max-sessions")?;
                max_sessions = value.parse::<usize>().unwrap_or(1).max(1);
                idx += 2;
            }
            "--exec" => {
                return Err(
                    "--exec has been removed; use --exec-bin with repeated --exec-arg".to_string(),
                );
            }
            "--exec-bin" => {
                exec_bin = Some(required_arg(args, idx, "--exec-bin")?.to_string());
                idx += 2;
            }
            "--exec-arg" => {
                exec_args.push(required_arg(args, idx, "--exec-arg")?.to_string());
                idx += 2;
            }
            "--exec-env" => {
                extra_exec_env.extend(parse_exec_env_value(required_arg(args, idx, "--exec-env")?));
                idx += 2;
            }
            "--listen" => match args.get(idx + 1) {
                Some(value) if !value.starts_with("--") => {
                    listen_arg = Some(Some(value.clone()));
                    idx += 2;
                }
                _ => {
                    listen_arg = Some(None);
                    idx += 1;
                }
            },
            _ => {
                idx += 1;
            }
        }
    }

    let exec_command = match exec_bin {
        Some(program) if program.is_empty() => {
            return Err("--exec-bin requires a non-empty program".to_string());
        }
        Some(program) => Some(crate::transport::ExecCommand::Argv {
            program,
            args: exec_args,
        }),
        None if !exec_args.is_empty() => {
            return Err("--exec-arg requires --exec-bin".to_string());
        }
        None => None,
    };

    Ok(CliOptions {
        forced_codec,
        max_sessions,
        exec_command,
        extra_exec_env,
        listen_arg,
        mock_mode,
        headless_mode,
    })
}

fn required_arg<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    args.get(idx + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_exec_env_value(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.contains('=') && !name.contains('\0'))
        .map(ToOwned::to_owned)
}

#[cfg(test)]
fn parse_exec_env(args: &[String]) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == "--exec-env")
        .flat_map(|w| parse_exec_env_value(&w[1]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_exec_env_names() {
        assert_eq!(
            parse_exec_env(&args(&[
                "plushie-renderer",
                "--exec-env",
                "MIX_HOME,HEX_HOME",
                "--exec-env",
                "BUNDLE_GEMFILE",
            ])),
            vec![
                "MIX_HOME".to_string(),
                "HEX_HOME".to_string(),
                "BUNDLE_GEMFILE".to_string(),
            ]
        );
    }

    #[test]
    fn ignores_empty_and_assignment_exec_env_entries() {
        assert_eq!(
            parse_exec_env(&args(&[
                "plushie-renderer",
                "--exec-env",
                "MIX_HOME,,BAD=value, HEX_HOME ",
            ])),
            vec!["MIX_HOME".to_string(), "HEX_HOME".to_string()]
        );
    }

    #[test]
    fn parses_structured_exec_command() {
        let parsed = parse_cli(&args(&[
            "plushie-renderer",
            "--exec-bin",
            "/usr/bin/host",
            "--exec-arg",
            "run app",
            "--exec-arg",
            "--flag",
        ]))
        .unwrap();

        assert_eq!(
            parsed.exec_command,
            Some(crate::transport::ExecCommand::Argv {
                program: "/usr/bin/host".to_string(),
                args: vec!["run app".to_string(), "--flag".to_string()],
            })
        );
        assert_eq!(parsed.listen_arg, None);
    }

    #[test]
    fn parses_listen_with_structured_exec_command() {
        let parsed = parse_cli(&args(&[
            "plushie-renderer",
            "--listen",
            ":0",
            "--exec-bin",
            "host",
            "--exec-arg",
            "connect",
        ]))
        .unwrap();

        assert_eq!(parsed.listen_arg, Some(Some(":0".to_string())));
        assert_eq!(
            parsed.exec_command,
            Some(crate::transport::ExecCommand::Argv {
                program: "host".to_string(),
                args: vec!["connect".to_string()],
            })
        );
    }

    #[test]
    fn rejects_removed_shell_exec_form() {
        let err = parse_cli(&args(&[
            "plushie-renderer",
            "--exec",
            "mix plushie.connect MyApp",
        ]))
        .unwrap_err();

        assert!(err.contains("--exec has been removed"));
    }

    #[test]
    fn rejects_exec_arg_without_exec_bin() {
        let err = parse_cli(&args(&["plushie-renderer", "--exec-arg", "connect"])).unwrap_err();

        assert!(err.contains("--exec-arg requires --exec-bin"));
    }
}

/// Switch stdin and stdout to binary mode on Windows.
#[cfg(windows)]
#[allow(unsafe_code)]
fn set_binary_mode() {
    unsafe extern "C" {
        fn _setmode(fd: i32, mode: i32) -> i32;
    }
    const O_BINARY: i32 = 0x8000;

    unsafe {
        _setmode(0, O_BINARY);
        _setmode(1, O_BINARY);
    }
}
