//! What a card's Devflow runs left, as the card draws it: the newest gate,
//! the newest review's reviewers and the newest run's files. Read from the
//! copies attached among the project's artifacts, so it does not matter which
//! checkout the card ran in.

use std::path::Path;

use devflow_engine::runs;
use devpit_rpc::{DevflowCardEvidence, DevflowExploit, DevflowGated, DevflowReviewed, RpcError};
use serde_json::Value;

/// The most of one JSON file read: gate.json and review.json are small.
const MOST_READ: u64 = 1024 * 1024;
/// The most files of one run listed.
const MOST_LISTED: usize = 200;

fn json_at(path: &Path) -> Option<Value> {
    // A link is never followed out of devpit's own folder.
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let raw = devpit_core::data_files::read_within(file, MOST_READ).ok()??;
    serde_json::from_slice(&raw).ok()
}

fn count(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|count| u32::try_from(count).ok())
        .unwrap_or_default()
}

fn words(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// The newest gate.json's verdict.
fn gated(value: &Value) -> DevflowGated {
    DevflowGated {
        passed: value["passed"] == true,
        run: value["run"].as_str().map(str::to_owned),
        at: value["at"].as_str().unwrap_or_default().to_owned(),
        with_evidence: count(&value["criteria_with_evidence"]),
        criteria: count(&value["criteria"]),
        missing: words(&value["missing"]),
    }
}

/// Each reviewer review.json names.
fn reviewed(value: &Value) -> Vec<DevflowReviewed> {
    value["ran"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|ran| DevflowReviewed {
            agent: ran["agent"].as_str().unwrap_or_default().to_owned(),
            persona: ran["persona"].as_str().unwrap_or_default().to_owned(),
            status: if ran["file"].is_string() {
                "answered"
            } else {
                "skipped"
            }
            .to_owned(),
            findings: count(&ran["findings"]),
            duration_ms: ran["duration_ms"].as_f64(),
            cost_usd: ran["cost_usd"].as_f64(),
            reason: ran["reason"].as_str().map(str::to_owned),
        })
        .collect()
}

/// Each exploit test findings.json keeps, the gate's regression tests.
fn exploits(findings: &Value) -> Vec<DevflowExploit> {
    let text = |value: &Value| value.as_str().map(str::to_owned);
    findings["exploits"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|one| {
            Some(DevflowExploit {
                test: one["test"]
                    .as_str()
                    .filter(|test| !test.is_empty())?
                    .to_owned(),
                file: text(&one["file"]),
                line: one["line"]
                    .as_u64()
                    .and_then(|line| u32::try_from(line).ok()),
                title: text(&one["title"]),
            })
        })
        .collect()
}

/// The files of one run, relative to it, looked at no deeper than the
/// screenshots and video folders a run keeps.
fn files_of(run: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut folders = vec![(run.to_path_buf(), String::new(), 0)];
    while let Some((folder, under, depth)) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let (Ok(kind), Ok(name)) = (entry.file_type(), entry.file_name().into_string()) else {
                continue;
            };
            let relative = if under.is_empty() {
                name
            } else {
                format!("{under}/{name}")
            };
            if kind.is_dir() && depth < 2 {
                folders.push((entry.path(), relative, depth + 1));
            } else if kind.is_file() && found.len() < MOST_LISTED {
                found.push(relative);
            }
        }
    }
    found.sort();
    found
}

/// What `card` left under `artifacts`, the project's artifacts folder.
pub(crate) fn evidence_in(artifacts: &Path, card: &str) -> DevflowCardEvidence {
    let mut said = DevflowCardEvidence {
        card_id: card.to_owned(),
        gate: None,
        reviewers: Vec::new(),
        verdict: None,
        folder: artifacts.display().to_string(),
        artifacts: Vec::new(),
        decisions: None,
        exploits: Vec::new(),
    };
    if !runs::plain_card(card) {
        return said;
    }
    let under = runs::attached_under(card);
    let folder = artifacts.join(&under);
    if let Some(run) = runs::newest_with(&folder, "gate.json") {
        said.gate = json_at(&folder.join(&run).join("gate.json")).map(|value| gated(&value));
    }
    if let Some(run) = runs::newest_with(&folder, "review.json") {
        if let Some(value) = json_at(&folder.join(&run).join("review.json")) {
            said.reviewers = reviewed(&value);
        }
        if let Some(findings) = json_at(&folder.join(&run).join("findings.json")) {
            said.verdict = findings["verdict"].as_str().map(str::to_owned);
            said.exploits = exploits(&findings);
        }
    }
    let newest = ["gate.json", "review.json", "evidence.json"]
        .iter()
        .filter_map(|file| runs::newest_with(&folder, file))
        .max_by(|a, b| runs::seconds_of(a).cmp(&runs::seconds_of(b)).then(a.cmp(b)));
    if let Some(run) = newest {
        said.artifacts = files_of(&folder.join(&run))
            .into_iter()
            .map(|file| format!("{under}/{run}/{file}"))
            .collect();
    }
    said
}

/// `devflow.card_evidence` — the card's newest gate, reviewers and files.
#[tauri::command]
#[specta::specta]
pub async fn devflow_card_evidence(
    project_id: String,
    card_id: String,
) -> Result<DevflowCardEvidence, RpcError> {
    crate::off_main::blocking(move || {
        let home = crate::projects::project_home(&project_id)?;
        let mut said = evidence_in(&home.artifacts(), &card_id);
        let store = crate::projects::store()?;
        said.decisions = store.project(&project_id)?.map(|project| {
            crate::devflow_jev::standing(&store, &project_id, Path::new(&project.root_path))
        });
        Ok(said)
    })
    .await
}

#[cfg(test)]
#[path = "devflow_card_tests.rs"]
mod tests;
