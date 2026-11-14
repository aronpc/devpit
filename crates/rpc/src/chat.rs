//! A conversation with an agent, and what one message is made of.
//!
//! Parts rather than one string: the screen draws a tool call, a thought and
//! an answer differently, and a flat string cannot tell them apart.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Who said it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// How a tool call ended, or that it has not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CallState {
    Running,
    Ok,
    Failed,
}

/// One piece of a message.
///
/// `parent` names the `Agent` call a subagent's part came from. Empty for the
/// agent's own work. Defaulted so a transcript written before it existed still
/// reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Part {
    Text {
        text: String,
        #[serde(default)]
        parent: Option<String>,
    },
    /// Reasoning the model showed. Separate because it is not the answer.
    Thinking {
        text: String,
        #[serde(default)]
        parent: Option<String>,
        /// How long it took, once it is over and the stream timed it.
        #[serde(default)]
        duration_ms: Option<u32>,
    },
    ToolCall {
        id: String,
        name: String,
        /// Verbatim. Parsing it here would be this crate guessing at a shape
        /// the provider is free to change.
        input: String,
        state: CallState,
        #[serde(default)]
        parent: Option<String>,
    },
    ToolResult {
        /// The call this answers.
        call_id: String,
        output: String,
        is_error: bool,
        #[serde(default)]
        parent: Option<String>,
    },
    /// Work the CLI runs beside the conversation: a backgrounded subagent or
    /// command. One part per change, so the latest for a `task_id` is its state.
    Task {
        task_id: String,
        /// The tool call that started it, when the CLI says.
        call_id: Option<String>,
        /// The CLI's own word: `local_agent`, `local_bash`.
        task_kind: Option<String>,
        description: Option<String>,
        /// `started`, `running`, then the CLI's own ending (`completed`, …).
        status: String,
        summary: Option<String>,
        /// How long it has run, as the CLI counts it.
        #[serde(default)]
        duration_ms: Option<u32>,
    },
    /// What a slash command itself answered, as the CLI printed it.
    Command { content: String },
    /// A permission question somebody answered, kept in the thread.
    Receipt {
        tool: String,
        /// The tool's input as the question showed it.
        input: String,
        allowed: bool,
    },
    /// What the turn changed in the checkout, and its tree before, to put back.
    Changes {
        files: Vec<ChangedFile>,
        #[serde(default)]
        before: Option<String>,
    },
    /// This conversation was forked from another at one of its turns, which
    /// left that one as it was.
    Rewound {
        from_conversation: String,
        turn: u32,
    },
    /// The CLI's own `/compact` ran on this conversation while it sat idle,
    /// when its context held this many tokens.
    Compacted { tokens: u32 },
    /// A line the driver did not recognise. Kept rather than dropped: losing
    /// output is worse than showing it plain.
    Unknown { text: String },
}

/// One file a turn changed, relative to the checkout it ran in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub added: u32,
    pub removed: u32,
}

/// What the CLI says about itself when a session starts: read from its own
/// report rather than a list kept here, since the commands, skills and models
/// differ per installation and change with every plugin.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionInit {
    pub model: Option<String>,
    pub models: Vec<crate::OfferedModel>,
    pub slash_commands: Vec<String>,
    /// Commands that only work in the terminal, which a chat must not offer.
    pub terminal_slash_commands: Vec<String>,
    pub skills: Vec<String>,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub turn_id: Option<String>,
    pub role: Role,
    pub parts: Vec<Part>,
    /// Unix seconds.
    pub created_at: f64,
    /// True while more of it is still arriving.
    pub streaming: bool,
    /// How an answer ended; absent while it streams and in older transcripts.
    #[serde(default)]
    pub ended: Option<crate::Ended>,
}
