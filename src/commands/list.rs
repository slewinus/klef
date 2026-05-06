use crate::cli::ListFormat;
use crate::error::KlefError;
use crate::store::Store;

/// Run the list command.
///
/// # Errors
/// Returns an error if the index can't be loaded.
pub fn run(
    store: &Store,
    format: ListFormat,
    verbose: bool,
    filter: Option<&str>,
    tag_filter: Option<&str>,
) -> Result<(), KlefError> {
    let mut entries = store.list()?;

    if let Some(pat) = filter {
        let needle = pat.to_lowercase();
        entries.retain(|(name, meta)| {
            name.to_lowercase().contains(&needle)
                || meta
                    .note
                    .as_deref()
                    .is_some_and(|n| n.to_lowercase().contains(&needle))
        });
    }
    if let Some(tag) = tag_filter {
        entries.retain(|(_, meta)| meta.tags.iter().any(|t| t == tag));
    }

    match format {
        ListFormat::Table => print_table(&entries, verbose),
        ListFormat::Json => print_json(&entries)?,
    }
    Ok(())
}

fn print_table(entries: &[(String, crate::store::KeyMeta)], verbose: bool) {
    if entries.is_empty() {
        println!("(no keys stored)");
        return;
    }
    let name_w = entries
        .iter()
        .map(|(n, _)| n.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let var_w = entries
        .iter()
        .map(|(_, m)| m.env_var.len())
        .max()
        .unwrap_or(7)
        .max(7);

    if verbose {
        let added_w = "ADDED".len();
        let tags_w = entries
            .iter()
            .map(|(_, m)| {
                if m.tags.is_empty() {
                    1
                } else {
                    m.tags.join(", ").len()
                }
            })
            .max()
            .unwrap_or(4)
            .max(4);
        println!(
            "{:<name_w$}  {:<var_w$}  {:<added_w$}  {:<tags_w$}  NOTE",
            "NAME", "ENV_VAR", "ADDED", "TAGS"
        );
        for (name, meta) in entries {
            let note = meta.note.as_deref().unwrap_or("-");
            let added = format_date(&meta.added_at);
            let tags = if meta.tags.is_empty() {
                "-".to_string()
            } else {
                meta.tags.join(", ")
            };
            println!(
                "{name:<name_w$}  {:<var_w$}  {added:<added_w$}  {tags:<tags_w$}  {note}",
                meta.env_var
            );
        }
    } else {
        println!("{:<name_w$}  {:<var_w$}  NOTE", "NAME", "ENV_VAR");
        for (name, meta) in entries {
            let note = meta.note.as_deref().unwrap_or("-");
            println!("{name:<name_w$}  {:<var_w$}  {note}", meta.env_var);
        }
    }
}

fn format_date(t: &time::OffsetDateTime) -> String {
    // Output `YYYY-MM-DD` — date only, no time. Stable across locales.
    format!("{:04}-{:02}-{:02}", t.year(), u8::from(t.month()), t.day())
}

fn print_json(entries: &[(String, crate::store::KeyMeta)]) -> Result<(), KlefError> {
    let map: std::collections::BTreeMap<_, _> = entries
        .iter()
        .map(|(n, m)| (n.clone(), m.clone()))
        .collect();
    let s = serde_json::to_string_pretty(&map).map_err(|e| KlefError::IndexCorrupt {
        path: std::path::PathBuf::new(),
        reason: e.to_string(),
    })?;
    println!("{s}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn format_date_renders_iso_yyyy_mm_dd() {
        let d = datetime!(2026-05-05 19:57:00 UTC);
        assert_eq!(format_date(&d), "2026-05-05");
    }

    #[test]
    fn format_date_pads_single_digit_month_day() {
        let d = datetime!(2026-01-09 00:00:00 UTC);
        assert_eq!(format_date(&d), "2026-01-09");
    }
}
