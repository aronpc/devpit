//! Running a tmux command and reading what it said.
//!
//! The plumbing every verb goes through, kept apart from the verbs: one place
//! decides how the binary is found, how the socket is named on the line, and
//! what a non-zero exit turns into.

use std::process::{Child, Output, Stdio};
use std::time::Duration;

use crate::{Server, TmuxError};

/// How long a command waits for the server.
///
/// A server busy with panes that flood answers nobody for minutes, and the
/// window asks it every two seconds. Waited on without an end, each question
/// held a thread until the pool every command runs on was spent, and then no
/// command ran at all.
pub(crate) const PATIENCE: Duration = Duration::from_secs(10);

pub(crate) fn require(server: &Server, args: &[&str]) -> Result<Output, TmuxError> {
    let output = run(server, args)?;
    if output.status.success() {
        return Ok(output);
    }
    Err(TmuxError::Failed {
        command: args.join(" "),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub(crate) fn run(server: &Server, args: &[&str]) -> Result<Output, TmuxError> {
    let mut command = match &server.via {
        Some(via) => {
            let argv = crate::remote::argv(via, args);
            let mut command = devpit_pty::host_env::command(&argv[0]);
            command.args(&argv[1..]);
            command
        }
        None => {
            let mut command = devpit_pty::host_env::command(crate::naming::program());
            command.arg("-S").arg(&server.socket).args(args);
            command
        }
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A console program started from a windowed app is given a console
        // window of its own, flashed on screen at every call. CREATE_NO_WINDOW.
        command.creation_flags(devpit_pty::host_env::NO_WINDOW);
    }
    // What `output()` would set, spelled out because the wait is ours.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let failed = |err: std::io::Error| match err.kind() {
        std::io::ErrorKind::NotFound => TmuxError::Missing,
        _ => TmuxError::Failed {
            command: args.join(" "),
            stderr: err.to_string(),
        },
    };
    let child = command.spawn().map_err(failed)?;
    match within(child, server.patience) {
        Some(output) => output.map_err(failed),
        None => Err(TmuxError::Unanswered {
            command: args.join(" "),
            seconds: server.patience.as_secs(),
        }),
    }
}

/// What the child said, or nothing once it has run for `patience` — when it
/// is asked to end, and the thread that asked goes on. The thread reading its
/// output may stay until the server answers or goes: tmux keeps a copy of
/// every client's stdout, so the end of it is the server's to give.
pub(crate) fn within(child: Child, patience: Duration) -> Option<std::io::Result<Output>> {
    let pid = child.id();
    let (said, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = said.send(child.wait_with_output());
    });
    match heard.recv_timeout(patience) {
        Ok(output) => Some(output),
        Err(_) => {
            devpit_pty::process::terminate(pid as i32);
            None
        }
    }
}
