//! A tool's answer as the agent reads it by default: compact, the summary
//! and where the rest is, never past about 2k tokens. Every turn after reads
//! it again, so what is not needed costs once per turn, not once.
//!
//! `full: true` on any tool gives the app's answer whole and pretty-printed,
//! as every answer was before, for a client that reads those shapes.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{json, Value};

/// The argument that asks for the whole answer.
pub(crate) const FULL: &str = "full";
/// `devpit_card`'s argument that asks for every comment whole.
pub(crate) const COMMENTS: &str = "comments";

/// The most characters a default answer takes: 2k tokens of JSON full of
/// ids and paths, which run about two characters a token.
pub(crate) const BUDGET: usize = 4000;

/// How many of a card's newest comments its summary lists.
const NEWEST_COMMENTS: usize = 3;
/// The most of a comment the card's summary shows.
const COMMENT_HEAD: usize = 200;
/// The most of the latest handoff the card's summary shows.
const HANDOFF: usize = 2000;
/// What opens a handoff note (`apps/desktop/src/handoff.rs`).
const HANDOFF_HEADER: &str = "**Handoff**";

/// The tools whose answer is a card after a comment was written on it.
const COMMENTED: [&str; 4] = [
    "devpit_comment",
    "devpit_comment_edit",
    "devpit_comment_delete",
    "devpit_handoff",
];

/// How many cards of each lane `devpit_board` asks for when the call says
/// neither that nor `full` (`apps/desktop/src/board_page.rs`).
const BOARD_PAGE: u64 = 20;

/// The board's arguments that already say which page.
const PAGING: [&str; 6] = ["limit", "cursor", "column", "epic", "text", "done"];

/// The arguments the app is asked with: a board a page at a time unless the
/// call asked for it whole or said which page.
pub(crate) fn asked(tool: &str, mut arguments: Value) -> Value {
    if tool == "devpit_board"
        && !wants_full(&arguments)
        && PAGING
            .iter()
            .all(|name| arguments.get(name).is_none_or(Value::is_null))
    {
        if let Some(fields) = arguments.as_object_mut() {
            fields.insert("limit".to_owned(), json!(BOARD_PAGE));
        }
    }
    arguments
}

/// What the project agent answers, never cut.
const WHOLE: [&str; 3] = [
    "devpit_project_agent_consult",
    "devpit_project_agent_ask",
    "devpit_project_agent_brief",
];

/// Whether the call asked for the whole answer.
pub(crate) fn wants_full(arguments: &Value) -> bool {
    arguments.get(FULL).and_then(Value::as_bool) == Some(true)
}

/// The text a tool answers with: the whole answer when asked, else its
/// summary, compact and within [`BUDGET`].
pub(crate) fn said(tool: &str, arguments: &Value, answer: Value) -> String {
    // Cut, it would be asked again and run the agent's whole turn again; and
    // a brief is passed whole as a session's prompt.
    if WHOLE.contains(&tool) {
        return answer.to_string();
    }
    if wants_full(arguments) {
        return serde_json::to_string_pretty(&answer).unwrap_or_default();
    }
    let mut lean = shaped(tool, arguments, answer);
    pruned(&mut lean);
    fitted(&mut lean);
    lean.to_string()
}

/// The summary of `answer`, by what `tool` answers.
fn shaped(tool: &str, arguments: &Value, answer: Value) -> Value {
    match tool {
        _ if COMMENTED.contains(&tool) => commented(tool, &answer, arguments),
        "devpit_card" => card(
            answer,
            arguments.get(COMMENTS).and_then(Value::as_bool) == Some(true),
        ),
        "devpit_board" => board(answer),
        "devpit_devflow_evidence" => evidence(answer),
        "devpit_devflow_review" => review(answer),
        "devpit_devflow_status" => without(answer, &["yaml"]),
        _ => answer,
    }
}

