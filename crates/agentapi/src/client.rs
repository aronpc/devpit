//! One question to the running app, over loopback.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

/// How long a question may take. A devpit that has gone away fails at once,
/// at the connection; one that took the question is alive, and five seconds
/// turned a store busy behind another writer into "not answered", and a retry
/// into a second card or comment.
const WAIT: Duration = Duration::from_secs(30);
/// What nobody waits on: the server polling what to offer, the door knocked
/// to see it answers, and a session-start hook that gets ten seconds.
const GLANCE: Duration = Duration::from_secs(5);
/// Starting a session waits for its terminal to settle and may make the
/// card's checkout first. Given up on at five seconds, it was told "devpit is
/// not open" while the session started anyway — and asked again.
const STARTING: Duration = Duration::from_secs(90);
/// A decision waits on a model over the network, given five seconds a call;
/// a plan asks it twice, about the brief and then the cards.
const DECIDING: Duration = Duration::from_secs(45);
/// The gate may ask Decisions too, but a stop hook calls it and gives up at
/// thirty seconds (`devflow_engine::plugin::launch::GATE_WAITS`): it answers first.
const GATING: Duration = Duration::from_secs(25);
/// A fetch over the network, a whole transcript parsed, a large file copied.
const READING: Duration = Duration::from_secs(60);
/// A card's evidence runs its checks: a test suite, an E2E run with video;
/// its review runs its reviewers. The app stops them short of this, so an
/// answer always comes.
const CHECKING: Duration = Duration::from_secs(20 * 60);

/// A project agent may be started, and reads the code before it answers.
const ASKING: Duration = Duration::from_secs(15 * 60);

/// How long devpit has to answer one question.
pub fn wait_for(method: &str) -> Duration {
    match method {
        "devflow_offer" | "devflow_session" | "methods" | "status" => GLANCE,
        "start" | "resume" => STARTING,
        "decide" | "rubric_run" | "devflow_plan" => DECIDING,
        "devflow_gate" => GATING,
        "repo" | "transcript" | "artifact_save" | "artifact_restore" | "devflow_publish" => READING,
        "devflow_evidence" | "devflow_review" | "devflow_stage" => CHECKING,
        "project_agent_ask" | "project_agent_brief" | "project_agent_consult" => ASKING,
        _ => WAIT,
    }
}

/// What to do about a question devpit took and did not answer in time: it
/// may still be done, and asking again would do it twice.
pub fn unanswered(method: &str) -> String {
    let next = match method {
        "create" | "remind" | "devflow_brief" | "devflow_plan" => {
            "devpit_board shows whether the card landed: check it before asking again"
        }
        "comment" | "comment_edit" | "comment_delete" | "handoff" | "finish_card" => {
            "devpit_card shows the card's latest comments: check them before asking again"
        }
        "update" | "move" | "start_card" => {
            "devpit_card shows where the card stands: check it before asking again"
        }
        "log" => "it was most likely written: do not write it again",
        "start" | "resume" | "stop" => {
            "devpit_sessions shows whether it runs: check it before asking again"
        }
        "draft" => "a new draft replaces the last, so asking again is safe",
        "context" | "board" | "card" | "projects" | "sessions" | "screen" | "transcript"
        | "repo" | "reminders" | "artifacts" | "devflow_status" | "devflow_skill" | "health"
        | "methods" => "it only reads, so asking again is safe",
        _ => "check whether it happened before asking again",
    };
    format!("devpit is still working on `{method}` and may yet finish it — {next}")
}

/// Asks the devpit keeping its state in `root`, from `cwd`, and answers with
/// what it said or why it could not.
pub fn ask(root: &Path, method: &str, params: Value, cwd: &Path) -> Result<Value, String> {
    let (address, secret) = door(root)?;
    // Who is asking, when devpit started the agent and said so; a comment is
    // signed with it. The app accepts only an agent it knows.
    let author = std::env::var("DEVPIT_AGENT_ID").unwrap_or_default();
    // The account the asking session runs on, which its reviewers follow.
    let config_dir = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let body = json!({ "method": method, "params": params, "cwd": cwd.display().to_string(), "author": author, "configDir": config_dir, "locale": crate::mcp::locale() }).to_string();
    let mut stream = TcpStream::connect(&address).map_err(|_| closed())?;
    let _ = stream.set_read_timeout(Some(wait_for(method)));
    let _ = stream.set_write_timeout(Some(WAIT));
    let request = format!(
        "POST /agent HTTP/1.1\r\nhost: {address}\r\ncontent-type: application/json\r\n{secret}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(|_| closed())?;
    // Connected and then silent is not closed: devpit took the question and
    // is still on it, and asking again would do it twice.
    let reply = read_reply(stream).map_err(|why| {
        if why == closed() {
            unanswered(method)
        } else {
            why
        }
    })?;
    let parsed: Value = serde_json::from_str(&reply)
        .map_err(|_| "devpit answered something that is not JSON".to_owned())?;
    match (parsed.get("ok"), parsed.get("error")) {
        (Some(ok), _) => Ok(ok.clone()),
        (_, Some(Value::String(why))) => Err(why.clone()),
        _ => Err("devpit answered without saying how it went".to_owned()),
    }
}

/// What a question to a devpit that is not running is answered.
pub fn closed() -> String {
    "devpit is not open: ask the person to open it, then ask again".to_owned()
}

/// Where the app listens and the header that gets a post in, read from the
/// files the app wrote when it started.
pub(crate) fn door(root: &Path) -> Result<(String, String), String> {
    let endpoint = std::fs::read_to_string(root.join("hook-endpoint")).map_err(|_| closed())?;
    let secret = std::fs::read_to_string(root.join("hook-auth")).map_err(|_| closed())?;
    let address = address_of(endpoint.trim()).ok_or_else(closed)?;
    Ok((address, secret.trim().to_owned()))
}

/// `host:port` out of `http://host:port/hook`.
pub(crate) fn address_of(endpoint: &str) -> Option<String> {
    let rest = endpoint.strip_prefix("http://")?;
    let address = rest.split('/').next()?;
    (!address.is_empty()).then(|| address.to_owned())
}

/// The body of an HTTP reply, refusing anything but a 200.
fn read_reply(stream: TcpStream) -> Result<String, String> {
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).map_err(|_| closed())?;
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(match status.split_whitespace().nth(1) {
            Some("401") => "devpit refused the question: its secret changed, which means it restarted — ask again".to_owned(),
            _ => match said_with(reader) {
                Some(said) => format!("devpit answered `{}`: {said}", status.trim()),
                None => format!("devpit answered `{}`", status.trim()),
            },
        });
    }
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|_| closed())? == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let mut body = String::new();
    match length {
        Some(length) => {
            let mut bytes = vec![0u8; length];
            reader.read_exact(&mut bytes).map_err(|_| closed())?;
            body = String::from_utf8_lossy(&bytes).into_owned();
        }
        None => {
            reader.read_to_string(&mut body).map_err(|_| closed())?;
        }
    }
    Ok(body)
}

/// The body of a refusal, cut short: what devpit said, past the headers.
fn said_with(reader: BufReader<TcpStream>) -> Option<String> {
    let mut rest = String::new();
    reader.take(2048).read_to_string(&mut rest).ok()?;
    let body = rest
        .split_once("\r\n\r\n")
        .map_or("", |(_, body)| body)
        .trim();
    (!body.is_empty()).then(|| body.chars().take(300).collect())
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
