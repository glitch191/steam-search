//! Minimal parser for Valve's KeyValues text format (`.vdf` and `.acf` files).
//!
//! Supports quoted and unquoted tokens, `\\` escapes, nested `{}` blocks and
//! `//` line comments. That covers `libraryfolders.vdf` and `appmanifest_*.acf`.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Obj(Vec<(String, Value)>),
}

impl Value {
    /// Case-insensitive lookup of a direct child.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(items) => items
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Value::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Str(s) => Some(s),
            Value::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(items) => items,
            Value::Str(_) => &[],
        }
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Str(String),
    Open,
    Close,
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '{' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' => {
                chars.next();
                tokens.push(Token::Close);
            }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    return Err("unexpected '/'".into());
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => return Err("unterminated escape".into()),
                        },
                        Some(other) => s.push(other),
                        None => return Err("unterminated string".into()),
                    }
                }
                tokens.push(Token::Str(s));
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(Token::Str(s));
            }
        }
    }
    Ok(tokens)
}

/// Parses a document into a root object holding its top-level keys.
pub fn parse(text: &str) -> Result<Value, String> {
    let tokens = tokenize(text)?;
    let mut pos = 0;
    let root = parse_items(&tokens, &mut pos, true)?;
    Ok(Value::Obj(root))
}

fn parse_items(tokens: &[Token], pos: &mut usize, top: bool) -> Result<Vec<(String, Value)>, String> {
    let mut items = Vec::new();
    loop {
        match tokens.get(*pos) {
            None if top => return Ok(items),
            None => return Err("missing '}'".into()),
            Some(Token::Close) if !top => {
                *pos += 1;
                return Ok(items);
            }
            Some(Token::Str(key)) => {
                *pos += 1;
                let value = match tokens.get(*pos) {
                    Some(Token::Str(v)) => {
                        *pos += 1;
                        Value::Str(v.clone())
                    }
                    Some(Token::Open) => {
                        *pos += 1;
                        Value::Obj(parse_items(tokens, pos, false)?)
                    }
                    _ => return Err(format!("missing value for key '{key}'")),
                };
                items.push((key.clone(), value));
            }
            Some(_) => return Err("unexpected brace".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_and_escapes() {
        let text = r#"
            // comment
            "libraryfolders"
            {
                "0"
                {
                    "path"  "C:\\Program Files (x86)\\Steam"
                    "apps" { "10" "123" }
                }
            }
        "#;
        let root = parse(text).unwrap();
        let lib = root.get("LibraryFolders").unwrap().get("0").unwrap();
        assert_eq!(lib.str("path"), Some(r"C:\Program Files (x86)\Steam"));
        assert_eq!(lib.get("apps").unwrap().str("10"), Some("123"));
    }

    #[test]
    fn rejects_unbalanced_braces() {
        assert!(parse(r#""a" { "b" "c""#).is_err());
        assert!(parse(r#""a" "b" }"#).is_err());
    }
}
