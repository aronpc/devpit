use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::*;

const TEN_MINUTES: Duration = Duration::from_secs(600);

pub(crate) fn asked(id: &str, tool: &str, conversation: Option<&str>) -> Question {
    Question {
        id: id.to_owned(),
        session_id: "ses_orch".to_owned(),
        tool: tool.to_owned(),
        input: "{}".to_owned(),
        cwd: "/w".to_owned(),
        touch: None,
        conversation_id: conversation.map(str::to_owned),
    }
}

/// The question waiting on its own thread, as the hook's connection does,
/// with the end that answers it.
pub(crate) fn waiting(
    question: Question,
    ends: &Arc<Ends>,
    clock: &Arc<Ticking>,
) -> (Sender<Answer>, JoinHandle<Answer>) {
    let (tell, hear) = channel();
    let (ends, clock) = (ends.clone(), clock.clone());
    let wait = std::thread::spawn(move || waits(&hear, &question, &ends, &*clock));
    (tell, wait)
}

/// Lets the wait run until it has seen `passed` go by, or it ends first.
pub(crate) fn let_pass(clock: &Ticking, passed: Duration, wait: &JoinHandle<Answer>) {
    let real = Instant::now();
    while clock.passed() < passed && !wait.is_finished() {
        assert!(
            real.elapsed() < Duration::from_secs(10),
            "the wait stopped reading the clock"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Ends within a real second, or the test fails rather than hangs.
fn ends_soon(wait: JoinHandle<Answer>) -> Answer {
    let real = Instant::now();
    while !wait.is_finished() {
        assert!(
            real.elapsed() < Duration::from_secs(5),
            "the wait did not end"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    wait.join().expect("the wait panicked")
}

/// Nobody answers for ten minutes, and the question is still asked: the
/// answer that comes then is the one that goes back.
#[test]
fn a_question_to_the_person_outlasts_ten_minutes_unanswered() {
    let (ends, clock) = (
        Arc::new(Ends::default()),
        Arc::new(Ticking::by(Duration::from_secs(30))),
    );
    let (tell, wait) = waiting(
        asked("q1", "AskUserQuestion", Some("conv_a")),
        &ends,
        &clock,
    );

    let_pass(&clock, TEN_MINUTES, &wait);
    assert!(
        !wait.is_finished(),
        "the person's question gave up after {:?}",
        clock.passed()
    );

    tell.send(Answer::Allow).expect("the wait is listening");
    assert_eq!(ends_soon(wait), Answer::Allow);
}

#[test]
fn a_question_ends_when_its_conversation_ends() {
    let (ends, clock) = (
        Arc::new(Ends::default()),
        Arc::new(Ticking::by(Duration::from_secs(30))),
    );
    let (_tell, other) = waiting(
        asked("q2", "AskUserQuestion", Some("conv_b")),
        &ends,
        &clock,
    );
    let (_tell_too, wait) = waiting(
        asked("q1", "AskUserQuestion", Some("conv_a")),
        &ends,
        &clock,
    );
    let_pass(&clock, Duration::from_secs(60), &wait);

    ends.end("conv_a");
    assert_eq!(ends_soon(wait), Answer::Deny);
    assert!(
        !other.is_finished(),
        "another chat's question ended with it"
    );
    assert!(!ends.ended("q1"), "an ended wait left itself behind");
}

/// Ending a conversation that has no question waiting changes nothing, and
/// ending one twice is ending it once.
#[test]
fn ending_a_conversation_with_no_question_waiting_ends_nothing() {
    let ends = Ends::default();
    ends.open("q1", Some("conv_a"));
    ends.end("conv_nobody");
    assert!(!ends.ended("q1"));

    ends.end("conv_a");
    ends.end("conv_a");
    assert!(ends.ended("q1"));
    ends.close("q1");
    ends.close("q1");
    assert!(!ends.ended("q1"));
}

/// Closing the app ends every question, a terminal's too.
#[test]
fn every_question_ends_when_the_app_closes() {
    let (ends, clock) = (
        Arc::new(Ends::default()),
        Arc::new(Ticking::by(Duration::from_secs(30))),
    );
    let (_tell, chat) = waiting(
        asked("q1", "AskUserQuestion", Some("conv_a")),
        &ends,
        &clock,
    );
    let (_tell_too, terminal) = waiting(asked("q2", "AskUserQuestion", None), &ends, &clock);
    let_pass(&clock, Duration::from_secs(60), &chat);

    ends.end_all();
    ends.end_all();
    assert_eq!(ends_soon(chat), Answer::Deny);
    assert_eq!(ends_soon(terminal), Answer::Deny);
}

/// The connection that would carry the answer is gone: nothing to wait for.
#[test]
fn a_question_ends_when_its_sender_is_gone() {
    let (ends, clock) = (
        Arc::new(Ends::default()),
        Arc::new(Ticking::by(Duration::from_secs(30))),
    );
    let (tell, wait) = waiting(
        asked("q1", "AskUserQuestion", Some("conv_a")),
        &ends,
        &clock,
    );
    let_pass(&clock, Duration::from_secs(60), &wait);

    drop(tell);
    assert_eq!(ends_soon(wait), Answer::Deny);
}

/// The two minutes a question that is not to the person still waits: not
/// one read of the clock past them.
fn ends_at_two_minutes(tool: &str) {
    let (ends, clock) = (
        Arc::new(Ends::default()),
        Arc::new(Ticking::by(Duration::from_secs(10))),
    );
    let (_tell, wait) = waiting(asked("q1", tool, Some("conv_a")), &ends, &clock);

    let_pass(&clock, TEN_MINUTES, &wait);
    assert_eq!(ends_soon(wait), Answer::Deny);
    assert_eq!(clock.passed(), WAITS, "{tool} waited the wrong time");
}

#[test]
fn a_supervised_permission_still_ends_at_two_minutes() {
    assert!(crate::question::changes_something("Bash"));
    ends_at_two_minutes("Bash");
}

#[test]
fn a_plan_still_ends_at_two_minutes() {
    ends_at_two_minutes("ExitPlanMode");
}
