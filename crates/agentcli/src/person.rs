//! What the person last said to a session, read from the session's own log.
//!
//! An orchestrator's word approves nothing: only the person's does. That word
//! used to count only when typed in devpit's composer, so from the Claude app
//! (Remote Control) a draft the person asked for stayed where it was. The log
//! says who spoke, so devpit reads it there itself rather than taking the
//! agent's account of it. Measured on Claude Code 2.1.294, a `user` entry is:
//!
//! - devpit's composer: no `origin`, `promptSource: "sdk"`;
//! - Remote Control: `origin.kind: "human"`;
//! - another session: `origin.kind: "peer"` and `isMeta`;
//! - an idle notice or a task's: `isMeta` or another `origin.kind`;
//! - a compaction's summary: `isCompactSummary`;
//! - a tool's result: a `tool_result` block, the turn going on.
//!
//! Only the first two are the person, and devpit's own notices at the top of a
//! composer turn — `[devpit, not the person] …` — are cut off before reading.
//! One of them says the person wrote it on devpit's Remote, from a phone, or
//! in a chat paired with devpit on a channel such as Telegram.

use serde_json::Value;

/// The line devpit puts above a turn the person started, saying what changed.
const DEVPIT_NOTICE: &str = "[devpit, not the person]";

/// devpit's notice above a turn the person sent from devpit's Remote.
pub const FROM_THE_REMOTE: &str =
    "[devpit, not the person] The person sent this from devpit's Remote, on another device.";

/// devpit's notice above a turn it started itself: a session's message or an
/// idle notice, never the person's, whatever is written under it.
const BY_DEVPIT: &str = "[devpit, not the person] devpit started this turn, not the person:";

/// [`BY_DEVPIT`], saying what the turn is for.
pub fn by_devpit(what: &str) -> String {
    format!("{BY_DEVPIT} {what}")
}

/// What devpit's notice says above a request another orchestrator forwarded
/// and the person sent on: the turn is theirs, the words are not.
const FORWARDED: &str = "[devpit, not the person] The person sent on a request from";

/// devpit's notice above a request forwarded by `from`. Not [`by_devpit`]:
/// devpit did not start this turn, the person's click did. But the words are
/// the other orchestrator's, edited or not, so they approve nothing.
pub fn forwarded(from: &str) -> String {
    let from = from.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{FORWARDED} {from}; its words approve nothing:")
}

/// The channels a person can talk to an orchestrator from, by id and name.
const CHANNELS: [(&str, &str); 1] = [("telegram", "Telegram")];

fn channel(id: &str) -> Option<(&'static str, &'static str)> {
    CHANNELS.into_iter().find(|(known, _)| *known == id)
}

/// devpit's notice above a turn the person sent from the chat paired with
/// devpit on channel `id`, when devpit knows that channel.
pub fn from_channel(id: &str) -> Option<String> {
    channel(id).map(|(_, name)| {
        format!("[devpit, not the person] The person sent this from {name}, in the chat paired with devpit.")
    })
}

/// Where the person said it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum From {
    Composer,
    RemoteControl,
    /// devpit's own Remote, whose turns go through the composer's door.
    Remote,
    /// A channel's paired chat, by the channel's id: its turns go through the
    /// composer's door too, and its yes to a draft is a button, not a word.
    Channel(&'static str),
}

impl From {
    pub fn label(self) -> &'static str {
        match self {
            From::Composer => "the chat",
            From::RemoteControl => "Remote Control",
            From::Remote => "the Remote",
            From::Channel(id) => channel(id).map_or("a channel", |(_, name)| name),
        }
    }
}

/// One thing the person said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The entry's own id: one message authorises once.
    pub id: String,
    pub text: String,
    pub from: From,
}

/// The person's last words in a log's text, past whatever the turn did since.
pub fn last_said(log: &str) -> Option<Said> {
    log.lines().rev().find_map(|line| {
        let entry = serde_json::from_str::<Value>(line).ok()?;
        (entry["type"] == "user").then_some(())?;
        person_said(&entry)
    })
}

