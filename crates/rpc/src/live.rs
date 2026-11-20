//! The agent sessions running on this machine for one account, as the CLI
//! itself lists them — what an orchestrator can message, and what it shows.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    /// The name another session messages it by.
    pub name: String,
    /// Its process: what tells apart two sessions that share a name.
    pub pid: i32,
    /// A background session's job, which a terminal attaches to watch it.
    pub job: Option<String>,
    /// The CLI's own word: `busy` or `idle`.
    pub status: String,
    /// `interactive` or `background`.
    pub kind: String,
    /// Local only: the folder it runs in.
    pub cwd: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    /// The card whose checkout it runs in, when it runs in one.
    pub card_id: Option<String>,
    /// Milliseconds since the epoch, when its status last changed.
    pub since: Option<f64>,
    /// Whether it runs in one of devpit's own terminals, where a reply can be
    /// typed for the person.
    pub in_devpit: bool,
    /// The question it is stopped on, read off its terminal, when it is in
    /// one of devpit's and stopped on one.
    pub waiting: Option<PendingPrompt>,
    /// The devpit terminal it runs in, to open right here: present exactly
    /// when `in_devpit` is.
    pub pane: Option<LivePane>,
    /// The CLI's own id for the conversation, which its hooks carry too.
    pub session_id: Option<String>,
    /// What it is on while it works — `Edit invoice.ts` — as its hooks said.
    pub step: Option<String>,
    /// A reply the orchestrator drafted for the person to send it as theirs.
    /// Never sent by the orchestrator: only the person's click types it.
    pub draft: Option<String>,
    /// Claude Code's Remote Control for it, as its listing says.
    pub remote: RemoteState,
}

/// A devpit terminal, as a tab attaches to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LivePane {
    pub project_id: String,
    pub pane_id: String,
}

/// A list, so tomorrow's field has somewhere to go.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveSessions {
    pub sessions: Vec<LiveSession>,
}

/// A session of the account that ran and ended, as devpit saw it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EndedSession {
    pub session_id: String,
    pub name: String,
    pub cwd: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub card_id: Option<String>,
    /// When it was first and last seen running, in ms since the epoch.
    pub started_at: f64,
    pub ended_at: f64,
    /// `orchestrator` or `person` when one of them stopped it; otherwise it
    /// ended by itself, or with the machine.
    pub ended_by: Option<String>,
    /// `busy` or `idle`, as it was last seen.
    pub last_status: Option<String>,
    /// Files changed and not committed in its folder now, when it is a repository.
    pub dirty: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EndedSessions {
    pub sessions: Vec<EndedSession>,
}

/// A session devpit saw working on a card, running or ended.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionOnCard {
    /// The account it ran as, which resumes it.
    pub profile_id: String,
    pub session_id: String,
    pub name: String,
    /// In ms since the epoch.
    pub started_at: f64,
    /// `None` while it runs.
    pub ended_at: Option<f64>,
    /// As in [`EndedSession`].
    pub ended_by: Option<String>,
    pub last_status: Option<String>,
    pub dirty: Option<u32>,
    /// The devpit terminal a running one is in.
    pub pane: Option<LivePane>,
    pub remote: RemoteState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionsOnCard {
    pub sessions: Vec<SessionOnCard>,
}

/// What a session said last, read from its transcript.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionTold {
    /// Its latest replies, oldest first.
    pub replies: Vec<String>,
    /// The latest prompts it was given, oldest first.
    pub prompts: Vec<String>,
}

/// Whether a conversation can be reached by Remote Control, and where.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub on: bool,
    /// The session's page on claude.ai, once known. Absent while a chat is on
    /// but not connected yet — its process starts with the next message.
    pub url: Option<String>,
}

/// A question a session is stopped on, as its screen shows it: the words
/// above the choices, the choices, and which one its cursor is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingPrompt {
    pub question: String,
    pub options: Vec<PromptOption>,
    /// Zero-based: the option the `❯` is on.
    pub cursor: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromptOption {
    pub label: String,
    /// The quieter line under it, when it has one.
    pub hint: Option<String>,
}

/// A change to the projects an orchestrator proposed, waiting on the
/// person: a folder to add, a project to link, a name or a group to give.
/// Nothing in it has happened; the window does it when the person says yes.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProposal {
    pub id: String,
    /// The project it is about, when it is already one of devpit's.
    pub project_id: Option<String>,
    /// The folder, whole. Local only.
    pub path: String,
    /// The name to give it, when one was proposed — for a new folder, its
    /// last segment otherwise.
    pub name: String,
    /// The group to put it in, when one was proposed.
    pub group: Option<String>,
    /// Link it to this orchestrator (`true`), unlink it (`false`), or leave
    /// its link as it is.
    pub link: Option<bool>,
}

/// A stop an orchestrator asked for that the person's own words did not
/// cover, waiting on their click above its chat's composer.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StopAsked {
    pub name: String,
    /// Only this process, when two sessions share the name.
    pub pid: Option<i32>,
    /// What stopping it loses — it is working, files not committed — as
    /// devpit saw it when it was asked.
    pub losing: Option<String>,
}

/// A list, so tomorrow's field has somewhere to go.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StopsAsked {
    pub stops: Vec<StopAsked>,
}

/// A request another orchestrator forwarded, waiting in this orchestrator's
/// conversation for the person. Nothing in it has been sent or run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedRequest {
    /// The address of the orchestrator that sent it.
    pub from: String,
    /// What the sender calls it: the same request sent again carries it again.
    pub id: String,
    /// The conversation it waits in.
    pub conversation: String,
    pub text: String,
}

/// A list, so tomorrow's field has somewhere to go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedRequests {
    pub requests: Vec<ForwardedRequest>,
}

/// Where the turn of a forwarded request the person sent on runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedSent {
    /// The conversation the turn started in, found again at the click.
    pub conversation: String,
}

/// What the person's click on the stops asked did.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StopsConfirmed {
    pub stopped: Vec<String>,
    /// `name: why`, for each that could not be stopped.
    pub failed: Vec<String>,
    /// `name: loss`, for each that would lose more now than the click
    /// showed: not stopped, and waiting again with it.
    pub changed: Vec<String>,
}

/// A confirmation an orchestrator needs from the person before it acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Guardrail {
    /// Stopping a session.
    Stop,
    /// A forced stop, or one that loses work.
    ForceStop,
    /// Sending a drafted reply as the person.
    SendDraft,
}

/// When a guardrail waits for the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum GuardrailMode {
    /// Asks unless the orchestrator's chat runs in Full access.
    Follow,
    Always,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailRow {
    pub guardrail: Guardrail,
    pub mode: GuardrailMode,
}

/// An orchestrator's guardrails, each with its mode.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Guardrails {
    pub rows: Vec<GuardrailRow>,
}
