//! What devpit writes to `codex app-server`: the handshake, the thread, each
//! turn, approvals, steers and stops.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::{held, Sent, Thread};
use crate::control::Prompt;
use crate::talk::Say;
use crate::wire::{Asked, Switch, Wire};

/// The JSON-RPC code for a method the receiver does not serve.
const NOT_SERVED: i64 = -32601;

const COMMAND_APPROVAL: &str = "item/commandExecution/requestApproval";
const EDIT_APPROVAL: &str = "item/fileChange/requestApproval";

pub(super) struct CodexWire {
    thread: Arc<Mutex<Thread>>,
}

impl CodexWire {
    pub(super) fn new(thread: Arc<Mutex<Thread>>) -> Self {
        Self { thread }
    }
}

/// devpit's permission mode as the app-server's approval policy and sandbox.
/// None leaves both to the person's own Codex config.
pub(super) fn policy(mode: Option<&str>) -> Option<(&'static str, &'static str)> {
    match mode? {
        "bypassPermissions" => Some(("never", "danger-full-access")),
        "acceptEdits" | "manual" => Some(("untrusted", "workspace-write")),
        "plan" => Some(("untrusted", "read-only")),
        _ => None,
    }
}

/// The sandbox a turn switches to, in the shape `turn/start` takes it.
fn sandbox_policy(sandbox: &str) -> Value {
    match sandbox {
        "danger-full-access" => json!({ "type": "dangerFullAccess" }),
        "read-only" => json!({ "type": "readOnly", "networkAccess": false }),
        _ => json!({
            "type": "workspaceWrite",
            "writableRoots": [],
            "networkAccess": false,
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false,
        }),
    }
}

/// Whether the mode answers this approval without the person.
pub(super) fn allowed(mode: Option<&str>, method: &str) -> bool {
    match mode {
        Some("bypassPermissions") => true,
        Some("acceptEdits") => method == EDIT_APPROVAL,
        _ => false,
    }
}

fn decision(id: &Value, decision: &str) -> String {
    json!({ "id": id, "result": { "decision": decision } }).to_string()
}

fn input_of(text: &str) -> Value {
    json!([{ "type": "text", "text": text, "text_elements": [] }])
}

impl Thread {
    fn request(&mut self, method: &str, params: Value, sent: Sent) -> String {
        let id = self.next_id;
        self.next_id += 1;
        self.sent.insert(id, sent);
        json!({ "id": id, "method": method, "params": params }).to_string()
    }

    fn greeting(&mut self) -> String {
        self.opening = true;
        let params = json!({
            "clientInfo": { "name": "devpit", "title": "devpit", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": false, "requestAttestation": false },
        });
        self.request("initialize", params, Sent::Initialize)
    }

    /// `thread/resume` for a conversation that has one, `thread/start` otherwise.
    fn open(&mut self, resuming: bool) -> String {
        let mut params = json!({ "cwd": self.cwd });
        if let Some(model) = &self.model {
            params["model"] = json!(model);
        }
        if let Some((approval, sandbox)) = policy(self.mode.as_deref()) {
            params["approvalPolicy"] = json!(approval);
            params["sandbox"] = json!(sandbox);
        }
        let method = match self.resume.clone().filter(|_| resuming) {
            Some(id) => {
                params["threadId"] = json!(id);
                params["excludeTurns"] = json!(true);
                "thread/resume"
            }
            None => "thread/start",
        };
        self.request(method, params, Sent::Open { resuming })
    }

    fn turn(&mut self, text: &str) -> String {
        let mut params = json!({ "threadId": self.id, "input": input_of(text) });
        if let Some(effort) = &self.effort {
            params["effort"] = json!(effort);
        }
        if std::mem::take(&mut self.switched_model) {
            params["model"] = json!(self.model);
            self.ran_on.clone_from(&self.model);
        }
        if std::mem::take(&mut self.switched_mode) {
            if let Some((approval, sandbox)) = policy(self.mode.as_deref()) {
                params["approvalPolicy"] = json!(approval);
                params["sandboxPolicy"] = sandbox_policy(sandbox);
            }
        }
        self.request("turn/start", params, Sent::Turn)
    }

    /// The app-server answered a request of devpit's.
    fn answered(&mut self, said: &Value) -> Vec<String> {
        let Some(sent) = said["id"].as_u64().and_then(|id| self.sent.remove(&id)) else {
            return Vec::new();
        };
        let error = said["error"]["message"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| said.get("error").map(Value::to_string));
        match (sent, error) {
            (Sent::Initialize, None) => {
                self.initialized = true;
                let resuming = self.resume.is_some();
                vec![
                    json!({ "method": "initialized" }).to_string(),
                    self.open(resuming),
                ]
            }
            // A thread the app-server no longer has starts over, said so.
            (Sent::Open { resuming: true }, Some(why)) => {
                self.notice = Some(format!(
                    "Codex could not resume this conversation ({why}), so it starts over here.\n\n"
                ));
                self.resume = None;
                vec![self.open(false)]
            }
            (Sent::Open { .. }, None) => {
                self.opening = false;
                self.id = said["result"]["thread"]["id"].as_str().map(str::to_owned);
                (self.switched_model, self.switched_mode) = (false, false);
                let held = std::mem::take(&mut self.held);
                held.iter().map(|text| self.turn(text)).collect()
            }
            (Sent::Turn, None) => {
                self.turn = said["result"]["turn"]["id"].as_str().map(str::to_owned);
                Vec::new()
            }
            (Sent::Initialize | Sent::Open { .. } | Sent::Turn, Some(why)) => {
                self.opening = false;
                self.held.clear();
                self.failed = Some(why);
                Vec::new()
            }
            (Sent::Other, _) => Vec::new(),
        }
    }

    /// The person's key for an approval request.
    fn key(&self, id: &Value) -> String {
        format!("{}#{id}", self.id.as_deref().unwrap_or_default())
    }
}

impl Wire for CodexWire {
    fn argv(&self, _turn: &Say<'_>) -> Vec<String> {
        vec!["app-server".to_owned()]
    }

