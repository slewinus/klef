//! Filesystem helpers for writing files with explicit, secure permissions.
//!
//! Used wherever klef writes a file that could contain metadata or
//! ciphertext that shouldn't inherit the user's umask (commonly 022 →
//! world-readable). Belt-and-suspenders: `OpenOptions::mode` is only
//! honored on create, so `set_permissions` re-applies after open to
//! tighten any pre-existing file too.

use std::path::Path;

/// Write `bytes` to `path` with mode 0600 on Unix (`O_CREAT|O_WRONLY|O_TRUNC`).
/// On non-Unix, falls back to `std::fs::write` (no mode control).
///
/// # Errors
/// Propagates the underlying `io::Error` from open / write / chmod.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_with_mode(path, bytes, 0o600)
}

/// Write `bytes` to `path`, mirroring `template`'s perms on Unix.
///
/// Falls back to 0600 if `template` is missing. Used to rewrite a file in
/// place without loosening intentional perms (e.g. an `.env` shared with a
/// teammate at 0640).
///
/// # Errors
/// Propagates the underlying `io::Error` from open / write / chmod.
pub fn write_inheriting(path: &Path, bytes: &[u8], template: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(template).map_or(0o600, |m| m.permissions().mode() & 0o777);
        write_with_mode(path, bytes, mode)
    }
    #[cfg(not(unix))]
    {
        let _ = template;
        std::fs::write(path, bytes)
    }
}

#[cfg(unix)]
fn write_with_mode(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)?;
    f.write_all(bytes)?;
    // `mode` is only honored on file create — re-apply for pre-existing files.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_with_mode(path: &Path, bytes: &[u8], _mode: u32) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn write_private_creates_0600() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("secret");
        write_private(&p, b"hi").unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn write_private_tightens_existing_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("loose");
        std::fs::write(&p, b"old").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&p, b"new").unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn write_inheriting_mirrors_template() {
        let dir = tempdir().unwrap();
        let template = dir.path().join("env");
        std::fs::write(&template, b"x").unwrap();
        std::fs::set_permissions(&template, std::fs::Permissions::from_mode(0o640)).unwrap();
        let tmp = dir.path().join("env.tmp");
        write_inheriting(&tmp, b"y", &template).unwrap();
        let mode = std::fs::metadata(&tmp).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[test]
    fn write_inheriting_falls_back_to_0600_when_template_missing() {
        let dir = tempdir().unwrap();
        let tmp = dir.path().join("tmp");
        write_inheriting(&tmp, b"y", &dir.path().join("does-not-exist")).unwrap();
        let mode = std::fs::metadata(&tmp).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
