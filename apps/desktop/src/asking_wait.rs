//! How long a question waits, and what ends the wait.
//!
//! A question to the person (`AskUserQuestion`) waits without end: they may
//! be away for an hour and still want to answer. It ends when its chat ends,
//! when the app closes, or when the one asking is gone. A question about a
//! tool that changes something, and a plan, still answer themselves at two
//! minutes — a turn held on a command is a turn going nowhere.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::asking::{Answer, Question};
use crate::asking_picks::to_the_person;

/// How long a question that is not to the person waits before it answers
/// itself. A hook that waits forever hangs the agent when the window is gone;
/// two minutes is long enough to look up and short enough that a forgotten
/// turn does not sit there for the afternoon.
pub(crate) const WAITS: Duration = Duration::from_secs(120);

/// How often a wait looks up to see whether it has ended.
const SLICE: Duration = Duration::from_secs(1);

/// Enough recent chats to cover registration races without keeping every
/// conversation the process has ever closed.
const ENDED_CONVERSATIONS: usize = 256;

/// One winner for a question while its waiting thread catches up.
#[derive(Default)]
pub(crate) struct Decisions(Mutex<HashMap<String, Decision>>);

enum Decision {
    Answered,
    Cancelled,
}

impl Decisions {
    /// Runs one answer while holding the claim. The answer stores its payload
    /// before waking the waiter, so another contender cannot replace it.
    pub(crate) fn answer(&self, id: &str, answer: impl FnOnce() -> bool) -> bool {
        let Ok(mut decided) = self.0.lock() else {
            return false;
        };
        if decided.contains_key(id) || !answer() {
            return false;
        }
        decided.insert(id.to_owned(), Decision::Answered);
        true
    }

    /// Claims a cancellation only while its registered question still waits.
    pub(crate) fn cancel(&self, id: &str, cancel: impl FnOnce() -> bool) -> bool {
        let Ok(mut decided) = self.0.lock() else {
            return false;
        };
        if decided.contains_key(id) || !cancel() {
            return false;
        }
        decided.insert(id.to_owned(), Decision::Cancelled);
        true
    }

    /// Takes whether cancellation won; an ordinary answer remains available
    /// to the picks until its reply has been built.
    pub(crate) fn cancelled(&self, id: &str) -> bool {
        let Ok(mut decided) = self.0.lock() else {
            return false;
        };
        if matches!(decided.get(id), Some(Decision::Cancelled)) {
            decided.remove(id);
            return true;
        }
        false
    }

    pub(crate) fn finished(&self, id: &str) {
        if let Ok(mut decided) = self.0.lock() {
            decided.remove(id);
        }
    }
}

/// The time a wait reads, so a test can let ten minutes pass in a moment.
pub(crate) trait Clock {
    fn now(&self) -> Instant;

    /// How long one look at the channel lasts.
    fn slice(&self) -> Duration {
        SLICE
    }
}

/// The system's clock.
pub(crate) struct Wall;

impl Clock for Wall {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// The questions to the person that are waiting, and the ones whose wait was
/// ended from outside: their chat closed, or the app did.
#[derive(Default)]
pub(crate) struct Ends(Mutex<Open>);

#[derive(Default)]
struct Open {
    /// Question to the conversation that asked it; none for a terminal's.
    asked_by: HashMap<String, Option<String>>,
    ended: HashSet<String>,
    ended_conversations: VecDeque<String>,
    app_ended: bool,
}

impl Ends {
    /// Registers a question and says whether its chat had already ended.
    pub(crate) fn open(&self, question_id: &str, conversation_id: Option<&str>) -> bool {
        if let Ok(mut open) = self.0.lock() {
            open.asked_by
                .insert(question_id.to_owned(), conversation_id.map(str::to_owned));
            let ended = open.app_ended
                || conversation_id
                    .is_some_and(|id| open.ended_conversations.iter().any(|ended| ended == id));
            if ended {
                open.ended.insert(question_id.to_owned());
            }
            return ended;
        }
        false
    }

    /// Ends every question this conversation is waiting on and remembers the
    /// end for a prompt that was already in flight but has not registered yet.
    pub(crate) fn end(&self, conversation_id: &str) -> Vec<String> {
        if let Ok(mut open) = self.0.lock() {
            let Open {
                asked_by, ended, ..
            } = &mut *open;
            let its = asked_by
                .iter()
                .filter(|(_, by)| by.as_deref() == Some(conversation_id));
            let its: Vec<String> = its.map(|(id, _)| id.clone()).collect();
            ended.extend(its.iter().cloned());
            open.ended_conversations
                .retain(|ended| ended != conversation_id);
            open.ended_conversations
                .push_back(conversation_id.to_owned());
            if open.ended_conversations.len() > ENDED_CONVERSATIONS {
                open.ended_conversations.pop_front();
            }
            return its;
        }
        Vec::new()
    }

    /// Ends the questions this conversation is waiting on now, and remembers
    /// nothing for one asked later.
    pub(crate) fn end_open(&self, conversation_id: &str) -> Vec<String> {
        let Ok(mut open) = self.0.lock() else {
            return Vec::new();
        };
        let Open {
            asked_by, ended, ..
        } = &mut *open;
        let its: Vec<String> = asked_by
            .iter()
            .filter(|(_, by)| by.as_deref() == Some(conversation_id))
            .map(|(id, _)| id.clone())
            .collect();
        ended.extend(its.iter().cloned());
        its
    }

