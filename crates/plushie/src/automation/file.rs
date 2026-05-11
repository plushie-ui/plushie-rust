//! `.plushie` automation file parser.
//!
//! Parses `.plushie` files into a typed header and a list of
//! instructions. The file format consists of a header section
//! (key-value pairs) followed by a `-----` separator and instruction
//! lines.
//!
//! ```text
//! app: Counter
//! viewport: 800x600
//! -----
//! click "#inc"
//! assert_text "#count" "Count: 1"
//! ```

use plushie_core::Selector;

/// Parsed header from a `.plushie` file.
///
/// Header fields are validated at parse time. Unknown keys are
/// silently ignored for forward compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Application module name (e.g. `"Counter"`).
    pub app: Option<String>,
    /// Viewport dimensions (width, height). Default: (800, 600).
    pub viewport: (u32, u32),
    /// Renderer backend name. Default: `"mock"`.
    pub backend: String,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            app: None,
            viewport: (800, 600),
            backend: "mock".to_string(),
        }
    }
}

/// A single instruction from a `.plushie` file.
///
/// Selector fields are parsed into [`Selector`] at parse time,
/// providing type-safe widget targeting throughout the automation
/// pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// Click the widget identified by the selector.
    Click(Selector),
    /// Type a string into the targeted text input.
    TypeText(Selector, String),
    /// Press and release the named key (chord syntax accepted).
    TypeKey(String),
    /// Press and hold the named key.
    Press(String),
    /// Release a held key.
    Release(String),
    /// Toggle a boolean widget. `None` flips; `Some(b)` sets to `b`.
    Toggle(Selector, Option<bool>),
    /// Select the named value on a picklist or radio group.
    Select(Selector, String),
    /// Move a slider to the given numeric value.
    Slide(Selector, f64),
    /// Move the pointer to absolute coordinates.
    MoveTo(f32, f32),
    /// Move the pointer over the widget identified by the selector.
    MoveToSelector(Selector),
    /// Scroll by `(dx, dy)` on the targeted widget.
    Scroll(Selector, f32, f32),
    /// Sleep for the given number of milliseconds.
    Wait(u64),

    // Assertions
    /// Assert a condition described by a free-form expression string.
    Expect(String),
    /// Assert the widget's visible text equals the given value.
    AssertText(Selector, String),
    /// Assert that a matching widget exists in the tree.
    AssertExists(Selector),
    /// Assert that no matching widget exists in the tree.
    AssertNotExists(Selector),
    /// Assert that the `{:?}` string of the model contains the
    /// given substring. Parity with Elixir's `assert_model`.
    AssertModel(String),

    // Capture
    /// Capture a screenshot and write it to the given path.
    Screenshot(String),
    /// Record the current tree hash under the given tag.
    TreeHash(String),
}

/// A parsed `.plushie` file.
#[derive(Debug, Clone)]
pub struct PlushieFile {
    /// Parsed header section.
    pub header: Header,
    /// Instructions paired with their source line numbers.
    pub instructions: Vec<(usize, Instruction)>,
}

/// Parse a `.plushie` file from its text content.
///
/// Returns the parsed header and instructions with line numbers
/// for error reporting.
///
/// # Errors
///
/// Returns a descriptive string when the file is missing the
/// `-----` separator between header and body, when a header field
/// has an invalid value (e.g. malformed `viewport`), or when an
/// instruction line cannot be parsed.
pub fn parse(content: &str) -> Result<PlushieFile, String> {
    let mut lines = content.lines().enumerate();
    let mut header = Header::default();
    let mut found_separator = false;

    // Parse header section.
    for (line_no, line) in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("-----") {
            found_separator = true;
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "app" => header.app = Some(value.to_string()),
                "viewport" => {
                    header.viewport = parse_viewport(value)
                        .map_err(|e| format!("line {}: viewport: {e}", line_no + 1))?;
                }
                "backend" => header.backend = value.to_string(),
                _ => {} // Unknown keys ignored for forward compatibility.
            }
        } else {
            return Err(format!(
                "line {}: expected 'key: value' or '-----'",
                line_no + 1
            ));
        }
    }

    if !found_separator {
        return Err("missing '-----' separator between header and instructions".to_string());
    }

    // Parse instruction lines.
    let mut instructions = Vec::new();
    for (line_no, line) in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let tokens = tokenize(trimmed).map_err(|e| format!("line {}: {e}", line_no + 1))?;
        if tokens.is_empty() {
            continue;
        }
        let instr = parse_instruction(&tokens).map_err(|e| format!("line {}: {e}", line_no + 1))?;
        instructions.push((line_no + 1, instr));
    }

    Ok(PlushieFile {
        header,
        instructions,
    })
}

/// Parse a `.plushie` file from a file path.
///
/// # Errors
///
/// Returns a descriptive string when the file cannot be read or
/// when parsing fails (see [`parse`] for parse-time errors).
pub fn parse_file(path: &str) -> Result<PlushieFile, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    parse(&content)
}

/// Minimum viewport dimension accepted by `parse_viewport`.
///
/// Below this size a window would be effectively unusable and some
/// compositors refuse to render at all.
const MIN_VIEWPORT: u32 = 64;

