use std::time::Duration;

use super::*;
use crate::live_sessions::Running;

const QUICK: Duration = Duration::from_millis(500);

fn running(pid: i32, started: Option<String>) -> Running {
    Running {
        pid,
        pane: None,
        job: None,
        status: "idle".to_owned(),
        cwd: "/".to_owned(),
        session_id: None,
        started,
    }
}

/// A `sleep` reaped as it ends, as its own parent would reap it: a zombie
/// still answers to its pid.
fn sleeping() -> (i32, std::thread::JoinHandle<()>) {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep");
    let pid = child.id() as i32;
    (pid, std::thread::spawn(move || drop(child.wait())))
}

/// The terminal a listing named was not the one the session ran in — the old
/// pane of a conversation resumed elsewhere — so closing it left the process
/// running. It is ended directly, and the stop says so, never "stopped" while
/// it runs on.
#[cfg(unix)]
#[test]
fn a_process_that_outlives_the_terminal_closed_for_it_is_ended_directly() {
    let (pid, reaped) = sleeping();

    let directly = outlived("worker", &running(pid, None), QUICK).expect("ended");

    assert!(directly, "said ended with its terminal");
    assert!(!devpit_pty::process::alive(pid), "still running");
    reaped.join().expect("reaped");
}

/// One that ended with its terminal, as it should, is not ended again.
#[cfg(unix)]
#[test]
fn a_process_that_ended_with_its_terminal_is_not_ended_again() {
    let mut child = std::process::Command::new("true").spawn().expect("true");
    let pid = child.id() as i32;
    child.wait().expect("ended");

    assert!(!outlived("worker", &running(pid, None), QUICK).expect("ended"));
}

/// A pid now held by another process, started after the listing's, is not
/// that session's: it is left alone. Told by /proc, so Linux only.
#[cfg(target_os = "linux")]
#[test]
fn a_pid_another_process_now_holds_is_left_alone() {
    let (pid, reaped) = sleeping();

    let directly = outlived("worker", &running(pid, Some("1".to_owned())), QUICK);

    assert!(!directly.expect("not this session's"));
    assert!(
        devpit_pty::process::alive(pid),
        "ended a process that was not the session's"
    );
    devpit_pty::process::terminate(pid);
    reaped.join().expect("reaped");
}

/// One that will not end is never called stopped: the stop answers that it
/// could not be, naming the process still running. Waits on /proc, so
/// Linux only.
#[cfg(target_os = "linux")]
#[test]
fn a_process_that_will_not_end_is_an_error_not_stopped() {
    let mut child = std::process::Command::new("sh")
        .args(["-c", "trap '' TERM; exec sleep 30"])
        .spawn()
        .expect("sh");
    let pid = child.id() as i32;
    // The shell has set the trap once it is sleep.
    while std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|comm| comm.trim() == "sh")
    {
        std::thread::sleep(Duration::from_millis(10));
    }

    let refused = outlived("worker", &running(pid, None), QUICK).expect_err("said stopped");

    assert_eq!(refused.code, ErrorCode::Conflict);
    assert!(
        refused.message.contains(&pid.to_string()),
        "{}",
        refused.message
    );
    child.kill().expect("killed");
    child.wait().expect("reaped");
}
