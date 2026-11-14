use super::*;

fn question(id: &str, session: &str) -> Question {
    Question {
        id: id.to_owned(),
        session_id: session.to_owned(),
        tool: "AskUserQuestion".to_owned(),
        input: "{}".to_owned(),
        cwd: "/w".to_owned(),
        touch: None,
    }
}

/// A chat opened after the question was asked still finds it waiting, and
/// only the ones its session asked.
#[test]
fn a_window_that_opens_late_finds_the_questions_of_its_session() {
    let waiting = Waiting::default();
    waiting.opened(&question("q1", "ses_orch"));
    waiting.opened(&question("q2", "ses_step"));

    assert_eq!(
        waiting
            .on_session("ses_orch")
            .iter()
            .map(|one| one.id.as_str())
            .collect::<Vec<_>>(),
        ["q1"]
    );
    assert!(waiting.on_session("ses_nobody").is_empty());
}

/// Two on one session are drawn in the order they were asked.
#[test]
fn the_questions_of_a_session_keep_the_order_they_were_asked_in() {
    let waiting = Waiting::default();
    waiting.opened(&question("q1", "ses_orch"));
    waiting.opened(&question("q2", "ses_orch"));

    assert_eq!(
        waiting
            .on_session("ses_orch")
            .iter()
            .map(|one| one.id.as_str())
            .collect::<Vec<_>>(),
        ["q1", "q2"]
    );
}

/// Settled is settled wherever it happened: an answered or timed-out question
/// is not offered to a window that opens next.
#[test]
fn a_settled_question_is_offered_to_no_window() {
    let waiting = Waiting::default();
    waiting.opened(&question("q1", "ses_orch"));
    waiting.settled("q1");

    assert!(waiting.on_session("ses_orch").is_empty());
}