/// Maximum viewport dimension accepted by `parse_viewport`.
///
/// Matches the window-dimension clamp applied by prop validation
/// (well above real displays, below the values that trip GPU drivers
/// or winit on most platforms).
const MAX_VIEWPORT: u32 = 32767;

fn parse_viewport(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| format!("expected 'WxH' format, got '{s}'"))?;
    let w: u32 = w.parse().map_err(|_| format!("invalid width '{w}'"))?;
    let h: u32 = h.parse().map_err(|_| format!("invalid height '{h}'"))?;
    if !(MIN_VIEWPORT..=MAX_VIEWPORT).contains(&w) {
        return Err(format!(
            "viewport width {w} out of range {MIN_VIEWPORT}..={MAX_VIEWPORT}"
        ));
    }
    if !(MIN_VIEWPORT..=MAX_VIEWPORT).contains(&h) {
        return Err(format!(
            "viewport height {h} out of range {MIN_VIEWPORT}..={MAX_VIEWPORT}"
        ));
    }
    Ok((w, h))
}

/// Convert a token string to a Selector.
///
/// .plushie files express selectors as bare or quoted strings.
/// These map to `Selector::Id` via the standard `From<&str>`
/// conversion.
fn sel(s: &str) -> Selector {
    Selector::from(s)
}

/// Tokenize an instruction line.
///
/// Supports quoted strings (`"hello world"`) and bare tokens.
fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '"' {
            chars.next(); // consume opening quote
            let mut token = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                if c == '\\' {
                    match chars.next() {
                        Some('"') => token.push('"'),
                        Some('\\') => token.push('\\'),
                        Some(other) => {
                            token.push('\\');
                            token.push(other);
                        }
                        None => token.push('\\'),
                    }
                    continue;
                }
                if c == '"' {
                    closed = true;
                    break;
                }
                token.push(c);
            }
            if !closed {
                return Err("unterminated quoted string".to_string());
            }
            tokens.push(token);
        } else {
            let mut token = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                token.push(c);
                chars.next();
            }
            tokens.push(token);
        }
    }

    Ok(tokens)
}

fn parse_instruction(tokens: &[String]) -> Result<Instruction, String> {
    let cmd = tokens
        .first()
        .ok_or_else(|| "missing instruction".to_string())?
        .as_str();
    let args = &tokens[1..];

    match cmd {
        "click" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::Click(sel(&args[0])))
        }
        "type" => {
            if args.len() == 1 {
                Ok(Instruction::TypeKey(args[0].clone()))
            } else if args.len() >= 2 {
                Ok(Instruction::TypeText(sel(&args[0]), args[1].clone()))
            } else {
                Err("type requires 1 or 2 arguments".to_string())
            }
        }
        "press" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::Press(args[0].clone()))
        }
        "release" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::Release(args[0].clone()))
        }
        "toggle" => {
            if args.is_empty() {
                return Err("toggle requires at least 1 argument".to_string());
            }
            let value = args
                .get(1)
                .map(|v| match v.as_str() {
                    "true" => Ok(true),
                    "false" => Ok(false),
                    _ => Err(format!("toggle: invalid boolean '{v}'")),
                })
                .transpose()?;
            Ok(Instruction::Toggle(sel(&args[0]), value))
        }
        "select" => {
            require_args(cmd, args, 2)?;
            Ok(Instruction::Select(sel(&args[0]), args[1].clone()))
        }
        "slide" => {
            require_args(cmd, args, 2)?;
            let value: f64 = args[1]
                .parse()
                .map_err(|_| format!("slide: invalid number '{}'", args[1]))?;
            Ok(Instruction::Slide(sel(&args[0]), value))
        }
        "scroll" => {
            require_args(cmd, args, 3)?;
            let dx: f32 = args[1]
                .parse()
                .map_err(|_| format!("scroll: invalid dx '{}'", args[1]))?;
            let dy: f32 = args[2]
                .parse()
                .map_err(|_| format!("scroll: invalid dy '{}'", args[2]))?;
            Ok(Instruction::Scroll(sel(&args[0]), dx, dy))
        }
        "move" => {
            require_args(cmd, args, 1)?;
            if let Some((x_str, y_str)) = args[0].split_once(',') {
                let x: f32 = x_str
                    .parse()
                    .map_err(|_| format!("move: invalid x '{x_str}'"))?;
                let y: f32 = y_str
                    .parse()
                    .map_err(|_| format!("move: invalid y '{y_str}'"))?;
                Ok(Instruction::MoveTo(x, y))
            } else {
                Ok(Instruction::MoveToSelector(sel(&args[0])))
            }
        }
        "wait" => {
            require_args(cmd, args, 1)?;
            let ms: u64 = args[0]
                .parse()
                .map_err(|_| format!("wait: invalid duration '{}'", args[0]))?;
            Ok(Instruction::Wait(ms))
        }
        "expect" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::Expect(args[0].clone()))
        }
        "assert_text" => {
            require_args(cmd, args, 2)?;
            Ok(Instruction::AssertText(sel(&args[0]), args[1].clone()))
        }
        "assert_exists" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::AssertExists(sel(&args[0])))
        }
        "assert_not_exists" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::AssertNotExists(sel(&args[0])))
        }
        "assert_model" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::AssertModel(args[0].clone()))
        }
        "screenshot" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::Screenshot(args[0].clone()))
        }
        "tree_hash" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::TreeHash(args[0].clone()))
        }
        _ => Err(format!("unknown instruction '{cmd}'")),
    }
}

