//! Low-level age encrypt/decrypt helpers, extracted from `age_backend` so
//! that file stays under the 300-line soft cap.

use crate::error::KlefError;
use age::secrecy::SecretString;
use std::io::{Read, Write};
use zeroize::Zeroizing;

pub(super) fn age_encrypt(plaintext: &[u8], pass: &SecretString) -> Result<Vec<u8>, KlefError> {
    let encryptor = age::Encryptor::with_user_passphrase(pass.clone());
    let mut out = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut out)
        .map_err(|e| KlefError::BackendUnavailable(format!("age encrypt init: {e}")))?;
    writer.write_all(plaintext).map_err(KlefError::Io)?;
    writer
        .finish()
        .map_err(|e| KlefError::BackendUnavailable(format!("age encrypt finish: {e}")))?;
    Ok(out)
}

pub(super) fn age_decrypt(
    ciphertext: &[u8],
    pass: &SecretString,
) -> Result<Zeroizing<Vec<u8>>, KlefError> {
    let identity = age::scrypt::Identity::new(pass.clone());
    let decryptor = age::Decryptor::new(ciphertext)
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt init: {e}")))?;
    let mut output = Zeroizing::new(Vec::new());
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt: {e}")))?;
    reader.read_to_end(&mut output).map_err(KlefError::Io)?;
    Ok(output)
}
