# klef MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the v0.1 of klef — a local-first CLI vault for API keys backed by the OS keychain, with `klef:` reference resolution in `.env` files via `klef run`.

**Architecture:** Bin + lib, layered. `src/main.rs` is a thin wrapper that parses args and delegates to the library entrypoint. A `Backend` trait abstracts the secret store (`KeychainBackend` in prod, `MemoryBackend` for in-process tests, `FileBackend` for cross-process E2E). A separate `IndexFile` persists metadata (env-var mapping, notes, timestamps) atomically as JSON. Each CLI command is its own module under `commands/`, all consuming a shared `Store` that pairs a `Backend` with an `IndexFile`. `klef run` parses `.env`, replaces `klef:<name>` references, and `execvp`s the wrapped command.

**Tech Stack:** Rust 2024, `clap` (derive), `keyring`, `rpassword`, `serde`/`serde_json`, `thiserror`, `time`, `dirs`. Tests: `assert_cmd`, `predicates`, `tempfile`.

**Reference spec:** `docs/design/2026-05-05-mvp-design.md`. Re-read it before starting any task — every decision (file paths, command shape, error variants) is defended there.

---

## File Structure

| Path | Responsibility |
|---|---|
| `src/main.rs` | Thin wrapper: parse args, call `klef::run()`, print top-level errors. |
| `src/cli.rs` | `clap` derive structs (Cli, Commands enum). Pure data, no logic. |
| `src/error.rs` | `KlefError` enum + exit-code mapping. |
| `src/store/mod.rs` | `Store` struct (backend + index), public API for commands. |
| `src/store/backend.rs` | `Backend` trait + `MemoryBackend` (test impl). |
| `src/store/keychain.rs` | `KeychainBackend` (wraps `keyring::Entry`). |
| `src/store/index.rs` | `IndexFile` (read/write/atomic), `KeyMeta` struct. |
| `src/envfile.rs` | `.env` parser + `klef:` reference resolution. |
| `src/commands/mod.rs` | Re-exports each command function. |
| `src/commands/add.rs` | `klef add` |
| `src/commands/get.rs` | `klef get` and `klef show` (closely related, share file). |
| `src/commands/list.rs` | `klef list` |
| `src/commands/rm.rs` | `klef rm` |
| `src/commands/edit.rs` | `klef edit` |
| `src/commands/rename.rs` | `klef rename` |
| `src/commands/export.rs` | `klef export` |
| `src/commands/run.rs` | `klef run` |
| `tests/cli.rs` | E2E `assert_cmd` tests using `FileBackend` via `KLEF_TEST_BACKEND=file:...`. |

Each non-doc file stays under 300 lines (enforced by `scripts/check-lines.sh` at pre-commit).

---

## Workflow Conventions

- **TDD strictly**: every step pair starts with a failing test before the implementation.
- **Commit cadence**: after each task passes its tests. Pre-commit hook will run `check-lines.sh` + `cargo fmt --check` + `cargo clippy -D warnings`.
- **Run `cargo fmt`** before committing if you've made changes (the Claude Code PostToolUse hook handles this automatically; the manual `cargo fmt --all` is a safety net).
- **`MemoryBackend` is for in-process Rust tests**; **`FileBackend` is for cross-process smoke/E2E tests**. Real keychain is verified manually only.
- **Conventional commit prefixes**: `feat:`, `test:`, `refactor:`, `chore:`, `docs:`.

---

## Task 1: Cargo Dependencies + Bin/Lib Split + Error Type

**Files:**
- Modify: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/error.rs`
- Modify: `src/main.rs`

The crate is set up as **bin + lib**: all logic lives in the library (`src/lib.rs` and the modules it declares), `src/main.rs` is a thin wrapper that parses args and prints errors. This pattern is needed so unit tests + integration tests can both reach the library API and so `cargo test --lib <path>` works as the plan assumes.

- [ ] **Step 1.1: Add runtime dependencies to `Cargo.toml`**

Append under `[dependencies]`:

```toml
clap = { version = "4", features = ["derive"] }
keyring = "3"
rpassword = "7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
time = { version = "0.3", features = ["serde", "formatting", "macros", "parsing"] }
dirs = "5"
```

Add a `[dev-dependencies]` section:

```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

Run: `cargo build`. Expected: compiles (with the placeholder `main.rs`).

- [ ] **Step 1.2: Write the failing test for error → exit code mapping**

Create `src/error.rs`:

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KlefError {
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("backend access denied")]
    BackendDenied,
    #[error("index file corrupt at {path}: {reason}")]
    IndexCorrupt { path: PathBuf, reason: String },
    #[error("failed to write index: {0}")]
    IndexWrite(std::io::Error),
    #[error("i/o error: {0}")]
    Io(std::io::Error),
    #[error("key '{0}' not found")]
    KeyNotFound(String),
    #[error("key '{0}' already exists (use --force to overwrite)")]
    KeyAlreadyExists(String),
    #[error("invalid key name '{0}': must be alphanumeric, dash, or underscore")]
    InvalidKeyName(String),
    #[error("env file not found: {0}")]
    EnvFileNotFound(PathBuf),
    #[error("broken reference: {var}=klef:{key} — key not found")]
    BrokenReference { var: String, key: String },
}

impl KlefError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::BackendUnavailable(_) | Self::BackendDenied => 4,
            Self::KeyNotFound(_) => 2,
            Self::BrokenReference { .. } => 3,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_for_key_not_found_is_2() {
        let e = KlefError::KeyNotFound("stripe".into());
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn exit_code_for_broken_ref_is_3() {
        let e = KlefError::BrokenReference {
            var: "STRIPE_KEY".into(),
            key: "stripe".into(),
        };
        assert_eq!(e.exit_code(), 3);
    }

    #[test]
    fn exit_code_for_backend_is_4() {
        let e = KlefError::BackendDenied;
        assert_eq!(e.exit_code(), 4);
    }
}
```

- [ ] **Step 1.3: Create the library entrypoint `src/lib.rs`**

```rust
pub mod error;

// Re-export the main entrypoint once Task 6 lands lib::run().
```

- [ ] **Step 1.4: Make `src/main.rs` a thin wrapper**

Replace `src/main.rs` with:

```rust
fn main() {
    println!("klef placeholder — implementation in progress");
}
```

(Task 6 will replace this with the real `Cli::parse() → klef::run()` flow.)

- [ ] **Step 1.5: Declare the binary target explicitly in `Cargo.toml`**

Append to `Cargo.toml` so cargo builds both the library and the binary:

```toml
[lib]
name = "klef"
path = "src/lib.rs"

