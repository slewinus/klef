//! Reading the encrypted side of a klef backup: passphrase or age identity.
//!
//! Split out of `restore` so that file stays under the 300-line cap, the same
//! way `klef_core::store::age_crypto` is split out of `age_backend`.

use klef_core::error::KlefError;
use std::io::Read as _;
use zeroize::Zeroizing;

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

/// Decrypt an age ciphertext, using identity files when the backup was made
/// with `--recipient`, or a prompted passphrase otherwise.
///
/// The two modes are mutually exclusive at the file level: an age file is
/// either scrypt (passphrase) or recipient-encrypted, and klef reads which one
/// it is rather than guessing from the flags.
///
/// # Errors
///
/// Returns an error if the file is not a valid age file, if the passphrase is
/// wrong, if `--identity` is missing for a recipient-encrypted backup (or
/// supplied for a passphrase one), or if no supplied identity can open it.
pub(super) fn age_decrypt(
    ciphertext: &[u8],
    identities: &[std::path::PathBuf],
) -> Result<Zeroizing<Vec<u8>>, KlefError> {
    let decryptor = age::Decryptor::new(ciphertext)
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt init: {e}")))?;

    let mut reader = if decryptor.is_scrypt() {
        if !identities.is_empty() {
            return Err(KlefError::BackendUnavailable(
                "this backup is passphrase-encrypted; --identity does not apply. \
                 Re-run without --identity."
                    .to_string(),
            ));
        }
        let pass = read_passphrase("Passphrase: ")?;
        let identity = age::scrypt::Identity::new(age::secrecy::SecretString::from(pass));
        decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt: {e}")))?
    } else {
        if identities.is_empty() {
            return Err(KlefError::BackendUnavailable(
                "this backup was encrypted to a recipient (`klef backup --recipient`), \
                 so it needs the matching private key: \
                 klef restore <file> --identity <KEYFILE>"
                    .to_string(),
            ));
        }
        let loaded = load_identities(identities)?;
        decryptor
            .decrypt(loaded.iter().map(AsRef::as_ref))
            .map_err(|e| {
                KlefError::BackendUnavailable(format!(
                    "age decrypt: {e}. None of the supplied identities match the \
                     recipient this backup was encrypted to."
                ))
            })?
    };

    let mut output = Zeroizing::new(Vec::new());
    reader.read_to_end(&mut output).map_err(KlefError::Io)?;
    Ok(output)
}

/// Load every age identity from the given key files, in order.
fn load_identities(paths: &[std::path::PathBuf]) -> Result<Vec<Box<dyn age::Identity>>, KlefError> {
    let mut all: Vec<Box<dyn age::Identity>> = Vec::new();
    for p in paths {
        // `IdentityFile::from_file` takes an owned String path; a non-UTF-8
        // path can't be expressed, so reject it with a clear message rather
        // than a lossy conversion that would open the wrong file.
        let as_str = p.to_str().ok_or_else(|| {
            KlefError::BackendUnavailable(format!(
                "identity path is not valid UTF-8: {}",
                p.display()
            ))
        })?;
        let file = age::IdentityFile::from_file(as_str.to_string()).map_err(|e| {
            KlefError::BackendUnavailable(format!("cannot read identity {}: {e}", p.display()))
        })?;
        let ids = file.into_identities().map_err(|e| {
            KlefError::BackendUnavailable(format!("invalid identity {}: {e}", p.display()))
        })?;
        all.extend(ids);
    }
    if all.is_empty() {
        return Err(KlefError::BackendUnavailable(
            "the supplied identity file(s) contain no age keys".to_string(),
        ));
    }
    Ok(all)
}
