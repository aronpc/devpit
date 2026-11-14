//! Talking to a turn while it runs.
//!
//! In stream-json input mode the CLI reads control requests on stdin for as
//! long as stdin is open — measured on Claude Code 2.1.270: a `stop_task` was
//! answered within a tenth of a second by the task ending as `killed`. So a
//! turn keeps its stdin, and this is the handle to it.
//!
//! When to close it is the decision here. Closed at the first `result` the
//! process exits a second later; closed while a background task still runs,
//! that task may be cut off before the wake-up pass it would have caused. So
//! it closes once both are true: a result has arrived and nothing is running.

use std::collections::BTreeSet;
use std::io::Write;
use std::process::ChildStdin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use devpit_rpc::Part;

use crate::wire::Wire;

/// A permission prompt the CLI put to its host on stdout.
pub struct Prompt {
    pub request_id: String,
    pub tool: String,
    pub input: serde_json::Value,
    /// The session it came from, once the stream named one.
    pub session_id: Option<String>,
}

/// Takes a prompt to answer later with [`Control::answer`], or leaves it to
/// be refused (false).
pub type Asker = Arc<dyn Fn(Prompt, &Control) -> bool + Send + Sync>;

/// A running turn's stdin, shared with whoever may need to steer it.
#[derive(Clone, Default)]
pub struct Control {
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    /// Steers written and not echoed back yet: the uuid sent, and devpit's id for it.
    awaiting: Arc<Mutex<Vec<(String, String)>>>,
    asker: Arc<Mutex<Option<Asker>>>,
    /// How this CLI is written to; Claude Code's stream-json until a turn says.
    wire: Arc<Mutex<Option<Arc<dyn Wire>>>>,
    /// A stop the wire took without a word to the CLI, until its turn ends.
    stopped_alone: Arc<AtomicBool>,
    /// The process, and how long it gets once its stdin closes, for a CLI
    /// that does not exit on its own.
    ends: Arc<Mutex<Option<(u32, std::time::Duration)>>>,
}

impl Control {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn attach(&self, stdin: ChildStdin) {
        if let Ok(mut held) = self.stdin.lock() {
            *held = Some(stdin);
        }
    }

    /// Takes the stdin of process `pid`, spoken to through `wire`, and writes
    /// the wire's handshake. False when the handshake could not be written.
    pub(crate) fn open(
        &self,
        stdin: ChildStdin,
        wire: Arc<dyn Wire>,
        turn: &crate::talk::Say<'_>,
        pid: u32,
    ) -> bool {
        let hello = wire.hello(turn);
        if let Ok(mut ends) = self.ends.lock() {
            *ends = wire.grace().map(|grace| (pid, grace));
        }
        if let Ok(mut held) = self.wire.lock() {
            *held = Some(wire);
        }
        self.attach(stdin);
        self.write_all(hello)
    }

    fn wire(&self) -> Arc<dyn Wire> {
        self.wire
            .lock()
            .ok()
            .and_then(|held| held.clone())
            .unwrap_or_else(|| Arc::new(crate::wire::Claude))
    }

    /// Writes each line in turn; with none, whether stdin is still held.
    fn write_all(&self, lines: Vec<String>) -> bool {
        if lines.is_empty() {
            return self.stdin.lock().is_ok_and(|held| held.is_some());
        }
        lines.iter().all(|line| self.write(line))
    }

    /// Writes one line; false when the turn has already let go of stdin.
    pub(crate) fn write(&self, line: &str) -> bool {
        let Ok(mut held) = self.stdin.lock() else {
            return false;
        };
        let Some(stdin) = held.as_mut() else {
            return false;
        };
        writeln!(stdin, "{line}")
            .and_then(|_| stdin.flush())
            .is_ok()
    }

    /// Hands the running turn one more message from the person.
    pub fn say(&self, prompt: &str) -> bool {
        self.write_all(self.wire().prompt(prompt))
    }