[[bin]]
name = "klef"
path = "src/main.rs"
```

- [ ] **Step 1.6: Run the tests**

Run: `cargo test --lib error`
Expected: 3 tests pass (resolved through the library target).

- [ ] **Step 1.7: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/lib.rs src/error.rs
git commit -m "feat: scaffold bin+lib crate with KlefError enum"
```

---

## Task 2: Backend Trait + MemoryBackend + FileBackend

**Files:**
- Modify: `src/lib.rs`
- Create: `src/store/mod.rs`
- Create: `src/store/backend.rs`
- Create: `src/store/file.rs`

`MemoryBackend` is for in-process unit tests. `FileBackend` is for cross-process E2E tests (and is the foundation for v0.3's encrypted file backend) — it persists a JSON map to disk so multiple `cargo run` invocations can share state.

- [ ] **Step 2.1: Declare the module from `lib.rs`**

Add to `src/lib.rs`:

```rust
pub mod store;
```

Create `src/store/mod.rs`:

```rust
pub mod backend;
pub mod file;

pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
```

- [ ] **Step 2.2: Write failing tests for `MemoryBackend`**

Create `src/store/backend.rs`:

```rust
use crate::error::KlefError;

pub trait Backend: Send + Sync {
    fn get(&self, name: &str) -> Result<String, KlefError>;
    fn set(&self, name: &str, value: &str) -> Result<(), KlefError>;
    fn remove(&self, name: &str) -> Result<(), KlefError>;
}

#[derive(Default)]
pub struct MemoryBackend {
    inner: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Backend for MemoryBackend {
    fn get(&self, name: &str) -> Result<String, KlefError> {
        self.inner
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        self.inner
            .lock()
            .unwrap()
            .insert(name.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let mut g = self.inner.lock().unwrap();
        g.remove(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_get_returns_value() {
        let b = MemoryBackend::new();
        b.set("stripe", "sk_live_xyz").unwrap();
        assert_eq!(b.get("stripe").unwrap(), "sk_live_xyz");
    }

    #[test]
    fn get_missing_returns_key_not_found() {
        let b = MemoryBackend::new();
        assert!(matches!(b.get("nope"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_then_get_returns_not_found() {
        let b = MemoryBackend::new();
        b.set("k", "v").unwrap();
        b.remove("k").unwrap();
        assert!(matches!(b.get("k"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_missing_returns_not_found() {
        let b = MemoryBackend::new();
        assert!(matches!(b.remove("nope"), Err(KlefError::KeyNotFound(_))));
    }
}
```

- [ ] **Step 2.3: Implement `FileBackend` for cross-process tests**

Create `src/store/file.rs`:

```rust
use crate::error::KlefError;
use crate::store::backend::Backend;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default, Serialize, Deserialize)]
struct FileData {
    secrets: BTreeMap<String, String>,
}

pub struct FileBackend {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileBackend {
    pub fn new(path: PathBuf) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    fn load(&self) -> Result<FileData, KlefError> {
        if !self.path.exists() {
            return Ok(FileData::default());
        }
        let bytes = std::fs::read(&self.path).map_err(KlefError::Io)?;
        serde_json::from_slice(&bytes).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })
    }

    fn save(&self, data: &FileData) -> Result<(), KlefError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::IndexWrite)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(data).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })?;
        std::fs::write(&tmp, bytes).map_err(KlefError::IndexWrite)?;
        std::fs::rename(&tmp, &self.path).map_err(KlefError::IndexWrite)?;
        Ok(())
    }
}

impl Backend for FileBackend {
    fn get(&self, name: &str) -> Result<String, KlefError> {
        let _g = self.lock.lock().unwrap();
        let data = self.load()?;
        data.secrets
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        let _g = self.lock.lock().unwrap();
        let mut data = self.load()?;
        data.secrets.insert(name.to_string(), value.to_string());
        self.save(&data)
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let _g = self.lock.lock().unwrap();
        let mut data = self.load()?;
        data.secrets
            .remove(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        self.save(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_persists_to_disk() {
        let d = tempdir().unwrap();
        let p = d.path().join("secrets.json");
        let b = FileBackend::new(p.clone());
        b.set("k", "v").unwrap();

        // Simulate another process opening the same file.
        let b2 = FileBackend::new(p);
        assert_eq!(b2.get("k").unwrap(), "v");
    }

    #[test]
    fn missing_key_returns_not_found() {
        let d = tempdir().unwrap();
        let b = FileBackend::new(d.path().join("s.json"));
        assert!(matches!(b.get("nope"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_then_get_fails() {
        let d = tempdir().unwrap();
        let b = FileBackend::new(d.path().join("s.json"));
        b.set("k", "v").unwrap();
        b.remove("k").unwrap();
        assert!(matches!(b.get("k"), Err(KlefError::KeyNotFound(_))));
    }
}
```

> **Note:** `FileBackend` stores values as plaintext JSON. That's fine for tests and for the v0.3 stepping stone — encryption (age) wraps this layer in v0.3 without changing its API.

- [ ] **Step 2.4: Run the tests**

Run: `cargo test --lib store`
Expected: 4 + 3 = 7 tests pass.

- [ ] **Step 2.5: Commit**

```bash
git add src/lib.rs src/store
git commit -m "feat: add Backend trait, MemoryBackend, FileBackend for testing"
```

---

## Task 3: KeychainBackend

**Files:**
- Create: `src/store/keychain.rs`
- Modify: `src/store/mod.rs`

- [ ] **Step 3.1: Implement `KeychainBackend` wrapping `keyring`**

Create `src/store/keychain.rs`:

```rust
use crate::error::KlefError;
use crate::store::backend::Backend;

pub struct KeychainBackend {
    service: String,
}

impl KeychainBackend {
    pub fn new() -> Self {
        Self { service: "klef".to_string() }
    }
}

impl Default for KeychainBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn map_err(e: keyring::Error) -> KlefError {
    use keyring::Error::*;
    match e {
        NoEntry => KlefError::KeyNotFound(String::new()),
        PlatformFailure(msg) | NoStorageAccess(msg) => {
            KlefError::BackendUnavailable(msg.to_string())
        }
        _ => KlefError::BackendUnavailable(e.to_string()),
    }
}

impl Backend for KeychainBackend {
    fn get(&self, name: &str) -> Result<String, KlefError> {
        let entry = keyring::Entry::new(&self.service, name).map_err(map_err)?;
        entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => KlefError::KeyNotFound(name.to_string()),
            other => map_err(other),
        })
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        let entry = keyring::Entry::new(&self.service, name).map_err(map_err)?;
        entry.set_password(value).map_err(map_err)
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let entry = keyring::Entry::new(&self.service, name).map_err(map_err)?;
        entry.delete_credential().map_err(|e| match e {
            keyring::Error::NoEntry => KlefError::KeyNotFound(name.to_string()),
            other => map_err(other),
        })
    }
}
```

- [ ] **Step 3.2: Re-export from `store/mod.rs`**

Update `src/store/mod.rs`:

```rust
pub mod backend;
pub mod file;
pub mod keychain;

pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
pub use keychain::KeychainBackend;
```

- [ ] **Step 3.3: Verify it compiles (no automated test — keychain is stubbed in CI)**

Run: `cargo build` and `cargo clippy --all-targets -- -D warnings`
Expected: clean build.

- [ ] **Step 3.4: Commit**

```bash
git add src/store
git commit -m "feat: add KeychainBackend wrapping the `keyring` crate"
```

---

## Task 4: Index File (metadata persistence)

**Files:**
- Create: `src/store/index.rs`
- Modify: `src/store/mod.rs`

- [ ] **Step 4.1: Write failing tests for round-trip + atomic write**

Create `src/store/index.rs`:

```rust
use crate::error::KlefError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyMeta {
    pub env_var: String,
    pub note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexData {
    pub version: u32,
    pub keys: BTreeMap<String, KeyMeta>,
}

impl Default for IndexData {
    fn default() -> Self {
        Self { version: 1, keys: BTreeMap::new() }
    }
}

pub struct IndexFile {
    path: PathBuf,
}

impl IndexFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<IndexData, KlefError> {
        if !self.path.exists() {
            return Ok(IndexData::default());
        }
        let bytes = std::fs::read(&self.path).map_err(KlefError::IndexWrite)?;
        serde_json::from_slice(&bytes).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })
    }

    pub fn save(&self, data: &IndexData) -> Result<(), KlefError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::IndexWrite)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(data).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })?;
        std::fs::write(&tmp, bytes).map_err(KlefError::IndexWrite)?;
        std::fs::rename(&tmp, &self.path).map_err(KlefError::IndexWrite)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn sample_meta() -> KeyMeta {
        KeyMeta {
            env_var: "STRIPE_API_KEY".into(),
            note: Some("prod".into()),
            added_at: datetime!(2026-05-05 19:57:00 UTC),
            updated_at: datetime!(2026-05-05 19:57:00 UTC),
        }
    }

    #[test]
    fn load_missing_returns_empty() {
        let dir = tempdir().unwrap();
        let f = IndexFile::new(dir.path().join("index.json"));
        let data = f.load().unwrap();
        assert_eq!(data.version, 1);
        assert!(data.keys.is_empty());
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempdir().unwrap();
        let f = IndexFile::new(dir.path().join("index.json"));
        let mut data = IndexData::default();
        data.keys.insert("stripe".into(), sample_meta());
        f.save(&data).unwrap();
        let reloaded = f.load().unwrap();
        assert_eq!(reloaded, data);
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let f = IndexFile::new(dir.path().join("nested/sub/index.json"));
        f.save(&IndexData::default()).unwrap();
        assert!(f.path().exists());
    }

    #[test]
    fn load_corrupt_returns_index_corrupt() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("index.json");
        std::fs::write(&p, b"not json").unwrap();
        let f = IndexFile::new(p);
        assert!(matches!(f.load(), Err(KlefError::IndexCorrupt { .. })));
    }
}
```

- [ ] **Step 4.2: Re-export from `store/mod.rs`**

Update `src/store/mod.rs`:

```rust
pub mod backend;
pub mod file;
pub mod index;
pub mod keychain;

pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
pub use index::{IndexData, IndexFile, KeyMeta};
pub use keychain::KeychainBackend;
```

- [ ] **Step 4.3: Run the tests**

Run: `cargo test --lib store::index`
Expected: 4 tests pass.

- [ ] **Step 4.4: Commit**

```bash
git add src/store
git commit -m "feat: add IndexFile with atomic write and round-trip tests"
```

---

## Task 5: Store (combine Backend + IndexFile)

**Files:**
- Modify: `src/store/mod.rs`

- [ ] **Step 5.1: Write failing tests for `Store` API**

Replace `src/store/mod.rs` with:

```rust
pub mod backend;
pub mod file;
pub mod index;
pub mod keychain;

pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
pub use index::{IndexData, IndexFile, KeyMeta};
pub use keychain::KeychainBackend;

use crate::error::KlefError;
use std::path::PathBuf;
use time::OffsetDateTime;

pub struct Store {
    backend: Box<dyn Backend>,
    index: IndexFile,
}

impl Store {
    pub fn new(backend: Box<dyn Backend>, index_path: PathBuf) -> Self {
        Self { backend, index: IndexFile::new(index_path) }
    }

    pub fn add(
        &self,
        name: &str,
        value: &str,
        env_var: Option<String>,
        note: Option<String>,
        force: bool,
    ) -> Result<(), KlefError> {
        validate_name(name)?;
        let mut data = self.index.load()?;
        if data.keys.contains_key(name) && !force {
            return Err(KlefError::KeyAlreadyExists(name.to_string()));
        }
        let now = OffsetDateTime::now_utc();
        let meta = KeyMeta {
            env_var: env_var.unwrap_or_else(|| default_env_var(name)),
            note,
            added_at: data.keys.get(name).map_or(now, |k| k.added_at),
            updated_at: now,
        };
        self.backend.set(name, value)?;
        data.keys.insert(name.to_string(), meta);
        self.index.save(&data)?;
        Ok(())
    }

    pub fn get_value(&self, name: &str) -> Result<String, KlefError> {
        let data = self.index.load()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        self.backend.get(name)
    }

    pub fn list(&self) -> Result<Vec<(String, KeyMeta)>, KlefError> {
        let data = self.index.load()?;
        Ok(data.keys.into_iter().collect())
    }

    pub fn remove(&self, name: &str) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        // Backend delete is best-effort (key may already be gone manually).
        let _ = self.backend.remove(name);
        data.keys.remove(name);
        self.index.save(&data)?;
        Ok(())
    }

    pub fn meta(&self, name: &str) -> Result<KeyMeta, KlefError> {
        let data = self.index.load()?;
        data.keys
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    pub fn update_meta(
        &self,
        name: &str,
        env_var: Option<String>,
        note: Option<Option<String>>,
    ) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        let meta = data
            .keys
            .get_mut(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        if let Some(v) = env_var {
            meta.env_var = v;
        }
        if let Some(n) = note {
            meta.note = n;
        }
        meta.updated_at = OffsetDateTime::now_utc();
        self.index.save(&data)?;
        Ok(())
    }

    pub fn rename(&self, old: &str, new: &str) -> Result<(), KlefError> {
        validate_name(new)?;
        let mut data = self.index.load()?;
        if !data.keys.contains_key(old) {
            return Err(KlefError::KeyNotFound(old.to_string()));
        }
        if data.keys.contains_key(new) {
            return Err(KlefError::KeyAlreadyExists(new.to_string()));
        }
        let value = self.backend.get(old)?;
        self.backend.set(new, &value)?;
        let _ = self.backend.remove(old);
        let mut meta = data.keys.remove(old).unwrap();
        meta.updated_at = OffsetDateTime::now_utc();
        data.keys.insert(new.to_string(), meta);
        self.index.save(&data)?;
        Ok(())
    }
}

fn default_env_var(name: &str) -> String {
    let upper: String = name
        .chars()
        .map(|c| if c == '-' { '_' } else { c.to_ascii_uppercase() })
        .collect();
    format!("{upper}_API_KEY")
}

fn validate_name(name: &str) -> Result<(), KlefError> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(KlefError::InvalidKeyName(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_store() -> (Store, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let s = Store::new(Box::new(MemoryBackend::new()), dir.path().join("index.json"));
        (s, dir)
    }

    #[test]
    fn add_then_get_round_trip() {
        let (s, _d) = make_store();
        s.add("stripe", "sk_live", None, None, false).unwrap();
        assert_eq!(s.get_value("stripe").unwrap(), "sk_live");
    }

    #[test]
    fn add_existing_without_force_fails() {
        let (s, _d) = make_store();
        s.add("stripe", "v1", None, None, false).unwrap();
        let r = s.add("stripe", "v2", None, None, false);
        assert!(matches!(r, Err(KlefError::KeyAlreadyExists(_))));
    }

    #[test]
    fn add_existing_with_force_overwrites() {
        let (s, _d) = make_store();
        s.add("stripe", "v1", None, None, false).unwrap();
        s.add("stripe", "v2", None, None, true).unwrap();
        assert_eq!(s.get_value("stripe").unwrap(), "v2");
    }

    #[test]
    fn default_env_var_uses_api_key_suffix() {
        assert_eq!(default_env_var("stripe"), "STRIPE_API_KEY");
        assert_eq!(default_env_var("stripe-prod"), "STRIPE_PROD_API_KEY");
    }

    #[test]
    fn rename_moves_value_and_meta() {
        let (s, _d) = make_store();
        s.add("a", "v", None, None, false).unwrap();
        s.rename("a", "b").unwrap();
        assert!(matches!(s.get_value("a"), Err(KlefError::KeyNotFound(_))));
        assert_eq!(s.get_value("b").unwrap(), "v");
    }

    #[test]
    fn invalid_name_rejected() {
        let (s, _d) = make_store();
        let r = s.add("has space", "v", None, None, false);
        assert!(matches!(r, Err(KlefError::InvalidKeyName(_))));
    }

    #[test]
    fn remove_clears_both_layers() {
        let (s, _d) = make_store();
        s.add("k", "v", None, None, false).unwrap();
        s.remove("k").unwrap();
        assert!(matches!(s.get_value("k"), Err(KlefError::KeyNotFound(_))));
        assert!(s.list().unwrap().is_empty());
    }
}
```

- [ ] **Step 5.2: Run the tests**

Run: `cargo test --lib store`
Expected: all `store` tests pass (incl. previous 4+4 + new 7 = 15).

- [ ] **Step 5.3: Commit**

```bash
git add src/store/mod.rs
git commit -m "feat: add Store coordinating Backend + IndexFile"
```

---

## Task 6: CLI Scaffold (clap derive)

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 6.1: Define CLI structure with clap**

Create `src/cli.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "klef", version, about = "Local-first vault for API keys.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add a new key. Reads value from a TTY prompt or stdin.
    Add {
        name: String,
        #[arg(long, value_name = "VAR")]
        r#as: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Print the value of a key on stdout.
    Get { name: String },
    /// Display a key's value formatted for human reading.
    Show { name: String },
    /// List stored keys (names + metadata, never values).
    List {
        #[arg(long, value_enum, default_value_t = ListFormat::Table)]
        format: ListFormat,
    },
    /// Remove a key.
    Rm {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Edit a key (prompts for new value if no flag given).
    Edit {
        name: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, value_name = "VAR")]
        r#as: Option<String>,
    },
    /// Rename a key.
    Rename { old: String, new: String },
    /// Print `export VAR=value` lines for eval.
    Export {
        names: Vec<String>,
        #[arg(long, value_enum, default_value_t = ExportFormat::Shell)]
        format: ExportFormat,
    },
    /// Run a command with `klef:<name>` references in `.env` resolved.
    Run {
        #[arg(long, value_name = "FILE", default_value = ".env")]
        env_file: PathBuf,
        #[arg(last = true)]
        cmd: Vec<String>,
    },
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ListFormat {
    Table,
    Json,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ExportFormat {
    Shell,
    Dotenv,
}
```

- [ ] **Step 6.2: Wire dispatch in `lib.rs` (logic) and slim `main.rs` (entrypoint)**

Replace `src/lib.rs`:

```rust
pub mod cli;
pub mod commands;
pub mod envfile;
pub mod error;
pub mod store;

use cli::{Cli, Command};
use error::KlefError;
use std::path::PathBuf;
use store::{Backend, FileBackend, KeychainBackend, Store};

pub fn run(cli: Cli) -> Result<(), KlefError> {
    let store = build_store()?;
    match cli.command {
        Command::Add { name, r#as, note, force } => {
            commands::add::run(&store, &name, r#as, note, force)
        }
        Command::Get { name } => commands::get::run_get(&store, &name),
        Command::Show { name } => commands::get::run_show(&store, &name),
        Command::List { format } => commands::list::run(&store, format),
        Command::Rm { name, yes } => commands::rm::run(&store, &name, yes),
        Command::Edit { name, note, r#as } => commands::edit::run(&store, &name, r#as, note),
        Command::Rename { old, new } => commands::rename::run(&store, &old, &new),
        Command::Export { names, format } => commands::export::run(&store, &names, format),
        Command::Run { env_file, cmd } => commands::run::run(&store, &env_file, &cmd),
    }
}

fn build_store() -> Result<Store, KlefError> {
    let index_path = index_path()?;
    let backend: Box<dyn Backend> = match std::env::var("KLEF_TEST_BACKEND").as_deref() {
        Ok(spec) if spec.starts_with("file:") => {
            Box::new(FileBackend::new(PathBuf::from(&spec[5..])))
        }
        _ => Box::new(KeychainBackend::new()),
    };
    Ok(Store::new(backend, index_path))
}

fn index_path() -> Result<PathBuf, KlefError> {
    if let Some(p) = std::env::var_os("KLEF_INDEX_PATH") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::config_dir().ok_or_else(|| {
        KlefError::BackendUnavailable("could not resolve config directory".into())
    })?;
    Ok(base.join("klef").join("index.json"))
}
```

Replace `src/main.rs`:

```rust
use clap::Parser;
use klef::cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match klef::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(1))
        }
    }
}
```

- [ ] **Step 6.3: Stub command modules so compile passes**

Create `src/commands/mod.rs`:

```rust
pub mod add;
pub mod edit;
pub mod export;
pub mod get;
pub mod list;
pub mod rename;
pub mod rm;
pub mod run;
```

For each of `add.rs`, `edit.rs`, `export.rs`, `get.rs`, `list.rs`, `rename.rs`, `rm.rs`, `run.rs`, create a stub that returns `Ok(())` matching the signatures above. Example for `src/commands/add.rs`:

```rust
use crate::error::KlefError;
use crate::store::Store;

pub fn run(
    _store: &Store,
    _name: &str,
    _env_var: Option<String>,
    _note: Option<String>,
    _force: bool,
) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
```

Stub the others with matching signatures. (`get.rs` exposes both `run_get` and `run_show`. `list.rs` takes `ListFormat`. `export.rs` takes `&[String]` and `ExportFormat`. `run.rs` takes `&Path` and `&[String]`.)

Create `src/envfile.rs` with a placeholder:

```rust
// envfile parser implemented in Task 14.
```

- [ ] **Step 6.4: Verify it compiles**

Run: `cargo build` and `cargo run -- --help`
Expected: clap renders help with all 9 subcommands.

- [ ] **Step 6.5: Commit**

```bash
git add src/
git commit -m "feat: add clap-based CLI scaffold with command stubs"
```

---

## Task 7: `klef add` (TDD)

**Files:**
- Modify: `src/commands/add.rs`

- [ ] **Step 7.1: Write failing test for the happy path**

Add to `src/commands/add.rs`:

```rust
use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Read};

pub fn run(
    store: &Store,
    name: &str,
    env_var: Option<String>,
    note: Option<String>,
    force: bool,
) -> Result<(), KlefError> {
    let value = read_value(name)?;
    store.add(name, value.trim(), env_var, note, force)?;
    println!("✓ '{name}' saved");
    Ok(())
}

fn read_value(name: &str) -> Result<String, KlefError> {
    if std::io::stdin().is_terminal() {
        let prompt = format!("Paste value for '{name}': ");
        let v = rpassword::prompt_password(prompt)
            .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;
        Ok(v)
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(KlefError::Io)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{MemoryBackend, Store};
    use tempfile::tempdir;

    fn store() -> (Store, tempfile::TempDir) {
        let d = tempdir().unwrap();
        (Store::new(Box::new(MemoryBackend::new()), d.path().join("i.json")), d)
    }

    #[test]
    fn add_persists_value_and_meta() {
        let (s, _d) = store();
        s.add("stripe", "v", None, Some("hi".into()), false).unwrap();
        let m = s.meta("stripe").unwrap();
        assert_eq!(m.env_var, "STRIPE_API_KEY");
        assert_eq!(m.note.as_deref(), Some("hi"));
        assert_eq!(s.get_value("stripe").unwrap(), "v");
    }
}
```

(Interactive prompt path is not unit-testable; covered by manual smoke test at the end of the plan.)

- [ ] **Step 7.2: Run the tests**

Run: `cargo test --lib commands::add`
Expected: 1 test passes.

- [ ] **Step 7.3: Smoke-test by piping a value**

Run: `KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/klef-smoke.json bash -c 'echo -n "sk_test" | cargo run -- add stripe'`
Expected: `✓ 'stripe' saved`. Verify: `cat /tmp/klef-smoke.json` shows the meta.

- [ ] **Step 7.4: Commit**

```bash
rm -f /tmp/klef-smoke.json /tmp/klef-smoke-secrets.json
git add src/commands/add.rs
git commit -m "feat: implement klef add with TTY prompt + stdin fallback"
```

---

## Task 8: `klef get` and `klef show`

**Files:**
- Modify: `src/commands/get.rs`

- [ ] **Step 8.1: Implement both with TTY-aware newline**

Replace `src/commands/get.rs`:

```rust
use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Write};

pub fn run_get(store: &Store, name: &str) -> Result<(), KlefError> {
    let value = store.get_value(name)?;
    let mut out = std::io::stdout().lock();
    out.write_all(value.as_bytes()).map_err(KlefError::Io)?;
    if std::io::stdout().is_terminal() {
        out.write_all(b"\n").map_err(KlefError::Io)?;
    }
    Ok(())
}

pub fn run_show(store: &Store, name: &str) -> Result<(), KlefError> {
    let value = store.get_value(name)?;
    let meta = store.meta(name)?;
    println!("name:    {name}");
    println!("env var: {}", meta.env_var);
    if let Some(note) = &meta.note {
        println!("note:    {note}");
    }
    println!("value:   {value}");
    Ok(())
}
```

- [ ] **Step 8.2: Verify it compiles + smoke**

Run: `KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n "sk_test" | cargo run -- add stripe &&
  cargo run -- get stripe &&
  cargo run -- show stripe'`
Expected: prints `sk_test`, then a `show` block.

- [ ] **Step 8.3: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json
git add src/commands/get.rs
git commit -m "feat: implement klef get (TTY-aware) and klef show"
```

---

## Task 9: `klef list`

**Files:**
- Modify: `src/commands/list.rs`

- [ ] **Step 9.1: Implement table + JSON formats**

Replace `src/commands/list.rs`:

```rust
use crate::cli::ListFormat;
use crate::error::KlefError;
use crate::store::Store;

pub fn run(store: &Store, format: ListFormat) -> Result<(), KlefError> {
    let entries = store.list()?;
    match format {
        ListFormat::Table => print_table(&entries),
        ListFormat::Json => print_json(&entries)?,
    }
    Ok(())
}

fn print_table(entries: &[(String, crate::store::KeyMeta)]) {
    if entries.is_empty() {
        println!("(no keys stored)");
        return;
    }
    let name_w = entries.iter().map(|(n, _)| n.len()).max().unwrap_or(4).max(4);
    let var_w = entries
        .iter()
        .map(|(_, m)| m.env_var.len())
        .max()
        .unwrap_or(7)
        .max(7);
    println!("{:<name_w$}  {:<var_w$}  NOTE", "NAME", "ENV_VAR");
    for (name, meta) in entries {
        let note = meta.note.as_deref().unwrap_or("-");
        println!("{name:<name_w$}  {:<var_w$}  {note}", meta.env_var);
    }
}

fn print_json(entries: &[(String, crate::store::KeyMeta)]) -> Result<(), KlefError> {
    let map: std::collections::BTreeMap<_, _> =
        entries.iter().map(|(n, m)| (n.clone(), m.clone())).collect();
    let s = serde_json::to_string_pretty(&map).map_err(|e| KlefError::IndexCorrupt {
        path: std::path::PathBuf::new(),
        reason: e.to_string(),
    })?;
    println!("{s}");
    Ok(())
}
```

- [ ] **Step 9.2: Smoke**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n a | cargo run -- add alpha --note hello &&
  echo -n b | cargo run -- add beta &&
  cargo run -- list &&
  cargo run -- list --format json'
```
Expected: table + JSON output.

- [ ] **Step 9.3: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json
git add src/commands/list.rs
git commit -m "feat: implement klef list with table + json formats"
```

---

## Task 10: `klef rm`

**Files:**
- Modify: `src/commands/rm.rs`

- [ ] **Step 10.1: Implement with confirmation**

Replace `src/commands/rm.rs`:

```rust
use crate::error::KlefError;
use crate::store::Store;
use std::io::{BufRead, IsTerminal, Write};

pub fn run(store: &Store, name: &str, yes: bool) -> Result<(), KlefError> {
    if !yes && std::io::stdin().is_terminal() {
        print!("Delete '{name}'? [y/N] ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).ok();
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("aborted");
            return Ok(());
        }
    }
    store.remove(name)?;
    println!("✓ '{name}' removed");
    Ok(())
}
```

- [ ] **Step 10.2: Smoke**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n v | cargo run -- add alpha &&
  cargo run -- rm alpha --yes &&
  cargo run -- list'
```
Expected: removed, then `(no keys stored)`.

- [ ] **Step 10.3: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json
git add src/commands/rm.rs
git commit -m "feat: implement klef rm with TTY confirmation"
```

---

## Task 11: `klef edit`

**Files:**
- Modify: `src/commands/edit.rs`

- [ ] **Step 11.1: Implement edit (value re-prompt or meta-only)**

Replace `src/commands/edit.rs`:

```rust
use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Read};

