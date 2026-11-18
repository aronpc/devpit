//! `devflow_review`: a card's change reviewed by devflow.yaml's reviewers,
//! headless on the person's own CLIs, kept in the card's run, attached, and
//! signed on the card — and a Devflow review's findings in the card's diff,
//! where the person can call one a false positive.
//!
//! The rules are `devflow_engine::review`'s and `findings`'s; this adds the
//! board, git, the CLIs on this machine and the window.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use devflow_engine::config::Agent;
use devflow_engine::evidence::Card;
use devflow_engine::review::{self, Git, Given, Reach};
use devflow_engine::{runs, Config};
use devpit_core::Store;
use devpit_rpc::{CardReview, Declared, Finding, Found, Project, Review, Severity};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::devflow_evidence::{checkout, runs_of, runs_of_card, SIGNED};
use crate::steps::context::{context_of, Checkout};

/// What the agent's wait keeps for attaching and answering once the reviewers stop.
const SPARE: Duration = Duration::from_secs(30);
/// How a Devflow review's run is named among a card's reviews.
const RUN: &str = "devflow:";
/// The most of a file read to count its lines, when a finding cites one.
pub(crate) const MOST_COUNTED: u64 = 8 * 1024 * 1024;

pub(crate) fn respond(
    app: Option<&AppHandle>,
    project: &Project,
    config: &Config,
    cwd: &Path,
    params: &Value,
    profile: Option<&Declared>,
) -> Result<Value, String> {
    let program = program_for(profile);
    let reach = |agent: Agent| reach_as(agent, profile, program.as_deref());
    respond_with(app, project, config, cwd, params, &reach, profile)
}

/// The resolved program `profile` starts, when devpit can spawn it.
pub(crate) fn program_for(profile: Option<&Declared>) -> Option<String> {
    let store = crate::projects::store().ok()?;
    crate::devflow_profile::program_of(&store, profile?)
}

/// The CLI on this machine that runs `agent` as `profile`, or why there is
/// none. `program` is the profile's own, resolved by [`program_for`].
pub(crate) fn reach_as(
    agent: Agent,
    profile: Option<&Declared>,
    program: Option<&str>,
) -> Result<String, String> {
    let known = crate::shell_launch::shell_knows();
    let program = picked(agent, program, &|one| known.contains(one))?;
    if agent == Agent::Claude && crate::devflow::claude_signed_in(profile) == Some(false) {
        return Err("not signed in".to_owned());
    }
    Ok(program.to_owned())
}

/// The program a reviewer on `agent` starts: a Claude profile's own, else
/// the plain CLI when the shell knows it.
pub(crate) fn picked<'a>(
    agent: Agent,
    own: Option<&'a str>,
    knows: &dyn Fn(&str) -> bool,
) -> Result<&'a str, String> {
    let cli = devpit_agentcli::reviewing::Cli::of(agent.word())
        .ok_or("devpit does not run this CLI headless")?;
    match own.filter(|_| agent == Agent::Claude) {
        Some(own) => Ok(own),
        None if knows(cli.program()) => Ok(cli.program()),
        None => Err("not installed".to_owned()),
    }
}

/// A card of this project's board, by id.
fn card_here(store: &Store, project: &Project, id: &str) -> Result<devpit_core::CardRow, String> {
    let on_board = store
        .project_id_of_card(id)
        .map_err(|err| err.to_string())?;
    store
        .card(id)
        .map_err(|err| err.to_string())?
        .filter(|_| on_board.as_deref() == Some(project.id.as_str()))
        .ok_or_else(|| format!("no card `{id}` on this project's board"))
}

type Pinned = (String, String, Vec<String>, String, u32);

/// BASE, HEAD and the diff between them, in the checkout.
fn pinned(root: &Path, base: Option<&str>) -> Result<Pinned, String> {
    let base = devpit_git::review_base(root, base).map_err(|err| err.to_string())?;
    pinned_from(root, base)
}

