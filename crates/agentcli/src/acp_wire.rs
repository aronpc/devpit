//! What devpit writes to an ACP agent: the handshake, the person's messages,
//! permission answers and stops.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::{error_of, held, Opening, Sent, Session, Waiting};
use crate::control::Prompt;
use crate::talk::Say;
use crate::wire::{Asked, Switch, Wire};

/// The ACP protocol version devpit speaks.
const VERSION: u64 = 1;

/// The JSON-RPC code for a method the receiver does not serve.
const NOT_SERVED: i64 = -32601;

pub(crate) struct AcpWire {
    args: &'static [&'static str],
    session: Arc<Mutex<Session>>,
}

impl AcpWire {
    pub(crate) fn new(args: &'static [&'static str], session: Arc<Mutex<Session>>) -> Self {
        Self { args, session }
    }
}

/// Whether the chat's mode answers a tool of this kind without the person:
/// Full access answers everything, Accept edits the file changes too, and
/// every mode what only reads.
pub fn allowed(mode: Option<&str>, kind: &str) -> bool {
    const READS: &[&str] = &["read", "search", "think"];
    const EDITS: &[&str] = &["edit", "delete", "move"];
    match mode {
        Some("bypassPermissions") => true,
        Some("acceptEdits") => READS.contains(&kind) || EDITS.contains(&kind),
        _ => READS.contains(&kind),
    }
}

