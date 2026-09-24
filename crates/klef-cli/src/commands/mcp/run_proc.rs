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

/// Extra time the output readers get past the timeout, so a timed-out group
/// (SIGTERM, 2s, SIGKILL) closes its pipes before we stop reading.
const DRAIN_GRACE: Duration = Duration::from_secs(3);

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
    let timeout_ms = req.timeout_ms.min(HARDCAP_TIMEOUT_MS);
    // A descendant that escaped the group (its own setsid) can hold the pipes
    // open forever; the readers give up at this deadline instead.
    // ponytail: an early-exiting leader with such a descendant still waits the
    // full timeout; signal the readers on exit if that latency ever matters.
    let read_deadline =
        tokio::time::Instant::now() + Duration::from_millis(timeout_ms) + DRAIN_GRACE;
    let stdout_task = tokio::spawn(read_capped(stdout, STDOUT_CAP_BYTES, read_deadline));
    let stderr_task = tokio::spawn(read_capped(stderr, STDERR_CAP_BYTES, read_deadline));

    let wait = child.wait();
    let outcome = tokio::time::timeout(Duration::from_millis(timeout_ms), wait).await;

    let timed_out = outcome.is_err();
    let exit_status = match outcome {
        Ok(Ok(s)) => {
            // The leader is done; anything left in its group is a background
            // descendant still holding the injected secrets. Kill it.
            #[cfg(unix)]
            if let Some(pgid) = pgid {
                // SAFETY: killpg() only signals the session we created via setsid().
                #[allow(unsafe_code)]
                unsafe {
                    libc::killpg(pgid, libc::SIGKILL);
                }
            }
            Some(s)
        }
        Ok(Err(e)) => return Err(ProcError::Spawn(e)),
        Err(_) => {
            #[cfg(unix)]
            if let Some(pgid) = pgid {
                kill_group(pgid).await;
            }
            #[cfg(not(unix))]
            {
                let _ = child.start_kill();
                // Bounded grace period for the OS to deliver the signal so
                // pipes close and read tasks unblock.
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
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

async fn read_capped<R: AsyncReadExt + Unpin>(
    mut r: R,
    cap: usize,
    deadline: tokio::time::Instant,
) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match tokio::time::timeout_at(deadline, r.read(&mut chunk)).await {
            Err(_) | Ok(Ok(0) | Err(_)) => break,
            Ok(Ok(n)) => {
                if buf.len() + n > cap {
                    let take = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                    truncated = true;
                    let mut sink = [0u8; 8192];
                    while let Ok(Ok(1..)) =
                        tokio::time::timeout_at(deadline, r.read(&mut sink)).await
                    {
                    }
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    }
    (buf, truncated)
}

#[cfg(unix)]
async fn kill_group(pgid: i32) {
    // SAFETY: killpg() is signal-safe and only signals the dedicated session
    // we created via setsid() in pre_exec. Sleep is async to avoid blocking
    // a tokio worker between SIGTERM and SIGKILL.
    #[allow(unsafe_code)]
    unsafe {
        libc::killpg(pgid, libc::SIGTERM);
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    #[allow(unsafe_code)]
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

#[cfg(test)]
#[path = "run_proc_tests.rs"]
mod tests;
