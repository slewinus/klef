use crate::cli::Cli;
use crate::error::KlefError;
use clap::CommandFactory;
use clap_complete::Shell;

const ZSH_DYNAMIC_HEADER: &str = r#"
# klef dynamic completion: list stored key names by invoking `klef _names`.
_klef_names() {
    local -a names
    names=( ${(f)"$(_call_program klef-names klef _names 2>/dev/null)"} )
    _describe -t klef-names 'klef key' names
}

"#;

/// Print the shell completion script for `shell` to stdout.
///
/// # Errors
///
/// Currently never returns an error; the signature stays consistent with the other commands so
/// future failures (e.g. writing the script through a file handle) can surface uniformly.
pub fn run(shell: Shell) -> Result<(), KlefError> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    if matches!(shell, Shell::Zsh) {
        let mut buf: Vec<u8> = Vec::new();
        clap_complete::generate(shell, &mut cmd, bin_name, &mut buf);
        let raw = String::from_utf8(buf).unwrap_or_default();
        let patched = inject_zsh_dynamic_names(&raw);
        print!("{patched}");
    } else {
        clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    }
    Ok(())
}

/// Inject `_klef_names` and rewrite `clap_complete`'s static positional slots
/// so they call our dynamic helper instead.
///
/// `clap_complete`'s zsh output uses `:name:_default` for unhinted positionals.
/// We replace the key-name slots specifically:
///   - `:name:_default`  — used by get, show, rm, edit, set-note
///   - `:old:_default`   — first positional of rename
///   - `*::names:_default` — variadic positional of export
///
/// Note: `:new:_default` (rename's second positional) is intentionally left
/// as `_default` because it accepts a new name that doesn't exist yet.
/// `:note:_default` (set-note's note argument) is also left unchanged.
/// TODO(#28): Add dynamic completion for bash and fish in a follow-up.
fn inject_zsh_dynamic_names(script: &str) -> String {
    let mut out = String::with_capacity(script.len() + ZSH_DYNAMIC_HEADER.len());

    // Insert the helper function once, right after the `#compdef klef` line.
    if let Some((first_line, rest)) = script.split_once('\n') {
        out.push_str(first_line);
        out.push('\n');
        out.push_str(ZSH_DYNAMIC_HEADER);
        out.push_str(rest);
    } else {
        out.push_str(ZSH_DYNAMIC_HEADER);
        out.push_str(script);
    }

    // Replace positional slots for existing key names.
    // clap_complete uses lowercase field names as-is in the spec strings.
    out = out.replace(":name:_default", ":name:_klef_names");
    out = out.replace(":old:_default", ":old:_klef_names");
    out = out.replace("*::names:_default", "*::names:_klef_names");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_defines_klef_names_function() {
        assert!(ZSH_DYNAMIC_HEADER.contains("_klef_names"));
        assert!(ZSH_DYNAMIC_HEADER.contains("klef _names"));
    }

    #[test]
    fn inject_adds_header_after_compdef() {
        let input = "#compdef klef\n_klef() {\n  echo hi\n}\n";
        let out = inject_zsh_dynamic_names(input);
        assert!(out.starts_with("#compdef klef\n"));
        assert!(out.contains("_klef_names"));
        assert!(out.contains("_klef() {"));
    }

    #[test]
    fn inject_replaces_name_positional() {
        let input = "#compdef klef\n':name:_default'\n";
        let out = inject_zsh_dynamic_names(input);
        assert!(out.contains(":name:_klef_names"), "got: {out}");
        assert!(!out.contains(":name:_default"));
    }

    #[test]
    fn inject_replaces_old_positional() {
        let input = "#compdef klef\n':old:_default'\n':new:_default'\n";
        let out = inject_zsh_dynamic_names(input);
        assert!(out.contains(":old:_klef_names"), "got: {out}");
        assert!(!out.contains(":old:_default"));
        // :new is intentionally left as _default (accepts non-existent names)
        assert!(out.contains(":new:_default"));
    }

    #[test]
    fn inject_replaces_variadic_names_positional() {
        let input = "#compdef klef\n'*::names:_default'\n";
        let out = inject_zsh_dynamic_names(input);
        assert!(out.contains("*::names:_klef_names"), "got: {out}");
        assert!(!out.contains("*::names:_default"));
    }

    #[test]
    fn inject_leaves_note_positional_unchanged() {
        let input = "#compdef klef\n':note:_default'\n";
        let out = inject_zsh_dynamic_names(input);
        assert!(
            out.contains(":note:_default"),
            "note should stay as _default"
        );
        assert!(!out.contains(":note:_klef_names"));
    }
}
