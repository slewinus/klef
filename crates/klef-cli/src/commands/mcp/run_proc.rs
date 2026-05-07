//! Spawn a child process with a curated env, capture stdout/stderr with
//! truncation, enforce a timeout, and kill the whole process group on
//! timeout to avoid orphan descendants.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const STDOUT_CAP_BYTES: usize = 1024 * 1024;
pub const STDERR_CAP_BYTES: usize = 1024 * 1024;
pub const HARDCAP_TIMEOUT_MS: u64 = 300_000;
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

const PARENT_ENV_WHITELIST: &[&str] = &["PATH", "HOME", "USER", "LANG", "LC_ALL", "TERM", "TMPDIR"];

#[derive(Debug)]
pub struct ProcRequest {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: u64,
}

#[derive(Debug)]
pub struct ProcResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration_ms: u64,
    pub timed_out: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcError {
    #[error("spawn: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("argv is empty")]
    EmptyArgv,
}

/// Run a child to completion or timeout. The child receives ONLY the env
/// vars in `req.env` plus the parent-whitelist (PATH, HOME, ...). Stdin is
/// `/dev/null`. Truncates each stream at 1 MB.
///
/// # Errors
///
/// Returns `ProcError::EmptyArgv` if `req.argv` is empty, or `ProcError::Spawn`
/// if the OS rejects the spawn or fails to wait.
///
/// # Panics
///
/// Panics if tokio's `Command` returns a successfully-spawned child without
/// `stdout`/`stderr` pipes — this should not happen because we explicitly
/// configured `Stdio::piped()` for both.
pub async fn spawn_and_capture(req: ProcRequest) -> Result<ProcResult, ProcError> {
    let (program, args) = req.argv.split_first().ok_or(ProcError::EmptyArgv)?;
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for k in PARENT_ENV_WHITELIST {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    if let Some(cwd) = &req.cwd {
        cmd.current_dir(cwd);
    }
    #[cfg(unix)]
    {
        // SAFETY: setsid() is async-signal-safe and the closure performs no
        // allocations or locks. Making the child a session leader is required
        // so killpg() on timeout cannot escape to our test runner / parent.
        #[allow(unsafe_code)]
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let started = Instant::now();
    let mut child = cmd.spawn()?;
    #[cfg(unix)]
    let pgid = child.id().and_then(|p| i32::try_from(p).ok());

    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let stdout_task = tokio::spawn(read_capped(stdout, STDOUT_CAP_BYTES));
    let stderr_task = tokio::spawn(read_capped(stderr, STDERR_CAP_BYTES));

    let timeout_ms = req.timeout_ms.min(HARDCAP_TIMEOUT_MS);
    let wait = child.wait();
    let outcome = tokio::time::timeout(Duration::from_millis(timeout_ms), wait).await;

    let timed_out = outcome.is_err();
    let exit_status = match outcome {
        Ok(Ok(s)) => Some(s),
        Ok(Err(e)) => return Err(ProcError::Spawn(e)),
        Err(_) => {
            #[cfg(unix)]
            if let Some(pgid) = pgid {
                kill_group(pgid);
            }
            None
        }
    };

    let (stdout_buf, stdout_truncated) = stdout_task.await.unwrap_or_else(|_| (Vec::new(), false));
    let (stderr_buf, stderr_truncated) = stderr_task.await.unwrap_or_else(|_| (Vec::new(), false));

    let exit_code = exit_status.and_then(|s| s.code()).unwrap_or(-1);
    Ok(ProcResult {
        exit_code,
        stdout: stdout_buf,
        stderr: stderr_buf,
        stdout_truncated,
        stderr_truncated,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        timed_out,
    })
}

async fn read_capped<R: AsyncReadExt + Unpin>(mut r: R, cap: usize) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match r.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() + n > cap {
                    let take = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                    truncated = true;
                    let mut sink = [0u8; 8192];
                    while r.read(&mut sink).await.unwrap_or(0) > 0 {}
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    }
    (buf, truncated)
}

#[cfg(unix)]
fn kill_group(pgid: i32) {
    // SAFETY: killpg() is signal-safe and only signals the dedicated session
    // we created via setsid() in pre_exec. Sleep on a thread is fine because
    // the caller is in an async context but kill_group() is invoked from
    // the timeout-handling branch where blocking briefly is acceptable.
    #[allow(unsafe_code)]
    unsafe {
        libc::killpg(pgid, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_secs(2));
    #[allow(unsafe_code)]
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn echo(args: &[&str]) -> ProcRequest {
        ProcRequest {
            argv: std::iter::once("echo".to_string())
                .chain(args.iter().map(|s| (*s).to_string()))
                .collect(),
            env: HashMap::new(),
            cwd: None,
            timeout_ms: 5000,
        }
    }

    #[tokio::test]
    async fn echo_returns_stdout_and_zero_exit() {
        let r = spawn_and_capture(echo(&["hello"])).await.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(!r.timed_out);
        assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "hello");
    }

    #[tokio::test]
    async fn injected_env_visible_to_child() {
        let mut env = HashMap::new();
        env.insert("KLEF_TEST_VAR".into(), "secret-xyz".into());
        let req = ProcRequest {
            argv: vec!["sh".into(), "-c".into(), "echo $KLEF_TEST_VAR".into()],
            env,
            cwd: None,
            timeout_ms: 5000,
        };
        // NOTE: uses sh for env-visibility test only; production policy will deny this argv.
        let r = spawn_and_capture(req).await.unwrap();
        assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "secret-xyz");
    }

    #[tokio::test]
    async fn parent_env_not_inherited_outside_whitelist() {
        // SAFETY: tokio::test runs on a dedicated runtime; env is process-global but
        // we set + unset within this test only. Other parallel tests do not read this var.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("KLEF_NOT_WHITELISTED", "leak-me");
        }
        let req = ProcRequest {
            argv: vec![
                "sh".into(),
                "-c".into(),
                "echo ${KLEF_NOT_WHITELISTED:-absent}".into(),
            ],
            env: HashMap::new(),
            cwd: None,
            timeout_ms: 5000,
        };
        let r = spawn_and_capture(req).await.unwrap();
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("KLEF_NOT_WHITELISTED");
        }
        assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "absent");
    }

    #[tokio::test]
    async fn timeout_marks_timed_out_and_kills() {
        let req = ProcRequest {
            argv: vec!["sleep".into(), "30".into()],
            env: HashMap::new(),
            cwd: None,
            timeout_ms: 200,
        };
        let r = spawn_and_capture(req).await.unwrap();
        assert!(r.timed_out);
        assert!(
            r.duration_ms < 5000,
            "kill must be prompt; got {} ms",
            r.duration_ms
        );
    }

    #[tokio::test]
    async fn stdout_truncates_at_cap() {
        let req = ProcRequest {
            argv: vec!["sh".into(), "-c".into(), "yes hello".into()],
            env: HashMap::new(),
            cwd: None,
            timeout_ms: 1500,
        };
        let r = spawn_and_capture(req).await.unwrap();
        assert!(r.stdout_truncated);
        assert!(r.stdout.len() <= STDOUT_CAP_BYTES);
    }
}
