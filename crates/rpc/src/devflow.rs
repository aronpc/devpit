//! Devflow changes only the person makes, as the window asks for them.

use serde::{Deserialize, Serialize};
use specta::Type;

/// What the click does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DevflowChange {
    /// Devflow switched on in the project.
    Enable,
    /// Decisions moved from shadow to enforce in the project's Devflow.
    Enforce,
    /// A gate command a team layer brought, run for the first time since it
    /// or its commit changed.
    Command,
}

/// One change waiting for the person's click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowAsk {
    pub id: String,
    pub project_id: String,
    pub project: String,
    pub change: DevflowChange,
    /// Seconds since the epoch.
    pub asked_at: f64,
    /// What the click is about, for a command: the command and where it came
    /// from.
    pub detail: Option<String>,
}

/// A list, so tomorrow's field has somewhere to go.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowAsks {
    pub asks: Vec<DevflowAsk>,
}

/// One card of a Devflow plan, before it is a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCard {
    /// The plan's own id for it, which `depends_on` names.
    pub key: String,
    /// Without the `[devflow] ` the card's title gets.
    pub title: String,
    pub build: String,
    pub criteria: Vec<String>,
    pub files: Vec<String>,
    pub verify: String,
    pub depends_on: Vec<String>,
    /// How the card's boundary could be abused, each proved refused.
    #[serde(default)]
    pub abuse: Vec<String>,
    /// The plan says the card crosses a trust boundary.
    #[serde(default)]
    pub boundary: bool,
}

/// A plan previewed for an epic, waiting for the person to create its cards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowPlan {
    pub epic_id: String,
    pub cards: Vec<PlannedCard>,
    /// The person changed it on the board.
    pub edited: bool,
    /// Seconds since the epoch.
    pub planned_at: f64,
}

/// Response of `devflow.plan_create`: the cards made, in the plan's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowPlanCreated {
    pub card_ids: Vec<String>,
}

/// How one rubric agrees with the person in a project, from the decisions
/// they labelled by what they did, and the threshold that would agree most.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RubricAccuracy {
    pub rubric: String,
    /// Answered in the last ninety days.
    pub decisions: u32,
    pub labelled: u32,
    /// Labelled, judged pass or fail, and the same as the person.
    pub agreed: u32,
    /// Labelled and left grey.
    pub grey: u32,
    /// `agreed` of the labelled ones not left grey.
    pub accuracy: Option<f64>,
    /// The lowest `min` its yes/no questions have now.
    pub min: Option<f64>,
    pub suggested_min: Option<f64>,
    pub suggested_grey: Option<f64>,
    /// What `suggestedMin` would have agreed on, of the labelled.
    pub suggested_accuracy: Option<f64>,
    /// Why nothing is suggested yet.
    pub note: Option<String>,
    /// Why it stays in shadow under enforce: devpit's finding rubric acts
    /// only once it keeps the findings the person confirmed.
    #[serde(default)]
    pub held: Option<String>,
}

/// Response of `devflow.calibration`: each rubric the project was judged by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowCalibration {
    pub project_id: String,
    pub rubrics: Vec<RubricAccuracy>,
    /// Where Decisions stands for the project's Devflow now.
    #[serde(default)]
    pub decisions: Option<DevflowDecisions>,
}

/// Whether Devflow's questions reach Decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DecisionsState {
    On,
    /// `decisions.use` is false, or the project's state is never sent.
    Off,
    NoKey,
    /// The last call came to nothing.
    Failing,
}

/// Where Decisions stands, said the same in every Devflow reply, on the Jev
/// panel and on the card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevflowDecisions {
    pub state: DecisionsState,
    /// Why it is not `on`.
    pub reason: Option<String>,
}
