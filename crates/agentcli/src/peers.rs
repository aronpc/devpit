//! What a session's own log says passed between it and other sessions: the
//! messages it sent, the ones it was sent, and the notices that another went
//! idle.
//!
//! Read from the CLI's session file, because its stream does not carry them:
//! a turn woken by another session opens with a new `init` and nothing else
//! (measured on Claude Code 2.1.282). The file does, on the entry that woke
//! it — `origin: {kind: "peer", name, body}` for a message, and a
//! "[Cross-session idle notice]" for an idle one.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// How much of a log's end is read: the latest exchanges, not a history.
const TAIL: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PeerEvent {
    /// This session wrote to another.
    Sent {
        at: String,
        to: String,
        summary: String,
        message: String,
    },
    /// Another session wrote to this one.
    Heard {
        at: String,
        from: String,
        body: String,
    },
    /// Another session this one was waiting on finished a turn.
    Idle {
        at: String,
        from: String,
        said: String,
    },
}

impl PeerEvent {
    /// The other session.
    pub fn peer(&self) -> &str {
        match self {
            Self::Sent { to, .. } => to,
            Self::Heard { from, .. } | Self::Idle { from, .. } => from,
        }
    }
}

/// Where the CLI keeps the session logs of a folder, under a config folder.
pub fn logs_of(config: &Path, cwd: &Path) -> PathBuf {
    let slug: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    config.join("projects").join(slug)
}

/// Every exchange in one log's text, in order.
pub fn events(text: &str) -> Vec<PeerEvent> {
    text.lines()
        .filter(|line| line.contains("SendMessage") || line.contains("\"isMeta\""))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .flat_map(|entry| of_entry(&entry))
        .collect()
}

pub(crate) fn of_entry(entry: &Value) -> Vec<PeerEvent> {
    let at = entry["timestamp"].as_str().unwrap_or_default().to_owned();
    let content = &entry["message"]["content"];
    match entry["type"].as_str() {
        Some("assistant") => content
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| block["type"] == "tool_use" && block["name"] == "SendMessage")
                    .map(|block| PeerEvent::Sent {
                        at: at.clone(),
                        to: text_at(&block["input"], "to"),
                        summary: text_at(&block["input"], "summary"),
                        message: text_at(&block["input"], "message"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some("user") if entry["origin"]["kind"] == "peer" => vec![PeerEvent::Heard {
            at,
            from: text_at(&entry["origin"], "name"),
            body: text_at(&entry["origin"], "body"),
        }],
        Some("user") => {
            let said = match content {
                Value::String(text) => text.clone(),
                Value::Array(blocks) => blocks
                    .iter()
                    .filter_map(|block| block["text"].as_str())
                    .collect::<Vec<_>>()
                    .join(""),
                _ => String::new(),
            };
            idle_notice(&said)
                .map(|(from, said)| vec![PeerEvent::Idle { at, from, said }])
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// What woke the session, when another did: the last thing said to it, if
/// that was another session's message or idle notice and not the person.
pub fn woken_by(text: &str) -> Option<PeerEvent> {
    text.lines().rev().find_map(|line| {
        let entry = serde_json::from_str::<Value>(line).ok()?;
        if entry["type"] != "user" {
            return None;
        }
        // A tool's result is the turn going on, not something said to it.
        let results = entry["message"]["content"]
            .as_array()
            .is_some_and(|blocks| blocks.iter().any(|block| block["type"] == "tool_result"));
        if results {
            return None;
        }
        Some(of_entry(&entry).into_iter().next())
    })?
}

/// `[Cross-session idle notice] "name", … Its harness reports: «…»`.
pub(crate) fn idle_notice(text: &str) -> Option<(String, String)> {
    let rest = text
        .trim_start()
        .strip_prefix("[Cross-session idle notice]")?;
    let name = rest.split('"').nth(1)?.to_owned();
    let said = rest
        .split_once('«')
        .and_then(|(_, after)| after.split_once('»'))
        .map(|(said, _)| said.to_owned())
        .unwrap_or_default();
    Some((name, said))
}

fn text_at(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}

/// The end of a log, from the first whole line.
pub fn tail(path: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let size = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let from = size.saturating_sub(TAIL);
    if file.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if from == 0 {
        return text;
    }
    text.split_once('\n')
        .map(|(_, rest)| rest.to_owned())
        .unwrap_or_default()
}

/// The newest log in a folder: the one a running session is writing.
pub fn newest(logs: &Path) -> Option<PathBuf> {
    std::fs::read_dir(logs)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

#[cfg(test)]
#[path = "peers_tests.rs"]
mod tests;
