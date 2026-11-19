//! The contract between the core and the interface.
//!
//! These structs are the source; TypeScript is generated from them as a build
//! step. A type hand-written on both sides is a type that will diverge.
//!
//! This crate does not know about Tauri (checked by `cargo xtask check`). It
//! declares the shape; `apps/desktop` adapts that shape to a transport, and
//! tomorrow an HTTP server and a CLI do the same over the same types.

pub mod account;
pub mod accounts;
pub mod agents;
pub mod blocks;
pub mod board;
pub mod card;
pub mod channels;
pub mod chat;
pub mod checkpoint;
pub mod conversation;
pub mod cut;
pub mod decisions;
pub mod devflow;
pub mod devflow_screen;
pub mod display;
pub mod ended;
pub mod error;
pub mod file;
pub mod focus;
mod frame;
pub mod front;
pub mod hosts;
pub mod island;
pub mod lessons;
pub mod live;
pub mod mcp_health;
pub mod pause;
pub mod plugin_data;
pub mod plugins;
pub mod plus;
pub mod prime;
pub mod profile;
pub mod project;
pub mod reminder;
pub mod remote;
pub mod remote_view;
pub mod review;
pub mod runs;
pub mod schedule;
pub mod search;
pub mod session;
pub mod session_regroup;
pub mod session_tree;
pub mod settings;
pub mod sign_ins;
pub mod spend;
pub mod sync;
pub mod tested;
mod threads;
pub mod tile;
pub mod tray;
pub mod update;
pub mod usage;
pub mod vault;
pub mod voice;

