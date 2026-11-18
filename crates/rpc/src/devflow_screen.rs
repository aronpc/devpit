//! Devflow as the window draws it: a project's devflow.yaml read through the
//! same engine the MCP tools use, and what a card's runs left.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::devflow::{DevflowAsk, DevflowDecisions};

/// Where a project's devflow.yaml stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DevflowState {
    /// No devflow.yaml: the setup.
    Missing,
    /// Valid, and not switched on by the person here.
    Disabled,
    Active,
    /// Refused, with the line and field; the tools stay off.
    Invalid,
}

/// Why devflow.yaml is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowError {
    pub line: Option<u32>,
    /// A dotted path, empty when the file as a whole is refused.
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowCheck {
    pub name: String,
    pub run: String,
    pub report: String,
    pub evidence: Vec<String>,
    /// `file:line` the command was found at, when detection found it.
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowReviewer {
    pub agent: String,
    pub persona: String,
    pub model: Option<String>,
}

/// The skill a stage follows, from the layer that has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowSkill {
    pub name: String,
    /// `project`, `personal` or `built-in`.
    pub layer: String,
    pub file: Option<String>,
    /// Why the layer that has it cannot be used.
    pub refused: Option<String>,
}

/// One stage, in the order the work reaches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowStage {
    pub name: String,
    /// `builtin/<stage>`, or `<where>/<skill>` for an own stage.
    pub uses: String,
    /// One of the project's or a team's own stages.
    pub own: bool,
    pub enabled: bool,
    /// `person` or `none`, for a stage whose result is approved.
    pub approve: Option<String>,
    /// The plan's: `board`, `jira`, `linear` or `github`, when said.
    pub publish: Option<String>,
    /// The plan's: `done` or `gate`, when said.
    #[serde(default)]
    pub unblock_on: Option<String>,
    /// The execution stage's checks.
    pub checks: Vec<DevflowCheck>,
    /// The execution stage's checks run once for a release, by name.
    #[serde(default)]
    pub release: Vec<DevflowCheck>,
    /// The execution stage's folder for E2E specs.
    pub e2e_dir: Option<String>,
    /// The completion stage's rubric, or an own stage's.
    pub gate: Option<String>,
    /// An own stage's gate command.
    pub command: Option<String>,
    /// Decisions' rubric for this stage, when one is named.
    pub rubric: Option<String>,
    pub skill: Option<DevflowSkill>,
}

/// A team or company layer the project extends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowTeam {
    /// What its own stages are used under: `<name>/<skill>`.
    pub name: String,
    pub git: String,
    /// The ref devflow.yaml names.
    pub wanted: String,
    /// What the lockfile pins, when it pins it.
    pub locked_ref: Option<String>,
    pub commit: Option<String>,
    /// `applied`, `approve` or `fetch`.
    pub state: String,
}

/// What approving a layer's ref changes: shown before the person's click.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowTeamChange {
    pub name: String,
    pub git: String,
    pub from_ref: Option<String>,
    pub from_commit: Option<String>,
    pub to_ref: String,
    pub to_commit: String,
    /// `added path`, `changed path` or `removed path`.
    pub files: Vec<String>,
    /// The layer's devflow.yaml: `- ` lines gone, `+ ` lines new.
    pub diff: Vec<String>,
}

/// Response of `devflow.team_fetch`: each layer's change, nothing applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowTeamChanges {
    pub changes: Vec<DevflowTeamChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowConfig {
    pub enabled: bool,
    pub version: u32,
    pub teams: Vec<DevflowTeam>,
    /// Fields a layer locked, which the project may not change.
    pub locked: Vec<String>,
    /// The layer the stages came from: `project`, a team's name, or `built-in`.
    pub stages_from: String,
    pub stages: Vec<DevflowStage>,
    pub reviewers: Vec<DevflowReviewer>,
    pub block_on: Vec<String>,
    pub consolidate: String,
    pub decisions_use: bool,
    /// `shadow` or `enforce`.
    pub decisions_mode: String,
}

/// A command detection found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowFound {
    /// `test`, `e2e`, `lint` or `typecheck`.
    pub kind: String,
    pub command: String,
    pub source: String,
    /// Why it cannot run here (a program not installed), so it is not proposed.
    pub unavailable: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowCli {
    pub name: String,
    pub installed: bool,
    pub signed_in: Option<bool>,
}

/// One kind of thing the repository already has, and each of them: the
/// window lays them out, so it is handed the parts and not a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowHint {
    pub label: String,
    pub items: Vec<String>,
}

/// What a setup starts from, read off the repository and this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowDetected {
    pub found: Vec<DevflowFound>,
    /// `name (source)` of each E2E tool set up.
    pub e2e_tools: Vec<String>,
    pub clis: Vec<DevflowCli>,
    pub decisions_key: bool,
    pub glossary: Option<String>,
    pub adrs: Vec<String>,
    /// What detection left to the person, one sentence each.
    pub open: Vec<String>,
    /// How the repository already works: guides, templates, git-flow's
    /// branches, ticket keys and installed skills, one kind each.
    pub hints: Vec<DevflowHint>,
    /// The recommended setup, before its commands move into scripts.
    pub recommended: DevflowConfig,
}

