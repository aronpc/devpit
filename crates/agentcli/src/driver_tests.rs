use devpit_rpc::{CallState, Part};

use crate::driver::{driver, Claude, Driver, Read};

#[test]
fn a_line_it_cannot_parse_is_kept_as_text() {
    // Losing output is worse than showing it plain: a CLI prints its errors
    // in exactly the lines a strict parser would drop.
    let read = Claude::default().read("panic: something went wrong");
    assert_eq!(
        read,
        Read::Parts(vec![Part::Unknown {
            text: "panic: something went wrong".to_owned()
        }])
    );
}

#[test]
fn a_blank_line_is_nothing() {
    assert_eq!(Claude::default().read("   "), Read::Nothing);
}

#[test]
fn assistant_text_becomes_a_text_part() {
    let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#;
    assert_eq!(
        Claude::default().read(line),
        Read::Parts(vec![Part::Text {
            text: "hi".to_owned(),
            parent: None,
        }])
    );
}

#[test]
fn thinking_is_not_the_answer() {
    let line =
        r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"}]}}"#;
    assert_eq!(
        Claude::default().read(line),
        Read::Parts(vec![Part::Thinking {
            text: "hmm".to_owned(),
            parent: None,
            duration_ms: None,
        }])
    );
}

#[test]
fn a_tool_call_arrives_running() {
    let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"c1","name":"Bash","input":{"cmd":"ls"}}]}}"#;
    let Read::Parts(parts) = Claude::default().read(line) else {
        panic!("expected parts");
    };
    assert!(matches!(
        &parts[0],
        Part::ToolCall { id, name, state, .. }
            if id == "c1" && name == "Bash" && *state == CallState::Running
    ));
}

#[test]
fn a_tool_result_points_at_its_call() {
    let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"c1","content":"ok"}]}}"#;
    assert_eq!(
        Claude::default().read(line),
        Read::Parts(vec![Part::ToolResult {
            call_id: "c1".to_owned(),
            output: "ok".to_owned(),
            is_error: false,
            parent: None,
        }])
    );
}

#[test]
fn the_end_carries_the_clis_own_reason() {
    let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.42,"is_error":false}"#;
    assert_eq!(
        Claude::default().read(line),
        Read::Ended {
            stop_reason: Some("success".to_owned()),
            cost_usd: Some(0.42),
            is_error: false,
            context: None,
        }
    );
}

/// Recorded from Claude Code 2.1.270, trimmed: the last request's tokens
/// against the main model's window — not the summed `usage`, which counts a
/// cached prompt once per request a tool turn made.
#[test]
fn the_end_says_how_full_the_context_is() {
    let line = r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.03,
        "usage":{"input_tokens":20,"cache_read_input_tokens":27000,"output_tokens":9,
          "iterations":[
            {"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":13868,"cache_creation_input_tokens":0},
            {"input_tokens":10,"output_tokens":4,"cache_read_input_tokens":13868,"cache_creation_input_tokens":14167}]},
        "modelUsage":{"claude-haiku-4-5-20251001":{"contextWindow":200000,"maxOutputTokens":32000}}}"#;
    let Read::Ended { context, .. } = Claude::default().read(line) else {
        panic!("an end");
    };
    assert_eq!(
        context,
        Some(devpit_rpc::Context {
            used: 10 + 13868 + 14167 + 4,
            window: 200_000,
        })
    );
}

#[test]
fn a_driver_that_is_not_installed_is_absent_rather_than_swapped() {
    // Falling back to another provider would change what the conversation is.
    assert!(driver("claude").is_some());
    assert!(driver("gemini").is_none());
}

#[test]
fn claude_offers_the_accounts_default_first_and_aliases_not_dated_ids() {
    // A dated id is right on one provider; the alias is right on all of them.
    let models = Claude::default().models();
    assert_eq!(models.first(), Some(&"default"));
    assert!(models.contains(&"fable"));
    assert!(models.iter().all(|model| !model.starts_with("claude-")));
}