/// A re-review's: from the HEAD the previous run read, not the merge base.
fn pinned_from(root: &Path, base: String) -> Result<Pinned, String> {
    let said = |err: devpit_git::GitError| err.to_string();
    let head = devpit_git::head_of(root).map_err(said)?;
    let read = devpit_git::review_diff(root, &base, &head).map_err(said)?;
    Ok((base, head, read.files, read.diff, read.commits))
}

/// Keeps where `card` began in the checkout `cwd` is in, when Devflow is set up
/// there: what a later review without a base diffs from.
pub(crate) fn began(project: &Project, cwd: &Path, card: &str) {
    let root = checkout(project, cwd);
    if !root.join(crate::devflow_dialect::file()).is_file() {
        return;
    }
    let (Ok(head), Ok(runs)) = (devpit_git::head_of(&root), runs_of(&project.id, &root)) else {
        return;
    };
    let _ = devflow_engine::started::keep_start(&runs, card, &head);
}

/// The brief of the epic the card was planned under.
fn brief_of(store: &Store, project: &Project, card: &str) -> Option<devflow_engine::brief::Brief> {
    let edges = store.card_edges(&project.id).ok()?;
    let (_, epic) = edges.parents.iter().find(|(child, _)| child == card)?;
    devflow_engine::brief::read(&store.card(epic).ok()??.body)
}

