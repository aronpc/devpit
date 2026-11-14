//! The sessions an orchestrator started that no Claude Code registry of its
//! own lists: one on another CLI, which keeps no list devpit reads, and every
//! one an orchestrator on another CLI started, since it has no Claude Code
//! account of its own. devpit typed each into a pane, so the pane is the
//! session: its name is devpit's, its state the pane's, and it ends with the
//! pane's agent.

use std::sync::{Mutex, OnceLock};

use devpit_rpc::{LivePane, LiveSession};

/// One session devpit started for an orchestrator.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Kept {
    /// The orchestrator's profile: whose session it is.
    pub profile: String,
    /// The profile it runs as.
    pub runs_as: String,
    pub name: String,
    pub project_id: String,
    pub project_name: Option<String>,
    pub card_id: Option<String>,
    pub cwd: String,
    pub pane_id: String,
}

/// How its pane stands, from one look.
pub(crate) struct Looked {
    /// Whether its agent, not the shell, is in front.
    pub agent: bool,
    pub screen: Option<String>,
    pub hook: Option<crate::card_activity::Doing>,
}

fn kept() -> &'static Mutex<Vec<Kept>> {
    static KEPT: OnceLock<Mutex<Vec<Kept>>> = OnceLock::new();
    KEPT.get_or_init(Mutex::default)
}

fn held() -> std::sync::MutexGuard<'static, Vec<Kept>> {
    kept()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether a session devpit starts for the orchestrator speaking as
/// `profile` is kept here: no registry that orchestrator reads lists it.
pub(crate) fn keeps(orchestrator_on_claude: bool, runs_on_claude: bool) -> bool {
    !(orchestrator_on_claude && runs_on_claude)
}

/// Remembers a session devpit just typed into a pane, in place of one of the
/// same name.
pub(crate) fn began(one: Kept) {
    let mut held = held();
    held.retain(|was| !(was.profile == one.profile && was.name == one.name));
    held.push(one);
}

/// The sessions of `profile`'s orchestrator still running, as rows like the
/// rest. One whose pane is gone, or whose agent left it, has ended.
pub(crate) fn rows(profile: &str) -> Vec<LiveSession> {
    rows_in(&mut held(), profile, look, crate::session_bus::now_ms())
}

/// [`rows`], over a list it is handed and a look it is given.
pub(crate) fn rows_in(
    held: &mut Vec<Kept>,
    profile: &str,
    look: impl Fn(&Kept) -> Option<Looked>,
    now: f64,
) -> Vec<LiveSession> {
    let mut rows = Vec::new();
    held.retain(|one| {
        if one.profile != profile {
            return true;
        }
        let Some(looked) = look(one).filter(|looked| looked.agent) else {
            return false;
        };
        rows.push(row(one, &looked, now));
        true
    });
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

/// A kept session as `devpit_sessions` lists it.
fn row(one: &Kept, looked: &Looked, now: f64) -> LiveSession {
    let seen = crate::session_bus::Seen {
        hook: looked.hook,
        agent: looked.agent,
        screen: looked.screen.as_deref(),
    };
    let status = match crate::session_bus::pane_now(&seen, None, now) {
        crate::session_bus::Pane::Free => "idle",
        _ => "busy",
    };
    LiveSession {
        name: one.name.clone(),
        pid: 0,
        job: None,
        status: status.to_owned(),
        kind: "interactive".to_owned(),
        cwd: one.cwd.clone(),
        project_id: Some(one.project_id.clone()),
        project_name: one.project_name.clone(),
        card_id: one.card_id.clone(),
        since: None,
        in_devpit: true,
        waiting: looked
            .screen
            .as_deref()
            .and_then(crate::live_prompt::pending),
        pane: Some(LivePane {
            project_id: one.project_id.clone(),
            pane_id: one.pane_id.clone(),
        }),
        session_id: None,
        step: None,
        draft: None,
        remote: Default::default(),
    }
}

/// The pane's terminal, as tmux or the daemon names it.
fn target(one: &Kept) -> Option<String> {
    crate::pane_io::target_of(&one.project_id, &one.pane_id).ok()
}

fn look(one: &Kept) -> Option<Looked> {
    let target = target(one)?;
    Some(Looked {
        agent: !crate::pane_io::shell_in_front(&target).ok()?,
        screen: crate::pane_io::screen(&target),
        hook: crate::pane_said::heard_of(&one.pane_id),
    })
}

/// The one called `name` of `profile`'s orchestrator.
fn named(profile: &str, name: &str) -> Option<Kept> {
    held()
        .iter()
        .find(|one| one.profile == profile && one.name == name)
        .cloned()
}

/// The terminal of the session called `name`, while devpit keeps it.
pub(crate) fn target_named(profile: &str, name: &str) -> Option<String> {
    target(&named(profile, name)?)
}

/// The session called `name`, as stopping it needs it.
pub(crate) fn running_named(profile: &str, name: &str) -> Option<crate::live_sessions::Running> {
    let one = named(profile, name)?;
    let looked = look(&one).filter(|looked| looked.agent)?;
    let row = row(&one, &looked, crate::session_bus::now_ms());
    Some(crate::live_sessions::Running {
        pid: 0,
        pane: row.pane,
        job: None,
        status: row.status,
        cwd: one.cwd,
        session_id: None,
    })
}

/// What an orchestrator is told of a session devpit keeps: up at once, since
/// a message waits in devpit for its prompt rather than being lost.
const UP: &str = "It runs in a terminal tab of the project, where the person can watch it. Message it by this name with devpit_session_message, and devpit_session_notify_idle tells you once when it is done.";

/// A start's answer once its session can be messaged: at once for one devpit
/// keeps, else once the CLI lists it.
pub(crate) fn once_up(
    mut answer: serde_json::Value,
    profile: &str,
    project_id: &str,
) -> serde_json::Value {
    if answer["kept"] == true {
        answer["up"] = serde_json::json!(true);
        answer["next"] = serde_json::json!(UP);
        return answer;
    }
    crate::starting::once_up(
        answer,
        &crate::starting::listed_in(profile, project_id),
        crate::starting::UP_WITHIN,
    )
}

#[cfg(test)]
#[path = "pane_sessions_tests.rs"]
mod tests;
