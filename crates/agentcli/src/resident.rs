//! A conversation whose process stays between turns.
//!
//! A turn that starts a process and lets it exit is deaf in between: the CLI
//! takes messages from the account's other sessions only while it runs, and a
//! session that is waited on can only say it finished to one that is still
//! there. So this process is started once and kept. Each person's message is a
//! line on its stdin; a message from another session wakes it, and the turn
//! that follows is read the same way.
//!
//! Measured on Claude Code 2.1.281: in stream-json input mode, `--print` stays
//! after its result while stdin is open, is listed as a live session, and an
//! idle one answers a message with a turn of its own.

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use devpit_rpc::{Part, TurnEnd};

use crate::control::Control;
use crate::driver::Driver;
use crate::talk::{Said, Say};
use crate::talk_stream::next_turn;
use crate::wire::Switch;
use crate::AgentError;

/// How long a closed process gets to finish on its own before it is ended.
const GRACE: std::time::Duration = std::time::Duration::from_secs(15);

/// Whether the process is still there and still this one: a zombie or a pid
/// already handed to someone else is not signalled.
///
/// `/proc` only exists on Linux; elsewhere it used to read as gone, and a
/// resident chat that hung was never ended on macOS.
pub(crate) fn still_running(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat
            .rsplit(')')
            .next()
            .and_then(|rest| rest.trim_start().chars().next())
            .is_some_and(|state| state != 'Z'),
        Err(_) if cfg!(target_os = "linux") => false,
        Err(_) => devpit_pty::process::alive(pid as i32),
    }
}

/// What the process says, a turn at a time.
pub enum Heard {
    Part(Part),
    /// A turn is over, whoever started it. Boxed: it is the largest by far.
    Ended(Box<Said>),
    /// The process is gone: exited, killed, or its stream closed.
    Gone,
    /// The CLI's answer to a control request, as it wrote it.
    Control(serde_json::Value),
}

/// The process that stays, and its stdin.
pub struct Resident {
    control: Control,
    pid: u32,
    /// Where what it says goes, for a switch devpit answers itself.
    told: Arc<dyn Fn(Heard) + Send + Sync>,
}

