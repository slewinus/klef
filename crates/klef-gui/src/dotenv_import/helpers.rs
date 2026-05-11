//! Pure helpers for the dotenv-import flow: name munging, path
//! canonicalization, in-place `.env` rewriting. No Tauri state, no IPC.

pub fn redact(v: &str) -> String {
    let n = v.chars().count();
    if n <= 6 {
        format!("*** ({n} chars)")
    } else {
        let prefix: String = v.chars().take(4).collect();
        format!("{prefix}*** ({n} chars)")
    }
}

pub fn klef_name_from_env_var(k: &str) -> String {
    k.chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

pub fn project_from_path(p: &std::path::Path) -> String {
    p.parent().and_then(|d| d.file_name()).map_or_else(
        || "unknown".to_string(),
        |n| n.to_string_lossy().replace([' ', '/'], "-").to_lowercase(),
    )
}

/// Resolve `raw` to a canonical, regular-file path. We do NOT restrict to
/// `$HOME` — users drag .env files from arbitrary locations. We DO refuse
/// directories and other non-regular targets. Symlinks are followed by
/// `fs::canonicalize`, returning the real path.
pub fn canonicalize_source(raw: &str) -> Result<std::path::PathBuf, String> {
    let pb = std::path::PathBuf::from(raw);
    let canonical = std::fs::canonicalize(&pb).map_err(|e| format!("cannot resolve path: {e}"))?;
    let meta = std::fs::metadata(&canonical).map_err(|e| format!("cannot stat: {e}"))?;
    if !meta.is_file() {
        return Err("path is not a regular file".to_string());
    }
    Ok(canonical)
}

/// Rewrite the source .env so each imported line becomes
/// `<ENV_VAR>=klef:<klef_name>`. Comments, empty values, already-refs,
/// skipped rows are left intact byte-for-byte. Atomic via tmp + rename.
pub fn rewrite_dotenv(
    path: &std::path::Path,
    imported: &[(String, String)],
) -> std::io::Result<()> {
    let original = std::fs::read_to_string(path)?;
    let map: std::collections::HashMap<&str, &str> = imported
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut out = String::with_capacity(original.len());
    for line in original.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.trim().is_empty() {
            out.push_str(line);
            continue;
        }
        if let Some((key, _rest)) = trimmed.split_once('=')
            && let Some(klef_name) = map.get(key.trim())
        {
            let indent_len = line.len() - trimmed.len();
            out.push_str(&line[..indent_len]);
            out.push_str(key.trim());
            out.push_str("=klef:");
            out.push_str(klef_name);
            out.push('\n');
        } else {
            out.push_str(line);
        }
    }
    let tmp = path.with_extension("env.tmp");
    // Mirror the original .env's perms (or 0600 if metadata missing) — never
    // loosen. Unimported lines may still hold plaintext secrets.
    klef_core::fsx::write_inheriting(&tmp, out.as_bytes(), path)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
