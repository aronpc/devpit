//! What `tools/list` shows a session: only the tools it can use.
//!
//! An orchestrator's tools, and the `project` that reaches another board, are
//! refused anywhere else; Decisions' tools are refused without a key. Listed
//! to every session they cost each turn about two thousand tokens and invited
//! calls that could only be refused. They stay callable by name, so nothing
//! that worked stops working: they are only no longer offered.

use serde_json::{json, Value};

use crate::mcp::Tool;

/// The tools only devpit's orchestrator may call.
pub(crate) const ORCHESTRATOR_ONLY: [&str; 15] = [
    "devpit_projects",
    "devpit_sessions",
    "devpit_session_screen",
    "devpit_stop_session",
    "devpit_resume_session",
    "devpit_session_transcript",
    "devpit_propose_project",
    "devpit_draft_reply",
    "devpit_send_draft",
    "devpit_log",
    "devpit_start_session",
    "devpit_session_notify_idle",
    "devpit_project_agent_ask",
    "devpit_project_agent_brief",
    // Unreachable when the door is down, and only a latency when it is up.
    "devpit_mcp_health",
];

/// The tools that answer only once the person kept a Decisions key.
pub(crate) const DECISIONS: [&str; 2] = ["devpit_decide", "devpit_rubric_run"];

/// The tools that change nothing, which a host may run without asking.
pub(crate) const READ_ONLY: [&str; 15] = [
    "devpit_context",
    "devpit_board",
    "devpit_card",
    "devpit_artifacts",
    "devpit_projects",
    "devpit_sessions",
    "devpit_session_screen",
    "devpit_session_transcript",
    "devpit_repo_state",
    "devpit_reminders",
    "devpit_devflow_status",
    "devpit_devflow_skill",
    "devpit_schedules",
    "devpit_heartbeat_status",
    "devpit_briefing",
];

/// The tools whose change cannot be taken back: work in flight, a file.
pub(crate) const DESTRUCTIVE: [&str; 3] = [
    "devpit_stop_session",
    "devpit_artifact_remove",
    "devpit_comment_delete",
];

/// Who the session is, as the app's answer to `devflow_offer` says. `None`
/// is an app too old to say, which gets the whole list it always had.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Seat {
    pub orchestrator: Option<bool>,
    pub decisions: Option<bool>,
    /// An orchestrator whose Assistant mode is on.
    pub assistant: Option<bool>,
}

impl Seat {
    pub(crate) fn of(answer: &Value) -> Seat {
        Seat {
            orchestrator: answer.get("orchestrator").and_then(Value::as_bool),
            decisions: answer.get("decisions").and_then(Value::as_bool),
            assistant: answer.get("assistant").and_then(Value::as_bool),
        }
    }

    fn orchestrates(&self) -> bool {
        self.orchestrator != Some(false)
    }

    /// Whether `tool` is offered to this session.
    pub(crate) fn offers(&self, tool: &Tool) -> bool {
        (self.orchestrates() || !ORCHESTRATOR_ONLY.contains(&tool.name))
            && (self.decisions != Some(false) || !DECISIONS.contains(&tool.name))
    }
}

/// `tool` as `tools/list` shows it to `seat`, or nothing when it is not
/// offered there.
pub(crate) fn listed(tool: &Tool, seat: &Seat) -> Option<Value> {
    if !seat.offers(tool) {
        return None;
    }
    let mut input = (tool.input)();
    if !seat.orchestrates() {
        if let Some(properties) = input.get_mut("properties").and_then(Value::as_object_mut) {
            properties.remove("project");
        }
    }
    crate::lean::offered(tool.name, &mut input);
    let mut listed = json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": input,
    });
    if READ_ONLY.contains(&tool.name) {
        listed["annotations"] = json!({ "readOnlyHint": true });
    } else if DESTRUCTIVE.contains(&tool.name) {
        listed["annotations"] = json!({ "destructiveHint": true });
    }
    if let Some(meta) = crate::apps::meta_for(tool.name) {
        listed["_meta"] = meta;
    }
    Some(listed)
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
