//! The choices that can take over a paused agent step.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunSwitchProfile {
    pub id: String,
    pub label: String,
    pub model: Option<String>,
}

/// Response of `run.switch_options`; an object so the pause can grow facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunSwitchOptions {
    pub reason: String,
    pub profiles: Vec<RunSwitchProfile>,
}
