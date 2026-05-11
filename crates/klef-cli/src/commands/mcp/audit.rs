//! Append-only NDJSON audit log. Fail-closed: any write error must propagate
//! so the caller can deny the request.

use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct Entry<'a> {
    pub ts: String,
    pub tool: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argv: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_refs: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<&'a str>,
    pub decision: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_rule_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_returned: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
#[error("audit: {0}")]
pub struct AuditError(String);

#[derive(Debug, Clone)]
pub struct Audit {
    path: PathBuf,
}

impl Audit {
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append `entry` as one NDJSON line. Returns an error if the file
    /// cannot be opened/written/synced — caller MUST refuse the request.
    ///
    /// # Errors
    ///
    /// Returns `AuditError` if the parent directory cannot be created,
    /// the entry cannot be serialized, or the file cannot be opened,
    /// written, or synced.
    pub fn record(&self, entry: &Entry<'_>) -> Result<(), AuditError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AuditError(e.to_string()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
        let mut line = serde_json::to_vec(entry).map_err(|e| AuditError(e.to_string()))?;
        line.push(b'\n');
        // Audit log entries include argv / env-var names / cwd — never the
        // resolved secret values, but still potentially-sensitive metadata.
        // Open with mode 0600 on Unix so it doesn't inherit umask (commonly
        // 022 → world-readable).
        let mut opts = OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&self.path)
            .map_err(|e| AuditError(format!("open {}: {e}", self.path.display())))?;
        #[cfg(unix)]
        {
            // Belt-and-suspenders: if the file pre-existed with looser perms
            // (mode is only honored on create), tighten it now.
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        f.write_all(&line).map_err(|e| AuditError(e.to_string()))?;
        f.sync_all().map_err(|e| AuditError(e.to_string()))?;
        Ok(())
    }
}

#[must_use]
pub fn now_iso() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn argv() -> Vec<String> {
        vec!["echo".into(), "hi".into()]
    }
    fn envs() -> Vec<String> {
        vec!["stripe".into()]
    }

    #[test]
    fn record_writes_one_ndjson_line_and_appends() {
        let tmp = TempDir::new().unwrap();
        let a = Audit::new(tmp.path().join("audit.log"));
        for _ in 0..3 {
            let av = argv();
            let er = envs();
            let e = Entry {
                ts: now_iso(),
                tool: "klef_run",
                argv: Some(&av),
                env_refs: Some(&er),
                cwd: None,
                decision: "allow",
                phase: None,
                matched_rule_index: Some(0),
                reason: None,
                exit_code: Some(0),
                duration_ms: Some(1),
                stdout_bytes: Some(0),
                stderr_bytes: Some(0),
                stdout_truncated: Some(false),
                stderr_truncated: Some(false),
                timed_out: Some(false),
                count_returned: None,
            };
            a.record(&e).unwrap();
        }
        let s = std::fs::read_to_string(tmp.path().join("audit.log")).unwrap();
        assert_eq!(s.matches('\n').count(), 3);
        for line in s.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v["tool"], "klef_run");
            assert_eq!(v["decision"], "allow");
        }
    }

    #[test]
    fn record_fails_when_path_is_unwritable() {
        // Path with a non-directory parent component can't be created.
        let tmp = TempDir::new().unwrap();
        let blocker = tmp.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        let a = Audit::new(blocker.join("audit.log"));
        let av = argv();
        let er = envs();
        let e = Entry {
            ts: now_iso(),
            tool: "klef_run",
            argv: Some(&av),
            env_refs: Some(&er),
            cwd: None,
            decision: "allow",
            phase: None,
            matched_rule_index: Some(0),
            reason: None,
            exit_code: Some(0),
            duration_ms: Some(0),
            stdout_bytes: Some(0),
            stderr_bytes: Some(0),
            stdout_truncated: Some(false),
            stderr_truncated: Some(false),
            timed_out: Some(false),
            count_returned: None,
        };
        assert!(a.record(&e).is_err());
    }
}