impl Resident {
    /// Starts the process with `turn`'s flags — its prompt is not sent — and
    /// reads it on a thread of its own, handing everything to `on`.
    pub fn start(
        driver: Box<dyn Driver>,
        turn: &Say<'_>,
        on_session: Arc<dyn Fn(&str) + Send + Sync>,
        on: impl FnMut(Heard) + Send + 'static,
    ) -> Result<Self, AgentError> {
        // Shared by the turn reader and the line watcher below, which both
        // hand things on while one turn is being read.
        let on = std::sync::Arc::new(std::sync::Mutex::new(on));
        let tell: Arc<dyn Fn(Heard) + Send + Sync> = Arc::new(move |heard: Heard| {
            if let Ok(mut on) = on.lock() {
                on(heard);
            }
        });
        let wire = driver.wire();
        let mut child = crate::talk::own_group(&mut devpit_pty::host_env::command(turn.command))
            .args(wire.argv(turn))
            .current_dir(turn.cwd)
            .envs(turn.env.iter().map(|(key, value)| (key, value)))
            // MCP Apps pages, and the Artifact tools a terminal of the account has.
            .envs(crate::apps_host::chat_switches(turn.env))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AgentError::NotInstalled)?;
        let pid = child.id();
        let control = turn.control.cloned().unwrap_or_default();
        let stdin = child.stdin.take().ok_or(AgentError::NotInstalled)?;
        if !control.open(stdin, wire, turn, pid) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AgentError::Unreadable(
                "the agent would not take its handshake".to_owned(),
            ));
        }
        let stdout = child.stdout.take().ok_or(AgentError::NotInstalled)?;

        let (reading, told) = (control.clone(), tell.clone());
        std::thread::spawn(move || {
            // Control answers are not turn frames; they are handed on as
            // they pass, and the turn reader never sees a difference.
            let (watching, answers) = (tell.clone(), reading.clone());
            let mut lines = BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
                .inspect(move |line| {
                    if let Some(said) = answers.control_answer(line) {
                        watching(Heard::Control(said));
                    }
                });
            // The CLI counts what the process has spent so far: a turn's cost
            // is what it added.
            let mut spent = 0.0;
            loop {
                let started = Instant::now();
                let (heard, whole) = next_turn(
                    driver.as_ref(),
                    lines.by_ref(),
                    &reading,
                    Some(on_session.as_ref()),
                    |part| tell(Heard::Part(part)),
                );
                if let Some(ending) = heard.ended {
                    let cost_usd = ending.cost_usd.map(|total| {
                        let turn = (total - spent).max(0.0);
                        spent = total;
                        turn
                    });
                    tell(Heard::Ended(Box::new(Said {
                        end: TurnEnd {
                            turn_id: String::new(),
                            cost_usd,
                            duration_ms: Some(started.elapsed().as_millis() as f64),
                            stop_reason: ending.stop_reason,
                            is_error: ending.is_error,
                            context: ending.context,
                            limit: heard.limit,
                            ..Default::default()
                        },
                        session_id: heard.session_id,
                        init: heard.init,
                        anchor: heard.anchor,
                    })));
                    reading.stopped_alone();
                }
                if !whole {
                    break;
                }
            }
            // Gone, it takes no message: one written into its stdin would wait forever.
            reading.close();
            // A turn stopped before the CLI could hear of it ended stopped, not
            // lost: one lost is sent again by a process of its own.
            if reading.stopped_alone() {
                tell(Heard::Ended(Box::new(Said {
                    end: TurnEnd {
                        turn_id: String::new(),
                        stop_reason: Some("cancelled".to_owned()),
                        ..Default::default()
                    },
                    session_id: None,
                    init: None,
                    anchor: None,
                })));
            }
            let _ = child.wait();
            tell(Heard::Gone);
        });

        Ok(Self { control, pid, told })
    }

    /// Hands it the person's next message. False when it no longer listens.
    pub fn say(&self, prompt: &str) -> bool {
        self.control.say(prompt)
    }

    /// See [`Control::steer`].
    pub fn steer(&self, prompt: &str, uuid: &str, id: &str) -> bool {
        self.control.steer(prompt, uuid, id)
    }

    /// See [`Control::unheard`].
    pub fn unheard(&self) -> Vec<String> {
        self.control.unheard()
    }

    /// Sends a control request under `id`; its answer comes back as
    /// [`Heard::Control`]. False when it no longer listens.
    pub fn control(&self, id: &str, request: serde_json::Value) -> bool {
        match self.control.request(id, request) {
            Some(Switch::Write(_)) => true,
            Some(Switch::Kept) => {
                (self.told)(Heard::Control(
                    serde_json::json!({ "subtype": "success", "request_id": id }),
                ));
                true
            }
            None => false,
        }
    }

    /// See [`Control::stop_task`].
    pub fn stop_task(&self, task_id: &str) -> bool {
        self.control.stop_task(task_id)
    }

    /// Stops the turn it is on and keeps it running.
    pub fn interrupt(&self) -> bool {
        self.control.interrupt()
    }

    /// Lets it exit: with stdin closed the CLI finishes and goes. One that
    /// is still there a while later — hung on a tool, a stalled retry — is
    /// ended, rather than left running for as long as devpit does.
    pub fn close(&self) {
        self.control.close();
        let pid = self.pid;
        std::thread::spawn(move || {
            std::thread::sleep(GRACE);
            if still_running(pid) {
                let _ = std::process::Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .status();
            }
        });
    }

    /// Lets it go as devpit quits: stdin closed, and a CLI that does not
    /// exit on its own ended now, since nothing is left to end it later.
    pub fn quit(&self) {
        let ended = self.control.ended_by_devpit();
        self.control.close();
        if ended && still_running(self.pid) {
            devpit_pty::process::terminate(self.pid as i32);
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

/// Ends `pid` when it is still there `grace` from now.
pub(crate) fn end_after(pid: u32, grace: std::time::Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(grace);
        if still_running(pid) {
            devpit_pty::process::terminate(pid as i32);
        }
    });
}

#[cfg(all(test, unix))]
#[path = "resident_tests.rs"]
mod tests;