/// A real turn of Claude Code 2.1.270, sanitized: a subagent, an edit, a
/// checklist and a backgrounded command. The shapes asserted here were read off
/// this recording, not the documentation.
mod recorded {
    use super::*;

    const TURN: &str =
        include_str!("../tests/fixtures/claude-2.1.270-subagent-edit-tasks-background.jsonl");

    fn reads() -> Vec<Read> {
        TURN.lines()
            .map(|line| Claude::default().read(line))
            .collect()
    }

    fn parts() -> Vec<Part> {
        reads()
            .into_iter()
            .flat_map(|read| match read {
                Read::Parts(parts) => parts,
                _ => Vec::new(),
            })
            .collect()
    }

    fn agent_call() -> String {
        parts()
            .into_iter()
            .find_map(|part| match part {
                Part::ToolCall { id, name, .. } if name == "Agent" => Some(id),
                _ => None,
            })
            .expect("the recording starts a subagent")
    }

    #[test]
    fn a_subagents_work_points_at_the_call_that_started_it() {
        let agent = agent_call();
        let inside: Vec<Part> = parts()
            .into_iter()
            .filter(|part| match part {
                Part::Text { parent, .. }
                | Part::Thinking { parent, .. }
                | Part::ToolCall { parent, .. }
                | Part::ToolResult { parent, .. } => parent.as_deref() == Some(agent.as_str()),
                _ => false,
            })
            .collect();
        // The subagent read notes.txt: its call and its result are both marked.
        assert!(inside
            .iter()
            .any(|part| matches!(part, Part::ToolCall { name, .. } if name == "Read")));
        assert!(inside
            .iter()
            .any(|part| matches!(part, Part::ToolResult { .. })));
    }

    #[test]
    fn the_agents_own_work_has_no_parent() {
        assert!(parts().iter().any(|part| matches!(
            part,
            Part::ToolCall { name, parent: None, .. } if name == "Edit"
        )));
    }

    #[test]
    fn both_background_tasks_start_and_finish() {
        let tasks: Vec<(String, String, Option<String>)> = parts()
            .into_iter()
            .filter_map(|part| match part {
                Part::Task {
                    task_id,
                    status,
                    task_kind,
                    ..
                } => Some((task_id, status, task_kind)),
                _ => None,
            })
            .collect();
        for kind in ["local_agent", "local_bash"] {
            let started = tasks
                .iter()
                .find(|(_, status, task_kind)| {
                    status == "started" && task_kind.as_deref() == Some(kind)
                })
                .unwrap_or_else(|| panic!("no {kind} task started"));
            assert!(
                tasks
                    .iter()
                    .any(|(id, status, _)| id == &started.0 && status == "completed"),
                "{kind} task never completed"
            );
        }
    }

    /// The subagent's timer is the CLI's own count, so a transcript read
    /// later still says how long it ran.
    #[test]
    fn a_subagent_says_how_long_it_ran() {
        let ran = parts().into_iter().find_map(|part| match part {
            Part::Task {
                task_kind,
                status,
                duration_ms,
                ..
            } if task_kind.is_none() && status == "completed" => duration_ms,
            _ => None,
        });
        assert_eq!(ran, Some(22611));
    }

    #[test]
    fn the_cli_describes_its_own_commands() {
        let init = reads()
            .into_iter()
            .find_map(|read| match read {
                Read::Init(init) => Some(init),
                _ => None,
            })
            .expect("an init line");
        assert_eq!(init.slash_commands, ["compact", "clear", "review", "tdd"]);
        assert_eq!(init.terminal_slash_commands, ["statusline"]);
        assert!(init.model.is_some());
    }

    #[test]
    fn hook_chatter_and_token_estimates_are_not_parts() {
        // Dropped from the fixture already, so asserted on hand-written lines.
        for line in [
            r#"{"type":"system","subtype":"hook_started","hook_id":"h"}"#,
            r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":3}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{}}"#,
        ] {
            assert_eq!(Claude::default().read(line), Read::Nothing, "{line}");
        }
    }
}

