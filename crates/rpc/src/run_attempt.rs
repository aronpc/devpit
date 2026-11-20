//! The facts that distinguish one try of a step from another.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunAttempt {
    /// Absent on runs made before attempts were recorded.
    pub number: Option<u32>,
    /// The shown model is the request because the CLI reported no effective one.
    pub model_is_requested: bool,
}