fn response(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// The answer to a permission request: the first option of a kind wanted.
fn chosen(id: &Value, options: &[Value], kinds: &[&str]) -> String {
    let picked = kinds
        .iter()
        .find_map(|kind| options.iter().find(|one| one["kind"] == *kind));
    let outcome = match picked {
        Some(option) => json!({ "outcome": "selected", "optionId": option["optionId"] }),
        None => json!({ "outcome": "cancelled" }),
    };
    response(id, json!({ "outcome": outcome }))
}

const ALLOW: &[&str] = &["allow_once", "allow_always"];
const REFUSE: &[&str] = &["reject_once", "reject_always"];

fn key(session: &Session, id: &Value) -> String {
    format!("{}#{id}", session.id.as_deref().unwrap_or_default())
}

impl Session {
    /// The kind of tool a permission request is about.
    fn kind_of(&self, call: &Value) -> String {
        call["kind"]
            .as_str()
            .or_else(|| {
                call["toolCallId"]
                    .as_str()
                    .and_then(|id| self.kinds.get(id))
                    .map(String::as_str)
            })
            .unwrap_or("other")
            .to_owned()
    }

    /// Opens the session the turn names, the way the agent offers.
    fn open(&mut self, offered: &Value) -> Vec<String> {
        let resumes = offered["sessionCapabilities"].get("resume").is_some();
        let loads = offered["loadSession"].as_bool().unwrap_or(false);
        let (method, opening) = match &self.resume {
            Some(_) if resumes => ("session/resume", Opening::Resume),
            Some(_) if loads => ("session/load", Opening::Load),
            Some(_) => {
                self.notice = Some(
                    "This agent does not offer to resume a conversation, so it starts over here.\n\n"
                        .to_owned(),
                );
                ("session/new", Opening::New)
            }
            None => ("session/new", Opening::New),
        };
        vec![self.opening(method, opening)]
    }

    fn opening(&mut self, method: &str, opening: Opening) -> String {
        let mut params = json!({ "cwd": self.cwd, "mcpServers": [] });
        if opening == Opening::New {
            self.cost_from = Some(0.0);
        } else {
            params["sessionId"] = json!(self.resume);
            self.replaying = opening == Opening::Load;
        }
        self.request(method, params, Sent::Open(opening))
    }

    /// The agent answered one of devpit's requests.
    fn answered(&mut self, id: u64, said: &Value) -> Vec<String> {
        let Some(sent) = self.sent.get(&id).cloned() else {
            return Vec::new();
        };
        match sent {
            Sent::Initialize => {
                self.sent.remove(&id);
                if let Some(why) = error_of(said) {
                    return self.fail(why);
                }
                self.offered = Some(said["result"]["agentCapabilities"].clone());
                self.open(&said["result"]["agentCapabilities"])
            }
            Sent::Open(opening) => {
                self.sent.remove(&id);
                if let Some(why) = error_of(said) {
                    if opening == Opening::New {
                        return self.fail(why);
                    }
                    // A session the agent no longer has starts over, said so.
                    self.notice = Some(format!(
                        "The agent could not resume this conversation ({why}), so it starts over here.\n\n"
                    ));
                    return vec![self.opening("session/new", Opening::New)];
                }
                self.replaying = false;
                self.opened_by = Some(id);
                self.id = said["result"]["sessionId"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| self.resume.clone());
                self.options(&said["result"]);
                let wanted = self
                    .model
                    .clone()
                    .filter(|model| self.current_model.as_deref() != Some(model.as_str()));
                match wanted.and_then(|model| self.pick(&model, Sent::Model)) {
                    Some(picking) => vec![picking],
                    None => self.ready(),
                }
            }
            Sent::Model => {
                self.sent.remove(&id);
                self.options(&said["result"]);
                self.ready()
            }
            Sent::Switch(_) | Sent::Prompt => Vec::new(),
        }
    }

    /// The session takes messages now: the ones held are sent.
    fn ready(&mut self) -> Vec<String> {
        (self.ready, self.opening) = (true, false);
        let held = std::mem::take(&mut self.held);
        if std::mem::take(&mut self.cancelled) {
            self.stop_now = true;
            return Vec::new();
        }
        held.iter().map(|text| self.prompt(text)).collect()
    }

    /// The session did not open: the turn waiting on it ends with why, and
    /// the next message tries again.
    fn fail(&mut self, why: String) -> Vec<String> {
        self.held.clear();
        (self.opening, self.cancelled) = (false, false);
        self.failed = Some(why);
        Vec::new()
    }

    fn greeting(&mut self) -> String {
        self.opening = true;
        let params = json!({
            "protocolVersion": VERSION,
            "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
            "clientInfo": { "name": "devpit", "version": env!("CARGO_PKG_VERSION") },
        });
        self.request("initialize", params, Sent::Initialize)
    }
}

impl Wire for AcpWire {
    fn argv(&self, _turn: &Say<'_>) -> Vec<String> {
        self.args.iter().map(|&arg| arg.to_owned()).collect()
    }

    fn hello(&self, turn: &Say<'_>) -> Vec<String> {
        let mut session = held(&self.session);
        session.cwd = turn.cwd.display().to_string();
        session.resume = turn.session_id.map(str::to_owned);
        session.model = turn
            .model
            .filter(|model| *model != "default")
            .map(str::to_owned);
        session.mode = turn.permission.map(str::to_owned);
        vec![session.greeting()]
    }

    /// Held while the session opens; a session that failed to open is opened
    /// again first.
    fn prompt(&self, text: &str) -> Vec<String> {
        let mut session = held(&self.session);
        if session.ready {
            return vec![session.prompt(text)];
        }
        session.held.push(text.to_owned());
        if session.opening {
            return Vec::new();
        }
        session.opening = true;
        match session.offered.clone() {
            Some(offered) => session.open(&offered),
            None => vec![session.greeting()],
        }
    }

    /// `session/cancel`, and every permission request still waiting answered
    /// cancelled, as the protocol asks. Before the session opens, the held
    /// message is dropped and the turn ends once the session answers.
    fn interrupt(&self) -> Option<Vec<String>> {
        let mut session = held(&self.session);
        let Some(id) = session.id.clone().filter(|_| session.ready) else {
            // A second stop while the first waits for the session is taken too.
            if session.held.is_empty() && !session.cancelled {
                return None;
            }
            session.held.clear();
            session.cancelled = true;
            return Some(Vec::new());
        };
        let cancel =
            json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": id } });
        let waiting: Vec<Waiting> = session.waiting.drain().map(|(_, one)| one).collect();
        Some(
            std::iter::once(cancel.to_string())
                .chain(
                    waiting.iter().map(|one| {
                        response(&one.id, json!({ "outcome": { "outcome": "cancelled" } }))
                    }),
                )
                .collect(),
        )
    }

    fn asked(&self, line: &str, session_id: Option<&str>) -> Option<Asked> {
        if !line.contains("\"session/request_permission\"") {
            return None;
        }
        let said: Value = serde_json::from_str(line).ok()?;
        let session = held(&self.session);
        let key = key(&session, &said["id"]);
        let waiting = session.waiting.get(&key)?;
        let call = &said["params"]["toolCall"];
        let mut input = call.clone();
        input["kind"] = json!(session.kind_of(call));
        Some(Asked {
            prompt: Some(Prompt {
                request_id: key,
                tool: call["title"].as_str().unwrap_or("tool").to_owned(),
                input,
                session_id: session_id.map(str::to_owned).or_else(|| session.id.clone()),
            }),
            refusal: chosen(&waiting.id, &waiting.options, REFUSE),
        })
    }

    fn answer(&self, request_id: &str, response: Value) -> Option<String> {
        let waiting = held(&self.session).waiting.remove(request_id)?;
        let kinds = if response["behavior"] == "allow" {
            ALLOW
        } else {
            REFUSE
        };
        Some(chosen(&waiting.id, &waiting.options, kinds))
    }

    /// The mode is devpit's own, kept here; a model goes to the agent when it
    /// offers to switch one.
    fn control(&self, id: &str, request: Value) -> Option<Switch> {
        let mut session = held(&self.session);
        match request["subtype"].as_str()? {
            "set_permission_mode" => {
                session.mode = request["mode"].as_str().map(str::to_owned);
                Some(Switch::Kept)
            }
            "set_model" => {
                let model = request["model"]
                    .as_str()
                    .filter(|model| *model != "default")?;
                session
                    .pick(model, Sent::Switch(id.to_owned()))
                    .map(Switch::Write)
            }
            _ => None,
        }
    }

    fn control_answer(&self, line: &str) -> Option<Value> {
        let said: Value = serde_json::from_str(line).ok()?;
        if said.get("method").is_some() {
            return None;
        }
        let id = said["id"].as_u64()?;
        let mut session = held(&self.session);
        let Some(Sent::Switch(switch)) = session.sent.get(&id).cloned() else {
            return None;
        };
        session.sent.remove(&id);
        let subtype = if said.get("error").is_some() {
            "error"
        } else {
            session.options(&said["result"]);
            "success"
        };
        Some(json!({ "subtype": subtype, "request_id": switch }))
    }

    /// The handshake's next step, a permission the mode already answers, and
    /// a refusal for any request devpit does not serve.
    fn replies(&self, line: &str) -> Vec<String> {
        let Ok(said) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };
        let mut session = held(&self.session);
        let Some(method) = said["method"].as_str() else {
            return said["id"]
                .as_u64()
                .map(|id| session.answered(id, &said))
                .unwrap_or_default();
        };
        let Some(id) = said.get("id") else {
            return Vec::new();
        };
        if method != "session/request_permission" {
            let error =
                json!({ "code": NOT_SERVED, "message": format!("devpit does not serve {method}") });
            return vec![json!({ "jsonrpc": "2.0", "id": id, "error": error }).to_string()];
        }
        let params = &said["params"];
        let options = params["options"].as_array().cloned().unwrap_or_default();
        let kind = session.kind_of(&params["toolCall"]);
        if allowed(session.mode.as_deref(), &kind) {
            return vec![chosen(id, &options, ALLOW)];
        }
        let key = key(&session, id);
        session.waiting.insert(
            key,
            Waiting {
                id: id.clone(),
                options,
            },
        );
        Vec::new()
    }
}