/// After a comment: which card, the comment written (or the newest) and how
/// many it has.
fn commented(tool: &str, detail: &Value, arguments: &Value) -> Value {
    let comments = detail["comments"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut answer = json!({
        "card": detail["card"]["id"],
        "comments": comments.len(),
        "newest": comments.last().map(|one| &one["id"]),
    });
    match tool {
        "devpit_comment_edit" => answer["edited"] = arguments["commentId"].clone(),
        "devpit_comment_delete" => answer["deleted"] = arguments["commentId"].clone(),
        _ => {}
    }
    answer
}

/// A card without its comments whole: how many, the newest few cut short,
/// and its latest handoff, which is where its work stands.
fn card(mut detail: Value, every_comment: bool) -> Value {
    if let Some(card) = detail["card"].as_object_mut() {
        card.remove("position");
        // The count is said beside the comments below.
        card.remove("comments");
    }
    if every_comment {
        return detail;
    }
    let comments = detail["comments"].as_array().cloned().unwrap_or_default();
    let handoff = comments.iter().rposition(|one| {
        one["body"]
            .as_str()
            .is_some_and(|body| body.starts_with(HANDOFF_HEADER))
    });
    let newest: Vec<Value> = comments
        .iter()
        .enumerate()
        .rev()
        .filter(|(at, _)| Some(*at) != handoff)
        .take(NEWEST_COMMENTS)
        .map(|(_, one)| {
            json!({
                "id": one["id"],
                "author": one["author"],
                "createdAt": one["createdAt"],
                "head": headed(one["body"].as_str().unwrap_or_default(), COMMENT_HEAD),
            })
        })
        .collect();
    detail["comments"] = json!({
        "total": comments.len(),
        "newest": newest,
        "handoff": handoff.map(|at| json!({
            "id": comments[at]["id"],
            "createdAt": comments[at]["createdAt"],
            "text": headed(comments[at]["body"].as_str().unwrap_or_default(), HANDOFF),
        })),
        "more": format!("{COMMENTS}: true for every comment whole"),
    });
    detail
}

/// Each column with its cards, one short row each. The app answers a page
/// as `{ columns }`, or the whole board as the bare list an app before pages
/// gave; either way the summary is an object.
fn board(answer: Value) -> Value {
    let mut answer = match answer {
        Value::Array(columns) => json!({ "columns": columns }),
        paged => paged,
    };
    if let Some(columns) = answer["columns"].as_array_mut() {
        for column in columns.iter_mut() {
            if let Some(cards) = column["cards"].as_array_mut() {
                for card in cards.iter_mut() {
                    *card = board_card(std::mem::take(card));
                }
            }
            if column["runsAStep"] == json!(false) {
                if let Some(column) = column.as_object_mut() {
                    column.remove("runsAStep");
                }
            }
        }
    }
    fewer(&mut answer);
    if let Some(columns) = answer["columns"].as_array_mut() {
        for column in columns.iter_mut().filter_map(Value::as_object_mut) {
            column.remove("reading");
        }
        for card in columns
            .iter_mut()
            .filter_map(|column| column["cards"].as_array_mut())
            .flatten()
        {
            if let Some(card) = card.as_object_mut() {
                card.remove("position");
            }
        }
    }
    answer
}

/// A page wider than [`BUDGET`] keeps fewer cards in each lane, and each
/// lane cut shorter names its last card kept and the first left out as
/// `nextCursor`, so reading on skips nothing. Only a page says each
/// card's place; the whole board is left to [`fitted`].
fn fewer(answer: &mut Value) {
    let Some(columns) = answer["columns"].as_array_mut() else {
        return;
    };
    let cards = |column: &Value| column["cards"].as_array().map_or(0, Vec::len);
    let placed = columns
        .iter()
        .filter_map(|column| column["cards"].as_array())
        .flatten()
        .all(|card| card["position"].is_i64());
    if !placed {
        return;
    }
    let mut most = columns.iter().map(cards).max().unwrap_or(0);
    while most > 1 && json!(columns).to_string().len() > BUDGET {
        most = (most * 3 / 4).max(1);
        for column in columns.iter_mut().filter(|column| cards(column) > most) {
            let lane = column["id"].as_str().unwrap_or_default().to_owned();
            let reading = column["reading"].as_str().map(str::to_owned);
            let Some(kept) = column["cards"].as_array_mut() else {
                continue;
            };
            let (last, next) = (&kept[most - 1], &kept[most]);
            // The app's cursor (`board_page::cursor`): the last card kept, the
            // first left out and the reading they belong to.
            let mut cursor = format!(
                "{lane}:{}:{}:{}:{}",
                last["position"],
                last["id"].as_str().unwrap_or_default(),
                next["position"],
                next["id"].as_str().unwrap_or_default()
            );
            if let Some(reading) = reading.as_deref() {
                cursor = format!("{cursor}:{reading}");
            }
            kept.truncate(most);
            column["nextCursor"] = json!(cursor);
        }
    }
}

fn board_card(mut card: Value) -> Value {
    if let Some(fields) = card.as_object_mut() {
        fields.remove("comments");
        if let Some(Value::Array(by)) = fields.get_mut("blockedBy") {
            for one in by.iter_mut() {
                *one = one["id"].clone();
            }
        }
        if let Some(criteria) = fields
            .get("criteria")
            .filter(|criteria| criteria.is_object())
        {
            let ticked = format!("{}/{}", criteria["done"], criteria["total"]);
            fields.insert("criteria".to_owned(), json!(ticked));
        }
    }
    card
}

/// Each check's result and counts, and the criteria not green in full: the
/// green ones are `criteria_green`'s count, the logs and reports under `run`.
fn evidence(mut answer: Value) -> Value {
    if let Some(checks) = answer["checks"].as_array_mut() {
        for check in checks.iter_mut() {
            *check = without(std::mem::take(check), &["log", "reports"]);
        }
    }
    if let Some(criteria) = answer["criteria"].as_array_mut() {
        criteria.retain(|criterion| criterion["green"] != json!(true));
    }
    without(answer, &["attached"])
}

/// The severities a review's summary counts instead of listing: they never
/// hold a card, and `findings_file` has them whole.
const COUNTED: [&str; 2] = ["low", "info"];

/// The verdict, the findings that can hold a card and how many others, and
/// each reviewer's result; the diff's ends and the costs are in
/// the run's folder.
fn review(mut answer: Value) -> Value {
    if let Some(reviewers) = answer["reviewers"].as_array_mut() {
        for reviewer in reviewers.iter_mut() {
            *reviewer = without(std::mem::take(reviewer), &["cost_usd", "duration_ms", "by"]);
        }
    }
    if let Some(findings) = answer["findings"].as_array_mut() {
        let mut counted = serde_json::Map::new();
        findings.retain(|finding| {
            let severity = finding
                .as_str()
                .and_then(|said| said.split(' ').next())
                .unwrap_or_default();
            let count = COUNTED.contains(&severity);
            if count {
                let seen = counted.get(severity).and_then(Value::as_u64).unwrap_or(0);
                counted.insert(severity.to_owned(), json!(seen + 1));
            }
            !count
        });
        if !counted.is_empty() {
            answer["findings_counted"] = Value::Object(counted);
        }
    }
    without(answer, &["attached", "base", "head", "consolidation"])
}

fn without(mut value: Value, fields: &[&str]) -> Value {
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            object.remove(*field);
        }
    }
    value
}

