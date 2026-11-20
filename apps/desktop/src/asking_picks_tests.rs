use super::*;
use crate::asking::{Answer, Asking};

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
