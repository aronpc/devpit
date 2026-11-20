use super::*;

fn task(id: &str, status: &str) -> Part {
    Part::Task {
        task_id: id.to_owned(),
        call_id: None,
        task_kind: Some("local_bash".to_owned()),
        description: None,
        status: status.to_owned(),
        summary: None,
        duration_ms: None,
    }
}

/// The shape the CLI's schema accepts, and the one the probe sent.
#[test]
fn stopping_a_task_is_a_control_request_naming_it() {
    let sent: serde_json::Value =
        serde_json::from_str(&stop_task_request("bxk020dqw")).expect("json");
    assert_eq!(sent["type"], "control_request");
    assert_eq!(sent["request"]["subtype"], "stop_task");
    assert_eq!(sent["request"]["task_id"], "bxk020dqw");
    assert!(sent["request_id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[test]
fn a_task_runs_from_its_start_until_any_other_status() {
    let mut running = Running::default();
    running.saw(&task("a", "started"));
    running.saw(&task("a", "running"));
    assert!(!running.idle());
    // `killed` and `stopped` are what a stop_task produced; `completed` is the
    // ordinary ending. All of them end it.
    running.saw(&task("a", "killed"));
    assert!(running.idle());
}

#[test]
fn stdin_closes_only_after_a_result_with_nothing_running() {
    let mut running = Running::default();
    assert!(!may_close(false, &running));
    running.saw(&task("a", "started"));
    // A result with a background task still going: closing now could cut it off
    // before the wake-up pass it would cause.
    assert!(!may_close(true, &running));
    running.saw(&task("a", "completed"));
    assert!(may_close(true, &running));
}

#[test]
fn a_control_with_no_turn_behind_it_refuses_quietly() {
    let control = Control::new();
    assert!(!control.stop_task("a"));
}

/// The resident is listening again at its `result`; only a process that ends
/// with the turn has to wait out its background tasks.
#[test]
fn a_staying_turn_is_over_at_its_result_even_with_a_task_running() {
    let mut running = Running::default();
    running.saw(&task("a", "started"));
    assert!(turn_over(true, true, &running));
    assert!(!turn_over(false, true, &running));
    assert!(!turn_over(true, false, &running));
}

/// Recorded from 2.1.318 with `--permission-prompt-tool stdio`: a tool the
/// mode does not allow and no hook decided.
const ASKED: &str = r#"{"type":"control_request","request_id":"37497bcb","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"python3 -c \"print(6*7)\""},"decision_reason":"This command requires approval","tool_use_id":"toolu_01"}}"#;

const CANCELLED: &str = r#"{"type":"control_cancel_request","request_id":"37497bcb","request":{"subtype":"can_use_tool"}}"#;

#[test]
fn a_cancel_for_a_pending_prompt_reaches_the_asker() {
    let control = Control::new();
    let cancelled = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = cancelled.clone();
    control.asks_with(std::sync::Arc::new(|_, _| true));
    control.on_cancel(std::sync::Arc::new(move |id| {
        seen.lock().expect("cancelled").push(id.to_owned());
    }));

    assert!(control.prompted(ASKED, None));
    assert!(control.prompted(CANCELLED, None));
    assert_eq!(*cancelled.lock().expect("cancelled"), ["37497bcb"]);
}

/// Refused as a printing turn without a prompt tool refuses it, in the
/// CLI's own words, so the agent hears what it heard before.
#[test]
fn a_permission_prompt_nobody_was_asked_is_refused_in_the_clis_words() {
    let reply: serde_json::Value =
        serde_json::from_str(&refusal(ASKED).expect("a reply")).expect("json");
    assert_eq!(reply["type"], "control_response");
    assert_eq!(reply["response"]["request_id"], "37497bcb");
    assert_eq!(reply["response"]["response"]["behavior"], "deny");
    assert_eq!(
        reply["response"]["response"]["message"],
        "This command requires approval"
    );
}

#[test]
fn only_a_permission_prompt_is_answered() {
    let ours = r#"{"type":"control_response","response":{"subtype":"success","request_id":"interrupt-1"}}"#;
    assert!(refusal(ours).is_none());
    assert!(refusal(&interrupt_request()).is_none());
    let not_asked = r#"{"type":"control_cancel_request","request_id":"r1","request":{"subtype":"can_use_tool"}}"#;
    assert!(refusal(not_asked).is_none());
    assert!(refusal(
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"can_use_tool"}]}}"#
    )
    .is_none());
}