/// The first `most` characters of `text`, marked when cut.
fn headed(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

/// Without the fields that say nothing: null, or an empty list or object.
fn pruned(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.retain(|_, field| !says_nothing(field));
            fields.values_mut().for_each(pruned);
        }
        Value::Array(items) => items.iter_mut().for_each(pruned),
        _ => {}
    }
}

fn says_nothing(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(items) => items.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        _ => false,
    }
}

/// The longest strings and lists, cut until the answer is within [`BUDGET`];
/// what was cut says so, and `full: true` has it whole.
fn fitted(value: &mut Value) {
    const STEPS: [(usize, usize); 4] = [(1200, 30), (500, 15), (200, 8), (100, 4)];
    if value.to_string().len() <= BUDGET {
        return;
    }
    let whole = std::mem::take(value);
    // Each step cuts the whole answer afresh, so a count of what went is right.
    for (longest, most) in STEPS {
        *value = whole.clone();
        cut(value, longest, most);
        if let Some(fields) = value.as_object_mut() {
            fields.insert(
                "cut".to_owned(),
                json!("cut to fit: full: true for the whole answer"),
            );
        }
        if value.to_string().len() <= BUDGET {
            return;
        }
    }
    // Still too wide: the biggest fields go, each saying it went.
    while value.to_string().len() > BUDGET {
        let Some(fields) = value.as_object_mut() else {
            return;
        };
        let biggest = fields
            .iter()
            .filter(|(name, field)| *name != "cut" && !field.is_string())
            .max_by_key(|(_, field)| field.to_string().len())
            .map(|(name, _)| name.clone());
        let Some(biggest) = biggest else {
            return;
        };
        fields.insert(biggest, json!("left out to fit: full: true has it"));
    }
}