fn require_args(cmd: &str, args: &[String], n: usize) -> Result<(), String> {
    if args.len() < n {
        Err(format!(
            "{cmd} requires {n} argument(s), got {}",
            args.len()
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_and_instructions() {
        let content = "app: Counter\nviewport: 800x600\n-----\nclick \"#inc\"\nassert_text \"#count\" \"1\"\n";
        let file = parse(content).unwrap();
        assert_eq!(file.header.app.as_deref(), Some("Counter"));
        assert_eq!(file.header.viewport, (800, 600));
        assert_eq!(file.instructions.len(), 2);
    }

    #[test]
    fn parse_ignores_comments_and_blanks() {
        let content = "app: Test\n-----\n# comment\n\nclick \"#btn\"\n";
        let file = parse(content).unwrap();
        assert_eq!(file.instructions.len(), 1);
    }

    #[test]
    fn tokenize_quoted_and_bare() {
        let tokens = tokenize("click \"#my btn\"").unwrap();
        assert_eq!(tokens, vec!["click", "#my btn"]);
    }

    #[test]
    fn tokenize_multiple_args() {
        let tokens = tokenize("type \"#input\" \"hello world\"").unwrap();
        assert_eq!(tokens, vec!["type", "#input", "hello world"]);
    }

    #[test]
    fn tokenize_supports_escaped_quotes() {
        let tokens = tokenize("click \"\\\"Save\\\"\"").unwrap();
        assert_eq!(tokens, vec!["click", "\"Save\""]);
    }

    #[test]
    fn unclosed_quote_is_error() {
        let err = parse("app: T\n-----\nclick \"#button\n").unwrap_err();
        assert!(err.contains("unterminated quoted string"));
    }

    #[test]
    fn parse_type_key() {
        let content = "app: T\n-----\ntype enter\n";
        let file = parse(content).unwrap();
        assert_eq!(file.instructions[0].1, Instruction::TypeKey("enter".into()));
    }

    #[test]
    fn parse_toggle_with_value() {
        let content = "app: T\n-----\ntoggle \"#cb\" true\n";
        let file = parse(content).unwrap();
        assert_eq!(
            file.instructions[0].1,
            Instruction::Toggle(Selector::id("#cb"), Some(true))
        );
    }

    #[test]
    fn parse_toggle_false_with_value() {
        let content = "app: T\n-----\ntoggle \"#cb\" false\n";
        let file = parse(content).unwrap();
        assert_eq!(
            file.instructions[0].1,
            Instruction::Toggle(Selector::id("#cb"), Some(false))
        );
    }

    #[test]
    fn parse_toggle_rejects_invalid_boolean() {
        let err = parse("app: T\n-----\ntoggle \"#cb\" yes\n").unwrap_err();
        assert!(err.contains("invalid boolean"));
    }

    #[test]
    fn parse_move_coordinates() {
        let content = "app: T\n-----\nmove 100,200\n";
        let file = parse(content).unwrap();
        assert_eq!(file.instructions[0].1, Instruction::MoveTo(100.0, 200.0));
    }

    #[test]
    fn parse_slide() {
        let content = "app: T\n-----\nslide \"#vol\" 0.75\n";
        let file = parse(content).unwrap();
        assert_eq!(
            file.instructions[0].1,
            Instruction::Slide(Selector::id("#vol"), 0.75)
        );
    }

    #[test]
    fn missing_separator_is_error() {
        let result = parse("app: Test\nclick \"#btn\"\n");
        assert!(result.is_err());
    }

    #[test]
    fn header_defaults() {
        let content = "-----\nclick \"#a\"\n";
        let file = parse(content).unwrap();
        assert_eq!(file.header.app, None);
        assert_eq!(file.header.viewport, (800, 600));
        assert_eq!(file.header.backend, "mock");
    }

    #[test]
    fn invalid_viewport_is_error() {
        let result = parse("viewport: bad\n-----\n");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("viewport"));
    }

    #[test]
    fn viewport_too_small_rejected() {
        let result = parse("viewport: 10x10\n-----\n");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("viewport") && err.contains("out of range"));
    }

    #[test]
    fn viewport_too_large_rejected() {
        let result = parse("viewport: 1000000x100\n-----\n");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("viewport") && err.contains("out of range"));
    }

    #[test]
    fn viewport_in_range_accepted() {
        let file = parse("viewport: 800x600\n-----\n").unwrap();
        assert_eq!(file.header.viewport, (800, 600));
    }

    #[test]
    fn unknown_header_keys_ignored() {
        let content = "app: T\ncustom_key: value\n-----\nclick \"#a\"\n";
        let file = parse(content).unwrap();
        assert_eq!(file.header.app.as_deref(), Some("T"));
    }
}
