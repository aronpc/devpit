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

/// `running`, in a devpit terminal.
fn in_terminal(pid: i32, started: Option<String>) -> Running {
    Running {
        pane: Some(devpit_rpc::LivePane {
            project_id: "p".to_owned(),
            pane_id: "leaf_gone".to_owned(),
        }),
        ..running(pid, started)
    }
}

/// When `pid` started, as a listing writes it.
#[cfg(target_os = "linux")]
fn started_of(pid: i32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    crate::live_sessions::start_of(&stat).map(ToOwned::to_owned)
}

/// A `sleep` reaped as it ends, as its own parent would reap it: a zombie
/// still answers to its pid.
#[cfg(unix)]
fn sleeping() -> (i32, std::thread::JoinHandle<()>) {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep");
    let pid = child.id() as i32;
    (pid, std::thread::spawn(move || drop(child.wait())))
}

/// Whether `pid` has ended within a few seconds: one reaped by init, not by
/// this test, takes a moment, and an ended one nobody reaps is a zombie.
#[cfg(target_os = "linux")]
fn gone_soon(pid: i32) -> bool {
    let zombie = || {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| Some(stat.rsplit_once(')')?.1.trim_start().starts_with('Z')))
            .unwrap_or(false)
    };
    (0..100).any(|_| {
        std::thread::sleep(Duration::from_millis(50));
        !devpit_pty::process::alive(pid) || zombie()
    })
}

/// The terminal a listing named was not the one the session ran in — the old
/// pane of a conversation resumed elsewhere — so closing it left the process
/// running. It is ended directly, and the stop says so, never "stopped" while
/// it runs on.
#[cfg(target_os = "linux")]
#[test]
fn a_process_that_outlives_the_terminal_closed_for_it_is_ended_directly() {
    let (pid, reaped) = sleeping();

    let directly = made_sure("worker", &[in_terminal(pid, started_of(pid))], QUICK).expect("ended");

    assert_eq!(directly, 1, "said ended with its terminal");
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

    assert_eq!(
        made_sure("worker", &[running(pid, None)], QUICK).expect("ended"),
        0
    );
}

/// A pid now held by another process, started after the listing's, is not
/// that session's: it is left alone. Told by /proc, so Linux only.
#[cfg(target_os = "linux")]
#[test]
fn a_pid_another_process_now_holds_is_left_alone() {
    let (pid, reaped) = sleeping();

    let directly = made_sure("worker", &[running(pid, Some("1".to_owned()))], QUICK);

    assert_eq!(directly.expect("not this session's"), 0);
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

    let refused =
        made_sure("worker", &[running(pid, started_of(pid))], QUICK).expect_err("said stopped");

    assert_eq!(refused.code, ErrorCode::Conflict);
    assert!(
        refused
            .message
            .contains(&format!("process {pid} would not end")),
        "{}",
        refused.message
    );
    child.kill().expect("killed");
    child.wait().expect("reaped");
}

/// A process devpit cannot tell is still the session's — no start to check
/// it by — is never signalled: the stop answers that it was left running.
#[cfg(unix)]
#[test]
fn a_process_devpit_cannot_tell_is_left_running_and_said() {
    let (pid, reaped) = sleeping();

    let refused = made_sure("worker", &[in_terminal(pid, None)], QUICK).expect_err("said stopped");

    assert!(
        refused.message.contains(&format!("process {pid} outlived")),
        "{}",
        refused.message
    );
    assert!(
        devpit_pty::process::alive(pid),
        "signalled a process it could not tell"
    );
    devpit_pty::process::terminate(pid);
    reaped.join().expect("reaped");
}

/// An agent leads its own process group and runs its tools in it: ended
/// directly, the group goes with it, as a closed terminal's does.
#[cfg(target_os = "linux")]
#[test]
fn an_agent_ended_directly_takes_its_tools_with_it() {
    use std::io::BufRead;
    use std::os::unix::process::CommandExt;
    let mut agent = std::process::Command::new("sh")
        .args(["-c", "sleep 30 & echo $!; wait"])
        .process_group(0)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("sh");
    let pid = agent.id() as i32;
    let mut line = String::new();
    std::io::BufReader::new(agent.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("its tool's pid");
    let tool: i32 = line.trim().parse().expect("a pid");
    let reaped = std::thread::spawn(move || drop(agent.wait()));

    let directly = made_sure("worker", &[running(pid, started_of(pid))], QUICK).expect("ended");

    assert_eq!(directly, 1);
    reaped.join().expect("reaped");
    assert!(gone_soon(tool), "its tool still runs");
}

/// What the stop answers, from one session to many, and when it had to end
/// processes itself.
#[test]
fn a_stop_says_what_it_stopped_and_what_it_ended_itself() {
    assert_eq!(said("w", 1, false, 0), "w was stopped");
    assert_eq!(
        said("w", 1, true, 0),
        "w was stopped, and its terminal closed"
    );
    assert_eq!(said("w", 3, true, 0), "3 sessions called w were stopped");
    assert_eq!(
        said("w", 1, true, 1),
        "w was stopped, and its terminal closed; one process still ran after that, and was ended directly"
    );
    assert_eq!(
        said("w", 3, true, 2),
        "3 sessions called w were stopped; 2 processes still ran after that, and were ended directly"
    );
}

/// A background job is its CLI's to stop: its supervisor starts a killed one
/// again. One still running once its CLI was asked is never signalled, and
/// the stop says it still runs.
#[cfg(target_os = "linux")]
#[test]
fn a_job_still_running_after_its_cli_was_asked_is_never_signalled() {
    let (pid, reaped) = sleeping();
    let job = Running {
        job: Some("bg-1".to_owned()),
        ..running(pid, started_of(pid))
    };

    let refused = made_sure("worker", &[job], QUICK).expect_err("said stopped");

    assert!(
        refused
            .message
            .contains(&format!("process {pid} was asked to end and still runs")),
        "{}",
        refused.message
    );
    assert!(devpit_pty::process::alive(pid), "signalled a job");
    devpit_pty::process::terminate(pid);
    reaped.join().expect("reaped");
}

/// What a stop answers when processes still run: why, one clause each.
#[test]
fn a_refused_stop_says_why_each_process_still_runs() {
    assert_eq!(
        refusal("w", &[(12, Left::Stuck)]),
        "w could not be stopped: process 12 would not end"
    );
    assert_eq!(
        refusal("w", &[(12, Left::Asked), (34, Left::Untold)]),
        "w could not be stopped: process 12 was asked to end and still runs; process 34 outlived its terminal, and devpit cannot tell it is still that session's, so it was left running"
    );
}

/// Never init's group, which `kill` reads as every process there is, nor
/// devpit's own.
#[cfg(unix)]
#[test]
fn no_group_is_signalled_for_init_or_devpit_itself() {
    assert_eq!(group_of(&running(1, None)), None);
    // SAFETY: getpgid only reads the process table.
    let ours = unsafe { libc::getpgid(0) };
    assert_eq!(group_of(&running(ours, None)), None);
}
