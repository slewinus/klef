use crate::cli::Cli;
use crate::error::KlefError;
use clap::CommandFactory;
use clap_complete::Shell;
use std::fmt::Write as _;

const ZSH_DYNAMIC_HEADER: &str = r#"
# klef dynamic completion: list stored key names by invoking `klef _names`.
_klef_names() {
    local -a names
    names=( ${(f)"$(_call_program klef-names klef _names 2>/dev/null)"} )
    _describe -t klef-names 'klef key' names
}

"#;

const BASH_DYNAMIC_HEADER: &str = "
# klef dynamic completion: emit stored key names from `klef _names`.
_klef_names() {
    klef _names 2>/dev/null
}

";

/// Print the shell completion script for `shell` to stdout.
///
/// # Errors
///
/// Currently never returns an error; the signature stays consistent with the other commands so
/// future failures (e.g. writing the script through a file handle) can surface uniformly.
pub fn run(shell: Shell) -> Result<(), KlefError> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    let mut buf: Vec<u8> = Vec::new();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut buf);
    let raw = String::from_utf8(buf).unwrap_or_default();

    let patched = match shell {
        Shell::Zsh => inject_zsh_dynamic_names(&raw),
        Shell::Bash => inject_bash_dynamic_names(&raw),
        Shell::Fish => inject_fish_dynamic_names(&raw),
        _ => raw,
    };
    print!("{patched}");
    Ok(())
}