    /// Hands the running turn a message under `uuid`, kept as `id` until the
    /// CLI echoes it back: written is not taken in, which only the echo says.
    pub fn steer(&self, prompt: &str, uuid: &str, id: &str) -> bool {
        // Kept before it is written: the echo may come back before this returns.
        self.awaiting(|held| held.push((uuid.to_owned(), id.to_owned())));
        let sent = self
            .wire()
            .steer(prompt, uuid)
            .is_some_and(|line| self.write(&line));
        if !sent {
            self.heard(uuid);
        }
        sent
    }

    /// The CLI echoed the message sent under `uuid`.
    pub(crate) fn heard(&self, uuid: &str) {
        self.awaiting(|held| held.retain(|(sent, _)| sent != uuid));
    }

    /// The steers the CLI never echoed back, by their ids, taken once.
    pub fn unheard(&self) -> Vec<String> {
        let mut ids = Vec::new();
        self.awaiting(|held| ids = held.drain(..).map(|(_, id)| id).collect());
        ids
    }

    fn awaiting(&self, change: impl FnOnce(&mut Vec<(String, String)>)) {
        if let Ok(mut held) = self.awaiting.lock() {
            change(&mut held);
        }
    }

    /// Asks the CLI to stop one background task, by the id it gave it.
    pub fn stop_task(&self, task_id: &str) -> bool {
        self.wire()
            .stop_task(task_id)
            .is_some_and(|line| self.write(&line))
    }

    /// Asks the CLI to stop the turn it is on, and keep running: what stopping
    /// is for a process that stays to hear the next message.
    pub fn interrupt(&self) -> bool {
        let Some(lines) = self.wire().interrupt() else {
            return false;
        };
        let alone = lines.is_empty();
        let taken = self.write_all(lines);
        if taken && alone {
            self.stopped_alone.store(true, Ordering::SeqCst);
        }
        taken
    }

    /// Whether a stop the wire took alone still waits for its turn to end; taken once.
    pub(crate) fn stopped_alone(&self) -> bool {
        self.stopped_alone.swap(false, Ordering::SeqCst)
    }

    /// A switch under `id`, in the CLI's shape: written now when it is a
    /// line. `None` when it takes none, or no longer listens.
    pub(crate) fn request(
        &self,
        id: &str,
        request: serde_json::Value,
    ) -> Option<crate::wire::Switch> {
        match self.wire().control(id, request)? {
            crate::wire::Switch::Write(line) => self
                .write(&line)
                .then_some(crate::wire::Switch::Write(line)),
            kept => Some(kept),
        }
    }

    /// Writes what `line` calls for in reply, if anything.
    pub(crate) fn reply(&self, line: &str) {
        for reply in self.wire().replies(line) {
            self.write(&reply);
        }
    }

    /// The CLI's answer to a switch, when `line` is one.
    pub(crate) fn control_answer(&self, line: &str) -> Option<serde_json::Value> {
        self.wire().control_answer(line)
    }

    /// Who answers the permission prompts this turn would otherwise refuse.
    pub fn asks_with(&self, asker: Asker) {
        if let Ok(mut held) = self.asker.lock() {
            *held = Some(asker);
        }
    }

    /// Answers the prompt sent under `request_id` — `{behavior: "allow",
    /// updatedInput}` or `{behavior: "deny", message}` — in the CLI's own shape.
    pub fn answer(&self, request_id: &str, response: serde_json::Value) -> bool {
        self.wire()
            .answer(request_id, response)
            .is_some_and(|line| self.write(&line))
    }

    /// A permission prompt on `line`, handed to the asker or refused. False
    /// when the line is not one.
    pub(crate) fn prompted(&self, line: &str, session_id: Option<&str>) -> bool {
        let Some(asked) = self.wire().asked(line, session_id) else {
            return false;
        };
        let asker = self.asker.lock().ok().and_then(|held| held.clone());
        let taken = asker
            .zip(asked.prompt)
            .is_some_and(|(ask, prompt)| ask(prompt, self));
        if !taken {
            self.write(&asked.refusal);
        }
        true
    }

    /// Whether devpit ends this process once its stdin closes, rather than
    /// leaving it to exit.
    pub(crate) fn ended_by_devpit(&self) -> bool {
        self.ends.lock().is_ok_and(|ends| ends.is_some())
    }

