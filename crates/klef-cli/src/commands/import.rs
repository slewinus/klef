use klef_core::envfile::{self, Value};
use klef_core::error::KlefError;
use klef_core::store::Store;
use std::fmt::Write as FmtWrite;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
enum PlanLine {
    Import {
        env_var: String,
        klef_name: String,
        value: String,
    },
    SkipReference {
        env_var: String,
        target: String,
    },
}

/// Build the import plan from parsed .env entries.
fn plan(entries: Vec<klef_core::envfile::Entry>, prefix: Option<&str>) -> Vec<PlanLine> {
    entries
        .into_iter()
        .map(|e| match e.value {
            Value::Reference(target) => PlanLine::SkipReference {
                env_var: e.key,
                target,
            },
            Value::Literal(value) => {
                let klef_name = derive_name(&e.key, prefix);
                PlanLine::Import {
                    env_var: e.key,
                    klef_name,
                    value,
                }
            }
        })
        .collect()
}

fn derive_name(env_key: &str, prefix: Option<&str>) -> String {
    let base: String = env_key
        .chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    match prefix {
        Some(p) if !p.is_empty() => format!("{p}-{base}"),
        _ => base,
    }
}

fn redact(value: &str) -> String {
    let len = value.len();
    if len <= 6 {
        format!("*** ({len} chars)")
    } else {
        let prefix: String = value.chars().take(4).collect();
        format!("{prefix}*** ({len} chars)")
    }
}

// clippy::missing_const_for_fn: String::len is not const-stable in this context
#[allow(clippy::missing_const_for_fn)]
fn env_var_width(line: &PlanLine) -> usize {
    match line {
        PlanLine::Import { env_var, .. } | PlanLine::SkipReference { env_var, .. } => env_var.len(),
    }
}

#[allow(clippy::missing_const_for_fn)]
fn klef_name_width(line: &PlanLine) -> usize {
    match line {
        PlanLine::Import { klef_name, .. } => klef_name.len(),
        PlanLine::SkipReference { .. } => 0,
    }
}

fn print_plan(plan: &[PlanLine]) {
    let env_w = plan.iter().map(env_var_width).max().unwrap_or(7).max(7);
    let name_w = plan.iter().map(klef_name_width).max().unwrap_or(9).max(9);
    println!("{:<env_w$}  {:<name_w$}  VALUE", "ENV VAR", "KLEF NAME");
    for line in plan {
        match line {
            PlanLine::Import {
                env_var,
                klef_name,
                value,
            } => {
                println!("{env_var:<env_w$}  {klef_name:<name_w$}  {}", redact(value));
            }
            PlanLine::SkipReference { env_var, target } => {
                println!("{env_var:<env_w$}  skip — already klef:{target}");
            }
        }
    }
}

/// # Errors
/// Returns an error if the env file can't be read/parsed or the index can't be saved.
pub fn run(
    store: &Store,
    file: &Path,
    prefix: Option<&str>,
    dry_run: bool,
    rewrite: bool,
    yes: bool,
) -> Result<(), KlefError> {
    let entries = envfile::parse(file)?;
    let plan_lines = plan(entries, prefix);
    print_plan(&plan_lines);

    if dry_run {
        return Ok(());
    }

    let to_import = plan_lines
        .iter()
        .filter(|l| matches!(l, PlanLine::Import { .. }))
        .count();
    if to_import == 0 {
        println!("nothing to import.");
        return Ok(());
    }

    if !yes && std::io::stdin().is_terminal() {
        print!("Import {to_import} key(s)? [y/N] ");
        std::io::stdout().flush().map_err(KlefError::Io)?;
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(KlefError::Io)?;
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("aborted");
            return Ok(());
        }
    }

    let mut imported = Vec::<(String, String)>::new();
    let mut skipped = Vec::<String>::new();

    for line in &plan_lines {
        if let PlanLine::Import {
            env_var,
            klef_name,
            value,
        } = line
        {
            match store.add(klef_name, value, Some(env_var.clone()), None, vec![], false) {
                Ok(()) => {
                    println!("✓ {env_var} → klef:{klef_name}");
                    imported.push((env_var.clone(), klef_name.clone()));
                }
                Err(KlefError::KeyAlreadyExists(_)) => {
                    skipped.push(klef_name.clone());
                }
                Err(other) => return Err(other),
            }
        }
    }

    println!();
    println!("Imported {} key(s).", imported.len());
    if !skipped.is_empty() {
        println!(
            "Skipped {} (already existed): {}",
            skipped.len(),
            skipped.join(", ")
        );
    }

    if rewrite && !imported.is_empty() {
        let count = rewrite_env_file(file, &imported)?;
        println!(
            "Rewrote {} ({count} reference(s) replaced).",
            file.display()
        );
    }

    Ok(())
}

fn rewrite_env_file(file: &Path, imported: &[(String, String)]) -> Result<usize, KlefError> {
    let content = std::fs::read_to_string(file).map_err(KlefError::Io)?;
    let map: std::collections::HashMap<&str, &str> = imported
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let mut out = String::with_capacity(content.len());
    let mut replaced = 0;
    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(raw);
            out.push('\n');
            continue;
        }
        if let Some((k, _v)) = trimmed.split_once('=') {
            let key = k.trim();
            if let Some(klef_name) = map.get(key) {
                let _ = writeln!(out, "{key}=klef:{klef_name}");
                replaced += 1;
                continue;
            }
        }
        out.push_str(raw);
        out.push('\n');
    }

    let tmp = file.with_extension("env.tmp");
    // Mirror the original file's perms (defaults to 0600 if missing) so
    // the rewrite never loosens an intentionally-restricted .env. Unimported
    // lines may still hold plaintext secrets, so a world-readable tmp would
    // be worse than the original.
    klef_core::fsx::write_inheriting(&tmp, out.as_bytes(), file).map_err(KlefError::Io)?;
    std::fs::rename(&tmp, file).map_err(KlefError::Io)?;
    Ok(replaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_name_basic() {
        assert_eq!(derive_name("STRIPE_API_KEY", None), "stripe-api-key");
    }

    #[test]
    fn derive_name_with_prefix() {
        assert_eq!(derive_name("API_KEY", Some("stripe")), "stripe-api-key");
    }

    #[test]
    fn derive_name_empty_prefix_treated_as_none() {
        assert_eq!(derive_name("API_KEY", Some("")), "api-key");
    }

    #[test]
    fn redact_short_value() {
        assert_eq!(redact("abc"), "*** (3 chars)");
    }

    #[test]
    fn redact_long_value() {
        assert_eq!(redact("sk_live_abc123"), "sk_l*** (14 chars)");
    }

    #[test]
    fn plan_separates_literal_and_reference_entries() {
        use klef_core::envfile::{Entry, Value};
        let entries = vec![
            Entry {
                key: "STRIPE".into(),
                value: Value::Literal("sk_live".into()),
            },
            Entry {
                key: "ALREADY".into(),
                value: Value::Reference("foo".into()),
            },
        ];
        let p = plan(entries, None);
        assert_eq!(p.len(), 2);
        assert!(matches!(&p[0], PlanLine::Import { klef_name, .. } if klef_name == "stripe"));
        assert!(matches!(&p[1], PlanLine::SkipReference { target, .. } if target == "foo"));
    }
}
