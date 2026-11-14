//! A profile is a command and the driver that reads it.
//!
//! The same CLI signed into two accounts is two commands — `claude` for one,
//! `claude2` for the other. Which account a conversation used is a fact about
//! it, so the profile is recorded and never changes.
//!
//! "Two commands on the PATH" was the first telling of this and it was wrong
//! about the machine it was written on, where both are shell functions with no
//! file anywhere. That is what `Reach` is for.

use serde::{Deserialize, Serialize};
use specta::Type;

/// How far this machine can get with a command.
///
/// Three answers and not two, because "installed" is not a yes or a no here.
/// On the machine this was written on `claude` is a **shell function** in
/// `.zshrc` with no file anywhere on `PATH`: a terminal starts it, because a
/// terminal types into that shell, and `Command::new` cannot — you cannot exec
/// a function. A single boolean has to pick one of those to be wrong about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// An executable file on the `PATH`. Works everywhere: terminal, chat and
    /// the board's headless turns.
    Runnable,
    /// The person's shell knows the name but no file answers to it — a
    /// function or an alias. The terminal can start it and nothing else can.
    ShellOnly,
    /// Neither. Nothing here can start it.
    Missing,
}

/// One environment variable a profile sets before its program starts.
///
/// A pair and not a `HashMap`, because order is what somebody typed and a map
/// would reshuffle their list every time the pane redrew.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    /// Often a secret. Never logged, never put in an error message.
    pub value: String,
}

/// A profile as the person wrote it.
///
/// The shape was measured rather than invented. Five of these existed as shell
/// functions in one `.zshrc` before devpit had anywhere to put them, and every
/// one of them was the same three things:
///
/// ```text
/// glm()     = {seven ANTHROPIC_* vars} + claude + [--permission-mode bypassPermissions]
/// claude2() = {CLAUDE_CONFIG_DIR}       + claude + [--permission-mode bypassPermissions]
/// ```
///
/// `glm` and `claude2` differ in **nothing but the environment**. So a profile
/// is environment, program and arguments — not a command line. That matters
/// twice: a command line would have to be handed to a shell, and a shell
/// function cannot be spawned at all, which is why those five worked in a
/// terminal and nowhere else devpit could reach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Declared {
    /// Minted once and never derived from the label, because the label is the
    /// part the person is invited to change.
    pub id: String,
    /// What they called it.
    pub label: String,
    /// The agent this behaves like, by `devpit_pty::agents` id. It carries the
    /// driver, the hook flag and the default program.
    pub base: String,
    /// The program to run. Empty means the base agent's own.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    /// The models offered in place of the driver's. A gateway such as z.ai
    /// serves none of Anthropic's, so its profile names its own. Empty keeps
    /// the driver's list.
    #[serde(default)]
    pub models: Vec<String>,
}

/// A profile that came from another machine and waits for the person here
/// before it can start: values only this machine may hold, or a base URL
/// changed elsewhere that would send its token somewhere new.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileWaiting {
    pub id: String,
    pub label: String,
    /// Made on another machine and not yet taken here.
    pub arrived: bool,
    /// Variables that arrived with no value, to fill here.
    pub needs: Vec<String>,
    /// What another machine changed, by field (`command`, `base`,
    /// `env.<NAME>`), to take or to leave.
    pub changed: Vec<EnvVar>,
}

/// The profiles that wait for the person on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesWaiting {
    pub profiles: Vec<ProfileWaiting>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    /// What the person calls this account.
    pub label: String,
    /// The binary to run. Two accounts differ here and nowhere else.
    pub command: String,
    /// Which driver reads its output.
    pub driver: String,
    /// Where the command resolves to, when it resolves to a file at all.
    /// Set exactly when `reach` is `Runnable`.
    pub path: Option<String>,
    /// How far this machine gets with `command`.
    pub reach: Reach,
    /// The agent this behaves like. Empty for a discovered command, which is
    /// only ever itself.
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    /// Whether the person declared this one, and may therefore rename, edit or
    /// delete it. A discovered command is devpit noticing, not a choice.
    #[serde(default)]
    pub mine: bool,
    /// The models the composer may pick from: the profile's own when it
    /// named some, the driver's otherwise. Carried here so one call answers
    /// the whole selector.
    #[serde(default)]
    pub models: Vec<String>,
    /// What the agent itself said of its models: their names, and why one it
    /// refused cannot be used.
    #[serde(default)]
    pub offered: Vec<OfferedModel>,
    /// The models the person listed, as written — what the editor opens with.
    /// Kept apart from `models`, which a save would otherwise make theirs.
    #[serde(default)]
    pub own_models: Vec<String>,
    /// How hard the agent may be asked to think. Empty when the CLI has no
    /// such control, and the composer then draws no chip at all.
    #[serde(default)]
    pub efforts: Vec<String>,
    #[serde(default)]
    pub effort_default: Option<String>,
    /// Whether its turns say what they cost and how full the context is.
    /// When not, the chat shows those as unavailable rather than as 0.
    #[serde(default)]
    pub metered: bool,
    /// Whether it is offered. Carried here because the composer and the step
    /// editor read this list, and a switch only `agents.known` saw switched
    /// nothing off for them.
    #[serde(default)]
    pub enabled: bool,
}

impl Profile {
    /// Whether anything on this machine can start it.
    pub fn installed(&self) -> bool {
        self.reach != Reach::Missing
    }

    /// Whether **devpit** can start it, as opposed to a terminal.
    ///
    /// The chat composer and the board's headless turns spawn a process; a
    /// shell function has no process to spawn. The two questions were one
    /// boolean until a person's own `claude2` came back as "not installed"
    /// while the terminal was running it in front of them.
    pub fn spawnable(&self) -> bool {
        self.reach == Reach::Runnable
    }
}

/// A model an agent offered in its session, or one it refused, with why.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OfferedModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub unavailable: Option<String>,
}

/// A command of the person's own — a shell function such as `claude2` —
/// read by running it: the agent it starts, and what it starts it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReadCommand {
    /// The agent it runs, by `devpit_pty::agents` id.
    pub base: String,
    pub args: Vec<String>,
    pub env: Vec<EnvVar>,
}

/// Whether a CLI configuration directory holds a sign-in.
///
/// `Unknown` because on macOS the sign-in lives in the Keychain and the
/// directory cannot say either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Credentials {
    Saved,
    Missing,
    Unknown,
    /// Saved, but past renewing itself: the CLI asks for /login.
    Expired,
}

/// `agent.signed_in`'s answer. An object, so the next field has somewhere to go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SignedIn {
    pub state: Credentials,
}

/// One of the CLIs devpit holds a chat with, or will: found on this machine
/// or not, and what it takes to get it there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFound {
    /// The agent's id in the catalogue.
    pub id: String,
    pub label: String,
    /// Where it was found, from `~` under the home folder; none when it is
    /// not on this machine.
    pub path: Option<String>,
    /// What `--version` said, when it said a version.
    pub version: Option<String>,
    pub signed_in: Credentials,
    /// Whether a driver reads it, so it can hold a chat and not only a terminal.
    pub chat: bool,
    /// The line that installs it, for a person to run.
    pub install: String,
    /// The line that signs it in, typed into a terminal by "Sign in".
    pub login: String,
    pub homepage: String,
}

/// A declared profile's own sign-in, read with its own variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSignIn {
    pub id: String,
    pub label: String,
    /// The provider it runs, by id.
    pub base: String,
    pub signed_in: Credentials,
}

/// `providers.detect`'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersFound {
    pub providers: Vec<ProviderFound>,
    pub profiles: Vec<ProfileSignIn>,
}