    /// A new process: its thread is opened from scratch.
    fn hello(&self, turn: &Say<'_>) -> Vec<String> {
        let mut thread = held(&self.thread);
        *thread = Thread {
            cwd: turn.cwd.display().to_string(),
            resume: turn.session_id.map(str::to_owned),
            model: turn
                .model
                .filter(|model| *model != "default")
                .map(str::to_owned),
            mode: turn.permission.map(str::to_owned),
            effort: turn.effort.map(str::to_owned),
            ..Thread::default()
        };
        vec![thread.greeting()]
    }

    /// Held while the thread opens; a thread that failed to open is opened again.
    fn prompt(&self, text: &str) -> Vec<String> {
        let mut thread = held(&self.thread);
        if thread.id.is_some() && !thread.opening {
            return vec![thread.turn(text)];
        }
        thread.held.push(text.to_owned());
        if thread.opening {
            return Vec::new();
        }
        if thread.initialized {
            thread.opening = true;
            let resuming = thread.resume.is_some();
            return vec![thread.open(resuming)];
        }
        vec![thread.greeting()]
    }

    fn steer(&self, text: &str, uuid: &str) -> Option<String> {
        let mut thread = held(&self.thread);
        let params = json!({
            "threadId": thread.id.clone()?,
            "expectedTurnId": thread.turn.clone()?,
            "clientUserMessageId": uuid,
            "input": input_of(text),
        });
        Some(thread.request("turn/steer", params, Sent::Other))
    }

    /// Empty before the app-server has named the turn: there is nothing yet
    /// to interrupt, and devpit stops it the other way.
    fn interrupt(&self) -> Vec<String> {
        let mut thread = held(&self.thread);
        let (Some(id), Some(turn)) = (thread.id.clone(), thread.turn.clone()) else {
            return Vec::new();
        };
        let params = json!({ "threadId": id, "turnId": turn });
        vec![thread.request("turn/interrupt", params, Sent::Other)]
    }

    fn asked(&self, line: &str, session_id: Option<&str>) -> Option<Asked> {
        if !line.contains(COMMAND_APPROVAL) && !line.contains(EDIT_APPROVAL) {
            return None;
        }
        let said: Value = serde_json::from_str(line).ok()?;
        let thread = held(&self.thread);
        let key = thread.key(&said["id"]);
        let id = thread.waiting.get(&key)?;
        let params = &said["params"];
        let (tool, mut input) = if said["method"] == COMMAND_APPROVAL {
            let command = super::command_of(params);
            ("Bash", json!({ "command": command, "cwd": params["cwd"] }))
        } else {
            let edit = thread.edits.get(params["itemId"].as_str()?);
            let changes = edit.map(|item| item["changes"].clone()).unwrap_or_default();
            (
                "Edit",
                json!({ "file_path": changes[0]["path"], "changes": changes }),
            )
        };
        if let Some(reason) = params["reason"].as_str() {
            input["reason"] = json!(reason);
        }
        Some(Asked {
            prompt: Some(Prompt {
                request_id: key,
                tool: tool.to_owned(),
                input,
                session_id: session_id.map(str::to_owned).or_else(|| thread.id.clone()),
            }),
            refusal: decision(id, "decline"),
        })
    }

    fn answer(&self, request_id: &str, response: Value) -> Option<String> {
        let id = held(&self.thread).waiting.remove(request_id)?;
        let said = if response["behavior"] == "allow" {
            "accept"
        } else {
            "decline"
        };
        Some(decision(&id, said))
    }

    /// Kept here and sent with the next turn, which `turn/start` takes.
    fn control(&self, _id: &str, request: Value) -> Option<Switch> {
        let mut thread = held(&self.thread);
        // Back to the person's own config is a new process's to take.
        match request["subtype"].as_str()? {
            "set_model" => {
                let model = request["model"]
                    .as_str()
                    .filter(|model| *model != "default")?;
                thread.model = Some(model.to_owned());
                thread.switched_model = true;
            }
            "set_permission_mode" => {
                let mode = request["mode"].as_str()?;
                policy(Some(mode))?;
                thread.mode = Some(mode.to_owned());
                thread.switched_mode = true;
            }
            _ => return None,
        }
        Some(Switch::Kept)
    }

    fn control_answer(&self, _line: &str) -> Option<Value> {
        None
    }

    /// The handshake's next step, an approval the mode already answers, and
    /// a refusal for any other request devpit does not serve.
    fn replies(&self, line: &str) -> Vec<String> {
        let Ok(said) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };
        let mut thread = held(&self.thread);
        let Some(method) = said["method"].as_str() else {
            return thread.answered(&said);
        };
        let Some(id) = said.get("id") else {
            return Vec::new();
        };
        if method != COMMAND_APPROVAL && method != EDIT_APPROVAL {
            let error =
                json!({ "code": NOT_SERVED, "message": format!("devpit does not serve {method}") });
            return vec![json!({ "id": id, "error": error }).to_string()];
        }
        if allowed(thread.mode.as_deref(), method) {
            return vec![decision(id, "accept")];
        }
        let key = thread.key(id);
        thread.waiting.insert(key, id.clone());
        Vec::new()
    }
}
