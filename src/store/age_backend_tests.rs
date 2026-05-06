// This file is `include!`'d into `age_backend::tests` so it can access
// private types (`AgeBackendInner`, `State`). It is NOT a module on its own.
// See `src/store/age_backend.rs` for the `#[cfg(test)] mod tests { include!(…) }`.
//
// Included context provides: `use super::*;` → all of age_backend's items.

use tempfile::tempdir;

fn set_passphrase(b: &AgeBackend, pass: &str) {
    b.inner.state.lock().unwrap().passphrase = Some(SecretString::from(pass.to_string()));
}

#[test]
fn round_trip_with_passphrase() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p.clone());
    set_passphrase(&b, "secret");

    b.set("k", "v").unwrap();
    assert_eq!(b.get("k").unwrap(), "v");

    let b2 = AgeBackend::new(p);
    set_passphrase(&b2, "secret");
    assert_eq!(b2.get("k").unwrap(), "v");
}

#[test]
fn wrong_passphrase_returns_error() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p.clone());
    set_passphrase(&b, "right");
    b.set("k", "v").unwrap();

    let b2 = AgeBackend::new(p);
    set_passphrase(&b2, "wrong");
    assert!(matches!(b2.get("k"), Err(KlefError::BackendUnavailable(_))));
}

#[test]
fn missing_key_is_keynotfound() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p);
    set_passphrase(&b, "x");
    b.set("a", "1").unwrap();
    assert!(matches!(b.get("nope"), Err(KlefError::KeyNotFound(_))));
}

#[test]
fn remove_then_get_fails() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p);
    set_passphrase(&b, "x");
    b.set("k", "v").unwrap();
    b.remove("k").unwrap();
    assert!(matches!(b.get("k"), Err(KlefError::KeyNotFound(_))));
}

#[test]
fn ciphertext_does_not_contain_value() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p.clone());
    set_passphrase(&b, "x");
    b.set("api-key", "sk_live_super_secret_value_xyz").unwrap();
    let bytes = std::fs::read(&p).unwrap();
    assert!(
        !bytes.windows(15).any(|w| w == b"sk_live_super_s"),
        "ciphertext leaked plaintext value"
    );
}

// New tests for embedded-metadata behaviour.

#[test]
fn vault_round_trips_metadata_through_encryption() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p.clone());
    set_passphrase(&b, "test");

    b.set("stripe", "sk_live").unwrap();

    let mut data = b.load_index().unwrap();
    data.keys.insert(
        "stripe".into(),
        KeyMeta {
            env_var: "STRIPE_KEY".into(),
            note: Some("prod".into()),
            tags: vec!["billing".into()],
            added_at: time::macros::datetime!(2026-05-06 0:00 UTC),
            updated_at: time::macros::datetime!(2026-05-06 0:00 UTC),
        },
    );
    b.save_index(&data).unwrap();

    let b2 = AgeBackend::new(p);
    set_passphrase(&b2, "test");
    assert_eq!(b2.get("stripe").unwrap(), "sk_live");
    let reloaded = b2.load_index().unwrap();
    let m = reloaded.keys.get("stripe").unwrap();
    assert_eq!(m.note.as_deref(), Some("prod"));
    assert_eq!(m.tags, vec!["billing".to_string()]);
    assert_eq!(m.env_var, "STRIPE_KEY");
}

#[test]
fn vault_metadata_not_in_ciphertext() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");
    let b = AgeBackend::new(p.clone());
    set_passphrase(&b, "test");

    b.set("stripe-prod", "sk_live").unwrap();
    let mut data = b.load_index().unwrap();
    data.keys.insert(
        "stripe-prod".into(),
        KeyMeta {
            env_var: "STRIPE_KEY".into(),
            note: Some("UNIQUE_NOTE_MARKER".into()),
            tags: vec![],
            added_at: time::macros::datetime!(2026-05-06 0:00 UTC),
            updated_at: time::macros::datetime!(2026-05-06 0:00 UTC),
        },
    );
    b.save_index(&data).unwrap();

    let bytes = std::fs::read(&p).unwrap();
    assert!(
        !bytes
            .windows(b"UNIQUE_NOTE_MARKER".len())
            .any(|w| w == b"UNIQUE_NOTE_MARKER"),
        "metadata note leaked plaintext into ciphertext"
    );
    assert!(
        !bytes
            .windows(b"stripe-prod".len())
            .any(|w| w == b"stripe-prod"),
        "metadata key name leaked plaintext into ciphertext"
    );
}

#[test]
fn legacy_vault_without_index_field_loads_with_synthesized_metadata() {
    let d = tempdir().unwrap();
    let p = d.path().join("v.age");

    let pass = SecretString::from("test".to_string());
    let raw = serde_json::json!({"secrets": {"foo": "bar"}});
    let plaintext = serde_json::to_vec(&raw).unwrap();
    let cipher = age_encrypt(&plaintext, &pass).unwrap();
    std::fs::write(&p, &cipher).unwrap();

    let b = AgeBackend::new(p);
    b.inner.state.lock().unwrap().passphrase = Some(pass);

    let data = b.load_index().unwrap();
    assert!(data.keys.contains_key("foo"), "synthesized metadata missing");
    assert_eq!(data.keys["foo"].env_var, "FOO_API_KEY");
    assert_eq!(b.get("foo").unwrap(), "bar");
}
