//! `klef backup` — encrypt and dump the entire vault to an age file.

use crate::error::KlefError;
use crate::store::Store;
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::Path;
use time::OffsetDateTime;
use zeroize::Zeroizing;

/// On-disk format for a klef backup. Strict schema; unknown fields are rejected.
///
/// Used by both `backup` (write side) and `restore` (read side, see
/// `commands::restore`).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub format_version: u32,
    pub tool: String,
    pub klef_version: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub source: BundleSource,
    pub entries: Vec<BundleEntry>,
}

/// Source metadata recorded at backup time.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSource {
    pub hostname: String,
    pub platform: String,
}

/// A single vault entry as recorded in a backup bundle.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleEntry {
    pub name: String,
    pub value: String,
    pub keychain_service: String,
    pub keychain_account: String,
    pub env_var: String,
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl Bundle {
    /// The only supported format version for read and write.
    pub const FORMAT_VERSION: u32 = 1;
    /// The tool identifier embedded in every bundle.
    pub const TOOL: &'static str = "klef";

    /// Build a `Bundle` from the current `Store` state.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be read or a value cannot be
    /// retrieved from the backend.
    pub fn from_store(store: &Store) -> Result<Self, KlefError> {
        let entries_meta = store.list()?;
        let mut entries = Vec::with_capacity(entries_meta.len());
        for (name, meta) in entries_meta {
            let value = store.get_value(&name)?;
            entries.push(BundleEntry {
                keychain_service: "klef".to_string(),
                keychain_account: name.clone(),
                env_var: meta.env_var,
                note: meta.note,
                tags: meta.tags,
                added_at: meta.added_at,
                updated_at: meta.updated_at,
                name,
                value,
            });
        }
        Ok(Self {
            format_version: Self::FORMAT_VERSION,
            tool: Self::TOOL.to_string(),
            klef_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: OffsetDateTime::now_utc(),
            source: BundleSource {
                hostname: gethostname::gethostname().to_string_lossy().into_owned(),
                platform: detect_platform().to_string(),
            },
            entries,
        })
    }
}

const fn detect_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "other"
    }
}

/// Run `klef backup`.
///
/// # Errors
///
/// Returns an error if the vault cannot be read, the bundle cannot be
/// serialized, or the encrypted file cannot be written.
pub fn run(store: &Store, output: &Path, recipients: &[String]) -> Result<(), KlefError> {
    let bundle = Bundle::from_store(store)?;
    let entry_count = bundle.entries.len();

    // Serialize into a Zeroizing buffer — this buffer holds plaintext values;
    // it is never written to disk.
    let plaintext: Zeroizing<Vec<u8>> = Zeroizing::new(serde_json::to_vec(&bundle).map_err(
        |e| KlefError::IndexCorrupt {
            path: output.to_path_buf(),
            reason: format!("failed to serialize bundle: {e}"),
        },
    )?);

    let ciphertext = encrypt(&plaintext, recipients)?;

    // Write atomically: <output>.age.tmp then rename.
    let tmp = {
        let mut p = output.to_path_buf();
        let mut fname = p
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        fname.push_str(".tmp");
        p.set_file_name(fname);
        p
    };
    std::fs::write(&tmp, &ciphertext).map_err(KlefError::IndexWrite)?;
    std::fs::rename(&tmp, output).map_err(KlefError::IndexWrite)?;

    println!(
        "✓ backup written: {entry_count} entries → {}",
        output.display()
    );
    Ok(())
}

