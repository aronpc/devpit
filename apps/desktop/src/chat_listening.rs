//! Who hears a resident conversation's process right now: the person's turn,
//! or a turn another session woke.
//!
//! Apart from `chat_resident.rs` because this is where the races were. The
//! person's turn takes the process only when it is quiet, under the same lock
//! a woken turn starts under, so neither lands in the other; and a Stop while
//! it waits stops the wait, not the woken turn it waits for.

use std::path::Path;
use std::sync::{mpsc, Condvar, Mutex};

use devpit_agentcli::resident::Heard;
use devpit_agentcli::store::append;
use devpit_agentcli::talk::Said;
use devpit_rpc::{Frame, Message, Part, Role, TurnEnd};
use tauri::ipc::Channel;

/// Where what the process says goes right now.
#[derive(Default)]
pub(crate) enum Listening {
    /// Nobody asked: a turn that starts now was woken.
    #[default]
    Nobody,
    /// The person's turn, read by the thread that sent it.
    Person(mpsc::Sender<Heard>),
    /// A turn another session woke, written here as it goes.
    Woken(Woken),
}

/// A woken turn in progress.
pub(crate) struct Woken {
    pub turn_id: String,
    pub answer_id: String,
    pub parts: Vec<Part>,
    /// When it opened, which is when the answer is said to have begun.
    pub began: f64,
    pub frames: Channel<Frame>,
    pub _relaying: crate::chat_relay::Relaying,
}

#[derive(Default)]
struct Now {
    listening: Listening,
    /// A person's turn is waiting for the process.
    waiting: bool,
    /// ...and was stopped before it had it.
    stopped: bool,
    /// Another conversation of the same project listens now: this process
    /// closes when the turn on it ends. A person's turn that begins here
    /// after undoes it.
    closing: Option<Closing>,
    /// The closer has left the ear: the closing is committed, and nothing
    /// more is taken from this process.
    closed: bool,
}

impl Now {
    /// A turn is on: the person's, one a session woke, or the person's
    /// waiting for the process. Displacing decides on this.
    fn is_busy(&self) -> bool {
        self.waiting || !matches!(self.listening, Listening::Nobody)
    }
}

type Closing = Box<dyn FnOnce() + Send>;

/// One conversation's process, as the turns on it see it.
#[derive(Default)]
pub(crate) struct Ear {
    now: Mutex<Now>,
    /// Told whenever the process goes quiet, or a wait is stopped.
    quiet: Condvar,
}

/// The person's turn takes a quiet process, and never one a woken turn is on.
pub(crate) fn taken(listening: &mut Listening, tell: &mpsc::Sender<Heard>) -> bool {
    if !matches!(listening, Listening::Nobody) {
        return false;
    }
    *listening = Listening::Person(tell.clone());
    true
}

/// How a person's wait for the process ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Claim {
    /// The process is the person's.
    Claimed,
    /// Stop: the turn ends cancelled, whatever else happened to the process.
    Stopped,
    /// Displaced and closed: the turn goes to a new process.
    Closed,
}

/// What displacing a conversation did to its process.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Displaced {
    /// Quiet: the closing is committed here and the caller closes the process.
    Now,
    /// A turn is on it: the closer runs when the turn ends.
    Later,
}

impl Ear {
    /// A person's turn is on its way to this process: it reads as busy from
    /// here, so nobody closes it before the turn has claimed it. Called under
    /// the residents' lock, where the process is published.
    pub(crate) fn expect(&self) {
        if let Ok(mut now) = self.now.lock() {
            now.waiting = true;
        }
    }

