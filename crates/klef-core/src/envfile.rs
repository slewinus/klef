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
    content.lines().filter_map(parse_line).collect()
}

fn parse_line(raw: &str) -> Option<Entry> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let body = strip_export_prefix(trimmed);
    let (key, raw_value) = body.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    let value = parse_value(raw_value.trim_start());
    let parsed = value.strip_prefix(REF_PREFIX).map_or_else(
        || Value::Literal(value.clone()),
        |name| Value::Reference(name.to_string()),
    );
    Some(Entry {
        key: key.to_string(),
        value: parsed,
    })
}

/// Strip a leading `export ` (or `export\t`) prefix if present.
/// Only strips if the word is exactly `export` followed by ASCII whitespace —
/// e.g. `exporting=v` is left untouched.
fn strip_export_prefix(line: &str) -> &str {
    if let Some(rest) = line.strip_prefix("export")
        && let Some(first) = rest.chars().next()
        && first.is_ascii_whitespace()
    {
        return rest.trim_start();
    }
    line
}

/// Parse the value portion (everything after the `=`, already `trim_start`-ed).
///
/// - Quoted values: content between the first matching pair of quotes is
///   returned verbatim; anything after the closing quote is ignored.
/// - Unquoted values: a run of whitespace immediately followed by `#` is
///   treated as an inline comment and cut. A `#` with no preceding whitespace
///   is kept as part of the value (e.g. `p#assw0rd`).
fn parse_value(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return String::new();
    }
    let first = bytes[0];
    if first == b'"' || first == b'\'' {
        // Quoted value: find closing quote and return everything between.
        let quote = first as char;
        if let Some(end) = raw[1..].find(quote) {
            return raw[1..=end].to_string();
        }
        // Unmatched opening quote — treat remainder as literal.
        return raw[1..].to_string();
    }
    // Unquoted: cut at first whitespace+# sequence.
    cut_inline_comment(raw)
}

/// Scan an unquoted value for an inline comment (`<whitespace>#…`) and return
/// the value with the comment stripped.
///
/// Rules:
/// - `"a b c"`            → `"a b c"`      (no `#`)
/// - `"a b # comment"`    → `"a b"`        (whitespace before `#`)
/// - `"a # b # c"`        → `"a"`          (first occurrence wins)
/// - `"a#b"`              → `"a#b"`        (no whitespace before `#`, kept)
/// - `"   "`              → `""`           (whitespace only)
fn cut_inline_comment(raw: &str) -> String {
    // Walk character by character tracking whether the previous character was
    // ASCII whitespace. When we hit a `#` AND the preceding character was
    // whitespace, that's the start of an inline comment.
    let mut prev_was_whitespace = false;
    for (i, c) in raw.char_indices() {
        if c == '#' && prev_was_whitespace {
            // Cut back to before the whitespace run that preceded `#`.
            return raw[..i].trim_end().to_string();
        }
        prev_was_whitespace = c.is_ascii_whitespace();
    }
    raw.trim_end().to_string()
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

    // --- Bug #50: export prefix ---

    #[test]
    fn export_prefix_stripped() {
        let entries = parse_str("export STRIPE_KEY=sk_live\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "STRIPE_KEY");
        assert_eq!(entries[0].value, Value::Literal("sk_live".into()));
    }

    #[test]
    fn export_with_tab_separator() {
        let entries = parse_str("export\tSTRIPE=sk\n");
        assert_eq!(entries[0].key, "STRIPE");
    }

    #[test]
    fn export_doesnt_match_word_prefix() {
        // "exportstuff" should not be stripped
        let entries = parse_str("exportstuff=v\n");
        assert_eq!(entries[0].key, "exportstuff");
    }

    // --- Bug #51: inline comments ---

    #[test]
    fn inline_comment_unquoted_stripped() {
        let entries = parse_str("PORT=3000 # default\n");
        assert_eq!(entries[0].value, Value::Literal("3000".into()));
    }

    #[test]
    fn inline_comment_with_tab_before_hash() {
        let entries = parse_str("PORT=3000\t# default\n");
        assert_eq!(entries[0].value, Value::Literal("3000".into()));
    }

    #[test]
    fn hash_inside_value_no_whitespace_kept() {
        let entries = parse_str("PWD=p#assw0rd\n");
        assert_eq!(entries[0].value, Value::Literal("p#assw0rd".into()));
    }

    #[test]
    fn hash_inside_double_quotes_kept() {
        let entries = parse_str("URL=\"http://foo.com#anchor\"\n");
        assert_eq!(
            entries[0].value,
            Value::Literal("http://foo.com#anchor".into())
        );
    }

    #[test]
    fn hash_inside_single_quotes_with_space_kept() {
        let entries = parse_str("MSG='hello # world'\n");
        assert_eq!(entries[0].value, Value::Literal("hello # world".into()));
    }

    #[test]
    fn export_combined_with_inline_comment() {
        let entries = parse_str("export PORT=3000 # default\n");
        assert_eq!(entries[0].key, "PORT");
        assert_eq!(entries[0].value, Value::Literal("3000".into()));
    }

    #[test]
    fn empty_value_after_equals() {
        let entries = parse_str("EMPTY=\n");
        assert_eq!(entries[0].value, Value::Literal(String::new()));
    }
}