/// Encrypt `plaintext` with age, using a passphrase (if `recipients` is empty)
/// or the provided public-key recipients.
///
/// # Errors
///
/// Returns an error if the passphrase prompts fail, a recipient key is invalid,
/// or the encryption operation fails.
pub fn encrypt(plaintext: &[u8], recipients: &[String]) -> Result<Vec<u8>, KlefError> {
    if recipients.is_empty() {
        let passphrase = prompt_passphrase(true)?;
        let encryptor = age::Encryptor::with_user_passphrase(passphrase);
        let mut out = Vec::new();
        let mut writer = encryptor
            .wrap_output(&mut out)
            .map_err(|e| backup_err("age encrypt init", &e))?;
        writer.write_all(plaintext).map_err(KlefError::Io)?;
        writer
            .finish()
            .map_err(|e| backup_err("age encrypt finish", &e))?;
        Ok(out)
    } else {
        let parsed: Result<Vec<age::x25519::Recipient>, _> = recipients
            .iter()
            .map(|s| {
                s.parse::<age::x25519::Recipient>()
                    .map_err(|e| backup_err(&format!("invalid recipient '{s}'"), &e))
            })
            .collect();
        let parsed = parsed?;
        let encryptor =
            age::Encryptor::with_recipients(parsed.iter().map(|r| r as &dyn age::Recipient))
                .map_err(|e| backup_err("age recipients", &e))?;
        let mut out = Vec::new();
        let mut writer = encryptor
            .wrap_output(&mut out)
            .map_err(|e| backup_err("age encrypt init", &e))?;
        writer.write_all(plaintext).map_err(KlefError::Io)?;
        writer
            .finish()
            .map_err(|e| backup_err("age encrypt finish", &e))?;
        Ok(out)
    }
}

/// Prompt for a passphrase on stdin, with optional confirmation.
///
/// When stdin is a TTY, uses `rpassword` to hide input. Otherwise reads a
/// line from stdin directly (for use in pipes and tests).
///
/// # Errors
///
/// Returns an error if stdin cannot be read or the passphrases do not match.
pub fn prompt_passphrase(confirm: bool) -> Result<age::secrecy::SecretString, KlefError> {
    let pass = read_passphrase("Passphrase for backup: ")?;
    if confirm {
        let pass2 = read_passphrase("Confirm passphrase: ")?;
        if pass != pass2 {
            return Err(KlefError::BackendUnavailable(
                "passphrases do not match".to_string(),
            ));
        }
    }
    Ok(age::secrecy::SecretString::from(pass))
}

/// Read a single passphrase line.
///
/// If stdin is a TTY, uses `rpassword` (hides input). Otherwise reads a
/// newline-terminated line from stdin (useful in tests and pipes).
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

fn backup_err<E: std::fmt::Display>(ctx: &str, e: &E) -> KlefError {
    KlefError::BackendUnavailable(format!("{ctx}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_serializes_and_strict_schema_rejects_unknown_fields() {
        let bundle = Bundle {
            format_version: 1,
            tool: "klef".to_string(),
            klef_version: "0.2.0".to_string(),
            created_at: time::macros::datetime!(2026-05-06 12:00:00 UTC),
            source: BundleSource {
                hostname: "test-host".to_string(),
                platform: "macos".to_string(),
            },
            entries: vec![BundleEntry {
                name: "stripe-prod".to_string(),
                value: "sk_live_xxxxx".to_string(),
                keychain_service: "klef".to_string(),
                keychain_account: "stripe-prod".to_string(),
                env_var: "STRIPE_API_KEY".to_string(),
                note: Some("compte prod".to_string()),
                tags: vec![],
                added_at: time::macros::datetime!(2026-05-05 19:57:00 UTC),
                updated_at: time::macros::datetime!(2026-05-06 08:30:00 UTC),
            }],
        };

        // round-trip through serde_json
        let json = serde_json::to_string(&bundle).unwrap();
        let back: Bundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.format_version, 1);
        assert_eq!(back.tool, "klef");
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].name, "stripe-prod");

        // strict schema: unknown fields must be rejected
        let bad = r#"{"format_version":1,"tool":"klef","klef_version":"0.2.0",
            "created_at":"2026-05-06T12:00:00Z",
            "source":{"hostname":"h","platform":"macos"},
            "entries":[],"unknown_field":"oops"}"#;
        assert!(
            serde_json::from_str::<Bundle>(bad).is_err(),
            "expected rejection of unknown field"
        );
    }

    #[test]
    fn detect_platform_returns_known_string() {
        let p = detect_platform();
        assert!(
            p == "macos" || p == "linux" || p == "other",
            "unexpected platform: {p}"
        );
    }
}
