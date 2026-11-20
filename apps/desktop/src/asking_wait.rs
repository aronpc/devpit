//! How long a question waits, and what ends the wait.
//!
//! A question to the person (`AskUserQuestion`) waits without end: they may
//! be away for an hour and still want to answer. It ends when its chat ends,
//! when the app closes, or when the one asking is gone. A question about a
//! tool that changes something, and a plan, still answer themselves at two
//! minutes — a turn held on a command is a turn going nowhere.

use std::collections::{HashMap, HashSet};
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
}

impl Ends {
    pub(crate) fn open(&self, question_id: &str, conversation_id: Option<&str>) {
        if let Ok(mut open) = self.0.lock() {
            open.asked_by
                .insert(question_id.to_owned(), conversation_id.map(str::to_owned));
        }
    }

    /// Ends every question this conversation is waiting on. One with none
    /// waiting is left as it was.
    pub(crate) fn end(&self, conversation_id: &str) {
        if let Ok(mut open) = self.0.lock() {
            let Open { asked_by, ended } = &mut *open;
            let its = asked_by
                .iter()
                .filter(|(_, by)| by.as_deref() == Some(conversation_id));
            ended.extend(its.map(|(id, _)| id.clone()));
        }
    }

    /// Ends every question waiting: the app is closing.
    pub(crate) fn end_all(&self) {
        if let Ok(mut open) = self.0.lock() {
            let Open { asked_by, ended } = &mut *open;
            ended.extend(asked_by.keys().cloned());
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

/// Waits as the question's tool says: the person's question without end, any
/// other for `WAITS`. Giving up denies — a question nobody answered is not a
/// question someone said yes to.
pub(crate) fn waits(
    hear: &Receiver<Answer>,
    question: &Question,
    ends: &Ends,
    clock: &dyn Clock,
) -> Answer {
    if !to_the_person(&question.tool) {
        return within(hear, clock);
    }
    ends.open(&question.id, question.conversation_id.as_deref());
    let answer = wait_for_person(hear, &question.id, ends, clock);
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