pub fn run(
    store: &Store,
    name: &str,
    env_var: Option<String>,
    note: Option<String>,
) -> Result<(), KlefError> {
    let _meta = store.meta(name)?; // confirms key exists
    let meta_only = env_var.is_some() || note.is_some();

    if meta_only {
        let note_update = note.map(Some);
        store.update_meta(name, env_var, note_update)?;
        println!("✓ '{name}' metadata updated");
        return Ok(());
    }

    let value = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("New value for '{name}': "))
            .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(KlefError::Io)?;
        buf
    };
    store.add(name, value.trim(), None, None, true)?;
    println!("✓ '{name}' value updated");
    Ok(())
}
```

- [ ] **Step 11.2: Smoke (meta-only path)**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n v | cargo run -- add stripe &&
  cargo run -- edit stripe --note "prod" --as STRIPE_KEY &&
  cargo run -- show stripe'
```
Expected: `env var: STRIPE_KEY`, `note: prod`.

- [ ] **Step 11.3: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json
git add src/commands/edit.rs
git commit -m "feat: implement klef edit (value or metadata)"
```

---

## Task 12: `klef rename`

**Files:**
- Modify: `src/commands/rename.rs`

- [ ] **Step 12.1: Implement (just delegate to Store)**

Replace `src/commands/rename.rs`:

```rust
use crate::error::KlefError;
use crate::store::Store;

