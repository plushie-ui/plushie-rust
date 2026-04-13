//! `.plushie` automation file parser.
//!
//! Parses `.plushie` files into a header and a list of instructions.
//! The file format consists of a header section (key-value pairs)
//! followed by a `-----` separator and instruction lines.
//!
//! ```text
//! app: Counter
//! viewport: 800x600
//! -----
//! click "#inc"
//! assert_text "#count" "Count: 1"
//! ```

use std::collections::HashMap;

/// Parsed header from a `.plushie` file.
#[derive(Debug, Clone, Default)]
pub struct Header {
    /// Key-value pairs from the header section.
    pub fields: HashMap<String, String>,
}

impl Header {
    /// Get the app module name.
    pub fn app(&self) -> Option<&str> {
        self.fields.get("app").map(|s| s.as_str())
    }

    /// Get the viewport dimensions (width, height).
    pub fn viewport(&self) -> (u32, u32) {
        self.fields
            .get("viewport")
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((800, 600))
    }

    /// Get the backend name.
    pub fn backend(&self) -> &str {
        self.fields
            .get("backend")
            .map(|s| s.as_str())
            .unwrap_or("mock")
    }
}

/// A single instruction from a `.plushie` file.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Click(String),
    TypeText(String, String),
    TypeKey(String),
    Press(String),
    Release(String),
    Toggle(String, Option<bool>),
    Select(String, String),
    Slide(String, f64),
    MoveTo(f32, f32),
    MoveToSelector(String),
    Scroll(String, f32, f32),
    Wait(u64),

    // Assertions
    Expect(String),
    AssertText(String, String),
    AssertExists(String),
    AssertNotExists(String),

    // Capture
    Screenshot(String),
    TreeHash(String),
}

/// A parsed `.plushie` file.
#[derive(Debug, Clone)]
pub struct PlushieFile {
    pub header: Header,
    pub instructions: Vec<(usize, Instruction)>,
}

/// Parse a `.plushie` file from its text content.
///
/// Returns the parsed header and instructions with line numbers
/// for error reporting.
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
            header
                .fields
                .insert(key.trim().to_string(), value.trim().to_string());
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
        let tokens = tokenize(trimmed);
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
pub fn parse_file(path: &str) -> Result<PlushieFile, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    parse(&content)
}

/// Tokenize an instruction line.
///
/// Supports quoted strings (`"hello world"`) and bare tokens.
fn tokenize(line: &str) -> Vec<String> {
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
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                token.push(c);
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

    tokens
}

fn parse_instruction(tokens: &[String]) -> Result<Instruction, String> {
    let cmd = tokens[0].as_str();
    let args = &tokens[1..];

    match cmd {
        "click" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::Click(args[0].clone()))
        }
        "type" => {
            if args.len() == 1 {
                // Special key: type enter, type escape, etc.
                Ok(Instruction::TypeKey(args[0].clone()))
            } else if args.len() >= 2 {
                Ok(Instruction::TypeText(args[0].clone(), args[1].clone()))
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
            let value = args.get(1).map(|v| v == "true");
            Ok(Instruction::Toggle(args[0].clone(), value))
        }
        "select" => {
            require_args(cmd, args, 2)?;
            Ok(Instruction::Select(args[0].clone(), args[1].clone()))
        }
        "slide" => {
            require_args(cmd, args, 2)?;
            let value: f64 = args[1]
                .parse()
                .map_err(|_| format!("slide: invalid number '{}'", args[1]))?;
            Ok(Instruction::Slide(args[0].clone(), value))
        }
        "scroll" => {
            require_args(cmd, args, 3)?;
            let dx: f32 = args[1]
                .parse()
                .map_err(|_| format!("scroll: invalid dx '{}'", args[1]))?;
            let dy: f32 = args[2]
                .parse()
                .map_err(|_| format!("scroll: invalid dy '{}'", args[2]))?;
            Ok(Instruction::Scroll(args[0].clone(), dx, dy))
        }
        "move" => {
            require_args(cmd, args, 1)?;
            // Check for X,Y coordinate format.
            if let Some((x_str, y_str)) = args[0].split_once(',') {
                let x: f32 = x_str
                    .parse()
                    .map_err(|_| format!("move: invalid x '{x_str}'"))?;
                let y: f32 = y_str
                    .parse()
                    .map_err(|_| format!("move: invalid y '{y_str}'"))?;
                Ok(Instruction::MoveTo(x, y))
            } else {
                Ok(Instruction::MoveToSelector(args[0].clone()))
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
            Ok(Instruction::AssertText(args[0].clone(), args[1].clone()))
        }
        "assert_exists" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::AssertExists(args[0].clone()))
        }
        "assert_not_exists" => {
            require_args(cmd, args, 1)?;
            Ok(Instruction::AssertNotExists(args[0].clone()))
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
        assert_eq!(file.header.app(), Some("Counter"));
        assert_eq!(file.header.viewport(), (800, 600));
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
        let tokens = tokenize("click \"#my btn\"");
        assert_eq!(tokens, vec!["click", "#my btn"]);
    }

    #[test]
    fn tokenize_multiple_args() {
        let tokens = tokenize("type \"#input\" \"hello world\"");
        assert_eq!(tokens, vec!["type", "#input", "hello world"]);
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
            Instruction::Toggle("#cb".into(), Some(true))
        );
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
            Instruction::Slide("#vol".into(), 0.75)
        );
    }

    #[test]
    fn missing_separator_is_error() {
        let result = parse("app: Test\nclick \"#btn\"\n");
        assert!(result.is_err());
    }
}
