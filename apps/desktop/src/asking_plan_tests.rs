use serde_json::json;

use super::*;
use crate::asking::{Answer, Asking};
use crate::asking_picks::{pick, refuse, Picks};

const PLAN: &str = "# Steps\n1. read";

/// Leaving plan mode is the prompt's to say: approved, the turn goes on in
/// the mode that asks before it changes anything.
#[test]
fn an_approved_plan_leaves_plan_mode_for_the_one_that_asks() {
    let said = plan_reply(
        &Picks::default(),
        "q1",
        json!({ "plan": PLAN }),
        Answer::Allow,
    );
    assert_eq!(said["behavior"], "allow");
    assert_eq!(said["updatedInput"]["plan"], PLAN);
    assert_eq!(
        said["updatedPermissions"],
        json!([{ "type": "setMode", "mode": "manual", "destination": "session" }])
    );
}

#[test]
fn a_plan_the_person_edited_is_approved_as_they_wrote_it() {
    let asking = Asking::default();
    let picks = Picks::default();
    let hear = asking.opened("q1");
    assert!(pick(
        &asking,
        &picks,
        "q1",
        vec![vec!["# Steps\n1. read\n2. test".to_owned()]]
    ));
    let answer = asking.wait("q1", hear);
    let said = plan_reply(&picks, "q1", json!({ "plan": PLAN }), answer);
    assert_eq!(said["behavior"], "allow");
    assert_eq!(said["updatedInput"]["plan"], "# Steps\n1. read\n2. test");
}

#[test]
fn a_refused_plan_tells_the_agent_what_to_change() {
    let asking = Asking::default();
    let picks = Picks::default();
    let hear = asking.opened("q1");
    assert!(refuse(&asking, &picks, "q1", "split step 1"));
    let answer = asking.wait("q1", hear);
    let said = plan_reply(&picks, "q1", json!({ "plan": PLAN }), answer);
    assert_eq!(said["behavior"], "deny");
    assert_eq!(said["message"], "Refused in devpit: split step 1");
    assert!(said.get("updatedPermissions").is_none());
}

#[test]
fn a_plan_refused_without_a_word_keeps_the_agent_planning() {
    let said = plan_reply(
        &Picks::default(),
        "q1",
        json!({ "plan": PLAN }),
        Answer::Deny,
    );
    assert_eq!(said["behavior"], "deny");
    assert_eq!(said["message"], NOT_YET);
}

/// A CLI that asks on its own prompt, with no hook: the person's answer goes
/// back as allowed, or refused with why.
#[test]
fn an_approval_on_the_prompt_is_answered_as_the_person_said() {
    let asking = Asking::default();
    let picks = Picks::default();
    assert_eq!(
        approval_reply(&picks, "q1", Answer::Allow),
        json!({ "behavior": "allow" })
    );
    let hear = asking.opened("q2");
    assert!(refuse(&asking, &picks, "q2", "not on main"));
    let answer = asking.wait("q2", hear);
    let said = approval_reply(&picks, "q2", answer);
    assert_eq!(said["behavior"], "deny");
    assert_eq!(said["message"], "Refused in devpit: not on main");
}

/// "Always in this chat" answers the same call again, never one that asks
/// for more reach on top of it, and never a plan.
#[test]
fn always_in_this_chat_never_covers_a_request_for_more_reach() {
    use crate::asking::Question;
    use crate::asking_kept::{rule_for, Kept};
    let kept = Kept::default();
    let plain = r#"{"command":"curl https://example.com"}"#;
    kept.keep("s1", rule_for("Bash", plain));
    let asked = |input: &str| Question {
        id: "q".to_owned(),
        session_id: "s1".to_owned(),
        tool: "Bash".to_owned(),
        input: input.to_owned(),
        cwd: String::new(),
        touch: None,
        conversation_id: None,
    };
    assert!(already_allowed(&kept, &asked(plain), false));
    let wider = r#"{"command":"curl https://example.com","grants":{"networkApprovalContext":{"host":"example.com","protocol":"https"}}}"#;
    assert!(!already_allowed(&kept, &asked(wider), false));
    assert!(!already_allowed(&kept, &asked(plain), true));
}