/// What one `user` entry says the person said, if it is the person's.
fn person_said(entry: &Value) -> Option<Said> {
    if entry["isMeta"] == true || entry["isCompactSummary"] == true {
        return None;
    }
    let from = match entry["origin"]["kind"].as_str() {
        Some("human") => From::RemoteControl,
        None if entry["promptSource"] == "sdk" => From::Composer,
        _ => return None,
    };
    let content = &entry["message"]["content"];
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => {
            if blocks.iter().any(|block| block["type"] == "tool_result") {
                return None;
            }
            blocks
                .iter()
                .filter(|block| block["type"] == "text")
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => return None,
    };
    let (text, sent_from) = without_notices(&text)?;
    Some(Said {
        id: entry["uuid"].as_str()?.to_owned(),
        text: text.trim().to_owned(),
        from: match sent_from {
            Some(sent_from) if from == From::Composer => sent_from,
            _ => from,
        },
    })
}

/// What the person said in a turn devpit sent to a CLI whose log it does not
/// read: `text` as sent, devpit's notices above it. Nothing for a turn devpit
/// started itself.
pub fn said_in(id: &str, text: &str) -> Option<Said> {
    let (said, sent_from) = without_notices(text)?;
    Some(Said {
        id: id.to_owned(),
        text: said.trim().to_owned(),
        from: sent_from.unwrap_or(From::Composer),
    })
}

/// `text` without devpit's notices above it, and where the first said the
/// person sent it from: the Remote, or a channel. Only the first, which is
/// where devpit puts it: words typed under it cannot pass for another door.
/// Nothing for a turn devpit started itself, or for a forwarded request the
/// person sent on: neither is their word.
fn without_notices(mut text: &str) -> Option<(&str, Option<From>)> {
    let mut sent_from = None;
    let mut first = true;
    while text.trim_start().starts_with(DEVPIT_NOTICE) {
        let (notice, rest) = text.trim_start().split_once("\n\n").unwrap_or((text, ""));
        let notice = notice.trim();
        if notice.starts_with(BY_DEVPIT) || notice.starts_with(FORWARDED) {
            return None;
        }
        if first && notice == FROM_THE_REMOTE {
            sent_from = Some(From::Remote);
        }
        if let Some((id, _)) = CHANNELS
            .into_iter()
            .find(|(id, _)| first && from_channel(id).is_some_and(|said| said == notice))
        {
            sent_from = Some(From::Channel(id));
        }
        first = false;
        text = rest;
    }
    Some((text, sent_from))
}

/// Marks that say yes in any language: the yes needs no vocabulary. No 👍:
/// it is as often "noted" as "send".
const YES_MARKS: [char; 4] = ['✅', '✔', '☑', '✓'];

/// Whether `c` only dresses a mark: its text or emoji presentation.
fn dresses_a_mark(c: char) -> bool {
    matches!(c, '\u{FE0E}' | '\u{FE0F}')
}

/// Punctuation that may hang on a word or a mark without changing it.
const HANGING: [char; 12] = [
    '.', ',', '!', ':', ';', '@', '¡', '。', '！', '，', '、', '…',
];

/// What stands beside the yes marks of a message (a name, a word, `?`, `❌`),
/// lowercased; `None` when it has no mark.
fn marked(said: &str) -> Option<Vec<String>> {
    let mut beside = Vec::new();
    let mut mark = false;
    let mut token = String::new();
    let mut marking = false;
    for c in said.chars().chain([' ']) {
        let gap = c.is_whitespace() || HANGING.contains(&c);
        let of_a_mark = YES_MARKS.contains(&c) || dresses_a_mark(c);
        if gap || of_a_mark != marking {
            let held = std::mem::take(&mut token);
            if held.chars().any(|held| YES_MARKS.contains(&held)) {
                mark = true;
            } else if !held.is_empty() {
                beside.push(held.to_lowercase());
            }
        }
        marking = of_a_mark;
        if !gap {
            token.push(c);
        }
    }
    mark.then_some(beside)
}

