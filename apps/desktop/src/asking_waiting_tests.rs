use super::*;

fn question(id: &str, session: &str) -> Question {
    asked_in(id, session, None)
}

fn asked_in(id: &str, session: &str, conversation: Option<&str>) -> Question {
    Question {
        id: id.to_owned(),
        session_id: session.to_owned(),
        tool: "AskUserQuestion".to_owned(),
        input: "{}".to_owned(),
        cwd: "/w".to_owned(),
        touch: None,
        conversation_id: conversation.map(str::to_owned),
    }
}

fn ids(questions: &[Question]) -> Vec<&str> {
    questions.iter().map(|one| one.id.as_str()).collect()
}

/// A chat opened after the question was asked still finds it waiting, and
/// only the ones its session asked.
#[test]
fn a_window_that_opens_late_finds_the_questions_of_its_session() {
    let waiting = Waiting::default();
    waiting.opened(&question("q1", "ses_orch"));
    waiting.opened(&question("q2", "ses_step"));

    assert_eq!(ids(&waiting.of("ses_orch", "conv_a")), ["q1"]);
    assert!(waiting.of("ses_nobody", "conv_a").is_empty());
}

/// Two on one session are drawn in the order they were asked.
#[test]
fn the_questions_of_a_session_keep_the_order_they_were_asked_in() {
    let waiting = Waiting::default();
    waiting.opened(&question("q1", "ses_orch"));
    waiting.opened(&question("q2", "ses_orch"));

    assert_eq!(ids(&waiting.of("ses_orch", "conv_a")), ["q1", "q2"]);
}

/// Settled is settled wherever it happened: an answered or timed-out question
/// is not offered to a window that opens next.
#[test]
fn a_settled_question_is_offered_to_no_window() {
    let waiting = Waiting::default();
    waiting.opened(&question("q1", "ses_orch"));
    waiting.settled("q1");

    assert!(waiting.of("ses_orch", "conv_a").is_empty());
}

/// One session can be two conversations' (a rewind copies it, an external
/// session is adopted twice): a question goes to the chat whose process asked.
#[test]
fn a_question_a_chat_asked_is_offered_to_that_chat_only_on_a_shared_session() {
    let waiting = Waiting::default();
    waiting.opened(&asked_in("q1", "ses_shared", Some("conv_a")));
    waiting.opened(&asked_in("q2", "ses_shared", Some("conv_b")));

    assert_eq!(ids(&waiting.of("ses_shared", "conv_a")), ["q1"]);
    assert_eq!(ids(&waiting.of("ses_shared", "conv_b")), ["q2"]);
    assert!(waiting.of("ses_other", "conv_a").is_empty());
}

/// A session started outside a chat, in a terminal, names no conversation:
/// any chat on its session is offered it, as before.
#[test]
fn a_question_from_outside_a_chat_is_offered_by_its_session_alone() {
    let waiting = Waiting::default();
    waiting.opened(&asked_in("q1", "ses_term", None));

    assert_eq!(ids(&waiting.of("ses_term", "conv_a")), ["q1"]);
    assert_eq!(ids(&waiting.of("ses_term", "conv_b")), ["q1"]);
    assert!(waiting.of("ses_other", "conv_a").is_empty());
}
