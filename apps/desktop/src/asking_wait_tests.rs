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

/// Ending one conversation changes no other, and ending it twice is once.
#[test]
fn ending_one_conversation_changes_no_other() {
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

#[test]
fn a_new_turn_clears_the_conversations_end() {
    let ends = Ends::default();
    ends.end("conv_a");
    assert!(ends.open("q1", Some("conv_a")));
    ends.close("q1");

    ends.started("conv_a");
    assert!(!ends.open("q2", Some("conv_a")));
}

/// A turn nobody here started — a Remote Control message, a followed
/// session — runs after the last one ended and never passes through the
/// start of a person's turn: its question waits, and the one the ended turn
/// left open is over.
#[test]
fn a_question_in_a_turn_woken_after_the_last_one_ended_waits() {
    let ends = Ends::default();
    ends.started("conv_a");
    assert!(!ends.open("q_old", Some("conv_a")));
    assert_eq!(
        ends.over(Over::TurnEnded("conv_a")),
        vec!["q_old".to_owned()]
    );
    assert!(ends.ended("q_old"));

    assert!(
        !ends.open("q_woken", Some("conv_a")),
        "the woken turn's question was born ended"
    );
    assert!(!ends.ended("q_woken"));
}

#[test]
fn ended_conversations_are_bounded() {
    let ends = Ends::default();
    for number in 0..=ENDED_CONVERSATIONS {
        ends.end(&format!("conv_{number}"));
    }
    assert!(!ends.open("old", Some("conv_0")));
    assert!(ends.open("recent", Some(&format!("conv_{ENDED_CONVERSATIONS}"))));
}

#[test]
fn no_answer_wins_answer_after_the_end() {
    let asking = crate::asking::Asking::default();
    let picks = crate::asking_picks::Picks::default();
    let ends = Ends::default();
    let question = asked("q1", "AskUserQuestion", Some("conv_a"));
    let hear = asking.opened(&question.id);
    ends.open(&question.id, question.conversation_id.as_deref());

    ends.settle(Over::Closed("conv_a"), &asking);

    assert!(!asking.answer(&question.id, Answer::Allow));
    assert!(!crate::asking_picks::pick(
        &asking,
        &picks,
        &question.id,
        vec![vec!["Green".to_owned()]],
    ));
    assert!(!crate::asking_picks::refuse(
        &asking,
        &picks,
        &question.id,
        "too late",
    ));
    assert_eq!(asking.wait_for(&question, hear, &ends), Answer::Deny);
}

#[test]
fn an_unknown_cancel_is_forgotten_unknown_cancel_is_forgotten() {
    let decisions = Decisions::default();
    assert!(!decisions.cancel("q_unknown", || false));
    assert!(decisions.answer("q_unknown", || true));
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

/// The chat's prompt registers every question it takes, a plan's too, before
/// its wait begins. A plan answered, or one that ran out of time, keeps
/// nothing: a later end of its conversation finds no question to end.
#[test]
fn a_plans_wait_keeps_nothing_once_it_is_over() {
    let (ends, clock) = (
        Arc::new(Ends::default()),
        Arc::new(Ticking::by(Duration::from_secs(10))),
    );
    assert!(!ends.open("plan_answered", Some("conv_a")));
    let (tell, wait) = waiting(
        asked("plan_answered", "ExitPlanMode", Some("conv_a")),
        &ends,
        &clock,
    );
    tell.send(Answer::Allow).expect("the wait is listening");
    assert_eq!(ends_soon(wait), Answer::Allow);

    assert!(!ends.open("plan_expired", Some("conv_a")));
    let (_tell, wait) = waiting(
        asked("plan_expired", "ExitPlanMode", Some("conv_a")),
        &ends,
        &clock,
    );
    let_pass(&clock, TEN_MINUTES, &wait);
    assert_eq!(ends_soon(wait), Answer::Deny);

    assert!(
        ends.over(Over::TurnEnded("conv_a")).is_empty(),
        "a plan whose wait was over was still registered"
    );
}
