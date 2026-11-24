//! The assistant's clock. Every minute the routines the person set run when
//! their time came, paused ones excepted; and, while an orchestrator's
//! Assistant mode is on, its heartbeat looks every few minutes for what needs
//! the person — CI failing, a session stuck on a question, a card blocked.
//!
//! The heartbeat speaks only when it found something it did not say at its
//! last beat: nothing found is silence, and a thing still there is said once.
//! What it and the routines say goes to the bell, the person's channels and,
//! when devpit is behind, the system. Nothing here starts work.

use std::path::Path;
use std::time::Duration;

use devpit_core::Store;
use devpit_rpc::{ChannelEvent, Heartbeat, Project, RoutineKind, RpcError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::schedules::Kept;
use crate::session_locale::SessionLocale;

/// What the bell files the assistant's notices under.
pub(crate) const KIND: &str = "assistant";
/// How often the clock looks.
const TICK: Duration = Duration::from_secs(60);
/// The heartbeat's settings, among the preferences.
const HEARTBEAT: &str = "assistant.heartbeat";
/// The last beat, among an orchestrator's settings.
const BEAT: &str = "beat";
/// The most lines one notice carries.
const MOST_LINES: usize = 8;
/// What each kind of finding's key starts with.
const CI: &str = "ci:";
const WAITING: &str = "waiting:";
const BLOCKED: &str = "blocked:";

/// One thing that needs the person, named so a beat can tell it said it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub key: String,
    pub line: String,
    /// The terminal of a session stopped on a question, whose choices go
    /// with the notice as buttons.
    pub asking: Option<String>,
}

/// The last beat: when, and what it found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Beat {
    pub at: Option<i64>,
    pub keys: Vec<String>,
    pub lines: Vec<String>,
}

/// The heartbeat's settings, as the person left them in Settings.
pub(crate) fn settings(store: &Store) -> Heartbeat {
    store
        .preference(HEARTBEAT)
        .ok()
        .flatten()
        .and_then(|kept| serde_json::from_str(&kept).ok())
        .unwrap_or(Heartbeat {
            minutes: 15,
            waiting_minutes: 10,
            ci: true,
            waiting: true,
            blocked: true,
        })
}

/// Whether the heartbeat beats at `now`: only with the mode on, and every
/// `minutes` after the last beat.
pub(crate) fn beats(on: bool, last: Option<i64>, minutes: u32, now: i64) -> bool {
    on && last.is_none_or(|last| now - last >= i64::from(minutes.max(1)) * 60)
}

/// What a beat says: what it found that the last beat did not. Nothing
/// found, or nothing new, is silence.
pub(crate) fn news<'a>(said: &[String], found: &'a [Finding]) -> Vec<&'a Finding> {
    found
        .iter()
        .filter(|one| !said.contains(&one.key))
        .collect()
}

/// What the next beat compares with: what this one found, and what the last
/// one found of a kind this one could not read — a `gh` that timed out is not
/// a failure fixed, and must not make it news again.
pub(crate) fn carried(last: &[String], found: &[Finding], unread: &[&str]) -> Vec<String> {
    let mut kept: Vec<String> = found.iter().map(|one| one.key.clone()).collect();
    for key in last {
        if unread.iter().any(|kind| key.starts_with(kind)) && !kept.contains(key) {
            kept.push(key.clone());
        }
    }
    kept
}

/// The notices a beat's news makes: each question alone, so its buttons
/// answer the one it names, and everything else together.
pub(crate) fn notices(new: &[&Finding]) -> Vec<(Vec<String>, Option<String>)> {
    let (asking, rest): (Vec<&&Finding>, Vec<&&Finding>) =
        new.iter().partition(|one| one.asking.is_some());
    let mut notices: Vec<(Vec<String>, Option<String>)> = asking
        .into_iter()
        .map(|one| (vec![one.line.clone()], one.asking.clone()))
        .collect();
    if !rest.is_empty() {
        notices.push((rest.iter().map(|one| one.line.clone()).collect(), None));
    }
    notices
}