/// An ACP agent's shell call is asked as the command it runs, so one
/// "Always" never allows another command the agent titled the same; and a
/// request it marks as asking every time is never covered at all.
#[test]
fn always_on_an_acp_shell_call_covers_that_command_and_no_other() {
    use crate::asking::Question;
    use crate::asking_kept::{rule_for, Kept};
    let kept = Kept::default();
    let asked = |tool: &str, input: &str| Question {
        id: "q".to_owned(),
        session_id: "ses_1".to_owned(),
        tool: tool.to_owned(),
        input: input.to_owned(),
        cwd: String::new(),
        touch: None,
        conversation_id: None,
    };
    let build = r#"{"command":"rm -rf build","description":"bash"}"#;
    kept.keep("ses_1", rule_for("Bash", build));
    assert!(already_allowed(&kept, &asked("Bash", build), false));
    let home = r#"{"command":"rm -rf ~","description":"bash"}"#;
    assert!(!already_allowed(&kept, &asked("Bash", home), false));
    let each_time = r#"{"title":"bash","kind":"delete","always":false}"#;
    kept.keep("ses_1", rule_for("bash", each_time));
    assert!(!already_allowed(&kept, &asked("bash", each_time), false));
}

fn asked_on_the_prompt(id: &str, tool: &str) -> Question {
    Question {
        id: id.to_owned(),
        session_id: "ses_orch".to_owned(),
        tool: tool.to_owned(),
        input: json!({ "plan": PLAN }).to_string(),
        cwd: String::new(),
        touch: None,
        conversation_id: Some("conv_a".to_owned()),
    }
}

fn ids(questions: &[Question]) -> Vec<&str> {
    questions.iter().map(|one| one.id.as_str()).collect()
}

/// A question the CLI's own prompt asked — a plan to approve, a control
/// channel's prompt — is in the Waiting before the window hears it asked,
/// and what `permission.questions` draws returns it while it waits: a chat
/// that opens or reloads then draws it, not an empty list.
#[test]
fn a_prompt_question_is_waiting_before_the_window_hears_it_asked() {
    let asking = Asking::default();
    let waiting = Waiting::default();
    let question = asked_on_the_prompt("q1", "ExitPlanMode");
    let hear = asking.opened(&question.id);
    assert!(asking.answer(&question.id, Answer::Deny));

    held_for_a_late_window(
        &waiting,
        &question,
        // The window may re-read the moment it hears this.
        |_| assert_eq!(ids(&waiting.of("ses_orch", "conv_a")), ["q1"]),
        // The person has not answered yet.
        || {
            assert_eq!(ids(&waiting.of("ses_orch", "conv_a")), ["q1"]);
            asking.wait(&question.id, hear)
        },
        |_| (),
        |_| (),
    );
}

/// Answered, refused or timed out — a question nobody answers denies — the
/// question is out of the Waiting before the window hears it settled, so a
/// chat that re-reads then draws nothing.
#[test]
fn a_settled_prompt_question_is_gone_before_the_window_hears_it_settled() {
    for answer in [Answer::Allow, Answer::Deny] {
        let asking = Asking::default();
        let waiting = Waiting::default();
        let question = asked_on_the_prompt("q1", "Bash");
        let hear = asking.opened(&question.id);
        assert!(asking.answer(&question.id, answer));

        held_for_a_late_window(
            &waiting,
            &question,
            |_| (),
            || asking.wait(&question.id, hear),
            |_| (),
            // The window may re-read the moment it hears this.
            |_| assert!(waiting.of("ses_orch", "conv_a").is_empty()),
        );
    }
}