/// A transcript line written before parts carried a parent still reads.
#[test]
fn a_part_saved_before_parents_existed_still_loads() {
    let old = r#"{"kind":"tool_call","id":"c1","name":"Bash","input":"{}","state":"ok"}"#;
    let part: Part = serde_json::from_str(old).expect("old shape");
    assert!(matches!(part, Part::ToolCall { parent: None, .. }));
}

/// What `/compact` or `/clear` answered, printed by the CLI as a system line.
#[test]
fn a_slash_commands_own_answer_is_kept() {
    let line = r#"{"type":"system","subtype":"local_command","content":"Compacted. ctrl+o to see full summary","level":"info"}"#;
    assert_eq!(
        Claude::default().read(line),
        Read::Parts(vec![Part::Command {
            content: "Compacted. ctrl+o to see full summary".to_owned()
        }])
    );
}

/// A rewind forks at the agent's own message, never at a subagent's: those
/// live in the subagent's transcript, where `--resume-session-at` cannot find them.
#[test]
fn a_rewind_forks_at_the_agents_own_message() {
    let own = r#"{"type":"assistant","uuid":"aab2","parent_tool_use_id":null,"session_id":"s","message":{"content":[]}}"#;
    assert_eq!(Claude::default().anchor(own).as_deref(), Some("aab2"));
    let subagent = own.replace("null", "\"toolu_1\"");
    assert_eq!(Claude::default().anchor(&subagent), None);
    let user = r#"{"type":"user","uuid":"u1","session_id":"s"}"#;
    assert_eq!(Claude::default().anchor(user), None);
}

/// Text streams in deltas, and the whole block the `assistant` line repeats
/// is not drawn twice; a message that did not stream keeps its text.
#[test]
fn streamed_text_arrives_in_pieces_and_once() {
    let claude = Claude::default();
    let start = r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","content":[]}},"parent_tool_use_id":null}"#;
    let delta = |text: &str| {
        format!(
            r#"{{"type":"stream_event","event":{{"type":"content_block_delta","index":0,"delta":{{"type":"text_delta","text":"{text}"}}}},"parent_tool_use_id":null}}"#
        )
    };
    let whole = |id: &str| {
        format!(
            r#"{{"type":"assistant","message":{{"id":"{id}","content":[{{"type":"text","text":"FIRST DONE"}}]}},"parent_tool_use_id":null}}"#
        )
    };
    let text = |said: &str| {
        Read::Parts(vec![Part::Text {
            text: said.to_owned(),
            parent: None,
        }])
    };

    assert_eq!(claude.read(start), Read::Nothing);
    assert_eq!(claude.read(&delta("FIRST ")), text("FIRST "));
    assert_eq!(claude.read(&delta("DONE")), text("DONE"));
    assert_eq!(claude.read(&whole("msg_1")), Read::Parts(vec![]));
    // Another message, never streamed: its text is the only copy.
    assert_eq!(claude.read(&whole("msg_2")), text("FIRST DONE"));
}

/// A tool's input streams too, but is drawn whole from the `assistant` line.
#[test]
fn a_tool_input_delta_is_not_text() {
    let claude = Claude::default();
    let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"comma"}},"parent_tool_use_id":null}"#;
    assert_eq!(claude.read(line), Read::Nothing);
}

/// Thinking is said when it begins, and the empty block repeated at its end
/// says how long it took.
#[test]
fn thinking_is_heard_when_it_begins_and_timed_when_it_ends() {
    let claude = Claude::default();
    let start = r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_t","content":[]}},"parent_tool_use_id":null}"#;
    let begins = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}},"parent_tool_use_id":null}"#;
    let ends = r#"{"type":"assistant","message":{"id":"msg_t","content":[{"type":"thinking","thinking":""}]},"parent_tool_use_id":null}"#;
    assert_eq!(claude.read(start), Read::Nothing);
    assert_eq!(
        claude.read(begins),
        Read::Parts(vec![Part::Thinking {
            text: String::new(),
            parent: None,
            duration_ms: None,
        }])
    );
    assert!(matches!(
        claude.read(ends),
        Read::Parts(parts) if matches!(parts.as_slice(), [Part::Thinking { duration_ms: Some(_), .. }])
    ));
}