/// Inject `_klef_names` helper and rewrite each NAME-taking subcommand block so
/// non-flag completions use `_klef_names`. The `opts=` line and the following
/// `if` block are replaced together as one atomic unit (the pair is unique).
fn inject_bash_dynamic_names(script: &str) -> String {
    // clap_complete indents subcommand cases with 12 spaces; every subcommand ends
    // its positional guard with this same if-block (unique together with opts=).
    let if_sfx = concat!(
        "\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n",
        "                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
        "                return 0\n",
        "            fi"
    );
    // Prepend a _klef_names guard; keep the original if-block for flag completion.
    let n_if = concat!(
        "\n            if [[ ${cur} != -* ]] ; then\n",
        "                COMPREPLY=( $(compgen -W \"$(_klef_names)\" -- \"${cur}\") )\n",
        "                return 0\n",
        "            fi",
        "\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n",
        "                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
        "                return 0\n",
        "            fi"
    );
    // rename: only <OLD> (COMP_CWORD==2) uses _klef_names; <NEW> is free-form.
    let rn_if = concat!(
        "\n            if [[ ${cur} != -* && ${COMP_CWORD} -eq 2 ]] ; then\n",
        "                COMPREPLY=( $(compgen -W \"$(_klef_names)\" -- \"${cur}\") )\n",
        "                return 0\n",
        "            fi",
        "\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n",
        "                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
        "                return 0\n",
        "            fi"
    );

    // (clap_complete opts line, cleaned opts, if-block replacement)
    // clap_complete inserts global flags (--backend) into each subcommand's opts.
    #[rustfmt::skip]
    let patches: &[(&str, &str, &str)] = &[
        (r#"opts="-h --backend --help <NAME>""#,   r#"opts="-h --backend --help""#,   n_if),
        (r#"opts="-h --yes --backend --help <NAME>""#, r#"opts="-h --yes --backend --help""#, n_if),
        (
            r#"opts="-h --note --as --value-from-file --tag --clear-tags --note-edit --backend --help <NAME>""#,
            r#"opts="-h --note --as --value-from-file --tag --clear-tags --note-edit --backend --help""#,
            n_if,
        ),
        (r#"opts="-h --backend --help <NAME> <NOTE>""#, r#"opts="-h --backend --help""#, n_if),
        (r#"opts="-h --backend --help <OLD> <NEW>""#,  r#"opts="-h --backend --help""#, rn_if),
        (r#"opts="-h --format --backend --help [NAMES]...""#, r#"opts="-h --format --backend --help""#, n_if),
    ];

    let mut out = String::with_capacity(script.len() + BASH_DYNAMIC_HEADER.len() + 1024);
    out.push_str(BASH_DYNAMIC_HEADER);
    out.push_str(script);

    for (old_opts, new_opts, if_repl) in patches {
        let old = format!("{old_opts}{if_sfx}");
        let new = format!("{new_opts}{if_repl}");
        if out.contains(old.as_str()) {
            out = out.replace(old.as_str(), new.as_str());
        }
    }

    out
}

/// Append fish completion directives that use `klef _names` for subcommands
/// whose first positional is a stored key name.
fn inject_fish_dynamic_names(script: &str) -> String {
    let dynamic_subcommands = ["get", "show", "rm", "edit", "rename", "set-note", "export"];

    let mut out = String::with_capacity(script.len() + 512);
    out.push_str(script);
    out.push_str("\n# klef dynamic name completion (added on top of clap_complete output)\n");

    for sub in &dynamic_subcommands {
        let _ = writeln!(
            out,
            "complete -c klef -n '__fish_klef_using_subcommand {sub}' -f -a '(klef _names)'"
        );
    }

    out
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
        assert!(out.contains(":new:_default")); // :new stays (free-form)
    }

    #[test]
    fn inject_replaces_variadic_names_positional() {
        let out = inject_zsh_dynamic_names("#compdef klef\n'*::names:_default'\n");
        assert!(out.contains("*::names:_klef_names"));
        assert!(!out.contains("*::names:_default"));
    }

    #[test]
    fn inject_leaves_note_positional_unchanged() {
        let out = inject_zsh_dynamic_names("#compdef klef\n':note:_default'\n");
        assert!(out.contains(":note:_default"));
        assert!(!out.contains(":note:_klef_names"));
    }

    #[test]
    fn bash_header_defines_klef_names() {
        assert!(BASH_DYNAMIC_HEADER.contains("_klef_names()"));
        assert!(BASH_DYNAMIC_HEADER.contains("klef _names"));
    }

    #[test]
    fn bash_inject_adds_helper_at_top() {
        let input = "_klef() { echo hi; }\n";
        let out = inject_bash_dynamic_names(input);
        assert!(out.find("_klef_names()").unwrap() < out.find("_klef()").unwrap());
    }

    #[test]
    fn bash_inject_rewrites_name_subcommand_opts() {
        // clap_complete now includes global flags (--backend) in each subcommand opts.
        let input = concat!(
            "        klef__subcmd__get)\n",
            "            opts=\"-h --backend --help <NAME>\"\n",
            "            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n",
            "                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
            "                return 0\n",
            "            fi\n",
        );
        let out = inject_bash_dynamic_names(input);
        assert!(
            out.contains(r#"COMPREPLY=( $(compgen -W "$(_klef_names)" -- "${cur}") )"#),
            "got: {out}"
        );
        assert!(!out.contains("<NAME>"), "got: {out}");
    }

    #[test]
    fn bash_inject_rename_only_patches_old_position() {
        // clap_complete now includes global flags (--backend) in each subcommand opts.
        let input = concat!(
            "        klef__subcmd__rename)\n",
            "            opts=\"-h --backend --help <OLD> <NEW>\"\n",
            "            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then\n",
            "                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n",
            "                return 0\n",
            "            fi\n",
        );
        let out = inject_bash_dynamic_names(input);
        assert!(out.contains("COMP_CWORD} -eq 2"), "COMP_CWORD guard: {out}");
        assert!(
            out.contains(r#"COMPREPLY=( $(compgen -W "$(_klef_names)" -- "${cur}") )"#),
            "got: {out}"
        );
    }

    #[test]
    fn fish_inject_adds_dynamic_directives_for_all_subcommands() {
        let out = inject_fish_dynamic_names("complete -c klef -f\n");
        assert!(out.contains("complete -c klef -f\n"), "got: {out}");
        for sub in &["get", "show", "rm", "edit", "rename", "set-note", "export"] {
            assert!(
                out.contains(&format!(
                    "complete -c klef -n '__fish_klef_using_subcommand {sub}' -f -a '(klef _names)'"
                )),
                "missing {sub}: {out}"
            );
        }
    }

    #[test]
    fn fish_inject_uses_klef_names_command() {
        assert!(inject_fish_dynamic_names("").contains("(klef _names)"));
    }
}