/// Response of `devflow.screen`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowScreen {
    pub project_id: String,
    pub state: DevflowState,
    /// Off in Settings for every project.
    pub globally_on: bool,
    /// Where devflow.yaml is, whole, to open in an editor.
    pub file: String,
    pub yaml: Option<String>,
    pub config: Option<DevflowConfig>,
    pub error: Option<DevflowError>,
    /// The changes waiting for the person's click here.
    pub waiting: Vec<DevflowAsk>,
    /// Only while there is no file.
    pub detected: Option<DevflowDetected>,
    /// The setup a new project starts from: `recommended` or `custom`.
    pub preset: String,
}

/// One file a setup writes, and what writing it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowFileChange {
    pub path: String,
    /// `create`, `replace`, `same`, `kept`, `append`, `remove` (a check
    /// script devpit wrote that no check runs) or `unused` (one the person
    /// changed, left for them).
    pub change: String,
}

/// Response of `devflow.preview`: the commented YAML and its files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowPreview {
    pub yaml: String,
    pub files: Vec<DevflowFileChange>,
}

/// Response of `devflow.skill_text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowSkillText {
    pub skill: DevflowSkill,
    pub text: String,
}

/// Devflow's preferences on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowPrefs {
    /// Offered to any project at all.
    pub enabled: bool,
    /// The setup a new project starts from: `recommended` or `custom`.
    pub preset: String,
}

/// Response of `devflow.codex_install`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowCodexInstalled {
    pub folder: String,
    pub said: String,
}

/// The standalone devflow's Claude Code plugin, `devflow@devflow`, in the
/// profile a project's sessions run as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowPlugin {
    pub project_id: String,
    /// The Claude Code configuration directory it is read from and installed in.
    pub directory: String,
    /// The profile's label; `None` is the machine's default.
    pub profile: Option<String>,
    /// The version installed there, if any.
    pub version: Option<String>,
    pub enabled: bool,
    /// The release devpit is built against.
    pub known: String,
    /// On, while devpit's own Devflow is on in the project too: the tools twice.
    pub twice: bool,
}

/// What the Devflow screen's plugin buttons do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DevflowPluginAction {
    Install,
    Update,
    Remove,
    Enable,
    Disable,
}

/// The standalone devflow binary in `~/.local/bin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowBinary {
    pub path: String,
    /// What `devflow --version` says, when it is there and runs.
    pub version: Option<String>,
    /// The release devpit downloads.
    pub known: String,
    /// `devflow doctor`'s lines, after an install.
    pub doctor: Vec<String>,
}

/// The standalone devflow as an MCP server (`devflow mcp`) in one tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowMcpTool {
    /// `codex`, `cursor`, `opencode` or `gemini`.
    pub tool: String,
    pub label: String,
    /// The tool's own configuration file, where devpit adds it.
    pub file: String,
    pub added: bool,
}

/// Response of `devflow.mcp` and `devflow.mcp_set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowMcp {
    /// The command each tool is given, before `mcp`.
    pub command: String,
    pub tools: Vec<DevflowMcpTool>,
}

/// The account a project's Claude Code reviewers run as (#249).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowProfile {
    pub project_id: String,
    /// The profile the project names, by id.
    pub named: Option<String>,
    /// The label of the profile they run as; `None` is the machine's default.
    pub runs_as: Option<String>,
    pub from: DevflowProfileFrom,
    /// The Claude profiles the project can name.
    pub choices: Vec<DevflowProfileChoice>,
}

/// Why the reviewers run as that account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DevflowProfileFrom {
    /// The project names it.
    Project,
    /// Every orchestrator linked to the project speaks as it.
    Orchestrator,
    /// None of them: the machine's default account.
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowProfileChoice {
    pub id: String,
    pub label: String,
}

/// One reviewer of a card's newest review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowReviewed {
    pub agent: String,
    pub persona: String,
    /// `answered` or `skipped`.
    pub status: String,
    pub findings: u32,
    pub duration_ms: Option<f64>,
    pub cost_usd: Option<f64>,
    pub reason: Option<String>,
}

/// The card's newest gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowGated {
    pub passed: bool,
    pub run: Option<String>,
    pub at: String,
    pub with_evidence: u32,
    pub criteria: u32,
    pub missing: Vec<String>,
}

/// An exploit test that confirmed an attack finding: it stays a regression
/// test the gate wants green.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowExploit {
    pub test: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub title: Option<String>,
}

/// Response of `devflow.card_evidence`: what a card's Devflow runs left
/// among the project's artifacts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowCardEvidence {
    pub card_id: String,
    pub gate: Option<DevflowGated>,
    pub reviewers: Vec<DevflowReviewed>,
    pub verdict: Option<String>,
    /// The artifacts folder, whole, which `artifacts` are relative to.
    pub folder: String,
    /// The newest run's attached files, as artifact names.
    pub artifacts: Vec<String>,
    /// Where Decisions stands for the card's project.
    #[serde(default)]
    pub decisions: Option<DevflowDecisions>,
    /// The newest review's confirmed exploits.
    pub exploits: Vec<DevflowExploit>,
}
