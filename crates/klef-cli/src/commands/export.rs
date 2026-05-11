use crate::cli::ExportFormat;
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
        let line = render_line(&meta.env_var, &value, format);
        println!("{line}");
    }
    Ok(())
}

fn render_line(var: &str, value: &str, format: ExportFormat) -> String {
    let escaped = shell_escape(value);
    match format {
        ExportFormat::Shell => format!("export {var}={escaped}"),
        ExportFormat::Dotenv => format!("{var}={escaped}"),
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@'))
    {
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
        assert_eq!(render_line("X", "v", ExportFormat::Shell), "export X=v");
    }

    #[test]
    fn dotenv_format_omits_export() {
        assert_eq!(render_line("X", "v", ExportFormat::Dotenv), "X=v");
    }
}