pub fn run(store: &Store, old: &str, new: &str) -> Result<(), KlefError> {
    store.rename(old, new)?;
    println!("✓ '{old}' renamed to '{new}'");
    Ok(())
}
```

- [ ] **Step 12.2: Smoke**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n v | cargo run -- add foo &&
  cargo run -- rename foo bar &&
  cargo run -- list'
```
Expected: `bar` listed, `foo` gone.

- [ ] **Step 12.3: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json
git add src/commands/rename.rs
git commit -m "feat: implement klef rename"
```

---

## Task 13: `klef export`

**Files:**
- Modify: `src/commands/export.rs`

- [ ] **Step 13.1: Write failing test for shell formatting**

Replace `src/commands/export.rs`:

```rust
use crate::cli::ExportFormat;
use crate::error::KlefError;
use crate::store::Store;

pub fn run(store: &Store, names: &[String], format: ExportFormat) -> Result<(), KlefError> {
    for name in names {
        let value = store.get_value(name)?;
        let meta = store.meta(name)?;
        let line = render_line(&meta.env_var, &value, format);
        println!("{line}");
    }
    Ok(())
}

fn render_line(var: &str, value: &str, format: ExportFormat) -> String {
    let escaped = shell_escape(value);
    match format {
        ExportFormat::Shell => format!("export {var}={escaped}"),
        ExportFormat::Dotenv => format!("{var}={escaped}"),
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@'))
    {
        value.to_string()
    } else {
        let escaped = value.replace('\'', "'\\''");
        format!("'{escaped}'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_value_unquoted() {
        assert_eq!(shell_escape("sk_live_abc"), "sk_live_abc");
    }

    #[test]
    fn value_with_space_quoted() {
        assert_eq!(shell_escape("a b"), "'a b'");
    }

    #[test]
    fn value_with_single_quote_escaped() {
        assert_eq!(shell_escape("a'b"), "'a'\\''b'");
    }

    #[test]
    fn shell_format_emits_export() {
        assert_eq!(render_line("X", "v", ExportFormat::Shell), "export X=v");
    }

    #[test]
    fn dotenv_format_omits_export() {
        assert_eq!(render_line("X", "v", ExportFormat::Dotenv), "X=v");
    }
}
```

- [ ] **Step 13.2: Run the tests**

Run: `cargo test --lib commands::export`
Expected: 5 tests pass.

- [ ] **Step 13.3: Smoke**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n sk_live | cargo run -- add stripe &&
  cargo run -- export stripe &&
  cargo run -- export stripe --format dotenv'
```
Expected: `export STRIPE_API_KEY=sk_live` then `STRIPE_API_KEY=sk_live`.

- [ ] **Step 13.4: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json
git add src/commands/export.rs
git commit -m "feat: implement klef export with shell escaping"
```

---

## Task 14: `.env` Parser

**Files:**
- Modify: `src/envfile.rs`

- [ ] **Step 14.1: Write tests for the parser**

Replace `src/envfile.rs`:

```rust
use crate::error::KlefError;
use std::path::Path;

pub const REF_PREFIX: &str = "klef:";

#[derive(Debug, PartialEq)]
pub enum Value {
    Literal(String),
    Reference(String),
}

#[derive(Debug, PartialEq)]
pub struct Entry {
    pub key: String,
    pub value: Value,
}

pub fn parse(path: &Path) -> Result<Vec<Entry>, KlefError> {
    if !path.exists() {
        return Err(KlefError::EnvFileNotFound(path.to_path_buf()));
    }
    let content = std::fs::read_to_string(path).map_err(KlefError::Io)?;
    Ok(parse_str(&content))
}

pub fn parse_str(content: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let value = strip_quotes(v.trim());
        let parsed = if let Some(name) = value.strip_prefix(REF_PREFIX) {
            Value::Reference(name.to_string())
        } else {
            Value::Literal(value.to_string())
        };
        out.push(Entry { key, value: parsed });
    }
    out
}

fn strip_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_blank_lines_and_comments() {
        let entries = parse_str("# comment\n\nA=1\n");
        assert_eq!(entries, vec![Entry { key: "A".into(), value: Value::Literal("1".into()) }]);
    }

    #[test]
    fn detects_klef_reference() {
        let entries = parse_str("STRIPE_KEY=klef:stripe\n");
        assert_eq!(
            entries,
            vec![Entry { key: "STRIPE_KEY".into(), value: Value::Reference("stripe".into()) }]
        );
    }

    #[test]
    fn strips_double_quotes() {
        assert_eq!(parse_str("A=\"hello\"\n")[0].value, Value::Literal("hello".into()));
    }

    #[test]
    fn strips_single_quotes() {
        assert_eq!(parse_str("A='hi'\n")[0].value, Value::Literal("hi".into()));
    }

    #[test]
    fn keeps_inner_equals_signs() {
        assert_eq!(
            parse_str("URL=postgres://u:p@h/db\n")[0].value,
            Value::Literal("postgres://u:p@h/db".into())
        );
    }

    #[test]
    fn dash_in_reference_name_ok() {
        assert_eq!(
            parse_str("X=klef:stripe-prod\n")[0].value,
            Value::Reference("stripe-prod".into())
        );
    }

    #[test]
    fn empty_key_skipped() {
        assert!(parse_str("=value\n").is_empty());
    }
}
```

- [ ] **Step 14.2: Run the tests**

Run: `cargo test --lib envfile`
Expected: 7 tests pass.

- [ ] **Step 14.3: Commit**

```bash
git add src/envfile.rs
git commit -m "feat: add .env parser with klef:reference detection"
```

---

## Task 15: `klef run`

**Files:**
- Modify: `src/commands/run.rs`

- [ ] **Step 15.1: Implement reference resolution + Unix `exec`**

Replace `src/commands/run.rs`:

```rust
use crate::envfile::{self, Value};
use crate::error::KlefError;
use crate::store::Store;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub fn run(store: &Store, env_file: &Path, cmd: &[String]) -> Result<(), KlefError> {
    if cmd.is_empty() {
        return Err(KlefError::BackendUnavailable(
            "no command provided after `--`".into(),
        ));
    }

    let entries = envfile::parse(env_file)?;
    let mut resolved: HashMap<String, String> = HashMap::new();
    for e in entries {
        let value = match e.value {
            Value::Literal(v) => v,
            Value::Reference(name) => store.get_value(&name).map_err(|err| match err {
                KlefError::KeyNotFound(_) => KlefError::BrokenReference {
                    var: e.key.clone(),
                    key: name,
                },
                other => other,
            })?,
        };
        resolved.insert(e.key, value);
    }

    let (program, args) = cmd.split_first().unwrap();
    let mut child = Command::new(program);
    child.args(args);
    for (k, v) in &resolved {
        child.env(k, v);
    }

    exec_replace(child, program)
}

