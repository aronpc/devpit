//! What an `agent` step declares, out of `step.config`.
//!
//! A module of its own because two things read it, and they must read it the
//! same way: the turn that runs the step, and the rule that accepts a step
//! when it is saved. While the rule had its own copy of these fields, a step
//! could be accepted with a shape the runner then refused.

use serde::Deserialize;

/// What an `agent` step needs to know, out of `step.config`.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct AgentConfig {
    /// The agent to run, by the name in its frontmatter.
    ///
    /// A subagent, not a CLI account — `profile` is that, and the two live in
    /// this JSON side by side without being related.
    pub(crate) agent: Option<String>,
    /// Which profile runs it: the program, its arguments and the environment
    /// that picks the account. Absent runs whatever the build's default is,
    /// which is what every step did before profiles existed.
    pub(crate) profile: Option<String>,
    /// What to ask. The card's title and body are appended to it.
    pub(crate) prompt: String,
    /// A JSON Schema the answer has to satisfy.
    #[serde(alias = "expects")]
    pub(crate) schema: Option<String>,
    /// The ceiling. A step without one does not run: an agent with no ceiling
    /// is a bill nobody agreed to.
    #[serde(alias = "capUsd")]
    pub(crate) budget_usd: Option<f64>,
    pub(crate) model: Option<String>,
    /// Context to put in front of the agent, by key. Reaches the process as
    /// environment variables and never as text pasted into a command.
    pub(crate) inject: Vec<String>,
    /// The field of the answer that carries the verdict, and the value that
    /// means "not yet". Absent means this step never sends a card back.
    pub(crate) verdict_field: Option<String>,
    pub(crate) sends_back_when: Option<String>,
}

/// Reads an agent step's config, or says why it cannot be read.
pub(crate) fn readable(config: &str) -> Result<AgentConfig, String> {
    serde_json::from_str(config).map_err(|err| format!("this step's config is not readable: {err}"))
}