pub(crate) fn respond_with(
    app: Option<&AppHandle>,
    project: &Project,
    config: &Config,
    cwd: &Path,
    params: &Value,
    reach: Reach,
    profile: Option<&Declared>,
) -> Result<Value, String> {
    let asked = review::asked_in(&crate::devflow_dialect::DEVPIT, params)?;
    let store = crate::projects::store().map_err(|err| err.message)?;
    let card = card_here(&store, project, &asked.card)?;
    let root = checkout(project, cwd);
    let runs = runs_of(&project.id, &root)?;
    // The card's own base, else where it was started here, else the default
    // branch: a card worked in the project's folder has only the second.
    let started = devflow_engine::started::start_of(&runs, &card.id);
    let previous = asked
        .previous
        .as_deref()
        .map(|run| review::previous_of_in(&crate::devflow_dialect::DEVPIT, &runs, &card.id, run))
        .transpose()?;
    let pinned = match (&asked.run, &previous) {
        (None, Some(previous)) => Some(pinned_from(&root, previous.head.clone())?),
        (None, None) => Some(pinned(
            &root,
            asked
                .base
                .as_deref()
                .or(card.base_ref.as_deref())
                .or(started.as_deref()),
        )?),
        (Some(_), _) => None,
    };
    let branch = devpit_git::branch_at(&root).unwrap_or_default();
    let mut env = context_of(
        &card,
        Some(Checkout {
            path: &root,
            branch: &branch,
        }),
        Some(&project.root_path),
    )
    .for_a_command()
    .environment();
    env.extend(crate::devflow_profile::env_of(profile));
    devflow_engine::security::with_targets(&mut env, &crate::devflow::targets_in_force(config));
    let secrets = crate::kept_out::what_devpit_gave(&store);
    let redact = |text: &str| crate::kept_out::kept_out(text, &secrets);
    let evidence = crate::devflow_gate::newest(&runs, &card.id, "evidence.json");
    let brief = brief_of(&store, project, &card.id);
    // Asked only where it can answer: else the review falls back as without it.
    let jev = crate::devflow_jev::Jev::of(project, config, &root, Some(&card.id), &[]);
    let judging = jev
        .as_ref()
        .filter(|jev| jev.answers())
        .map(|jev| move |rubric: &str, state: &Value| jev.judge(rubric, state));
    // Personas follow the layers as skills do, read from the project's root.
    let teams = crate::devflow::layered(Path::new(&project.root_path))
        .map(|layered| layered.applied)
        .unwrap_or_default();
    let personal = devpit_core::Store::root()
        .map(|home| devpit_core::home::devflow_reviewers(&home))
        .ok();
    let lines_at = |commit: &str, file: &str| {
        devpit_git::lines_at(&root, commit, file, MOST_COUNTED).map_err(|err| err.to_string())
    };
    // The project's approved lessons join the checklist the change is held to.
    let lessons = crate::project_memory::lessons(Path::new(&project.root_path));
    let body = crate::project_lessons::checklist(&card.body, &lessons);
    let done = review::reviewed(&Given {
        root: &root,
        runs: &runs,
        config,
        asked: &asked,
        card: Card {
            id: &card.id,
            title: &card.title,
            body: &body,
        },
        git: pinned.as_ref().map(|(base, head, files, diff, _)| Git {
            base,
            head,
            files,
            diff,
        }),
        brief: brief.as_ref(),
        evidence: evidence.as_ref().map(|(_, evidence)| evidence),
        env: &env,
        at: SystemTime::now(),
        budget: devpit_agentapi::client::wait_for("devflow_review").saturating_sub(SPARE),
        reach,
        redact: &redact,
        jev: judging.as_ref().map(|judge| judge as review::Judge),
        personas: review::Personas {
            dialect: &crate::devflow_dialect::DEVPIT,
            personal: personal.as_deref(),
            teams: &teams,
        },
        previous: previous.as_ref(),
        lines_at: &lines_at,
        retro: false,
    })?;

    let run = PathBuf::from(&done.folder);
    let artifacts = crate::projects::project_home(&project.id)
        .map_err(|err| err.message)?
        .artifacts();
    let under = format!("{}/{}", runs::attached_under(&card.id), done.stamp);
    let attached = runs::attach(&run, &done.files(), &artifacts, &under);
    let links: Vec<(String, String)> = attached
        .names
        .iter()
        .map(|name| {
            let short = name.strip_prefix(&format!("{under}/")).unwrap_or(name);
            (short.to_owned(), artifacts.join(name).display().to_string())
        })
        .collect();
    let said = review::comment(&done, &links);
    let sayable = crate::cards::sayable(&said).map_err(|err| err.message)?;
    store
        .add_comment(&card.id, SIGNED, sayable)
        .map_err(|err| err.to_string())?;
    if let Some(app) = app {
        let _ = app.emit("board:changed", &project.id);
    }
    let mut answer = review::summary(&done);
    let commits = pinned.as_ref().map_or(0, |pinned| pinned.4);
    let ranged = asked.base.is_some() || previous.is_some();
    if let Some(warning) = review::wide_in(&crate::devflow_dialect::DEVPIT, commits, ranged) {
        answer["warning"] = json!(warning);
    }
    answer["attached"] =
        json!({ "under": under, "files": attached.names.len(), "left_out": attached.left_out });
    answer["decisions"] = crate::devflow_jev::decisions_of(config, jev.as_ref());
    Ok(answer)
}

/// The newest Devflow review of a card, with its run's name.
fn newest_review(store: &Store, card_id: &str) -> Option<(PathBuf, String, Value)> {
    let root = crate::devflow_jev::checkout_of(store, card_id)?;
    let runs = runs_of_card(store, card_id, &root)?;
    let (run, findings) = crate::devflow_gate::newest(&runs, card_id, review::FINDINGS)?;
    Some((root, run, findings))
}

/// A Devflow severity as the card's diff draws it.
fn drawn(severity: &str) -> Severity {
    match severity {
        "critical" | "high" => Severity::Blocking,
        "medium" => Severity::Worth,
        _ => Severity::Noted,
    }
}