    /// Waits for a woken turn to finish and takes the process, without letting
    /// go of the lock in between: a turn woken in that gap used to be
    /// overwritten, unsaved, and its words went into the person's answer.
    pub(crate) fn claim(&self, tell: &mpsc::Sender<Heard>) -> Claim {
        let Ok(mut now) = self.now.lock() else {
            return Claim::Closed;
        };
        now.waiting = true;
        loop {
            // Stop first: it wins over a closing committed beside it.
            if std::mem::take(&mut now.stopped) {
                now.waiting = false;
                // Quiet with a closer armed in the gap: no turn is left to
                // end and run it, so it goes now. A woken turn still on
                // keeps it for its own end.
                let close = if matches!(now.listening, Listening::Nobody) {
                    let close = now.closing.take();
                    now.closed |= close.is_some();
                    close
                } else {
                    None
                };
                drop(now);
                // Outside the lock: the closer takes locks of its own.
                if let Some(close) = close {
                    close();
                }
                return Claim::Stopped;
            }
            // Displaced and closed: this process is going. The person's turn
            // goes to a new one instead.
            if now.closed {
                now.waiting = false;
                return Claim::Closed;
            }
            if taken(&mut now.listening, tell) {
                now.waiting = false;
                return Claim::Claimed;
            }
            now = match self.quiet.wait(now) {
                Ok(now) => now,
                Err(_) => return Claim::Closed,
            };
        }
    }

    /// Whether a turn is on: the person's, one a session woke, or the
    /// person's waiting for the process. An ear that will not say reads as
    /// busy: the side that does not close is the safe one.
    #[cfg(test)]
    pub(crate) fn busy(&self) -> bool {
        self.now.lock().map(|now| now.is_busy()).unwrap_or(true)
    }

    /// Displaces this conversation: another of the same project listens now.
    /// Whether a turn is on it and the closing is armed are decided in one
    /// step, so a turn that ends in between cannot leave the closer unrun.
    /// Quiet: committed here, and the caller closes it. Busy: `close` runs
    /// once, when the turn ends — never under it.
    pub(crate) fn displace(&self, close: impl FnOnce() + Send + 'static) -> Displaced {
        let Ok(mut now) = self.now.lock() else {
            // An ear that will not say reads as busy, and nothing closes it.
            return Displaced::Later;
        };
        if now.is_busy() {
            now.closing = Some(Box::new(close));
            return Displaced::Later;
        }
        now.closed = true;
        now.closing = None;
        self.quiet.notify_all();
        Displaced::Now
    }

    /// Undoes the displacement: a person's turn is beginning here, so this
    /// conversation listens again. Whether there was one to undo. Once the
    /// closer has left the ear the closing is committed: the process is
    /// closed, and the turn goes to a new one.
    pub(crate) fn unmark(&self) -> bool {
        self.now
            .lock()
            .map(|mut now| now.closing.take().is_some())
            .unwrap_or(false)
    }

    /// Whether the displacement has closed this process.
    pub(crate) fn closed(&self) -> bool {
        self.now.lock().map(|now| now.closed).unwrap_or(true)
    }

    /// Whether another conversation has taken this one's place.
    pub(crate) fn displaced(&self) -> bool {
        self.now
            .lock()
            .map(|now| now.closing.is_some() || now.closed)
            .unwrap_or(true)
    }

    /// Stops a person's turn still waiting for the process. The woken turn it
    /// waits for goes on: Stop is for the person's own turn. False when none
    /// waits.
    pub(crate) fn stop_waiting(&self) -> bool {
        let Ok(mut now) = self.now.lock() else {
            return false;
        };
        if !now.waiting {
            return false;
        }
        now.stopped = true;
        self.quiet.notify_all();
        true
    }

