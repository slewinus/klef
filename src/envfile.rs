use crate::error::KlefError;
use std::path::Path;

pub const REF_PREFIX: &str = "klef:";

#[derive(Debug, PartialEq, Eq)]
pub enum Value {
    Literal(String),
    Reference(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: Value,
}

/// Parse a `.env` file from the given path.
///
/// # Errors
///
/// Returns `KlefError::EnvFileNotFound` if the file does not exist, or
/// `KlefError::Io` if reading the file fails.
pub fn parse(path: &Path) -> Result<Vec<Entry>, KlefError> {
    if !path.exists() {
        return Err(KlefError::EnvFileNotFound(path.to_path_buf()));
    }
    let content = std::fs::read_to_string(path).map_err(KlefError::Io)?;
    Ok(parse_str(&content))
}

#[must_use]
pub fn parse_str(content: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let value = strip_quotes(v.trim());
        let parsed = value.strip_prefix(REF_PREFIX).map_or_else(
            || Value::Literal(value.to_string()),
            |name| Value::Reference(name.to_string()),
        );
        out.push(Entry { key, value: parsed });
    }
    out
}

fn strip_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_blank_lines_and_comments() {
        let entries = parse_str("# comment\n\nA=1\n");
        assert_eq!(
            entries,
            vec![Entry {
                key: "A".into(),
                value: Value::Literal("1".into())
            }]
        );
    }

    #[test]
    fn detects_klef_reference() {
        let entries = parse_str("STRIPE_KEY=klef:stripe\n");
        assert_eq!(
            entries,
            vec![Entry {
                key: "STRIPE_KEY".into(),
                value: Value::Reference("stripe".into())
            }]
        );
    }

    #[test]
    fn strips_double_quotes() {
        assert_eq!(
            parse_str("A=\"hello\"\n")[0].value,
            Value::Literal("hello".into())
        );
    }

    #[test]
    fn strips_single_quotes() {
        assert_eq!(parse_str("A='hi'\n")[0].value, Value::Literal("hi".into()));
    }

    #[test]
    fn keeps_inner_equals_signs() {
        assert_eq!(
            parse_str("URL=postgres://u:p@h/db\n")[0].value,
            Value::Literal("postgres://u:p@h/db".into())
        );
    }

    #[test]
    fn dash_in_reference_name_ok() {
        assert_eq!(
            parse_str("X=klef:stripe-prod\n")[0].value,
            Value::Reference("stripe-prod".into())
        );
    }

    #[test]
    fn empty_key_skipped() {
        assert!(parse_str("=value\n").is_empty());
    }
}
