//! What the project agent proposed for its memory, waiting for the person's
//! approve, edit or discard, and the decisions that name their cards.

use serde::{Deserialize, Serialize};
use specta::Type;

/// One proposal: a lesson or a decision a closed card left, or the update or
/// removal of a stale entry a re-check found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LessonProposal {
    /// Its name, from its line: what the orchestrator approves it by.
    pub id: String,
    pub project_id: String,
    pub project: String,
    /// The day it was proposed, as YYYY-MM-DD.
    pub day: String,
    /// The card it came from, as `#12`, or the stale entry's own source.
    pub source: String,
    /// The memory file it goes to: `lessons.md`, `decisions.md` or `facts.md`.
    pub file: String,
    /// What it says; empty when it removes the stale entry it replaces.
    pub text: String,
    /// The stale entry it updates or removes.
    pub replaces: Option<String>,
    /// Proposed by the agent's own read of the project, which "Approve all"
    /// takes at once.
    pub onboarding: bool,
    /// Who proposed it, in a few words: "a session on card #360", "onboarding".
    pub by: String,
    /// Its one plain line, shown first; `text` is behind "details".
    pub summary: String,
}

/// A decision of a project's memory, with the cards that made it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDecision {
    pub project_id: String,
    pub day: String,
    /// The cards it names, as `#12`.
    pub cards: Vec<String>,
    /// Its whole source: the cards and the files it rests on.
    pub source: String,
    pub text: String,
}

/// A list, so tomorrow's field has somewhere to go.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LessonProposals {
    pub proposals: Vec<LessonProposal>,
    pub decisions: Vec<MemoryDecision>,
}

/// What the project agent answered in the "Ask the project" chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentAnswer {
    pub answer: String,
    /// Whether this question started the agent's process.
    pub woke: bool,
    /// The project's files the answer names, from the project's root.
    pub cited: Vec<String>,
}

/// One entry of a project agent's memory, or one waiting for the person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    /// The day it was written or proposed, as YYYY-MM-DD.
    pub day: String,
    /// Its whole source, as the file keeps it.
    pub source: String,
    pub text: String,
    /// The project's files it rests on, from the project's root.
    pub files: Vec<String>,
    /// The cards it came from, as `#12`.
    pub cards: Vec<String>,
    /// A fact that entered without the person, on Labs' switch: it can be undone.
    pub auto: bool,
}

/// A project agent's memory as its page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemory {
    pub project_id: String,
    /// Whether the project has its memory folder yet.
    pub kept: bool,
    pub facts: Vec<MemoryEntry>,
    pub lessons: Vec<MemoryEntry>,
    pub decisions: Vec<MemoryEntry>,
    /// What waits for the person, each with the files it rests on.
    pub proposals: Vec<MemoryProposal>,
    /// Whether the agent is reading the project to learn it now.
    pub learning: bool,
    /// The day it last learned the project on its own, as YYYY-MM-DD.
    pub learned: Option<String>,
}

/// A proposal, with what its page shows beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryProposal {
    pub proposal: LessonProposal,
    pub files: Vec<String>,
    pub cards: Vec<String>,
}