    /// Hands what the process said to whoever it is for. A part nobody asked
    /// for starts a woken turn, made by `wake` — unless the conversation is
    /// displaced, when the process closes instead of taking one.
    pub(crate) fn hear(
        &self,
        heard: Heard,
        wake: impl FnOnce() -> Woken,
        transcript: &Path,
        head: &Path,
    ) {
        let close = {
            let Ok(mut now) = self.now.lock() else { return };
            // Quiet already with a displacement armed or done: the closer
            // goes, not what arrived.
            let close = if (now.closed || now.closing.is_some())
                && matches!(now.listening, Listening::Nobody)
            {
                now.closed = true;
                now.closing.take()
            } else {
                match (&mut now.listening, heard) {
                    (Listening::Person(tell), Heard::Ended(said)) => {
                        let _ = tell.send(Heard::Ended(said));
                        now.listening = Listening::Nobody;
                    }
                    // Gone mid-turn: that turn fails, and the conversation is left
                    // quiet so the next one starts a new process instead of waiting
                    // for this one for ever.
                    (Listening::Person(tell), Heard::Gone) => {
                        let _ = tell.send(Heard::Gone);
                        now.listening = Listening::Nobody;
                    }
                    (Listening::Person(tell), heard) => {
                        let _ = tell.send(heard);
                    }
                    (Listening::Nobody, Heard::Part(part)) => {
                        let mut woken = wake();
                        woken.heard(part);
                        now.listening = Listening::Woken(woken);
                    }
                    (Listening::Woken(woken), Heard::Part(part)) => woken.heard(part),
                    (Listening::Woken(_), Heard::Ended(said)) => {
                        if let Listening::Woken(woken) = std::mem::take(&mut now.listening) {
                            woken.finish(transcript, head, *said);
                        }
                    }
                    (Listening::Woken(_), Heard::Gone) => {
                        if let Listening::Woken(woken) = std::mem::take(&mut now.listening) {
                            woken.gone(transcript);
                        }
                    }
                    (Listening::Nobody, _) | (_, Heard::Control(_)) => {}
                }
                if !matches!(now.listening, Listening::Nobody) {
                    return;
                }
                // Quiet with a displacement armed: the process goes, and a
                // part that arrives after wakes no turn on it.
                now.closed = now.closing.is_some();
                now.closing.take()
            };
            self.quiet.notify_all();
            close
        };
        // Outside the ear's lock, and off this thread. `hear` runs on the
        // process's reader, which still holds the Resident's `on` mutex, and
        // the closer takes the residents' lock; `Live::switch` runs under the
        // residents' lock and, when the CLI keeps the process (Codex model
        // switch, ACP mode), answers through that same `on`. Closing inline
        // made the two wait on each other. A thread of its own keeps the
        // order one way (residents, then ear) whatever the reader holds; the
        // closer is one-shot, so it still runs exactly once. Chosen over
        // answering `Kept` outside the caller's lock: that changes the
        // agentcli contract for every adapter, this changes one call.
        if let Some(close) = close {
            std::thread::spawn(close);
        }
    }
}

impl Woken {
    fn heard(&mut self, part: Part) {
        // Joined as the person's own turn joins them: a delta kept as a part
        // of its own was read back as a paragraph of its own.
        crate::chat_relay::kept(&mut self.parts, part.clone());
        self.frames
            .send(Frame::Part {
                message_id: self.answer_id.clone(),
                part,
            })
            .ok();
    }

    fn written(&self, end: &TurnEnd) -> Message {
        Message {
            id: self.answer_id.clone(),
            turn_id: Some(self.turn_id.clone()),
            role: Role::Assistant,
            parts: self.parts.clone(),
            created_at: self.began,
            streaming: false,
            ended: Some(devpit_rpc::Ended::of(end, now())),
        }
    }

    fn finish(self, transcript: &Path, head: &Path, said: Said) {
        let _ = append(transcript, &self.written(&said.end));
        crate::chat_turn::settle(
            head,
            None,
            &self.turn_id,
            &said.end,
            said.session_id,
            said.anchor,
        );
        let end = TurnEnd {
            turn_id: self.turn_id,
            ..said.end
        };
        self.frames.send(Frame::Ended { end }).ok();
    }

    /// The process died mid-answer: what it had said is kept, and the end
    /// names the turn, so the chat closes the answer it was showing.
    pub(crate) fn gone(self, transcript: &Path) {
        let end = TurnEnd {
            turn_id: self.turn_id.clone(),
            cost_usd: None,
            duration_ms: None,
            stop_reason: Some("interrupted".to_owned()),
            is_error: true,
            context: None,
            ..Default::default()
        };
        let _ = append(transcript, &self.written(&end));
        self.frames.send(Frame::Ended { end }).ok();
    }
}

pub(crate) fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs_f64())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "chat_listening_tests.rs"]
mod tests;