/// The routines whose time came by `now`, paused ones left: each moved to its
/// next run, and answered with when it last ran before.
pub(crate) fn due(all: &mut [Kept], now: i64) -> Vec<(RoutineKind, Option<i64>)> {
    all.iter_mut()
        .filter(|kept| !kept.paused && kept.next_at.is_some_and(|at| at <= now))
        .map(|kept| {
            let before = kept.last_run.replace(now);
            kept.next_at = Some(crate::schedules::next_run(&kept.routine, now));
            (kept.routine.kind, before)
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pull {
    number: u32,
    title: String,
    #[serde(default)]
    status_check_rollup: Vec<Check>,
}

#[derive(Deserialize)]
struct Check {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
}

/// The open pull requests of `project`, with whether their checks fail;
/// nothing when `gh` could not say.
fn pulls(project: &Project) -> Option<Vec<(u32, String, bool)>> {
    let out = crate::island_checks::gh(
        Path::new(&project.root_path),
        &[
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "30",
            "--json",
            "number,title,statusCheckRollup",
        ],
    )?;
    let pulls = serde_json::from_str::<Vec<Pull>>(&out).ok()?;
    let read = pulls
        .into_iter()
        .map(|pull| {
            let checks = pull.status_check_rollup.iter().map(|check| {
                (
                    check
                        .status
                        .as_deref()
                        .or(check.state.as_deref())
                        .unwrap_or(""),
                    check.conclusion.as_deref().unwrap_or(""),
                )
            });
            let failing = crate::island_checks::rollup(checks) == Some("failing");
            (pull.number, pull.title, failing)
        })
        .collect();
    Some(read)
}

/// The cards of `project` in a lane whose role is `role`; nothing when its
/// board did not read.
fn in_lane(project: &Project, role: &str) -> Option<Vec<devpit_rpc::Card>> {
    let board = crate::board::board_get_now(project.id.clone()).ok()?;
    let lanes: Vec<&str> = board
        .columns
        .iter()
        .filter(|column| column.role.as_deref() == Some(role))
        .map(|column| column.id.as_str())
        .collect();
    Some(
        board
            .cards
            .into_iter()
            .filter(|card| lanes.contains(&card.column_id.as_str()))
            .collect(),
    )
}

fn numbered(project: &Project, card: &devpit_rpc::Card) -> String {
    match card.number {
        Some(number) => format!("{} #{number} {}", project.name, card.title),
        None => format!("{} {}", project.name, card.title),
    }
}

/// What needs the person in `here` and its linked projects, by `beat`'s
/// triggers, and the kinds of finding a source could not be read for.
pub(crate) fn findings(
    here: &Project,
    projects: &[Project],
    beat: &Heartbeat,
    now: i64,
) -> (Vec<Finding>, Vec<&'static str>) {
    let linked = crate::orchestrator_links::linked(Path::new(&here.root_path));
    let mine: Vec<&Project> = projects
        .iter()
        .filter(|one| linked.contains(&one.id))
        .collect();
    let mut found = Vec::new();
    let mut unread = Vec::new();
    if beat.ci {
        for project in &mine {
            let Some(read) = pulls(project) else {
                unread.push(CI);
                continue;
            };
            for (number, title, _) in read.into_iter().filter(|pull| pull.2) {
                found.push(Finding {
                    key: format!("{CI}{}#{number}", project.id),
                    line: format!("CI fails on {} #{number} {title}", project.name),
                    asking: None,
                });
            }
        }
    }
    if beat.waiting {
        if let Some(profile) = here.orchestrator.as_deref() {
            match crate::live_sessions::orchestrator_sessions_now(profile) {
                Ok(live) => {
                    let live = crate::orchestrator_links::reachable(here, live.sessions);
                    found.extend(stuck(&live, beat.waiting_minutes, now, |pane| {
                        crate::pane_io::target_of(&pane.project_id, &pane.pane_id).ok()
                    }));
                }
                Err(_) => unread.push(WAITING),
            }
        }
    }
    if beat.blocked {
        for project in &mine {
            let Some(cards) = in_lane(project, "doing") else {
                unread.push(BLOCKED);
                continue;
            };
            for card in cards {
                let holding: Vec<String> = card
                    .blocked_by
                    .iter()
                    .filter(|by| by.holds())
                    .map(|by| {
                        by.number
                            .map_or_else(|| by.title.clone(), |n| format!("#{n}"))
                    })
                    .collect();
                if !holding.is_empty() {
                    found.push(Finding {
                        key: format!("{BLOCKED}{}", card.id),
                        line: format!(
                            "{} is blocked by {}",
                            numbered(project, &card),
                            holding.join(", ")
                        ),
                        asking: None,
                    });
                }
            }
        }
    }
    (found, unread)
}

/// The sessions stopped on a question for more than `minutes` at `now`, each
/// with its terminal as `target` finds it.
pub(crate) fn stuck(
    live: &[devpit_rpc::LiveSession],
    minutes: u32,
    now: i64,
    target: impl Fn(&devpit_rpc::LivePane) -> Option<String>,
) -> Vec<Finding> {
    live.iter()
        .filter_map(|one| {
            let question = one.waiting.as_ref()?;
            let since = (one.since? / 1000.0) as i64;
            let waited = (now - since) / 60;
            (waited > i64::from(minutes)).then(|| Finding {
                key: format!("{WAITING}{}:{}:{}", one.name, one.pid, question.question),
                line: format!(
                    "{} has waited {waited} min on: {}",
                    one.name, question.question
                ),
                asking: one.pane.as_ref().and_then(&target),
            })
        })
        .collect()
}

/// What a routine of `kind` says for `here`, `since` the run before.
fn report(kind: RoutineKind, here: &Project, projects: &[Project], since: i64) -> Vec<String> {
    let linked = crate::orchestrator_links::linked(Path::new(&here.root_path));
    let mine = projects.iter().filter(|one| linked.contains(&one.id));
    let lines: Vec<String> = match kind {
        RoutineKind::Morning => {
            crate::briefing::briefing(SessionLocale::capture(), here, projects, since).lines
        }
        RoutineKind::Ci => mine
            .flat_map(|project| {
                pulls(project)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|pull| pull.2)
                    .map(|(number, title, _)| format!("{} #{number} {title}", project.name))
            })
            .collect(),
        RoutineKind::Prs => mine
            .flat_map(|project| {
                pulls(project)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(number, title, _)| format!("{} #{number} {title}", project.name))
            })
            .collect(),
        RoutineKind::Review => mine
            .flat_map(|project| {
                in_lane(project, "check")
                    .unwrap_or_default()
                    .into_iter()
                    .map(|card| numbered(project, &card))
            })
            .collect(),
    };
    if lines.is_empty() {
        return vec!["Nothing to report.".to_owned()];
    }
    lines
}

/// The notice for `lines` under `title`: its body, at most [`MOST_LINES`]
/// lines, and what goes on the person's channels — with the terminal of a
/// question, whose choices go as buttons.
pub(crate) fn told(
    title: &str,
    lines: &[String],
    asking: Option<String>,
) -> (String, crate::channels::Told) {
    let mut shown: Vec<String> = lines.iter().take(MOST_LINES).cloned().collect();
    if lines.len() > MOST_LINES {
        shown.push(format!("and {} more", lines.len() - MOST_LINES));
    }
    let body = shown.join("\n");
    let told = crate::channels::Told {
        event: ChannelEvent::Assistant,
        line: format!("{title}\n{body}"),
        bare: "Your assistant has something for you.".to_owned(),
        actions: Vec::new(),
        asking,
        about: None,
    };
    (body, told)
}

/// Says `lines` under `title` for `here`: in the bell, on the person's
/// channels and, when devpit is behind, as a system notification.
pub(crate) fn tell(
    app: &AppHandle,
    store: &Store,
    here: &Project,
    title: &str,
    lines: &[String],
    asking: Option<String>,
) {
    let (body, told) = told(title, lines, asking);
    let _ = store.add_notice(Some(&here.id), KIND, title, Some(&body), None);
    let _ = app.emit(crate::notices::RANG, ());
    crate::channels::emit(told);
    if !crate::island_notify::in_front(app) {
        crate::island_notify::show(app, title, &body, false);
    }
}

fn beat_of(store: &Store, folder: &Path) -> Beat {
    store
        .orchestrator_settings(folder)
        .ok()
        .and_then(|all| serde_json::from_value(all[BEAT].clone()).ok())
        .unwrap_or_default()
}

/// One look at every orchestrator: its routines, and its heartbeat.
fn tick(app: &AppHandle, now: i64) {
    let (Ok(store), Ok(listed)) = (
        Store::open_default(),
        crate::projects::project_list_plain_now(),
    ) else {
        return;
    };
    let projects = listed.projects;
    let beat = settings(&store);
    for here in projects.iter().filter(|one| one.orchestrator.is_some()) {
        let folder = Path::new(&here.root_path);
        let mut all = crate::schedules::routines(&store, folder);
        let ran = due(&mut all, now);
        if !ran.is_empty() && crate::schedules::keep_routines(&store, folder, &all).is_ok() {
            for (kind, before) in ran {
                let since = before.unwrap_or(now - 86_400);
                let lines = report(kind, here, &projects, since);
                tell(
                    app,
                    &store,
                    here,
                    crate::schedules::title_of(kind),
                    &lines,
                    None,
                );
            }
            let _ = app.emit(crate::schedules::CHANGED, &here.id);
        }
        // Paused, the heartbeat holds what it would say until the pause ends.
        let last = beat_of(&store, folder);
        if crate::pausing::paused()
            || !beats(
                crate::assistant::on(&store, folder),
                last.at,
                beat.minutes,
                now,
            )
        {
            continue;
        }
        let (found, unread) = findings(here, &projects, &beat, now);
        for (lines, asking) in notices(&news(&last.keys, &found)) {
            tell(app, &store, here, "Needs you", &lines, asking);
        }
        let kept = Beat {
            at: Some(now),
            keys: carried(&last.keys, &found, &unread),
            lines: found.iter().map(|one| one.line.clone()).collect(),
        };
        let _ = store.set_orchestrator_setting(folder, BEAT, json!(kept));
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

/// Starts the clock. It lives as long as the app.
pub(crate) fn watch(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK);
        tick(&app, now());
    });
}