#[cfg(unix)]
fn exec_replace(mut child: Command, program: &str) -> Result<(), KlefError> {
    use std::os::unix::process::CommandExt;
    // exec() only returns on failure; on success, klef is replaced by the child
    // and the parent shell sees the child's exit code directly.
    let err = child.exec();
    Err(KlefError::BackendUnavailable(format!(
        "failed to exec '{program}': {err}"
    )))
}

#[cfg(not(unix))]
fn exec_replace(mut child: Command, program: &str) -> Result<(), KlefError> {
    let status = child
        .status()
        .map_err(|e| KlefError::BackendUnavailable(format!("failed to spawn '{program}': {e}")))?;
    std::process::exit(status.code().unwrap_or(1));
}
```

> **Note:** the `#[cfg(unix)]` path is what runs on macOS and Linux (the MVP targets). It's a true `execvp`-style replacement: signals propagate naturally, no zombie process. The `#[cfg(not(unix))]` fallback exists only to keep the crate buildable if someone tries it on Windows; Windows is not a supported MVP target.

- [ ] **Step 15.2: Smoke happy path**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  echo -n sk_live | cargo run -- add stripe &&
  printf "STRIPE_KEY=klef:stripe\nPORT=3000\n" > /tmp/test.env &&
  cargo run -- run --env-file /tmp/test.env -- /bin/sh -c "echo $STRIPE_KEY $PORT"'
