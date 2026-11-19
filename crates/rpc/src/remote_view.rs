//! Reaching this machine from another device: what the viewer and the host
//! say to each other, and what the settings show about it.
//!
//! The viewer is a page this machine serves over the person's tailnet, or
//! devpit.app serves to reach it through the relay. It speaks JSON, a message
//! at a time, tagged by `t`: one per WebSocket frame on the tailnet, sealed in
//! Noise frames through the relay (`docs/remote-relay.md`). What
//! it may do is decided here, on the machine, per paired device: every
//! device sees; typing and answering are granted one by one.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    Attachment, Board, Conversation, Conversations, Frame, LiveSession, Message, StopAsked,
    Transcript,
};

/// What a viewer asks.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum RemoteIn {
    /// The first message: the device's token, which only its pairing gave it.
    Hello {
        token: String,
    },
    /// Through the relay, the first message of a device the machine does not
    /// know yet: the one-time secret its QR code carried, sent only after the
    /// machine proved the key that code named. The device is then known by
    /// its own key, proven in every handshake after.
    Pair {
        secret: String,
        name: String,
    },
    Projects,
    /// Watch a terminal, sized for the viewer's screen. The desk's own size
    /// does not change.
    PaneOpen {
        project: String,
        pane: String,
        cols: u16,
        rows: u16,
        /// Given by a viewer that reads raw bytes, to a host whose welcome
        /// said `binary`: its bytes then come as `[u32 handle][u8 kind][bytes]`
        /// frames, little-endian, instead of `paneBytes`.
        #[serde(default)]
        handle: Option<u32>,
    },
    /// Keys, as bytes in base64. Only a device allowed to type.
    PaneInput {
        pane: String,
        b64: String,
    },
    /// A line written and sent, as if pasted and Enter pressed.
    PanePaste {
        pane: String,
        text: String,
    },
    PaneClose {
        pane: String,
    },
    Board {
        project: String,
    },
    /// Into a lane without a step. Only a device allowed to type.
    CardMove {
        project: String,
        card: String,
        column: String,
    },
    Waiting,
    /// A question an agent is waiting on. Only a device allowed to answer.
    Answer {
        id: String,
        allow: bool,
    },
    Chats {
        project: String,
    },
    Chat {
        project: String,
        conversation: String,
    },
    /// The replies orchestrators drafted for the person to send.
    Drafts,
    /// Types a draft into its session's terminal. Only a device allowed to type.
    DraftSend {
        profile: String,
        name: String,
    },
    /// The stops orchestrators asked for, waiting on the person's click. Sent
    /// only to a host whose welcome said `stops`.
    Stops,
    /// The person's click on the stops as the viewer showed them: each stops
    /// on the loss it showed. Only a device allowed to type.
    StopsConfirm {
        stops: Vec<RemoteStop>,
    },
    /// The person keeps these running. Only a device allowed to type.
    StopsDrop {
        stops: Vec<RemoteStop>,
    },
    AgentHealth,
    /// A card in the project's first lane. Only a device allowed to type.
    CardCreate {
        project: String,
        title: String,
    },
    /// A new session of the default Claude account in the project, on a card
    /// or not. Only a device allowed to type. Refused, with the command, when
    /// a sign-in the project needs is missing, unless `anyway`.
    SessionStart {
        project: String,
        card: Option<String>,
        prompt: String,
        anyway: bool,
    },
    /// Opens devpit's own MCP again. Only a device allowed to type.
    AgentRestart,
    /// The orchestrators and the sessions of their accounts, to talk to.
    Talks,
    /// The person's message in a project's conversation: a turn of theirs,
    /// marked as sent from the Remote. Only a device allowed to type.
    ChatSay {
        project: String,
        conversation: String,
        text: String,
    },
    /// What a session said, read from its own transcript.
    Session {
        profile: String,
        name: String,
    },
    /// Typed into a session's terminal as the person. Only a device allowed
    /// to type.
    SessionSay {
        profile: String,
        name: String,
        text: String,
    },
    /// A piece of a photo or a recording, in base64. A file goes in pieces
    /// under the ceiling a message has, and is held here until finished.
    UploadPart {
        upload: String,
        b64: String,
    },
    /// The pieces sent under `upload`, a photo of the type `mime`: kept in
    /// the project as an attachment. Only a device allowed to type.
    Photo {
        upload: String,
        project: String,
        mime: String,
    },
    /// The pieces sent under `upload`, a recording: made into words for the
    /// composer, then thrown away. Only a device allowed to type.
    Voice {
        upload: String,
        mime: String,
    },
    /// A secret put in the project's `.env` by the window's own write. Only a
    /// device allowed to type; the value is never said back or logged.
    SecretSet {
        project: String,
        name: String,
        value: String,
    },
    Ping,
}

