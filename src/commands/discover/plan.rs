use crate::cli::ConflictMode;
use crate::envfile::{self, Value};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub(super) const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".cache",
    ".idea",
    ".vscode",
];

pub(super) const DEFAULT_INCLUDE: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    ".env.development",
    ".env.dev",
    ".env.staging",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscoveredEntry {
    pub(super) env_var: String,
    pub(super) klef_name: String,
    pub(super) value: String,
    pub(super) source: PathBuf,
}

#[derive(Debug)]
pub(super) struct DiscoverPlan {
    /// Final picks per env-var (after conflict resolution).
    pub(super) picks: Vec<DiscoveredEntry>,
    /// Number of conflicts that were resolved.
    pub(super) conflicts: usize,
    /// Number of entries skipped by pattern matching.
    pub(super) skipped_by_pattern: usize,
    /// All source files seen (used to print the per-file breakdown).
    pub(super) files: Vec<PathBuf>,
}

pub(super) fn walk(root: &Path, max_depth: usize, patterns: &[String]) -> Vec<PathBuf> {
    let mut hits: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir(e))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if patterns.contains(&name) {
            hits.push(entry.path().to_path_buf());
        }
    }
    hits.sort();
    hits
}

fn is_skipped_dir(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    SKIP_DIRS.iter().any(|skip| name == *skip)
}

pub(super) fn build_plan(
    files: &[PathBuf],
    on_conflict: ConflictMode,
    skip_patterns: &[regex::Regex],
) -> DiscoverPlan {
    let mut by_var: BTreeMap<String, DiscoveredEntry> = BTreeMap::new();
    let mut conflicts = 0_usize;
    let mut skipped_by_pattern = 0_usize;

    for path in files {
        let Ok(entries) = envfile::parse(path) else {
            continue;
        };
        for e in entries {
            if let Value::Literal(value) = e.value {
                // Skip matching patterns first.
                if skip_patterns.iter().any(|r| r.is_match(&e.key)) {
                    skipped_by_pattern += 1;
                    continue;
                }
                let klef_name = derive_name(&e.key);
                let new_entry = DiscoveredEntry {
                    env_var: e.key.clone(),
                    klef_name,
                    value,
                    source: path.clone(),
                };
                match by_var.entry(e.key) {
                    std::collections::btree_map::Entry::Vacant(slot) => {
                        slot.insert(new_entry);
                    }
                    std::collections::btree_map::Entry::Occupied(mut slot) => {
                        conflicts += 1;
                        if matches!(on_conflict, ConflictMode::LastFound) {
                            *slot.get_mut() = new_entry;
                        }
                    }
                }
            }
        }
    }

    let picks: Vec<DiscoveredEntry> = by_var.into_values().collect();
    DiscoverPlan {
        picks,
        conflicts,
        skipped_by_pattern,
        files: files.to_vec(),
    }
}

pub(super) fn derive_name(env_key: &str) -> String {
    env_key
        .chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

pub(super) fn redact(value: &str) -> String {
    let len = value.len();
    if len <= 6 {
        format!("*** ({len} chars)")
    } else {
        let prefix: String = value.chars().take(4).collect();
        format!("{prefix}*** ({len} chars)")
    }
}

pub(super) fn print_plan(plan: &DiscoverPlan) {
    if plan.picks.is_empty() {
        if plan.files.is_empty() {
            println!("(no .env files found in scan root)");
        } else {
            println!(
                "(no literal secrets found across {} file(s))",
                plan.files.len()
            );
        }
        return;
    }

    let mut by_file: BTreeMap<&Path, Vec<&DiscoveredEntry>> = BTreeMap::new();
    for entry in &plan.picks {
        by_file
            .entry(entry.source.as_path())
            .or_default()
            .push(entry);
    }

    let env_w = plan
        .picks
        .iter()
        .map(|e| e.env_var.len())
        .max()
        .unwrap_or(7)
        .max(7);
    let name_w = plan
        .picks
        .iter()
        .map(|e| e.klef_name.len())
        .max()
        .unwrap_or(9)
        .max(9);

    for (path, entries) in &by_file {
        println!("{}", path.display());
        for e in entries {
            println!(
                "  {:<env_w$}  →  {:<name_w$}  {}",
                e.env_var,
                e.klef_name,
                redact(&e.value)
            );
        }
        println!();
    }

    // Files where every entry lost the conflict.
    let pick_files: HashSet<&Path> = plan.picks.iter().map(|e| e.source.as_path()).collect();
    for f in &plan.files {
        if !pick_files.contains(f.as_path()) {
            println!("{}: (all keys taken from earlier files)", f.display());
            println!();
        }
    }

    let mut suffix = String::new();
    if plan.conflicts > 0 {
        let _ = write!(suffix, " {} conflict(s) resolved.", plan.conflicts);
    }
    if plan.skipped_by_pattern > 0 {
        let _ = write!(suffix, " {} skipped by pattern.", plan.skipped_by_pattern);
    }
    println!(
        "{} unique key(s) across {} file(s).{}",
        plan.picks.len(),
        plan.files.len(),
        suffix
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn derive_name_snake_to_kebab_lowercase() {
        assert_eq!(derive_name("STRIPE_API_KEY"), "stripe-api-key");
    }

    #[test]
    fn redact_short_value() {
        assert_eq!(redact("abc"), "*** (3 chars)");
    }

    #[test]
    fn redact_long_value() {
        assert_eq!(redact("sk_live_abc123"), "sk_l*** (14 chars)");
    }

    fn tempenv(content: &str) -> (tempfile::TempDir, PathBuf) {
        let d = tempdir().unwrap();
        let p = d.path().join(".env");
        fs::write(&p, content).unwrap();
        (d, p)
    }

    #[test]
    fn build_plan_skips_by_pattern() {
        let (_d, p) = tempenv("STRIPE_API_KEY=secret\nPORT=3000\nDB_NAME=app\n");
        let skip = vec![regex::Regex::new(r"^(PORT|DB_NAME)$").unwrap()];
        let plan = build_plan(&[p], ConflictMode::FirstFound, &skip);
        assert_eq!(plan.picks.len(), 1);
        assert_eq!(plan.picks[0].env_var, "STRIPE_API_KEY");
        assert_eq!(plan.skipped_by_pattern, 2);
    }

    #[test]
    fn build_plan_no_skip_patterns() {
        let (_d, p) = tempenv("STRIPE_API_KEY=secret\nPORT=3000\n");
        let plan = build_plan(&[p], ConflictMode::FirstFound, &[]);
        assert_eq!(plan.picks.len(), 2);
        assert_eq!(plan.skipped_by_pattern, 0);
    }

    #[test]
    fn walk_skips_node_modules() {
        let d = tempdir().unwrap();
        let nm = d.path().join("node_modules/some-pkg");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join(".env"), "X=1\n").unwrap();
        fs::write(d.path().join(".env"), "Y=1\n").unwrap();
        let hits = walk(d.path(), 5, &[".env".to_string()]);
        assert_eq!(hits.len(), 1, "node_modules should be skipped");
    }
}