fn cut(value: &mut Value, longest: usize, most: usize) {
    match value {
        Value::String(text) => {
            if text.chars().count() > longest {
                *text = headed(text, longest);
            }
        }
        Value::Array(items) => {
            let more = items.len().saturating_sub(most);
            items.truncate(most);
            items.iter_mut().for_each(|item| cut(item, longest, most));
            if more > 0 {
                items.push(json!(format!("…and {more} more")));
            }
        }
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|field| cut(field, longest, most)),
        _ => {}
    }
}

/// The skills already given in this session, by name and part, with the
/// digest each had: the same skill again is a pointer to that copy.
#[derive(Default)]
pub(crate) struct Given(Mutex<HashMap<String, String>>);

impl Given {
    /// Whether this skill, at this digest, was given already; remembers it.
    pub(crate) fn again(&self, skill: &Value) -> bool {
        let Some(digest) = skill["skill"]["resources"][0]["digest"].as_str() else {
            return false;
        };
        let key = format!(
            "{}/{}",
            skill["name"].as_str().unwrap_or_default(),
            skill["part"].as_str().unwrap_or_default()
        );
        let Ok(mut given) = self.0.lock() else {
            return false;
        };
        given.insert(key, digest.to_owned()).as_deref() == Some(digest)
    }
}

/// What the skill tool says for a skill given earlier and unchanged.
pub(crate) fn given_before(skill: &Value) -> String {
    let name = skill["name"].as_str().unwrap_or("This skill");
    let name = match skill["part"].as_str() {
        Some(part) => format!("{name}'s part {part}"),
        None => name.to_owned(),
    };
    format!("{name} was given earlier in this session and has not changed since: follow that copy. Without it in your context (a subagent, or after a compaction), call again with {FULL}: true.")
}

/// The tools whose summary leaves out enough to offer `full` for.
const SHAPED: [&str; 9] = [
    "devpit_card",
    "devpit_board",
    "devpit_comment",
    "devpit_handoff",
    "devpit_devflow_evidence",
    "devpit_devflow_review",
    "devpit_devflow_status",
    "devpit_devflow_plan",
    crate::skills::TOOL,
];

/// `input` with the arguments that ask for more, for a tool [`said`] shapes.
pub(crate) fn offered(tool: &str, input: &mut Value) {
    if !SHAPED.contains(&tool) {
        return;
    }
    let Some(properties) = input.get_mut("properties").and_then(Value::as_object_mut) else {
        return;
    };
    let mut add = |name: &str, description: &str| {
        properties.insert(
            name.to_owned(),
            json!({ "type": "boolean", "description": description }),
        );
    };
    if tool == crate::skills::TOOL {
        add(
            FULL,
            "Send the skill again even when this session was given it already.",
        );
        return;
    }
    add(
        FULL,
        "The whole answer, uncut and pretty-printed. Leave out for its summary.",
    );
    if tool == "devpit_card" {
        add(COMMENTS, "Every comment whole. Without it: the count, the newest few cut short and the latest handoff.");
    }
}

#[cfg(test)]
#[path = "lean_tests.rs"]
mod tests;
