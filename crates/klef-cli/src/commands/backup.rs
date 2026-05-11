//! `klef backup` — encrypt and dump the entire vault to an age file.
//!
//! The bundle schema (`Bundle`, `BundleEntry`, `BundleSource`) lives in
//! `klef_core::backup` so the GUI and a future MCP server can produce the
//! same on-disk format. This module owns only the age encryption, file I/O,
//! and TTY prompts.

use klef_core::backup::Bundle;
use klef_core::error::KlefError;
use klef_core::store::Store;
use std::io::Write as _;
use std::path::Path;
use zeroize::Zeroizing;

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
    // 0600 on the backup .tmp + final file. Ciphertext only, but narrowing
    // read access keeps the file from sitting world-readable on shared hosts.
    klef_core::fsx::write_private(&tmp, &ciphertext).map_err(KlefError::IndexWrite)?;
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
