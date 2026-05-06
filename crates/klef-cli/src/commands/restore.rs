//! `klef restore` — restore a vault from an age-encrypted backup.

use klef_core::backup::Bundle;
use klef_core::error::KlefError;
use klef_core::store::Store;
use std::io::Read as _;
use std::path::Path;
use zeroize::Zeroizing;

/// Run `klef restore`.
///
/// Three-phase contract:
/// - **Phase 0** — Decrypt, parse, validate schema, detect conflicts. Aborts
///   cleanly on any failure; no writes occur.
/// - **Phase 1** — Write all values to the backend sequentially. On failure,
///   stops immediately and reports progress. The index is NOT touched.
/// - **Phase 2** — Rewrite the index atomically from the bundle entries.
///
/// Guarantee: klef's view of the vault is atomic. Either restore fully
/// succeeds or klef stays on the previous state. The keychain itself may have
/// orphaned entries on partial Phase 1 failure.
///
/// # Errors
///
/// Returns an error if decryption fails, the bundle is malformed, any conflict
/// is detected (without `--force`), or any backend/index write fails.
pub fn run(store: &Store, input: &Path, force: bool) -> Result<(), KlefError> {
    // Phase 0: Preflight — no writes.
    let bundle = decrypt_and_parse(input)?;
    validate_bundle(&bundle)?;
    let conflicts = detect_conflicts(store, &bundle)?;
    if !conflicts.is_empty() && !force {
        return Err(KlefError::BackendUnavailable(format!(
            "restore would conflict with {} existing key(s): {}; use --force to overwrite",
            conflicts.len(),
            format_conflict_list(&conflicts),
        )));
    }

    // Phase 1: Backend writes — sequential, fail-fast, no index writes.
    let total = bundle.entries.len();
    for (idx, entry) in bundle.entries.iter().enumerate() {
        if let Err(e) = store.restore_phase_1(entry) {
            eprintln!(
                "restore failed at entry index {idx}: {e}\n\
                 restored {idx} of {total} entries to backend before failing"
            );
            return Err(e);
        }
    }

    // Phase 2: Index commit — atomic.
    store.restore_phase_2(&bundle.entries)?;

    println!("✓ restore complete: {total} entries written");
    Ok(())
}

fn decrypt_and_parse(input: &Path) -> Result<Bundle, KlefError> {
    let ciphertext = std::fs::read(input).map_err(KlefError::Io)?;
    let plaintext = age_decrypt(&ciphertext)?;
    let bundle: Bundle =
        serde_json::from_slice(&plaintext).map_err(|e| KlefError::IndexCorrupt {
            path: input.to_path_buf(),
            reason: format!("invalid bundle: {e}"),
        })?;
    Ok(bundle)
}

/// Read a single passphrase line from stdin, hiding input when on a TTY.
fn read_passphrase(prompt: &str) -> Result<String, KlefError> {
    use std::io::IsTerminal as _;
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password(prompt).map_err(|e| KlefError::BackendUnavailable(e.to_string()))
    } else {
        use std::io::BufRead as _;
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(KlefError::Io)?;
        Ok(line
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string())
    }
}

