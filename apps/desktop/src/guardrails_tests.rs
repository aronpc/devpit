use std::path::Path;

use devpit_agentcli::head::{head_path, write_head};
use devpit_rpc::{Guardrail, GuardrailMode};
use serde_json::json;

use super::{full_access, reported, rows, set, waits};
use crate::stop_asked::guardrail;

/// A conversation of the orchestrator's in `sessions`, its last turn in `mode`.
fn chat(sessions: &Path, id: &str, mode: &str) {
    let head = crate::chat_turn::opening(None, "p", None, None, Some(mode.to_owned()), None, 0.0);
    write_head(&head_path(sessions, id), &head).expect("head");
    std::fs::write(sessions.join(format!("{id}.jsonl")), "").expect("transcript");
}

#[test]
fn settings_list_each_guardrail_and_untouched_they_ask_as_today_outside_full_access() {
    let untouched = json!({});
    let listed: Vec<(Guardrail, GuardrailMode)> = rows(&untouched)
        .rows
        .into_iter()
        .map(|row| (row.guardrail, row.mode))
        .collect();
    assert_eq!(
        listed,
        [
            (Guardrail::Stop, GuardrailMode::Follow),
            (Guardrail::ForceStop, GuardrailMode::Always),
            (Guardrail::SendDraft, GuardrailMode::Follow),
        ]
    );
    let dir = tempfile::tempdir().expect("tempdir");
    chat(dir.path(), "conv_1", "acceptEdits");
    for (one, _) in listed {
        assert!(waits(&untouched, dir.path(), one), "{one:?} stopped asking");
    }
}

#[test]
fn a_mode_set_is_kept_beside_the_others() {
    let mut settings = json!({ "assistant": true });
    set(&mut settings, Guardrail::SendDraft, GuardrailMode::Never);
    set(&mut settings, Guardrail::Stop, GuardrailMode::Always);
    let modes: Vec<GuardrailMode> = rows(&settings).rows.iter().map(|row| row.mode).collect();
    assert_eq!(
        modes,
        [
            GuardrailMode::Always,
            GuardrailMode::Always,
            GuardrailMode::Never
        ]
    );
    assert_eq!(settings["assistant"], json!(true));
}

#[test]
fn in_full_access_a_stop_on_follow_goes_ahead_and_is_reported_in_the_answer_and_the_log() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions).expect("sessions");
    chat(&sessions, "conv_1", "bypassPermissions");
    // No words from the person and no click: only the orchestrator's call.
    let stop = guardrail(false, None);
    assert_eq!(stop, Guardrail::Stop);
    assert!(!waits(&json!({}), &sessions, stop));

    let folder = dir.path().join("orchestrator");
    let answer = reported(
        &folder,
        stop,
        "api-worker was stopped; it lost nothing",
        json!({ "stopped": "api-worker was stopped" }),
    );
    let line = answer["unasked"].as_str().expect("the line the chat shows");
    assert!(
        line.contains("api-worker was stopped; it lost nothing"),
        "{line}"
    );
    assert!(line.contains("Stop a session"), "{line}");
    assert!(answer["note"]
        .as_str()
        .is_some_and(|note| note.contains("one line")));
    let log = crate::orchestrator_notes::log_file(&folder, "sessions").expect("log");
    let kept = std::fs::read_to_string(log).expect("the sessions log");
    assert!(
        kept.contains("api-worker was stopped; it lost nothing"),
        "{kept}"
    );
}

#[test]
fn a_force_stop_or_one_that_loses_work_still_waits_for_the_click_in_full_access() {
    let dir = tempfile::tempdir().expect("tempdir");
    chat(dir.path(), "conv_1", "bypassPermissions");
    let forced = guardrail(true, None);
    let losing = guardrail(false, Some("its folder has 2 file(s) not committed"));
    assert_eq!(
        (forced, losing),
        (Guardrail::ForceStop, Guardrail::ForceStop)
    );
    assert!(waits(&json!({}), dir.path(), forced));
    // Changed by the person, it goes too.
    let mut settings = json!({});
    set(&mut settings, Guardrail::ForceStop, GuardrailMode::Follow);
    assert!(!waits(&settings, dir.path(), losing));
}

#[test]
fn sending_a_draft_follows_its_own_guardrail() {
    let dir = tempfile::tempdir().expect("tempdir");
    chat(dir.path(), "conv_1", "bypassPermissions");
    let mut settings = json!({});
    assert!(!waits(&settings, dir.path(), Guardrail::SendDraft));
    set(&mut settings, Guardrail::SendDraft, GuardrailMode::Always);
    assert!(waits(&settings, dir.path(), Guardrail::SendDraft));
    // Its own: a stop asking does not make the draft ask, nor the other way.
    set(&mut settings, Guardrail::SendDraft, GuardrailMode::Follow);
    set(&mut settings, Guardrail::Stop, GuardrailMode::Always);
    assert!(!waits(&settings, dir.path(), Guardrail::SendDraft));

    let outside = tempfile::tempdir().expect("tempdir");
    chat(outside.path(), "conv_1", "acceptEdits");
    assert!(waits(&json!({}), outside.path(), Guardrail::SendDraft));
    set(&mut settings, Guardrail::SendDraft, GuardrailMode::Never);
    assert!(!waits(&settings, outside.path(), Guardrail::SendDraft));
}

#[test]
fn the_chat_is_in_full_access_only_when_its_newest_conversation_is() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(!full_access(dir.path()));
    chat(dir.path(), "conv_1", "bypassPermissions");
    assert!(full_access(dir.path()));
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    chat(dir.path(), "conv_2", "acceptEdits");
    std::fs::File::options()
        .write(true)
        .open(dir.path().join("conv_2.jsonl"))
        .and_then(|file| file.set_modified(later))
        .expect("touch");
    assert!(!full_access(dir.path()));
}

/// Follow chat permission, Always ask and Never ask decide the same on every
/// CLI an orchestrator runs on: the chat's mode is devpit's word for it,
/// kept in the conversation's head whatever CLI holds the chat.
#[test]
fn every_guardrail_mode_decides_the_same_on_every_provider() {
    for profile in ["claude", "codex", "opencode"] {
        for (mode, full, waits_on) in [
            (GuardrailMode::Follow, false, true),
            (GuardrailMode::Follow, true, false),
            (GuardrailMode::Always, true, true),
            (GuardrailMode::Never, false, false),
        ] {
            let dir = tempfile::tempdir().expect("tempdir");
            let permission = if full {
                "bypassPermissions"
            } else {
                "acceptEdits"
            };
            let head = crate::chat_turn::opening(
                None,
                profile,
                None,
                None,
                Some(permission.to_owned()),
                None,
                0.0,
            );
            write_head(&head_path(dir.path(), "conv_1"), &head).expect("head");
            std::fs::write(dir.path().join("conv_1.jsonl"), "").expect("transcript");
            let mut settings = json!({});
            for one in [Guardrail::Stop, Guardrail::ForceStop, Guardrail::SendDraft] {
                set(&mut settings, one, mode);
                assert_eq!(
                    waits(&settings, dir.path(), one),
                    waits_on,
                    "{profile}: {one:?} on {mode:?} in {permission}"
                );
            }
        }
    }
}