    /// Lets go of stdin, which is what lets the CLI exit; one whose wire
    /// gives it a grace is ended when it is still there after it.
    pub(crate) fn close(&self) {
        if let Ok(mut held) = self.stdin.lock() {
            held.take();
        }
        if let Some((pid, grace)) = self.ends.lock().ok().and_then(|mut ends| ends.take()) {
            crate::resident::end_after(pid, grace);
        }
    }
}

/// One message from the person, as stream-json input carries it. Written into
/// a turn that is still running, the CLI takes it into that turn.
pub fn user_line(prompt: &str) -> String {
    serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": prompt }] }
    })
    .to_string()
}

/// [`user_line`] under `uuid`, which the CLI's echo of it carries back.
pub fn steer_line(prompt: &str, uuid: &str) -> String {
    serde_json::json!({
        "type": "user",
        "uuid": uuid,
        "message": { "role": "user", "content": [{ "type": "text", "text": prompt }] }
    })
    .to_string()
}

/// The control request that stops the turn in flight: `{subtype: "interrupt"}`.
pub fn interrupt_request() -> String {
    serde_json::json!({
        "type": "control_request",
        "request_id": format!("interrupt-{}", std::process::id()),
        "request": { "subtype": "interrupt" },
    })
    .to_string()
}

/// The control request that stops one task, as the CLI's own schema names it:
/// `{subtype: "stop_task", task_id}`.
pub fn stop_task_request(task_id: &str) -> String {
    serde_json::json!({
        "type": "control_request",
        "request_id": format!("stop-{task_id}"),
        "request": { "subtype": "stop_task", "task_id": task_id },
    })
    .to_string()
}

/// A permission prompt's request, read off the line [`refusal`] answers.
pub(crate) fn prompt_of(line: &str, session_id: Option<&str>) -> Option<Prompt> {
    let asked: serde_json::Value = serde_json::from_str(line).ok()?;
    let request = asked.get("request")?;
    Some(Prompt {
        request_id: asked.get("request_id")?.as_str()?.to_owned(),
        tool: request.get("tool_name")?.as_str()?.to_owned(),
        input: request.get("input").cloned().unwrap_or_default(),
        session_id: session_id.map(str::to_owned),
    })
}

/// The answer to a permission prompt no hook decided: refused in the CLI's
/// own words, as a printing turn without a prompt tool refuses it.
pub(crate) fn refusal(line: &str) -> Option<String> {
    // Every streamed line passes here; only a prompt is worth parsing.
    if !line.contains("\"can_use_tool\"") {
        return None;
    }
    let asked: serde_json::Value = serde_json::from_str(line).ok()?;
    let request = asked.get("request")?;
    if asked.get("type")?.as_str()? != "control_request"
        || request.get("subtype")?.as_str()? != "can_use_tool"
    {
        return None;
    }
    let reason = request
        .get("decision_reason")
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("This command requires approval");
    Some(
        serde_json::json!({
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": asked.get("request_id")?,
                "response": { "behavior": "deny", "message": reason },
            },
        })
        .to_string(),
    )
}

/// The tasks still running, kept current as parts arrive.
#[derive(Debug, Default)]
pub struct Running {
    ids: BTreeSet<String>,
}

impl Running {
    pub fn saw(&mut self, part: &Part) {
        if let Part::Task {
            task_id, status, ..
        } = part
        {
            if status == "started" || status == "running" {
                self.ids.insert(task_id.clone());
            } else {
                self.ids.remove(task_id);
            }
        }
    }

    pub fn idle(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Whether the turn may let go of stdin now.
pub fn may_close(result_seen: bool, running: &Running) -> bool {
    result_seen && running.idle()
}

/// Whether a turn is over for whoever is waiting on it.
///
/// A process that ends with the turn holds it until its background tasks are
/// done: closing stdin earlier could cut them off. One that stays has no such
/// reason — the agent is idle at its `result`, and what a task says later
/// comes back as a turn of its own. Held for those tasks, a message queued
/// behind the turn waited minutes for an agent that was already listening.
pub fn turn_over(stays: bool, result_seen: bool, running: &Running) -> bool {
    result_seen && (stays || running.idle())
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
