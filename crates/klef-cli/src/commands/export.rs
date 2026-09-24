use crate::cli::ExportFormat;
use crate::outln;
use klef_core::error::KlefError;
use klef_core::store::{Store, validate_env_var};

/// Export key values as shell or dotenv formatted lines.
///
/// Revalidates `env_var` at render time so a legacy index from a
/// pre-validation klef install can't smuggle a shell-injection payload
/// through `eval "$(klef export ...)"`.
///
/// # Errors
///
/// `KeyNotFound`, or `InvalidEnvVar` if a stored env-var fails the
/// POSIX-identifier check (legacy index).
pub fn run(store: &Store, names: &[String], format: ExportFormat) -> Result<(), KlefError> {
    for name in names {
        let value = store.get_value(name)?;
        let meta = store.meta(name)?;
        validate_env_var(&meta.env_var)?;
        let line = render_line(&meta.env_var, &value, format)?;
        outln!("{line}");
    }
    Ok(())
}

fn render_line(var: &str, value: &str, format: ExportFormat) -> Result<String, KlefError> {
    Ok(match format {
        ExportFormat::Shell => format!("export {var}={}", shell_escape(value)),
        ExportFormat::Dotenv => format!("{var}={}", dotenv_quote(var, value)?),
    })
}

fn is_bare_safe(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@'))
}

/// Quote for a dotenv reader, which has no escape syntax: wrap in whichever
/// quote the value doesn't contain. Values holding both quotes, or a newline,
/// have no faithful single-line form — refuse rather than emit a corrupt line.
fn dotenv_quote(var: &str, value: &str) -> Result<String, KlefError> {
    if is_bare_safe(value) {
        return Ok(value.to_string());
    }
    let quote = if value.contains('\n') || value.contains('\r') {
        None
    } else if !value.contains('\'') {
        Some('\'')
    } else if !value.contains('"') {
        Some('"')
    } else {
        None
    };
    quote.map(|q| format!("{q}{value}{q}")).ok_or_else(|| {
        KlefError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "value of {var} has a newline or both quote kinds and can't be written \
                 as a dotenv line; use --format shell"
            ),
        ))
    })
}

fn shell_escape(value: &str) -> String {
    if is_bare_safe(value) {
        value.to_string()
    } else {
        let escaped = value.replace('\'', "'\\''");
        format!("'{escaped}'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_value_unquoted() {
        assert_eq!(shell_escape("sk_live_abc"), "sk_live_abc");
    }

    #[test]
    fn value_with_space_quoted() {
        assert_eq!(shell_escape("a b"), "'a b'");
    }

    #[test]
    fn value_with_single_quote_escaped() {
        assert_eq!(shell_escape("a'b"), "'a'\\''b'");
    }

    #[test]
    fn shell_format_emits_export() {
        assert_eq!(
            render_line("X", "v", ExportFormat::Shell).unwrap(),
            "export X=v"
        );
    }

    #[test]
    fn dotenv_format_omits_export() {
        assert_eq!(render_line("X", "v", ExportFormat::Dotenv).unwrap(), "X=v");
    }

    /// `'\''` is shell syntax; dotenv readers (klef's own included) used to
    /// read `it's a b#c` back as `it`.
    #[test]
    fn dotenv_round_trips_through_klef_parser() {
        use klef_core::envfile::{Value, parse_str};
        for v in [
            "plain",
            "it's a b#c",
            "say \"hi\" # x",
            "a b",
            "",
            " lead",
            "klef:x",
        ] {
            let line = render_line("X", v, ExportFormat::Dotenv).unwrap();
            let parsed = parse_str(&line);
            assert_eq!(parsed.len(), 1, "{line}");
            let got = match &parsed[0].value {
                Value::Literal(s) => s.clone(),
                Value::Reference(n) => format!("klef:{n}"),
            };
            assert_eq!(got, v, "{line}");
        }
    }

    #[test]
    fn dotenv_refuses_values_it_cannot_represent() {
        assert!(render_line("X", "a'b\"c", ExportFormat::Dotenv).is_err());
        assert!(render_line("X", "line1\nline2", ExportFormat::Dotenv).is_err());
    }
}