```
Expected: prints `sk_live 3000`.

- [ ] **Step 15.3: Smoke broken reference**

```bash
KLEF_TEST_BACKEND=file:/tmp/klef-smoke-secrets.json KLEF_INDEX_PATH=/tmp/k.json bash -c '
  printf "X=klef:nope\n" > /tmp/test.env &&
  cargo run -- run --env-file /tmp/test.env -- /bin/echo hi; echo "exit=$?"'
```
Expected: error message about broken reference, `exit=3`.

- [ ] **Step 15.4: Commit**

```bash
rm -f /tmp/k.json /tmp/klef-smoke-secrets.json /tmp/test.env
git add src/commands/run.rs
git commit -m "feat: implement klef run with klef:reference resolution"
```

---

## Task 16: End-to-End CLI Tests

**Files:**
- Create: `tests/cli.rs`

- [ ] **Step 16.1: Write E2E tests**

Create `tests/cli.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

/// Build a `klef` command pre-configured with isolated index + secrets paths.
/// Each test owns a tempdir that's cleaned up on drop.
fn klef(dir: &Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

#[test]
fn add_get_list_rm_round_trip() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add").arg("stripe")
        .write_stdin("sk_live")
        .assert().success();

    klef(d.path())
        .arg("get").arg("stripe")
        .assert().success().stdout(predicate::str::contains("sk_live"));

    klef(d.path())
        .arg("list")
        .assert().success().stdout(predicate::str::contains("stripe"));

    klef(d.path())
        .arg("rm").arg("stripe").arg("--yes")
        .assert().success();

    klef(d.path())
        .arg("get").arg("stripe")
        .assert().failure().code(2);
}

