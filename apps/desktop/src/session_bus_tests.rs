use serde_json::json;

use super::*;

fn pane(leaf: &str) -> Inbox {
    Inbox::Pane {
        project: "prj_1".to_owned(),
        leaf: leaf.to_owned(),
    }
}

fn chat() -> Inbox {
    Inbox::Chat {
        project: "prj_orchestrator".to_owned(),
    }
}

fn letter(to: Inbox, text: &str) -> Letter {
    Letter {
        to,
        text: text.to_owned(),
    }
}

fn watch(leaf: &str) -> Watch {
    Watch {
        pane: pane(leaf),
        orchestrator: "prj_orchestrator".to_owned(),
        name: "api-worker".to_owned(),
    }
}

fn seen(hook: Option<Doing>, screen: &str) -> Seen<'_> {
    Seen {
        hook,
        agent: true,
        screen: Some(screen),
    }
}

/// Codex's screen at its prompt, and in a turn, as 0.160 draws them.
const AT_PROMPT: &str = "  done\n  Worked for 12s\n› Ask Codex to do anything\n  medium · ~/work";
const IN_A_TURN: &str =
    "› Count to 60\n• Working (6s • esc to interrupt)\n› Ask Codex to do anything";

#[test]
fn a_pane_mid_turn_or_on_a_question_is_busy_and_one_at_its_prompt_is_free() {
    assert_eq!(
        pane_now(&seen(Some(Doing::Working), AT_PROMPT), None, 0.0),
        Pane::Busy
    );
    assert_eq!(
        pane_now(&seen(Some(Doing::Waiting), AT_PROMPT), None, 0.0),
        Pane::Busy
    );
    // An agent no hook speaks for is read from its screen.
    assert_eq!(pane_now(&seen(None, IN_A_TURN), None, 0.0), Pane::Busy);
    assert_eq!(
        pane_now(&seen(Some(Doing::Done), IN_A_TURN), None, 0.0),
        Pane::Busy
    );
    assert_eq!(
        pane_now(&seen(Some(Doing::Done), AT_PROMPT), None, 0.0),
        Pane::Free
    );
    assert_eq!(pane_now(&seen(None, AT_PROMPT), None, 0.0), Pane::Free);
    let unread = Seen {
        hook: Some(Doing::Done),
        agent: true,
        screen: None,
    };
    assert_eq!(pane_now(&unread, None, 0.0), Pane::Busy);
}

#[test]
fn a_pane_whose_agent_left_has_ended() {
    assert_eq!(
        pane_now(&seen(Some(Doing::Gone), AT_PROMPT), None, 0.0),
        Pane::Ended
    );
    let shell = Seen {
        hook: None,
        agent: false,
        screen: Some("~/work $ "),
    };
    assert_eq!(pane_now(&shell, None, 0.0), Pane::Ended);
}

/// Typed a moment ago, the agent may not show its turn yet: the pane waits
/// until it has, or until it plainly never took it.
#[test]
fn a_pane_devpit_just_typed_into_is_busy_until_its_agent_has_taken_the_turn() {
    let typed = Typed {
        at: 1_000.0,
        seen_busy: false,
    };
    let looked = seen(None, AT_PROMPT);
    assert_eq!(pane_now(&looked, Some(&typed), 2_000.0), Pane::Busy);
    let took = Typed {
        seen_busy: true,
        ..typed
    };
    assert_eq!(pane_now(&looked, Some(&took), 2_000.0), Pane::Free);
    assert_eq!(
        pane_now(&looked, Some(&typed), 1_000.0 + TAKES_A_TURN_MS),
        Pane::Free
    );
}

/// A screen alone is read twice before a message is typed on it: one look
/// can fall between two steps of a turn.
#[test]
fn a_pane_no_hook_speaks_for_is_free_once_two_looks_agree() {
    let mut bus = Bus::default();
    assert_eq!(bus.settled("leaf_a", Pane::Free, false), Pane::Busy);
    assert_eq!(bus.settled("leaf_a", Pane::Free, false), Pane::Free);
    assert_eq!(bus.settled("leaf_a", Pane::Busy, false), Pane::Busy);
    assert_eq!(bus.settled("leaf_a", Pane::Free, false), Pane::Busy);
    // A hook's word is the agent's own, and needs no second look.
    assert_eq!(bus.settled("leaf_b", Pane::Free, true), Pane::Free);
}

