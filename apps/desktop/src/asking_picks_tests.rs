use super::*;
use crate::asking::{Answer, Asking};
use std::sync::{Arc, Barrier};

/// As the CLI sent it on 2.1.318, `description` and all.
const COLOUR: &str = r#"{"questions":[{"question":"Which colour?","header":"Colour","options":[{"label":"Red","description":"Red"},{"label":"Green","description":"Green"},{"label":"Blue","description":"Blue"}],"multiSelect":false}]}"#;

fn answers(said: &str) -> serde_json::Value {
    let said: serde_json::Value = serde_json::from_str(said).expect("json");
    assert_eq!(
        said["hookSpecificOutput"]["permissionDecision"], "allow",
        "{said}"
    );
    said["hookSpecificOutput"]["updatedInput"].clone()
}

/// The shape the CLI turned into "Your questions have been answered:
/// \"Which colour?\"=\"Green\"" on 2.1.318.
#[test]
fn a_pick_reaches_the_agent_as_the_answer_to_its_question() {
    let asking = Asking::default();
    let picks = Picks::default();
    let hear = asking.opened("q1");
    assert!(pick(&asking, &picks, "q1", vec![vec!["Green".to_owned()]]));
    let answer = asking.wait("q1", hear);
    let input = answers(&reply_for(&picks, "q1", COLOUR, answer));
    assert_eq!(input["answers"]["Which colour?"], "Green");

    assert_eq!(
        input["questions"][0]["header"], "Colour",
        "the questions go back as asked"
    );
}

#[test]
fn several_choices_to_one_question_go_back_as_one_answer() {
    let picks = Picks::default();
    picks.picked("q1", vec![vec!["Red".to_owned(), "Blue".to_owned()]]);
    let input = answers(&reply_for(&picks, "q1", COLOUR, Answer::Allow));
    assert_eq!(input["answers"]["Which colour?"], "Red, Blue");
}

#[test]
fn a_question_refused_sends_no_answer() {
    let picks = Picks::default();
    picks.picked("q1", vec![vec!["Red".to_owned()]]);
    let said: serde_json::Value =
        serde_json::from_str(&reply_for(&picks, "q1", COLOUR, Answer::Deny)).expect("json");
    assert_eq!(said["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(said["hookSpecificOutput"].get("updatedInput").is_none());
}

#[test]
fn picking_for_a_question_nobody_waits_on_keeps_nothing() {
    let asking = Asking::default();
    let picks = Picks::default();
    assert!(!pick(
        &asking,
        &picks,
        "q_gone",
        vec![vec!["Red".to_owned()]]
    ));
    assert!(picks.taken("q_gone").is_none());
}

#[test]
fn a_refusal_with_no_words_is_the_plain_refusal() {
    let asking = Asking::default();
    let picks = Picks::default();
    let hear = asking.opened("q1");
    assert!(refuse(&asking, &picks, "q1", "   "));
    let said = reply_for(&picks, "q1", "{}", asking.wait("q1", hear));
    assert_eq!(said, decision(Answer::Deny));
}

#[test]
fn a_late_answer_fails_without_touching_the_picks() {
    let asking = Asking::default();
    let picks = Picks::default();
    let hear = asking.opened("q1");
    assert!(pick(&asking, &picks, "q1", vec![vec!["Green".to_owned()]]));

    assert!(!pick(&asking, &picks, "q1", vec![vec!["Red".to_owned()]]));
    assert!(!refuse(&asking, &picks, "q1", "changed my mind"));
    assert!(!cancel(&asking, &picks, "q1"));

    let answer = asking.wait("q1", hear);
    let input = answers(&reply_for(&picks, "q1", COLOUR, answer));
    assert_eq!(input["answers"]["Which colour?"], "Green");

    let hear = asking.opened("q2");
    assert!(refuse(&asking, &picks, "q2", "not tonight"));
    assert!(!pick(&asking, &picks, "q2", vec![vec!["Red".to_owned()]]));
    assert!(!cancel(&asking, &picks, "q2"));
    let refused: serde_json::Value =
        serde_json::from_str(&reply_for(&picks, "q2", COLOUR, asking.wait("q2", hear)))
            .expect("refusal");
    assert_eq!(
        refused["hookSpecificOutput"]["permissionDecisionReason"],
        "Refused in devpit: not tonight"
    );

    let hear = asking.opened("q3");
    assert!(cancel(&asking, &picks, "q3"));
    assert!(!pick(&asking, &picks, "q3", vec![vec!["Blue".to_owned()]]));
    assert!(!refuse(&asking, &picks, "q3", "too late"));
    let answer = asking.wait("q3", hear);
    let mut writes = 0;
    crate::asking_plan::answer_if_not_cancelled(&picks, "q3", answer, |_| writes += 1);
    assert_eq!(writes, 0);
}

#[test]
fn window_remote_and_cancel_race_to_one_winner() {
    let asking = Arc::new(Asking::default());
    let picks = Arc::new(Picks::default());
    let hear = asking.opened("q1");
    let barrier = Arc::new(Barrier::new(4));

    let contender = |chosen: &'static str| {
        let (asking, picks, barrier) = (asking.clone(), picks.clone(), barrier.clone());
        std::thread::spawn(move || {
            barrier.wait();
            pick(&asking, &picks, "q1", vec![vec![chosen.to_owned()]])
        })
    };
    let window = contender("Green");
    let remote = contender("Red");
    let (cancel_asking, cancel_picks, cancel_barrier) =
        (asking.clone(), picks.clone(), barrier.clone());
    let cancelled = std::thread::spawn(move || {
        cancel_barrier.wait();
        cancel(&cancel_asking, &cancel_picks, "q1")
    });

    barrier.wait();
    let won = [
        window.join().expect("window"),
        remote.join().expect("remote"),
        cancelled.join().expect("cancel"),
    ];
    assert_eq!(won.into_iter().filter(|won| *won).count(), 1);

    let answer = asking.wait("q1", hear);
    let mut writes = 0;
    crate::asking_plan::answer_if_not_cancelled(&picks, "q1", answer, |_| writes += 1);
    assert_eq!(writes, if won[2] { 0 } else { 1 });
}