/// Decrypt an age ciphertext, prompting for a passphrase if needed.
///
/// Only passphrase-encrypted backups are supported on the restore side.
/// Recipient-encrypted backups require an identity file — not yet supported.
///
/// # Errors
///
/// Returns an error if the file is not a valid age file, if the passphrase is
/// wrong, or if recipient-based encryption is detected.
pub fn age_decrypt(ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, KlefError> {
    let decryptor = age::Decryptor::new(ciphertext)
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt init: {e}")))?;

    if !decryptor.is_scrypt() {
        return Err(KlefError::BackendUnavailable(
            "restore from recipient-encrypted backup is not supported in v0.x; \
             re-encrypt with a passphrase or wait for a follow-up release."
                .to_string(),
        ));
    }

    let pass = read_passphrase("Passphrase: ")?;
    let passphrase = age::secrecy::SecretString::from(pass);

    let identity = age::scrypt::Identity::new(passphrase);
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt: {e}")))?;

    let mut output = Zeroizing::new(Vec::new());
    reader.read_to_end(&mut output).map_err(KlefError::Io)?;
    Ok(output)
}

fn validate_bundle(bundle: &Bundle) -> Result<(), KlefError> {
    if bundle.format_version != Bundle::FORMAT_VERSION {
        return Err(KlefError::IndexCorrupt {
            path: std::path::PathBuf::new(),
            reason: format!(
                "unsupported format_version {} (expected {}); \
                 is this a klef backup from a newer version?",
                bundle.format_version,
                Bundle::FORMAT_VERSION
            ),
        });
    }
    if bundle.tool != Bundle::TOOL {
        return Err(KlefError::IndexCorrupt {
            path: std::path::PathBuf::new(),
            reason: format!("not a klef backup (tool='{}')", bundle.tool),
        });
    }
    Ok(())
}

fn detect_conflicts(store: &Store, bundle: &Bundle) -> Result<Vec<String>, KlefError> {
    let existing: std::collections::HashSet<String> =
        store.list()?.into_iter().map(|(name, _)| name).collect();
    Ok(bundle
        .entries
        .iter()
        .filter(|e| existing.contains(&e.name))
        .map(|e| e.name.clone())
        .collect())
}

/// Show up to 3 conflicting names; redact the rest with a count.
fn format_conflict_list(names: &[String]) -> String {
    if names.len() <= 3 {
        names.join(", ")
    } else {
        format!("{}, ... ({} more)", names[..3].join(", "), names.len() - 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klef_core::backup::BundleEntry;
    use time::macros::datetime;

    fn make_entry(name: &str) -> BundleEntry {
        BundleEntry {
            name: name.to_string(),
            value: "val".to_string(),
            keychain_service: "klef".to_string(),
            keychain_account: name.to_string(),
            env_var: "MY_VAR".to_string(),
            note: None,
            tags: vec![],
            added_at: datetime!(2026-05-05 12:00:00 UTC),
            updated_at: datetime!(2026-05-05 12:00:00 UTC),
        }
    }

    #[test]
    fn format_conflict_list_short() {
        let names = vec!["a".to_string(), "b".to_string()];
        assert_eq!(format_conflict_list(&names), "a, b");
    }

    #[test]
    fn format_conflict_list_truncated() {
        let names = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let s = format_conflict_list(&names);
        assert!(s.contains("1 more"), "got: {s}");
    }

    #[test]
    fn validate_bundle_rejects_wrong_version() {
        let bundle = Bundle {
            format_version: 999,
            tool: "klef".to_string(),
            klef_version: "0.2.0".to_string(),
            created_at: datetime!(2026-05-06 12:00:00 UTC),
            source: klef_core::backup::BundleSource {
                hostname: "h".to_string(),
                platform: "macos".to_string(),
            },
            entries: vec![],
        };
        let err = validate_bundle(&bundle).unwrap_err();
        assert!(
            err.to_string().contains("unsupported format_version"),
            "got: {err}"
        );
    }

    #[test]
    fn validate_bundle_rejects_wrong_tool() {
        let bundle = Bundle {
            format_version: 1,
            tool: "not-klef".to_string(),
            klef_version: "0.2.0".to_string(),
            created_at: datetime!(2026-05-06 12:00:00 UTC),
            source: klef_core::backup::BundleSource {
                hostname: "h".to_string(),
                platform: "macos".to_string(),
            },
            entries: vec![],
        };
        let err = validate_bundle(&bundle).unwrap_err();
        assert!(err.to_string().contains("not a klef backup"), "got: {err}");
    }

    #[test]
    fn validate_bundle_accepts_valid() {
        let bundle = Bundle {
            format_version: 1,
            tool: "klef".to_string(),
            klef_version: "0.2.0".to_string(),
            created_at: datetime!(2026-05-06 12:00:00 UTC),
            source: klef_core::backup::BundleSource {
                hostname: "h".to_string(),
                platform: "macos".to_string(),
            },
            entries: vec![make_entry("foo")],
        };
        assert!(validate_bundle(&bundle).is_ok());
    }
}
