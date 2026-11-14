//! One turn at a time in a conversation, and what a turn lets go of when it
//! ends.
//!
//! Apart from `chat.rs` because it is a rule, not a step of the send: the
//! claim is taken before the process exists and released however the turn
//! leaves — including the `?` at the end, which used to skip the cleanup and
//! leave a pid behind. A later `chat.cancel` then sent SIGTERM to a number the
//! operating system had already given to somebody else.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use devpit_rpc::{ErrorCode, RpcError};

use crate::chat::Talking;
use crate::steering::Steering;

/// The claim a turn holds while it runs.
///
/// Dropping it removes the pid and releases the steering handle, so every exit
/// path — answered, failed, cancelled, or a `?` on the way out — leaves the
/// conversation ready for the next turn. The pid itself is written by the
/// callback the turn already reports its process through.
pub(crate) struct TurnGuard {
    running: Arc<Mutex<HashMap<String, Option<u32>>>>,
    stopping: Arc<Mutex<HashMap<String, Signal>>>,
    steering: Steering,
    conversation_id: String,
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        if let Ok(mut held) = self.running.lock() {
            held.remove(&self.conversation_id);
            if let Ok(mut stopping) = self.stopping.lock() {
                stopping.remove(&self.conversation_id);
            }
        }
        self.steering.release(&self.conversation_id);
        // Between turns: what waits for this chat can go now.
        crate::session_bus::chat_free();
    }
}

impl Talking {
    /// Claims this conversation for a turn, or says it is busy.
    ///
    /// Two turns in one conversation are not two turns: they are two processes
    /// appending to the same transcript, with one pid recorded for both.
    pub(crate) fn begin(
        &self,
        conversation_id: &str,
        steering: &Steering,
    ) -> Result<TurnGuard, RpcError> {
        let mut held = self
            .running
            .lock()
            .map_err(|_| RpcError::internal("the turns in flight could not be read"))?;
        if held.contains_key(conversation_id) {
            return Err(RpcError::new(
                ErrorCode::Conflict,
                "this conversation already has a turn running".to_owned(),
            ));
        }
        held.insert(conversation_id.to_owned(), None);
        drop(held);

        Ok(TurnGuard {
            running: self.running.clone(),
            stopping: self.stopping.clone(),
            steering: steering.clone(),
            conversation_id: conversation_id.to_owned(),
        })
    }
}

/// How a stop ends a turn's process.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signal {
    Term,
    Kill,
}

impl Talking {
    /// Stops this conversation's turn with `signal`: its process now, or —
    /// claimed, its process not started yet — as soon as it starts. Whether a
    /// turn was there to stop.
    pub(crate) fn stop(&self, conversation: &str, signal: Signal) -> bool {
        let Ok(held) = self.running.lock() else {
            return false;
        };
        match held.get(conversation) {
            None => false,
            Some(Some(pid)) => {
                signalled(*pid, signal);
                true
            }
            Some(None) => {
                self.ask(conversation, signal);
                true
            }
        }
    }

    /// A stop of a turn still starting, kept for it; false when the turn is
    /// not starting — none, or one whose process already runs.
    pub(crate) fn stop_starting(&self, conversation: &str) -> bool {
        let Ok(held) = self.running.lock() else {
            return false;
        };
        let starting = held.get(conversation) == Some(&None);
        if starting {
            self.ask(conversation, Signal::Term);
        }
        starting
    }

    fn ask(&self, conversation: &str, signal: Signal) {
        if let Ok(mut stopping) = self.stopping.lock() {
            let asked = stopping.entry(conversation.to_owned()).or_insert(signal);
            if signal == Signal::Kill {
                *asked = signal;
            }
        }
    }

    /// What the turn calls with its process once it starts: kept to be
    /// stopped. Answers whether the turn goes on. A stop asked before is
    /// carried out on the process, or — a `resident` process, which outlives
    /// the turn — left to the turn, which interrupts itself.
    pub(crate) fn on_start(
        &self,
        conversation: &str,
        resident: bool,
    ) -> impl Fn(u32) -> bool + Send + Sync + 'static {
        let (running, stopping) = (self.running.clone(), self.stopping.clone());
        let key = conversation.to_owned();
        move |pid| {
            let Ok(mut held) = running.lock() else {
                return true;
            };
            held.insert(key.clone(), Some(pid));
            let asked = stopping.lock().ok().and_then(|mut all| all.remove(&key));
            match asked {
                None => true,
                Some(_) if resident => false,
                Some(signal) => {
                    signalled(pid, signal);
                    false
                }
            }
        }
    }

    /// Force-stops this conversation's turn, unless its process is the one
    /// `ended` already. Whether a turn was there.
    pub(crate) fn force(&self, conversation: &str, ended: Option<u32>) -> bool {
        let spared = self
            .running
            .lock()
            .is_ok_and(|held| ended.is_some() && held.get(conversation) == Some(&ended));
        spared || self.stop(conversation, Signal::Kill)
    }

    /// The turn's process is gone: its pid is let go of, so no stop reaches a
    /// number the system may have given to another process.
    pub(crate) fn exited(&self, conversation: &str) {
        if let Ok(mut held) = self.running.lock() {
            if let Some(pid) = held.get_mut(conversation) {
                *pid = None;
            }
        }
    }
}

/// A polite stop to the process; a forced one to its group, the tools it
/// started included.
fn signalled(pid: u32, signal: Signal) {
    match signal {
        Signal::Term => {
            let _ = i32::try_from(pid).is_ok_and(devpit_pty::process::terminate);
        }
        Signal::Kill => forced(pid),
    }
}

/// Ends a turn's process at once with everything it started: its group, and
/// what left the group for a session of its own, as Claude Code's Bash tool does.
pub(crate) fn forced(pid: u32) {
    // Read first: once the CLI is gone, its children name init as their parent.
    let tools = devpit_pty::session_members(&[pid]).unwrap_or_default();
    devpit_pty::end_group(pid);
    devpit_pty::stop_members(&tools, std::time::Duration::ZERO);
}

#[cfg(test)]
#[path = "chat_running_tests.rs"]
mod tests;
