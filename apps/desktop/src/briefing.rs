//! What the orchestrator's projects did while the person was away, in a few
//! lines: the sessions, the pull requests merged, and the decisions waiting
//! on them. Read from devpit and `gh`, never from a model.

use std::path::Path;

use devpit_core::i18n::t_for;
use devpit_rpc::{Briefing, Project};
use serde::Deserialize;

use crate::session_locale::SessionLocale;

/// What a briefing counts, gathered before it is said.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Gathered {
    pub running: usize,
    pub waiting: usize,
    pub ended: usize,
    /// Each pull request merged: its project, number and title.
    pub merged: Vec<(String, u32, String)>,
    pub memory: usize,
    pub schedules: usize,
    pub drafts: usize,
    pub plans: usize,
}

/// The most merged pull requests named before "and N more".
const NAMED: usize = 3;

/// `count` of something, said by the catalog's `key` in the session's
/// language; nothing for none.
fn counted(locale: SessionLocale, count: usize, key: &str) -> Option<String> {
    (count > 0).then(|| t_for(locale.locale(), key, &[("count", &count.to_string())]))
}

/// The briefing's lines: at most five, each a sentence, in the language of
/// the session they are said to.
pub(crate) fn lines(locale: SessionLocale, gathered: &Gathered) -> Vec<String> {
    let say = |key: &str, args: &[(&str, &str)]| t_for(locale.locale(), key, args);
    let sessions = [
        counted(locale, gathered.running, "briefing.running"),
        counted(locale, gathered.waiting, "briefing.waitingOnYou"),
        counted(locale, gathered.ended, "briefing.ended"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let sessions = if sessions.is_empty() {
        say("briefing.sessionsNone", &[])
    } else {
        say("briefing.sessions", &[("list", &sessions.join(", "))])
    };
    let merged = match gathered.merged.len() {
        0 => say("briefing.mergedNone", &[]),
        n => {
            let named: Vec<String> = gathered
                .merged
                .iter()
                .take(NAMED)
                .map(|(project, number, title)| format!("{project} #{number} {title}"))
                .collect();
            let more = n.saturating_sub(NAMED);
            let more = if more > 0 {
                say("briefing.mergedMore", &[("count", &more.to_string())])
            } else {
                String::new()
            };
            say(
                "briefing.merged",
                &[("list", &named.join("; ")), ("more", &more)],
            )
        }
    };
    let waiting = [
        counted(locale, gathered.memory, "briefing.memory"),
        counted(locale, gathered.schedules, "briefing.schedule"),
        counted(locale, gathered.drafts, "briefing.draft"),
        counted(locale, gathered.plans, "briefing.plan"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let waiting = if waiting.is_empty() {
        say("briefing.decisionsNone", &[])
    } else {
        say("briefing.decisions", &[("list", &waiting.join(", "))])
    };
    vec![sessions, merged, waiting]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Merged {
    number: u32,
    title: String,
    #[serde(default)]
    merged_at: Option<String>,
}

/// Whether a session that ended at `ended_at`, in milliseconds as the list of
/// ended sessions keeps it, ended after `since`, in seconds.
pub(crate) fn ended_since(ended_at: f64, since: i64) -> bool {
    ended_at >= since as f64 * 1000.0
}

/// The pull requests of the project in `root` merged since `since`, as `gh`
/// lists them; none when `gh` cannot say.
fn merged_in(root: &Path, since: i64) -> Vec<(u32, String)> {
    let Some(out) = crate::island_checks::gh(
        root,
        &[
            "pr",
            "list",
            "--state",
            "merged",
            "--limit",
            "30",
            "--json",
            "number,title,mergedAt",
        ],
    ) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<Merged>>(&out)
        .unwrap_or_default()
        .into_iter()
        .filter(|pull| {
            pull.merged_at
                .as_deref()
                .and_then(|at| crate::reminder_time::instant_of(at).ok())
                .is_some_and(|at| at >= since)
        })
        .map(|pull| (pull.number, pull.title))
        .collect()
}

/// What happened since `since` in `here` and the projects linked to it.
pub(crate) fn gathered(here: &Project, projects: &[Project], since: i64) -> Gathered {
    let folder = Path::new(&here.root_path);
    let linked = crate::orchestrator_links::linked(folder);
    let mine: Vec<&Project> = projects
        .iter()
        .filter(|one| linked.contains(&one.id))
        .collect();
    let mut gathered = Gathered::default();
    if let Some(profile) = here.orchestrator.as_deref() {
        if let Ok(live) = crate::live_sessions::orchestrator_sessions_now(profile) {
            let live = crate::orchestrator_links::reachable(here, live.sessions);
            gathered.waiting = live.iter().filter(|one| one.waiting.is_some()).count();
            gathered.running = live.len() - gathered.waiting;
            gathered.drafts = live.iter().filter(|one| one.draft.is_some()).count();
        }
        if let Ok(ended) = crate::ended_sessions::ended_now(profile) {
            gathered.ended = ended
                .sessions
                .iter()
                .filter(|one| ended_since(one.ended_at, since))
                .filter(|one| {
                    one.project_id
                        .as_deref()
                        .is_some_and(|id| id == here.id || linked.iter().any(|link| link == id))
                })
                .count();
        }
    }
    for project in &mine {
        let root = Path::new(&project.root_path);
        gathered.merged.extend(
            merged_in(root, since)
                .into_iter()
                .map(|(number, title)| (project.name.clone(), number, title)),
        );
        gathered.memory += crate::project_memory::proposals(root).len();
        if let Ok(board) = crate::board::board_get_now(project.id.clone()) {
            gathered.plans += board
                .cards
                .iter()
                .filter(|card| card.plan_waiting > 0)
                .count();
        }
    }
    if let Ok(store) = crate::projects::store() {
        gathered.schedules = crate::schedules::proposed_count(&store, folder);
    }
    gathered
}

/// The briefing for `here` since `since`.
pub(crate) fn briefing(
    locale: SessionLocale,
    here: &Project,
    projects: &[Project],
    since: i64,
) -> Briefing {
    Briefing {
        since: since as f64,
        lines: lines(locale, &gathered(here, projects, since)),
    }
}

/// `assistant.briefing` — what happened since `since`, for an orchestrator
/// whose Assistant mode is on; nothing otherwise.
#[tauri::command]
#[specta::specta]
pub async fn assistant_briefing(
    project_id: String,
    since: f64,
) -> Result<Option<Briefing>, devpit_rpc::RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let folder = crate::assistant::folder_of(&store, &project_id)?;
        if !crate::assistant::on(&store, &folder) || !since.is_finite() {
            return Ok(None);
        }
        let projects = crate::projects::project_list_plain_now()?.projects;
        Ok(projects
            .iter()
            .find(|one| one.id == project_id)
            .map(|here| briefing(SessionLocale::capture(), here, &projects, since as i64)))
    })
    .await
}

#[cfg(test)]
#[path = "briefing_tests.rs"]
mod tests;