#[test]
fn export_emits_shell_export() {
    let d = TempDir::new().unwrap();
    klef(d.path()).arg("add").arg("stripe").write_stdin("v").assert().success();
    klef(d.path())
        .arg("export").arg("stripe")
        .assert()
        .success()
        .stdout("export STRIPE_API_KEY=v\n");
}

#[test]
fn run_resolves_references() {
    let d = TempDir::new().unwrap();
    let envf = d.path().join(".env");
    std::fs::write(&envf, "STRIPE_KEY=klef:stripe\nPORT=3000\n").unwrap();

    klef(d.path()).arg("add").arg("stripe").write_stdin("sk_live").assert().success();

    klef(d.path())
        .arg("run")
        .arg("--env-file").arg(&envf)
        .arg("--")
        .arg("/bin/sh").arg("-c").arg("printf '%s|%s' \"$STRIPE_KEY\" \"$PORT\"")
        .assert()
        .success()
        .stdout("sk_live|3000");
}

#[test]
fn run_with_broken_reference_exits_3() {
    let d = TempDir::new().unwrap();
    let envf = d.path().join(".env");
    std::fs::write(&envf, "X=klef:missing\n").unwrap();

    klef(d.path())
        .arg("run")
        .arg("--env-file").arg(&envf)
        .arg("--")
        .arg("/bin/echo").arg("hi")
        .assert()
        .failure()
        .code(3);
}