/// `devpit_heartbeat_status`: whether it beats, how, and what it last found.
pub(crate) fn status(here: &Project) -> Result<Value, String> {
    let store = crate::projects::store().map_err(|err| err.message)?;
    let folder = Path::new(&here.root_path);
    let last = beat_of(&store, folder);
    Ok(json!({
        "on": crate::assistant::on(&store, folder),
        "settings": settings(&store),
        "lastBeat": last.at,
        "found": last.lines,
    }))
}

/// `devpit_telegram_send`: words from the orchestrator to the person's
/// Telegram, as much of them as the chat's privacy lets out.
pub(crate) fn telegram(params: &Value) -> Result<Value, String> {
    let text = params
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or("say what to send in `text`")?;
    if text.chars().count() > crate::channel_talk::LONGEST_MESSAGE {
        return Err(format!(
            "at most {} characters go in one message",
            crate::channel_talk::LONGEST_MESSAGE
        ));
    }
    if crate::pausing::paused() {
        return Err(
            "devpit is paused: nothing goes to the person's channels until the pause ends"
                .to_owned(),
        );
    }
    if crate::telegram::connected().is_none() && crate::telegram_devpit::connected().is_none() {
        return Err(
            "Telegram is not set up: the person links it in Settings → Channels".to_owned(),
        );
    }
    let (_, mut said) = told("From your assistant", &[], None);
    text.clone_into(&mut said.line);
    crate::channels::send_on(crate::channel_rules::Rung::Telegram, &[&said]);
    Ok(json!({ "sent": true, "titles": crate::telegram::titles() }))
}

/// `assistant.heartbeat` — the heartbeat's settings.
#[tauri::command]
#[specta::specta]
pub async fn assistant_heartbeat() -> Result<Heartbeat, RpcError> {
    crate::off_main::blocking(|| Ok(settings(&crate::projects::store()?))).await
}

/// `assistant.heartbeat_set` — the heartbeat's settings, as the person chose.
#[tauri::command]
#[specta::specta]
pub async fn assistant_heartbeat_set(heartbeat: Heartbeat) -> Result<Heartbeat, RpcError> {
    crate::off_main::blocking(move || {
        if heartbeat.minutes == 0 || heartbeat.minutes > 24 * 60 {
            return Err(RpcError::new(
                devpit_rpc::ErrorCode::Invalid,
                "the heartbeat beats between every minute and once a day",
            ));
        }
        let store = crate::projects::store()?;
        let text =
            serde_json::to_string(&heartbeat).map_err(|err| RpcError::internal(err.to_string()))?;
        store.set_preference(HEARTBEAT, &text)?;
        Ok(settings(&store))
    })
    .await
}

#[cfg(test)]
#[path = "assistant_watch_tests.rs"]
mod tests;