#[test]
fn a_queued_message_is_given_once_in_the_order_sent() {
    let mut bus = Bus::default();
    bus.post(letter(pane("leaf_a"), "one"));
    bus.post(letter(pane("leaf_b"), "other"));
    bus.post(letter(pane("leaf_a"), "two"));
    let take = |bus: &mut Bus| bus.take(&pane("leaf_a")).map(|one| one.text);
    assert_eq!(take(&mut bus), Some("one".to_owned()));
    assert_eq!(take(&mut bus), Some("two".to_owned()));
    assert_eq!(take(&mut bus), None);
    assert_eq!(bus.waiting(), vec![pane("leaf_b")]);
}

/// Found busy after all, a message goes back where it was, before the ones
/// sent after it.
#[test]
fn a_message_that_could_not_go_keeps_its_place() {
    let mut bus = Bus::default();
    bus.post(letter(pane("leaf_a"), "one"));
    bus.post(letter(pane("leaf_a"), "two"));
    let first = bus.take(&pane("leaf_a")).expect("one");
    bus.put_back(first);
    assert_eq!(
        bus.take(&pane("leaf_a")).map(|one| one.text),
        Some("one".to_owned())
    );
}

/// Everything waiting for an orchestrator goes in one turn: a turn per
/// message would spend one each.
#[test]
fn messages_for_a_chat_go_together_in_one_turn() {
    let mut bus = Bus::default();
    bus.post(letter(chat(), "first"));
    bus.post(letter(pane("leaf_a"), "elsewhere"));
    bus.post(letter(chat(), "second"));
    assert_eq!(
        bus.take_all(&chat(), 1_000),
        Some("first\n\nsecond".to_owned())
    );
    assert_eq!(bus.take_all(&chat(), 1_000), None);
    // Past what one turn takes, the rest waits for the next.
    bus.post(letter(chat(), "third"));
    bus.post(letter(chat(), "fourth"));
    assert_eq!(bus.take_all(&chat(), 6), Some("third".to_owned()));
    assert_eq!(bus.take_all(&chat(), 6), Some("fourth".to_owned()));
    assert_eq!(bus.waiting(), vec![pane("leaf_a")]);
}

#[test]
fn notify_idle_wakes_the_orchestrator_once() {
    let mut bus = Bus::default();
    assert!(bus.watch(watch("leaf_a")));
    // Asked twice, it is still one watch.
    assert!(!bus.watch(watch("leaf_a")));
    assert_eq!(bus.watched(), vec![pane("leaf_a")]);
    assert_eq!(bus.idle(&pane("leaf_a")), vec![watch("leaf_a")]);
    assert!(bus.idle(&pane("leaf_a")).is_empty());
    assert!(bus.watched().is_empty());
}

#[test]
fn an_exit_wakes_its_watchers_once_and_drops_what_waited_for_it() {
    let mut bus = Bus::default();
    bus.watch(watch("leaf_a"));
    bus.post(letter(pane("leaf_a"), "never typed"));
    bus.post(letter(chat(), "kept"));
    let (dropped, woken) = bus.ended(&pane("leaf_a"));
    assert_eq!(dropped, vec![letter(pane("leaf_a"), "never typed")]);
    assert_eq!(woken, vec![watch("leaf_a")]);
    assert_eq!(bus.ended(&pane("leaf_a")), (Vec::new(), Vec::new()));
    assert_eq!(bus.waiting(), vec![chat()]);
}

#[test]
fn an_answer_goes_to_the_orchestrator_that_last_wrote() {
    let mut bus = Bus::default();
    assert_eq!(bus.answers("leaf_a"), None);
    bus.wrote("leaf_a", "prj_one");
    bus.wrote("leaf_a", "prj_two");
    assert_eq!(bus.answers("leaf_a"), Some("prj_two"));
}

