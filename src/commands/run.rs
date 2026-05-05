use crate::envfile::{self, Value};
use crate::error::KlefError;
use crate::store::Store;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Parse the env file, resolve `klef:<name>` references via the store, and execute
/// the child process with the resolved environment.
///
/// # Errors
///
/// Returns an error if the env file cannot be read, a reference cannot be resolved,
/// or the command fails to execute.
///
/// # Panics
///
/// Never panics; the `expect()` after `split_first()` is safe because we check
/// `is_empty()` above.
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

    let (program, args) = cmd.split_first().expect("checked above");
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
