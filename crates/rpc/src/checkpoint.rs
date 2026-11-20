//! Three questions about a run, kept apart because they have three answers.
//!
//! *Did the process end well?* is [`RunState`], which already exists and is
//! not touched here. *Did the thing it was checking pass?* is [`Result`]. *Is
//! that still true of the code in front of you?* is [`Validity`].
//!
//! Collapsing them is the mistake this module exists to prevent. Exit code
//! zero proves the **command** succeeded, not that the tests passed: a runner
//! that could not find a test file exits zero and runs nothing. A run nobody
//! reaped is `Lost`, and `Lost` is never `Passed` however green the last line
//! looked. And a result from two commits ago is not wrong — it is about other
//! code, which is a different thing from being wrong and reads differently on
//! a screen.
//!
//! Everything here is a function of its arguments, so the screen and the
//! backend cannot disagree about what a row means.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::board::RunState;

/// What a run says about the thing it was checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// A report was read and everything in it passed.
    Passed,
    /// A report was read and something in it failed.
    Failed,
    /// The check never happened: the process never ran, or a person stopped it.
    NotRun,
    /// It ran and left nothing that answers the question. The commonest one,
    /// and the one a screen must never draw as green.
    Inconclusive,
}

/// Whether a verdict is still about the code in front of the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Validity {
    /// The revision and the local changes are the ones the run saw.
    Current,
    /// The code moved under it. Not wrong — about other code.
    Stale,
    /// Nothing recorded what the run saw, so nothing can say. Every run from
    /// before migration 017 is this.
    Unknown,
}

/// What a run left that can be read as a result.
///
/// `None` is a run that produced no report devpit recognises — which is most
/// of them, and is the case the rest of this module refuses to round up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub passed: u32,
    pub failed: u32,
}

/// The verdict a run earns, from how its process ended and what it reported.
///
/// The exit code is deliberately absent. It answers whether the command
/// succeeded, which [`RunState`] already carries, and a rule that reads it
/// here would be the rule that turns "the runner found no tests" into green.
pub fn verdict(state: RunState, report: Option<Report>) -> Verdict {
    match state {
        // Still going: it has not said anything yet, and a spinner is not a
        // result.
        RunState::Running => Verdict::NotRun,
        // A person stopping work is not the work failing, and it is not the
        // work passing either.
        RunState::Cancelled => Verdict::NotRun,
        // Waiting out a pause is not a result either: nothing ran, and a
        // verdict over a pause would approve work that never happened.
        RunState::Paused => Verdict::NotRun,
        // The process vanished. Whatever its last line said, nobody watched it
        // end, so nobody knows.
        RunState::Lost => Verdict::Inconclusive,
        RunState::Failed => match report {
            Some(read) if read.failed > 0 => Verdict::Failed,
            // The command failed and no report says which check did. That is
            // a build that did not compile, or a runner that is not installed:
            // something went wrong and this does not claim to know what.
            _ => Verdict::Inconclusive,
        },
        RunState::Ok => match report {
            Some(read) if read.failed > 0 => Verdict::Failed,
            // Exit zero and a report with nothing in it: the runner matched no
            // tests. Passing zero checks is not passing.
            Some(read) if read.passed == 0 => Verdict::Inconclusive,
            Some(_) => Verdict::Passed,
            // The one that matters. Exit zero with no report devpit can read
            // is *not* `Passed`, however much a green tick would be nicer.
            None => Verdict::Inconclusive,
        },
    }
}

/// What the run saw, and what is there now.
///
/// A revision alone is not enough: uncommitted work is most of what a person
/// is looking at, so the fingerprint carries both. Two identical fingerprints
/// mean the tracked state matches — they do **not** prove the run was isolated
/// from anything else on the machine, and no screen built on this may say they
/// do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    /// The commit, when a repository could answer.
    pub revision: Option<String>,
    /// What `git status --porcelain` said, hashed. `None` when nothing asked.
    pub changes: Option<String>,
}

impl Fingerprint {
    /// Whether anything was recorded at all.
    pub fn is_known(&self) -> bool {
        self.revision.is_some() || self.changes.is_some()
    }
}

/// Whether a verdict is still about the code in front of the person.
///
/// `Unknown` unless **both** sides said something: a run that recorded no
/// revision cannot be current, and a working tree nobody looked at cannot make
/// one stale. Half an answer is not an answer.
pub fn validity(saw: &Fingerprint, now: &Fingerprint) -> Validity {
    if !saw.is_known() || !now.is_known() {
        return Validity::Unknown;
    }
    if saw.revision != now.revision {
        return Validity::Stale;
    }
    match (&saw.changes, &now.changes) {
        (Some(before), Some(after)) if before != after => Validity::Stale,
        // The revision matches and nobody looked at the working tree. Saying
        // `Current` there would be claiming the tree is clean because nothing
        // checked it.
        (Some(_), Some(_)) => Validity::Current,
        _ => Validity::Unknown,
    }
}

/// Everything the screen needs about one run, in one answer.
///
/// The three states are separate fields and never one: a screen that had to
/// derive `Verdict` from `RunState` would be the screen that draws exit code
/// zero as a green tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    pub run_id: String,
    pub state: RunState,
    pub verdict: Verdict,
    pub validity: Validity,
    /// What the run ran, or nothing for a row that never said. The screen
    /// showing nothing here says "unknown", not a blank that reads like none.
    pub ran: Option<WhatRan>,
    /// The evidence's shape, when a run left any. The payload itself is asked
    /// for separately: a list of runs is not a place to send megabytes.
    pub evidence_version: Option<f64>,
    /// Who asked and what did the work. Two answers, because Claude can ask
    /// for a test the local runner executes and Codex reviews.
    pub whose: Whose,
}

/// Who asked for a run and what carried it out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Whose {
    /// `board`, `card`, `checkpoint`, `chain`, or absent for a run that never
    /// said — including every one from before this was recorded.
    pub asked: Option<String>,
    /// The surface that asked, as a **reference**. The screen shows it and
    /// never looks it up: a terminal that has closed is unavailable, and
    /// finding another that shares its name would point at work that is not
    /// this run's.
    pub asked_from: Option<String>,
    /// `process` or `agent`, or absent for a run that never said.
    pub carried: Option<String>,
    /// The profile an agent session ran under, as it stood then.
    pub profile: Option<String>,
}

/// The circumstances of a run, as the screen reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WhatRan {
    pub command: Option<String>,
    pub in_directory: Option<String>,
    /// The **names** of what devpit put in the environment, never the values.
    pub declared_env: Vec<String>,
    pub base_revision: Option<String>,
    pub head_revision: Option<String>,
    pub in_a_worktree: Option<bool>,
}

/// What running a step on a card would do, said before anybody runs it.
///
/// A check is somebody else's command against your machine and your checkout.
/// A button that says only "Run" asks for trust the screen has not earned:
/// the same word can be `pnpm test` and can be a deploy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WouldRun {
    pub step_id: String,
    pub step_name: String,
    /// Marked as having no undo. The screen asks twice for one of these.
    pub irreversible: bool,
    /// The command, for a step that has one. An agent or a session step has
    /// none, and saying nothing is better than an empty line that reads like a
    /// command that does nothing.
    pub command: Option<String>,
    /// Where it **would** run. Absent when the checkout does not exist yet:
    /// answering a question must not make one.
    pub in_directory: Option<String>,
    pub in_a_worktree: bool,
    /// The **names** of what devpit would declare, never the values.
    pub declared_env: Vec<String>,
    pub timeout_seconds: Option<f64>,
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