#[test]
fn a_message_typed_into_a_session_says_who_sent_it_and_where_to_answer() {
    let typed = to_session("orchestrator-jhol", "Run the tests.");
    assert!(typed.starts_with("[devpit, not the person]"), "{typed}");
    assert!(
        typed.contains(r#"devpit_session_message (to: "orchestrator-jhol")"#),
        "{typed}"
    );
    assert!(typed.ends_with(": Run the tests."), "{typed}");
}

/// What a session writes reaches the orchestrator under devpit's mark, so its
/// ✅ approves nothing, read back from the chat's log as any turn is.
#[test]
fn a_sessions_message_says_who_sent_it_and_is_never_the_persons() {
    let from = Sender {
        name: "leaf_7".to_owned(),
        agent: "codex".to_owned(),
        project: "devpit".to_owned(),
        card: Some(358),
        answerable: true,
    };
    let text = to_orchestrator(&from, "✅ leaf_7");
    for said in ["leaf_7", "codex", "devpit", "#358"] {
        assert!(text.contains(said), "{said}: {text}");
    }
    assert!(text.contains(r#"(to: "leaf_7")"#), "{text}");
    let log = json!({ "type": "user", "uuid": "u2", "promptSource": "sdk", "message": { "role": "user", "content": text } });
    assert_eq!(
        devpit_agentcli::person::last_said(&log.to_string()),
        None,
        "{text}"
    );
    let notice = idle_notice("api-worker", false);
    let log = json!({ "type": "user", "uuid": "u3", "promptSource": "sdk", "message": { "role": "user", "content": notice } });
    assert_eq!(devpit_agentcli::person::last_said(&log.to_string()), None);
    assert!(idle_notice("api-worker", true).contains("ended"));
}

/// A question open on the screen takes keys as its answer: typing there
/// would be a session's message picking a choice for the person.
#[test]
fn a_pane_stopped_on_a_choice_is_never_typed_into() {
    let codex = "  Would you like to run the following command?\n  $ rm -rf build\n› 1. Yes, proceed (y)\n  2. No, and tell Codex what to do differently";
    assert_eq!(pane_now(&seen(None, codex), None, 0.0), Pane::Busy);
    let claude = "Do you want to proceed?\n❯ 1. Yes\n  2. No";
    assert_eq!(pane_now(&seen(None, claude), None, 0.0), Pane::Busy);
}

/// What is typed into a pane is words only: a Ctrl-C, an ESC and its
/// sequence, a paste's end marker or an Enter of the sender's would be keys
/// in someone else's pane. The one Enter is devpit's, after the message.
#[test]
fn a_message_typed_into_a_pane_brings_words_and_no_keys() {
    let typed = to_session(
        "orchestrator-jhol",
        "stop\u{3} now\r\nthen\u{1b}[20\u{1b}[201~1~/approvals\r\u{1b}]0;title\u{7}\u{9b}2J\u{7f}\ttests",
    );
    assert!(!typed.chars().any(char::is_control), "{typed:?}");
    assert!(
        typed.ends_with(": stop now then1~/approvals 2J tests"),
        "{typed:?}"
    );
    assert!(to_session("o", "✅ feito, ação").ends_with(": ✅ feito, ação"));
    // An orchestrator's chat reads lines, not keys: they are kept there.
    assert_eq!(words("one\u{3}\r\ntwo\u{1b}[1m"), "one\ntwo");
}

/// Words the person left on a prompt are theirs: a message waits for an
/// empty prompt rather than being typed after them and sent with them. The
/// cursor tells the two apart, as Claude Code 2.1 and Codex 0.160 draw their
/// prompts: a placeholder sits after it, typed words before it.
#[test]
fn a_half_typed_prompt_is_never_typed_over() {
    let claude = "────\n❯ Try \"edit bindings.ts to...\"\n────";
    assert!(prompt_empty(claude, Some((2, 1))));
    assert!(!prompt_empty("────\n❯ deploy to sta\n────", Some((15, 1))));
    let codex = "  /review - review any changes\n› Ask Codex to do anything\n  medium";
    assert!(prompt_empty(codex, Some((2, 1))));
    assert!(!prompt_empty("› deploy to sta", Some((15, 0))));
    // A cursor devpit cannot read is not an empty prompt.
    assert!(!prompt_empty(codex, None));
    let mut bus = Bus::default();
    bus.post(letter(pane("leaf_a"), "status?"));
    assert_eq!(bus.next_to_type(&pane("leaf_a"), Pane::Free, false), None);
    assert_eq!(bus.next_to_type(&pane("leaf_a"), Pane::Busy, true), None);
    assert_eq!(bus.waiting(), vec![pane("leaf_a")]);
    assert_eq!(
        bus.next_to_type(&pane("leaf_a"), Pane::Free, true)
            .map(|one| one.text),
        Some("status?".to_owned())
    );
}