pub use account::{Account, Membership, SignIn, SignInState};
pub use accounts::{
    AccountQuota, AccountQuotas, Accounts, ClaudeAccount, ProjectAccount, QuotaState,
};
pub use agents::{Agent, Agents, RejectedAgent};
pub use blocks::{BlockChanged, CommandBlock, FolderGlance, PaneBlocks};
pub use board::{Board, CardChanged, Column, ColumnDeleted, Run, RunState, Step, StepKind};
pub use card::{
    ArchivedCard, ArchivedCards, CardDeleted, CardDetail, CardTerminal, Checkout, Comment,
    DeleteRefusal, Notice, Notices, Pinned, Played,
};
pub use channels::{
    CalendarFeed, ChannelEvent, ChannelInfo, ChannelOrchestrator, ChannelRoute, ChannelRules,
    Channels, HubStatus, Privacy, QuietHours, TelegramStatus,
};
pub use chat::{CallState, ChangedFile, Message, Part, Role, SessionInit};
pub use checkpoint::{
    validity, verdict, Checked, Fingerprint, Report, Validity, Verdict, WhatRan, Whose, WouldRun,
};
pub use conversation::{
    Ask, Attachment, ChatQueue, Conversation, ConversationAfter, ConversationFound,
    ConversationHead, ConversationPrompts, FoundIn, OutsideSession, Prompt, QueuedMessage,
    TurnFrom,
};
pub use decisions::{Deciding, DecisionTried};
pub use devflow::{
    DecisionsState, DevflowAsk, DevflowAsks, DevflowCalibration, DevflowChange, DevflowDecisions,
    DevflowPlan, DevflowPlanCreated, PlannedCard, RubricAccuracy,
};
pub use devflow_screen::{
    DevflowBinary, DevflowCardEvidence, DevflowCheck, DevflowCli, DevflowCodexInstalled,
    DevflowConfig, DevflowDetected, DevflowError, DevflowExploit, DevflowFileChange, DevflowFound,
    DevflowGated, DevflowHint, DevflowMcp, DevflowMcpTool, DevflowPlugin, DevflowPluginAction,
    DevflowPrefs, DevflowPreview, DevflowProfile, DevflowProfileChoice, DevflowProfileFrom,
    DevflowReviewed, DevflowReviewer, DevflowScreen, DevflowSkill, DevflowSkillText, DevflowStage,
    DevflowState, DevflowTeam, DevflowTeamChange, DevflowTeamChanges,
};
pub use display::{Display, DisplayBackend, DisplayWhy};
pub use ended::{Ended, Limit, LimitKind, TurnEnd};
pub use error::{ErrorCode, RpcError};
pub use file::{FileContents, FileKind, FileSaved};
pub use focus::{HeadsDown, Waiting};
pub use frame::{Context, Frame};
pub use front::{Elsewhere, ElsewhereFile, Front, SessionChanges};
pub use hosts::{
    Host, HostCheck, HostCheckRow, HostDraft, HostEnvironment, HostLink, HostLinks, HostOfTab,
    HostTerminal, Hosts, TabHost,
};
pub use island::{
    IslandAsked, IslandChange, IslandChecks, IslandDrawn, IslandHow, IslandNow, IslandPull,
    IslandQuestion, IslandRect, IslandSession, IslandStep, IslandVerdict, Touch,
};
pub use lessons::{
    LessonProposal, LessonProposals, MemoryDecision, MemoryEntry, MemoryProposal,
    ProjectAgentAnswer, ProjectMemory,
};
pub use live::{
    EndedSession, EndedSessions, Guardrail, GuardrailMode, GuardrailRow, Guardrails, LivePane,
    LiveSession, LiveSessions, PendingPrompt, ProjectProposal, PromptOption, RemoteState,
    SessionOnCard, SessionTold, SessionsOnCard, StopAsked, StopsAsked, StopsConfirmed,
};
pub use mcp_health::{AgentHealth, McpHealth, McpServerHealth, McpState};
pub use pause::Paused;
pub use plugin_data::{
    PluginFile, PluginFileRemoved, PluginFileSaved, PluginFileText, PluginFiles,
};
pub use plugins::{
    catalogue, validate, validate_catalogue, DataSpec, Permission, PluginError, PluginList,
    PluginManifest, PluginState, PluginUninstalled, Surface,
};
pub use plus::PlusStatus;
pub use prime::WorktreeSetup;
pub use profile::{
    Credentials, Declared, EnvVar, OfferedModel, Profile, ProfileSignIn, ProfileWaiting,
    ProfilesWaiting, ProviderFound, ProvidersFound, Reach, ReadCommand, SignedIn,
};
pub use project::{
    Change, Commit, FileNode, GitStatus, Project, ProjectChanges, ProjectHistory, ProjectList,
    ProjectTree, Worktree, WorktreeOrigin,
};
pub use reminder::{Reminder, Reminders};
pub use remote::{CommitDraft, RemoteAct};
pub use remote_view::{
    RemoteActivities, RemoteActivity, RemoteAsking, RemoteDevice, RemoteDraft, RemoteIn,
    RemoteOrchestrator, RemoteOut, RemotePairing, RemoteProject, RemoteQuestion, RemoteRelay,
    RemoteSession, RemoteShapes, RemoteStop, RemoteTalks, RemoteTerminal, RemoteVia, RemoteView,
    TailscaleState,
};
pub use review::{
    blocking, reviewed, standing, CardFindings, CardReview, Finding, Found, Review, Severity,
    Standing, REVIEW_EVIDENCE,
};
pub use runs::{ProjectRun, RunCursor, RunsPage, RunsQuery};
pub use schedule::{
    AssistantMode, Briefing, Heartbeat, ProposedReminder, Routine, RoutineKind, Schedule,
    ScheduleKind, ScheduleProposal, ScheduleState, Schedules,
};
pub use search::{SearchFile, SearchHits, SearchLine};
pub use session::{
    AgentPresence, KnownAgent, LayoutNode, PaneInTmux, PaneKind, PaneRunning, PaneSize,
    SessionLayout, SplitDirection,
};
pub use settings::{ErrorReportsPreview, Language, SessionRelay, Settings, Theme};
pub use sign_ins::{AccountSignIn, AgentLaunch, MissingSignIn, NeededSignIn, SignIns};
pub use spend::{
    PlanLimits, PlanWindow, SessionCost, SpendCard, SpendDay, SpendHistory, SpendInstallation,
    SpendRow, SpendSession, SpendShare, TokenCounts,
};
pub use sync::{SyncMachine, SyncOffer, SyncRecovery, SyncStatus};
pub use tested::{FailedTest, Tested, MOST_FAILURES_KEPT, TESTS_EVIDENCE};
pub use threads::{ConversationMarks, Conversations, Thread};
pub use tile::{
    Card, CardHappening, CardRef, CardSession, Criterion, Doing, Progress, SessionKind,
    UnreadCriteria,
};
pub use tray::Tray;
pub use update::{InstallKind, UpdateBlocking, UpdateStatus, UpdateWork};
pub use usage::{PaneCost, Usage};
pub use vault::{Secret, SecretList};
pub use voice::{Transcribing, Transcript};

use serde::{Deserialize, Serialize};
use specta::Type;

/// Response of `app.info`.
///
/// An object, never a bare list — and that holds for every response in the
/// contract. Tomorrow's extra field must not break today's consumer, and a
/// bare list has nowhere to put it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    /// Where this build keeps state. Exposed on purpose: it is the first
    /// question of anyone taking a backup or filing a bug.
    pub state_path: String,
    /// Whether this is a devpit being worked on rather than the installed one.
    /// A debug build keeps a home of its own, and the window says so on its
    /// face: two devpits open side by side look identical otherwise, and the
    /// one you are testing in is not the one holding your real work.
    pub dev: bool,
}