    /// Ends every question waiting: the app is closing.
    pub(crate) fn end_all(&self) -> Vec<String> {
        if let Ok(mut open) = self.0.lock() {
            let ids: Vec<String> = open.asked_by.keys().cloned().collect();
            open.ended.extend(ids.iter().cloned());
            open.app_ended = true;
            return ids;
        }
        Vec::new()
    }

    /// Ends what this way of going leaves nobody to answer: a chat's own
    /// questions, or every one when the app quits. Never a deadline.
    pub(crate) fn over(&self, gone: Over<'_>) -> Vec<String> {
        match gone {
            Over::Closed(id) | Over::Interrupted(id) | Over::Exited(id) => self.end(id),
            // A turn ends only once the CLI has its answers, so nothing of it
            // is in flight: ending what is open is enough, and the next turn,
            // a woken one included, may ask again.
            Over::TurnEnded(id) => self.end_open(id),
            Over::AppQuit => self.end_all(),
        }
    }

    /// Marks the end and denies its open questions before another answer can
    /// take their senders.
    pub(crate) fn settle(&self, gone: Over<'_>, asking: &crate::asking::Asking) {
        for id in self.over(gone) {
            asking.end(&id);
        }
    }

    /// A new turn is allowed to ask again in the same conversation.
    pub(crate) fn started(&self, conversation_id: &str) {
        if let Ok(mut open) = self.0.lock() {
            open.ended_conversations
                .retain(|ended| ended != conversation_id);
        }
    }

    pub(crate) fn ended(&self, question_id: &str) -> bool {
        self.0
            .lock()
            .is_ok_and(|open| open.ended.contains(question_id))
    }

    /// The wait is over, however it ended: nothing is kept for it.
    pub(crate) fn close(&self, question_id: &str) {
        if let Ok(mut open) = self.0.lock() {
            open.asked_by.remove(question_id);
            open.ended.remove(question_id);
        }
    }
}

/// The ways a chat stops being there to hear an answer, by conversation.
pub(crate) enum Over<'a> {
    Closed(&'a str),
    /// The process stays, but the turn that asked is stopped.
    Interrupted(&'a str),
    /// The conversation's resident process exited.
    Exited(&'a str),
    /// The turn's process is done, however it ended.
    TurnEnded(&'a str),
    AppQuit,
}

/// Waits as the question's tool says: the person's question without end, any
/// other for `WAITS`. Giving up denies — a question nobody answered is not a
/// question someone said yes to. However it ends, nothing is kept for it: the
/// chat's prompt registers every question it takes, a plan's too.
pub(crate) fn waits(
    hear: &Receiver<Answer>,
    question: &Question,
    ends: &Ends,
    clock: &dyn Clock,
) -> Answer {
    let answer = if to_the_person(&question.tool) {
        ends.open(&question.id, question.conversation_id.as_deref());
        wait_for_person(hear, &question.id, ends, clock)
    } else {
        within(hear, clock)
    };
    ends.close(&question.id);
    answer
}

/// Waits for the person with no deadline: until they answer, the question is
/// ended (`Ends`), or the one that would hear the answer is gone.
pub(crate) fn wait_for_person(
    hear: &Receiver<Answer>,
    question_id: &str,
    ends: &Ends,
    clock: &dyn Clock,
) -> Answer {
    sliced(hear, clock, |_| ends.ended(question_id))
}

/// Waits `WAITS` at most.
pub(crate) fn within(hear: &Receiver<Answer>, clock: &dyn Clock) -> Answer {
    sliced(hear, clock, |waited| waited >= WAITS)
}

/// Looks at the channel a slice at a time, until an answer comes, the sender
/// is gone, or `over` — told how long it has waited — says to stop.
fn sliced(hear: &Receiver<Answer>, clock: &dyn Clock, over: impl Fn(Duration) -> bool) -> Answer {
    let start = clock.now();
    loop {
        match hear.recv_timeout(clock.slice()) {
            Ok(answer) => return answer,
            Err(RecvTimeoutError::Disconnected) => return Answer::Deny,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if over(clock.now().saturating_duration_since(start)) {
            return Answer::Deny;
        }
    }
}

/// A clock that moves `step` each time it is read, and looks at the channel
/// for a millisecond: minutes pass as fast as the wait reads them.
#[cfg(test)]
pub(crate) struct Ticking {
    start: Instant,
    step: Duration,
    reads: std::sync::atomic::AtomicU32,
}

#[cfg(test)]
impl Ticking {
    pub(crate) fn by(step: Duration) -> Self {
        Self {
            start: Instant::now(),
            step,
            reads: Default::default(),
        }
    }

    /// How much time the wait has seen pass.
    pub(crate) fn passed(&self) -> Duration {
        let reads = self.reads.load(std::sync::atomic::Ordering::SeqCst);
        self.step * reads.saturating_sub(1)
    }
}

#[cfg(test)]
impl Clock for Ticking {
    fn now(&self) -> Instant {
        let reads = self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.start + self.step * reads
    }

    fn slice(&self) -> Duration {
        Duration::from_millis(1)
    }
}

#[cfg(test)]
#[path = "asking_wait_tests.rs"]
pub(crate) mod tests;