/// What the host says.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum RemoteOut {
    Welcome {
        device: String,
        host: String,
        typing: bool,
        answering: bool,
        /// The host reads and sends raw terminal bytes; absent from an older one.
        #[serde(default)]
        binary: bool,
        /// The host shows the stops waiting on the person's click; absent
        /// from an older one.
        #[serde(default)]
        stops: bool,
        /// The language the host resolved, `en` or `pt-BR`: the Remote has no
        /// settings of its own to read it from. Empty from an older host, and
        /// the viewer then stays in English.
        #[serde(default)]
        locale: String,
    },
    /// The device proved its pairing code and waits for the person at the
    /// machine to allow it. An older viewer passes it by.
    Approving,
    /// Said once, then the connection ends.
    Refused {
        why: String,
    },
    Projects {
        projects: Vec<RemoteProject>,
    },
    PaneBytes {
        pane: String,
        b64: String,
    },
    PaneClosed {
        pane: String,
    },
    Board {
        project: String,
        board: Box<Board>,
    },
    BoardChanged {
        project: String,
    },
    Waiting {
        questions: Vec<RemoteQuestion>,
    },
    Chats {
        project: String,
        conversations: Box<Conversations>,
    },
    Chat {
        project: String,
        conversation: Box<Conversation>,
    },
    Drafts {
        drafts: Vec<RemoteDraft>,
    },
    /// The stops waiting on the person's click, and what the last click on
    /// them came to. An older viewer passes it by.
    Stops {
        stops: Vec<RemoteStop>,
        said: Option<String>,
    },
    AgentHealth {
        health: crate::AgentHealth,
    },
    /// A session started from here, by the name to find it under.
    Started {
        name: String,
    },
    /// A secret was kept, said by its name alone.
    SecretKept {
        project: String,
        name: String,
    },
    Talks {
        talks: RemoteTalks,
    },
    /// A moment of a turn in a conversation this device follows.
    ChatFrame {
        project: String,
        conversation: String,
        // Boxed: a frame is far larger than every other message here.
        frame: Box<Frame>,
    },
    Session {
        profile: String,
        name: String,
        messages: Vec<Message>,
    },
    /// A photo kept, as a message names it.
    Attached {
        upload: String,
        attachment: Attachment,
    },
    /// A recording heard: its words, for the person to read before sending.
    Heard {
        upload: String,
        transcript: Transcript,
    },
    /// One request could not be done; the connection stays.
    Failed {
        why: String,
    },
    Pong,
}

/// Who there is to talk to from the Remote.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTalks {
    pub orchestrators: Vec<RemoteOrchestrator>,
    /// The sessions of the orchestrators' accounts, each account once.
    pub sessions: Vec<RemoteSession>,
    /// This machine makes a recording into words, as Settings set it up.
    pub voice: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOrchestrator {
    pub project: String,
    pub name: String,
    /// The account it speaks as.
    pub profile: String,
    /// Its latest conversation, or a new one's id when it has had none.
    pub conversation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSession {
    /// The account it runs as.
    pub profile: String,
    pub session: LiveSession,
}

/// A reply an orchestrator drafted, waiting on the person.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDraft {
    /// The orchestrator's account.
    pub profile: String,
    /// The session it is for.
    pub name: String,
    pub text: String,
}

/// A stop an orchestrator asked for, waiting on the person's click.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStop {
    /// The orchestrator's account.
    pub profile: String,
    pub stop: StopAsked,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProject {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub terminals: Vec<RemoteTerminal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTerminal {
    pub pane: String,
    /// What runs in front of it: `claude`, `zsh`.
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteQuestion {
    pub id: String,
    /// `terminal` or `chat`.
    pub from: String,
    pub project: Option<String>,
    pub tool: String,
    pub input: String,
}

/// Every message shape, for the generated contract the viewer is typed by.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteShapes {
    pub incoming: Vec<RemoteIn>,
    pub outgoing: Vec<RemoteOut>,
}

/// Where Tailscale stands on this machine.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleState {
    pub installed: bool,
    /// Logged in and connected.
    pub running: bool,
    /// The machine's MagicDNS name, without the trailing dot.
    pub name: Option<String>,
    /// HTTPS certificates are on in the tailnet, so `tailscale serve` can
    /// publish the viewer with one.
    pub https: bool,
    /// Who the machine belongs to in the tailnet: the only one let in.
    pub login: Option<String>,
    /// The machine's tailnet IPv4.
    pub ip: Option<String>,
}

/// A device paired with this machine.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub typing: bool,
    pub answering: bool,
    /// Seconds since the epoch.
    pub paired_at: f64,
    pub last_seen: Option<f64>,
    pub connected: bool,
    /// Paired through devpit.app's relay, and known by its key.
    pub relay: bool,
    /// An MCP client on another machine: it reads devpit's MCP through the
    /// relay, and nothing else.
    pub mcp: bool,
}

/// How paired devices reach this machine.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteVia {
    /// Over the person's tailnet, served by this machine.
    #[default]
    Tailscale,
    /// From anywhere, through devpit.app's relay, encrypted end to end.
    Relay,
}

/// The relay, as this machine sees it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelay {
    /// This machine is signed in to a devpit account, which the relay needs.
    pub signed_in: bool,
    /// Connected to the relay now, so a device can reach it.
    pub online: bool,
    /// Why it is not, when it should be.
    pub problem: Option<String>,
}

/// Remote, as the settings show it.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteView {
    pub enabled: bool,
    pub via: RemoteVia,
    pub relay: RemoteRelay,
    pub tailscale: TailscaleState,
    /// Where a paired device opens the viewer, once it is reachable.
    pub address: Option<String>,
    pub devices: Vec<RemoteDevice>,
    /// Paired MCP clients may read this devpit through the relay. Off unless
    /// turned on.
    pub mcp: bool,
    /// Why it is not reachable, when it is on and is not.
    pub problem: Option<String>,
    /// Devices that proved a pairing code and wait for the person to allow them.
    pub asking: Vec<RemoteAsking>,
}

/// A device asking to be paired.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAsking {
    pub id: String,
    pub name: String,
}

/// One thing a paired device did, from the Remote's own log.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteActivity {
    /// Seconds since the epoch.
    pub at: f64,
    pub device: String,
    pub what: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteActivities {
    /// The latest first.
    pub entries: Vec<RemoteActivity>,
}

/// A pairing, offered on the machine's screen and good once, briefly.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemotePairing {
    pub code: String,
    pub url: String,
    pub qr_svg: String,
    /// Seconds since the epoch.
    pub expires_at: f64,
}