/// One finding as the card's diff reads it: where, how bad, and why.
pub(crate) fn as_drawn(finding: &Value) -> Finding {
    let text = |key: &str| finding[key].as_str().unwrap_or_default();
    let mut why = format!(
        "[{}] {} — {}",
        text("axis"),
        text("title"),
        text("description")
    );
    if let Some(agreement) = finding["agreement"].as_str() {
        why.push_str(&format!(" ({agreement})"));
    }
    if let Some(acted) = finding["jev"]["acted"].as_array() {
        let acted: Vec<&str> = acted.iter().filter_map(Value::as_str).collect();
        why.push_str(&format!(" — Jev: {}", acted.join("; ")));
    }
    Finding {
        file: text("file").to_owned(),
        line: finding["line"]
            .as_u64()
            .and_then(|line| u32::try_from(line).ok()),
        severity: drawn(text("severity")),
        why,
    }
}

/// The card's newest Devflow review, as one more review its diff draws;
/// what the person called a false positive is what it shows set aside.
pub(crate) fn card_review(store: &Store, card_id: &str) -> Option<CardReview> {
    let (root, run, kept) = newest_review(store, card_id)?;
    let list = kept["findings"].as_array()?;
    if list.is_empty() {
        return None;
    }
    let review = Review {
        findings: list.iter().map(as_drawn).collect(),
        at_revision: kept["head"].as_str().map(str::to_owned),
        rubric: Some("devflow-review".to_owned()),
    };
    let now = devpit_git::head_of(&root).ok();
    let dismissed = list
        .iter()
        .enumerate()
        .filter(|(_, finding)| !devflow_engine::findings::standing(finding))
        .filter_map(|(at, _)| u32::try_from(at).ok())
        .collect();
    Some(CardReview {
        run_id: format!("{RUN}{card_id}/{run}"),
        found: Found {
            standing: devpit_rpc::standing(&review, now.as_deref()),
            at_revision: review.at_revision.clone(),
            now,
            rubric: review.rubric.clone(),
            findings: review.findings,
        },
        dismissed,
    })
}

/// A review run as the card's diff names a Devflow one: its card and run.
pub(crate) fn devflow_run(run_id: &str) -> Option<(&str, &str)> {
    let (card, run) = run_id.strip_prefix(RUN)?.split_once('/')?;
    runs::plain_card(card).then_some((card, run))
}

/// The person's "not a problem" on one finding of a Devflow review, or
/// taking it back: kept on the finding in findings.json, which the gate
/// then no longer blocks on, beside Jev's word and as a label on its
/// decision. Bringing back one Jev set aside overrules Jev.
pub(crate) fn mark(
    store: &Store,
    run_id: &str,
    at: u32,
    false_positive: bool,
) -> Result<(), String> {
    let (card_id, run) = devflow_run(run_id).ok_or("not a Devflow review")?;
    let root = crate::devflow_jev::checkout_of(store, card_id).ok_or("the card has no checkout")?;
    let path = runs_of_card(store, card_id, &root)
        .and_then(|runs| runs.found(card_id, Some(run), review::FINDINGS))
        .ok_or_else(|| format!("run {run} has no {}", review::FINDINGS))?;
    let mut kept: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or_else(|| format!("{} is not there", path.display()))?;
    let finding = kept["findings"]
        .get_mut(at as usize)
        .filter(|finding| finding.is_object())
        .ok_or("no such finding")?;
    devflow_engine::findings::person_said(finding, false_positive);
    crate::devflow_jev::finding_marked(store, finding, false_positive);
    let config = crate::devflow::read(&root);
    if let devflow_engine::State::Valid(_, config) = config {
        let list = kept["findings"].as_array().cloned().unwrap_or_default();
        kept["verdict"] = json!(devflow_engine::findings::verdict(
            &list,
            &config.review.block_on,
            config.review.verify
        ));
    }
    let text = serde_json::to_string_pretty(&kept).unwrap_or_default();
    std::fs::write(&path, text).map_err(|err| format!("{}: {err}", path.display()))
}

#[cfg(test)]
#[path = "devflow_review_tests.rs"]
pub(crate) mod tests;