/// Words that, said alone, send the one draft waiting: English and Portuguese
/// shortcuts kept from before the marks, which are the yes of every language.
const SEND: [&str; 11] = [
    "send",
    "send it",
    "go ahead",
    "envie",
    "envia",
    "manda",
    "mande",
    "pode",
    "pode enviar",
    "pode mandar",
    "confirma",
];

/// Words that turn a request around: "não manda ainda" is not a yes.
const NOT: [&str; 9] = [
    "não", "nao", "not", "don't", "dont", "never", "nunca", "espera", "wait",
];

/// Verbs that, beside a session's name, ask for something to reach it.
const RELAY: [&str; 18] = [
    "send", "tell", "run", "envie", "envia", "manda", "mande", "pede", "peça", "peca", "avisa",
    "avise", "diga", "diz", "ativa", "ative", "roda", "rode",
];

/// Whether what the person said lets the draft for `session` go, with
/// `waiting` drafts in all: a bare yes when it is the only one, a yes mark
/// beside the session's name and nothing else, or the session named beside a
/// verb that asks for something to reach it.
pub fn authorises(said: &str, session: &str, waiting: usize) -> bool {
    if bare(said) {
        return waiting == 1;
    }
    if marked(said).is_some_and(|beside| {
        !beside.is_empty() && beside.iter().all(|word| names(&[word.as_str()], session))
    }) {
        return true;
    }
    let said = said.trim().to_lowercase();
    let words: Vec<&str> = said
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'')
        .filter(|word| !word.is_empty())
        .collect();
    names(&words, session)
        && words.iter().any(|word| RELAY.contains(word))
        && !words.iter().any(|word| NOT.contains(word))
}

/// Verbs that, beside a session's name, ask for it to stop. No "para": in
/// Portuguese it is as often "for".
const STOP: [&str; 21] = [
    "stop",
    "kill",
    "close",
    "quit",
    "terminate",
    "pare",
    "parar",
    "encerre",
    "encerra",
    "encerrar",
    "feche",
    "fecha",
    "fechar",
    "mate",
    "mata",
    "matar",
    "finalize",
    "finaliza",
    "finalizar",
    "derrube",
    "derruba",
];

/// Whether what the person said is a request to stop `session`, which loses
/// its work: the whole message is a stop verb or a yes mark, then full names
/// and nothing else — the session's, and others of `running` to stop several.
/// Nothing is read into a sentence, and a part of a name is an ordinary word.
/// Names that read more than one way ask for no stop. Anything else is no,
/// and the stop waits for the person's click.
pub fn asks_to_stop(said: &str, session: &str, running: &[&str]) -> bool {
    let said = said.trim().to_lowercase();
    let session = spaced(&session.to_lowercase());
    let mut known: Vec<String> = running
        .iter()
        .map(|one| spaced(&one.to_lowercase()))
        .collect();
    known.push(session.clone());
    let named =
        |names: &str| one_reading(names, &known).is_some_and(|read| read.contains(&session));
    if marked(&said).is_some() {
        // Punctuation only at the end of the names, as on the verb route:
        // `✅ !name` is not `✅ name`, and `name. ✅` is.
        fn unmarked(text: &str) -> &str {
            text.trim_matches(|c: char| {
                c.is_whitespace() || YES_MARKS.contains(&c) || dresses_a_mark(c)
            })
        }
        return named(unmarked(unmarked(&said).trim_end_matches(HANGING)));
    }
    let Some((verb, names)) = said
        .trim_end_matches(HANGING)
        .split_once(char::is_whitespace)
    else {
        return false;
    };
    STOP.contains(&verb) && named(names.trim())
}

