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

/// A background descendant used to hold the stdout pipe open, so the call
/// outlived its timeout and the descendant kept the injected secrets.
#[cfg(unix)]
#[tokio::test]
async fn background_descendant_is_killed_when_leader_exits() {
    let req = ProcRequest {
        argv: vec!["sh".into(), "-c".into(), "sleep 30 & echo $!".into()],
        env: HashMap::new(),
        cwd: None,
        timeout_ms: 10_000,
    };
    let started = Instant::now();
    let r = spawn_and_capture(req).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert!(!r.timed_out);
    let pid = String::from_utf8(r.stdout).unwrap().trim().to_string();
    // SIGKILL is async w.r.t. reaping; poll briefly for the pid to vanish.
    let alive = || {
        std::process::Command::new("kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    for _ in 0..20 {
        if !alive() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("background descendant {pid} survived the call");
}

/// A descendant that escapes the process group (own `setsid`) can't be
/// killed, but it must not hold the call open past timeout + grace.
#[cfg(unix)]
#[tokio::test]
async fn escaped_descendant_cannot_hold_the_call_open() {
    let script = "import os, time\n\
                  if os.fork() == 0:\n    os.setsid(); time.sleep(8); os._exit(0)\n\
                  print('hi')";
    let req = ProcRequest {
        argv: vec!["python3".into(), "-c".into(), script.into()],
        env: HashMap::new(),
        cwd: None,
        timeout_ms: 500,
    };
    let started = Instant::now();
    let r = spawn_and_capture(req).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "hi");
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