#[test]
fn rename_moves_key() {
    let d = TempDir::new().unwrap();
    klef(d.path()).arg("add").arg("foo").write_stdin("v").assert().success();
    klef(d.path()).arg("rename").arg("foo").arg("bar").assert().success();
    klef(d.path()).arg("get").arg("bar").assert().success().stdout(predicate::str::contains("v"));
    klef(d.path()).arg("get").arg("foo").assert().failure().code(2);
}
```

> **Why `TempDir` per test instead of a shared path:** each test gets its own empty `secrets.json` + `index.json`, avoiding cross-test pollution and parallel-execution races. `tempfile::TempDir` deletes the directory on drop.

- [ ] **Step 16.2: Run the E2E tests**

Run: `cargo test --test cli`
Expected: 5 tests pass.

- [ ] **Step 16.3: Run the full test suite + clippy**

Run: `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: all pass.

- [ ] **Step 16.4: Commit**

```bash
git add tests/cli.rs
git commit -m "test: add end-to-end CLI tests covering all commands"
```

---

## Task 17: README polish + Manual Smoke Against Real Keychain

**Files:**
- Modify: `README.md`

- [ ] **Step 17.1: Update README "Dev" and "Statut" sections**

Replace the Dev/Statut sections with up-to-date instructions, including E2E run and the manual real-keychain smoke procedure below.

- [ ] **Step 17.2: Manual smoke against real macOS Keychain**

(Not automated — the only time we touch the real Keychain.)

```bash
cargo install --path .
echo -n "smoke_test_value_DELETE_ME" | klef add klef-smoke
klef get klef-smoke      # should print value
klef list                # should show klef-smoke
klef rm klef-smoke --yes
```

Verify in Keychain Access (macOS) that the entry `klef/klef-smoke` was created and removed.

- [ ] **Step 17.3: Commit**

```bash
git add README.md
git commit -m "docs: refresh README for v0.1 release"
```

- [ ] **Step 17.4: Tag**

```bash
git tag -a v0.1.0 -m "klef v0.1.0 — MVP"
```

---

## Self-Review

Cross-checked against `docs/design/2026-05-05-mvp-design.md`:

- §3.3 commands: all 9 implemented (Tasks 7–13, 15). ✓
- §3.4 env-var convention `<NAME>_API_KEY` + `--as` override: Task 5 (`default_env_var`) + Task 11 + Task 13. ✓
- §4.2 `Backend` trait + `MemoryBackend` + `KeychainBackend`: Tasks 2, 3. ✓
- §5 Keychain + Index + atomic write: Tasks 3, 4. ✓
- §6 `klef run` reference resolution: Tasks 14, 15. ✓
- §7 error variants + exit codes: Task 1, surfaced via `main.rs` (Task 6). ✓
- §8 testing strategy (unit + MemoryBackend integration + E2E `assert_cmd`): Tasks 5, 7, 13, 14, 16. ✓
- §3.1 platform support (macOS + Linux desktop): both compile against the same `keyring` crate; nothing platform-specific in the code. ✓

**Open spec questions** (§12 of design) resolved during the plan:
- `time` chosen over `chrono` (used in Task 4).
- `klef show` format = simple key/value lines (Task 8).
- `klef rename` to existing key = `KeyAlreadyExists` (Task 5 `rename` impl).

No placeholders, all code shown, type names consistent across tasks.

**Plan-review corrections applied (after first draft):**
- Crate is now **bin + lib**: logic in `src/lib.rs`, `main.rs` is a thin wrapper. Tests use `cargo test --lib` cleanly (Task 1).
- Added `FileBackend` for cross-process tests and as foundation for the v0.3 encrypted backend (Task 2). E2E and smoke tests use `KLEF_TEST_BACKEND=file:<path>` instead of `memory` (which is in-process only).
- `klef run` uses `std::os::unix::process::CommandExt::exec()` on Unix (true `execvp`-style replacement) with a `cfg(not(unix))` fallback for portability (Task 15).
- `KlefError` split: `IndexWrite` reserved for index I/O failures; new `Io(io::Error)` variant covers stdin/stdout/.env reads (Task 1, used in Tasks 7, 8, 11, 14).