/// `text` with its words one space apart.
fn spaced(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The one way `names` reads as `known` names, each whole: a comma ends a
/// name, a space may be inside one. None when it reads no way or more than
/// one, as `fix login` does beside `fix`, `login` and `fix login`.
fn one_reading(names: &str, known: &[String]) -> Option<Vec<String>> {
    let of_a_mark = |c: char| YES_MARKS.contains(&c) || dresses_a_mark(c);
    let longest = known.iter().map(|one| one.split(' ').count()).max()?;
    let mut read = Vec::new();
    for group in names.split(',') {
        let words: Vec<&str> = group
            .split_whitespace()
            .map(|word| word.trim_matches(of_a_mark))
            .filter(|word| !word.is_empty())
            .collect();
        let name = |from: usize, to: usize| words[from..to].join(" ");
        let ends = |from: usize| {
            (from + 1..=words.len().min(from + longest))
                .filter(move |&to| known.contains(&name(from, to)))
        };
        // How many readings the words from each place on have; 2 is several.
        let mut ways = vec![0usize; words.len() + 1];
        ways[words.len()] = 1;
        for from in (0..words.len()).rev() {
            ways[from] = ends(from).map(|to| ways[to]).sum::<usize>().min(2);
        }
        if ways[0] != 1 {
            return None;
        }
        let mut from = 0;
        while from < words.len() {
            let to = ends(from).find(|&to| ways[to] == 1)?;
            read.push(name(from, to));
            from = to;
        }
    }
    Some(read)
}

/// Whether `said` carries a yes mark, which also answers a draft.
pub fn by_mark(said: &str) -> bool {
    marked(said).is_some()
}

/// Whether `said` is a yes mark or a send word alone, which names no session:
/// it lets one draft go, whichever it is, and so only once.
pub fn bare(said: &str) -> bool {
    if let Some(beside) = marked(said) {
        return beside.is_empty() || SEND.contains(&beside.join(" ").as_str());
    }
    let said = said.trim().to_lowercase();
    SEND.contains(&said.trim_end_matches(['.', '!', ' ']))
}

/// What the person's word does to the memory proposals put to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Approve,
    Discard,
}

/// Words that, said alone, approve the proposals put to the person.
const APPROVE: [&str; 12] = [
    "approve",
    "approve all",
    "approve them",
    "approve it",
    "aprova",
    "aprove",
    "aprovo",
    "aprovar",
    "aprova tudo",
    "aprova todos",
    "aprova todas",
    "pode aprovar",
];

/// Words that, said alone, discard them.
const DISCARD: [&str; 11] = [
    "discard",
    "discard all",
    "discard them",
    "discard it",
    "descarta",
    "descarte",
    "descartar",
    "descarta tudo",
    "descarta todos",
    "descarta todas",
    "pode descartar",
];

/// Marks that say no in any language.
const NO_MARKS: [char; 2] = ['❌', '✖'];

/// What the person's whole message says to the proposals put to them: a yes
/// mark or an approve word alone approves, a no mark or a discard word alone
/// discards. A sentence says neither: it waits for their click.
pub fn on_proposals(said: &str) -> Option<Verdict> {
    let said: String = said
        .trim()
        .trim_end_matches(HANGING)
        .to_lowercase()
        .chars()
        .filter(|c| !dresses_a_mark(*c))
        .collect();
    let only = |marks: &[char]| {
        !said.trim().is_empty()
            && said
                .chars()
                .all(|c| marks.contains(&c) || c.is_whitespace())
    };
    if only(&YES_MARKS) {
        return Some(Verdict::Approve);
    }
    if only(&NO_MARKS) {
        return Some(Verdict::Discard);
    }
    let words = spaced(&said);
    if APPROVE.contains(&words.as_str()) {
        Some(Verdict::Approve)
    } else if DISCARD.contains(&words.as_str()) {
        Some(Verdict::Discard)
    } else {
        None
    }
}

/// Whether `words` name the session: whole, or by a part of its name long
/// enough to mean it (`noiseless` for `noiseless-mvp`).
fn names(words: &[&str], session: &str) -> bool {
    let session = session.to_lowercase();
    words.iter().any(|word| {
        *word == session
            || session
                .split('-')
                .filter(|part| part.chars().count() >= 4)
                .any(|part| part == *word)
    })
}

#[cfg(test)]
#[path = "person_tests.rs"]
mod tests;
