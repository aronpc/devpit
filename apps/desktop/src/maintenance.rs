//! Settings → Maintenance: how hard devpit works in the background, how long
//! its logs and run artifacts stay, and Labs, the switches for what is still
//! in beta.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use devflow_engine::runs;
use devpit_core::home::ProjectHome;
use devpit_core::{preference, Store};
use devpit_rpc::{ErrorCode, RpcError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::Manager as _;

/// Maintenance as Settings shows it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Maintenance {
    pub power_saver: bool,
    pub on_battery: bool,
    /// Days logs and run artifacts are kept; 0 keeps them until deleted.
    pub keep_days: u32,
    pub project_agent: bool,
    /// The projects linked to an orchestrator, each with its own switch.
    pub project_agent_projects: Vec<LabsProject>,
    pub canvas: bool,
    pub last: Option<CleanRun>,
}

/// A project the project agent can learn, and whether the person turned it on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabsProject {
    pub id: String,
    pub name: String,
    pub on: bool,
    /// Its facts resting only on files enter the memory without the person.
    pub accept_facts: bool,
}

/// What one cleaning deleted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanRun {
    /// Seconds since the epoch.
    pub at: f64,
    pub removed: u32,
    pub freed_bytes: f64,
}

/// The choices Settings offers; 0 is never.
pub(crate) const KEEP_DAYS: [u32; 5] = [0, 7, 14, 30, 90];
/// How much less often a probe runs while power is being saved.
pub(crate) const SLOWER: u32 = 4;
const POWER_SUPPLY: &str = "/sys/class/power_supply";
const FIRST_AFTER: Duration = Duration::from_secs(10 * 60);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

static SAVER: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
/// One cleaning at a time: the schedule and the button never race.
static CLEANING: Mutex<()> = Mutex::new(());

fn store() -> Result<Store, RpcError> {
    Ok(Store::open_default()?)
}

fn flag(store: &Store, key: &str) -> bool {
    store.preference_flag(key).ok().flatten().unwrap_or(false)
}

/// A Labs flag: on unless turned off, as everything in it was before Labs.
fn beta(store: &Store, key: &str) -> bool {
    store.preference_flag(key).ok().flatten().unwrap_or(true)
}

pub(crate) fn project_agent_on(store: &Store) -> bool {
    beta(store, preference::LABS_PROJECT_AGENT)
}

pub(crate) fn canvas_on(store: &Store) -> bool {
    beta(store, preference::LABS_CANVAS)
}

fn keep_days(store: &Store) -> u32 {
    store
        .preference(preference::MAINTENANCE_KEEP_DAYS)
        .ok()
        .flatten()
        .and_then(|days| days.parse().ok())
        .filter(|days| KEEP_DAYS.contains(days))
        .unwrap_or(0)
}

fn read(store: &Store) -> Result<Maintenance, RpcError> {
    Ok(Maintenance {
        power_saver: flag(store, preference::MAINTENANCE_POWER_SAVER),
        on_battery: on_battery_in(Path::new(POWER_SUPPLY)),
        keep_days: keep_days(store),
        project_agent: project_agent_on(store),
        project_agent_projects: Vec::new(),
        canvas: canvas_on(store),
        last: store
            .preference(preference::MAINTENANCE_CLEAN_LAST)?
            .and_then(|raw| serde_json::from_str(&raw).ok()),
    })
}

/// [`read`], with the projects linked to an orchestrator.
fn shown(store: &Store) -> Result<Maintenance, RpcError> {
    let mut shown = read(store)?;
    let projects = crate::projects::project_list_now()?.projects;
    shown.project_agent_projects = crate::project_onboarding::labs_projects(&projects, store);
    Ok(shown)
}

/// `maintenance.read`
#[tauri::command]
#[specta::specta]
pub async fn maintenance_read() -> Result<Maintenance, RpcError> {
    crate::off_main::blocking(|| shown(&store()?)).await
}

/// `maintenance.write` — what is given changes; what is not stays.
#[tauri::command]
#[specta::specta]
pub async fn maintenance_write(
    power_saver: Option<bool>,
    keep_days: Option<u32>,
    project_agent: Option<bool>,
    canvas: Option<bool>,
) -> Result<Maintenance, RpcError> {
    crate::off_main::blocking(move || {
        let store = store()?;
        if let Some(days) = keep_days {
            if !KEEP_DAYS.contains(&days) {
                return Err(RpcError::new(
                    ErrorCode::Invalid,
                    format!("keep logs for one of {KEEP_DAYS:?} days"),
                ));
            }
            store.set_preference(preference::MAINTENANCE_KEEP_DAYS, &days.to_string())?;
        }
        if let Some(on) = power_saver {
            store.set_preference_flag(preference::MAINTENANCE_POWER_SAVER, on)?;
            SAVER.store(on, Ordering::Relaxed);
        }
        if let Some(on) = project_agent {
            store.set_preference_flag(preference::LABS_PROJECT_AGENT, on)?;
        }
        if let Some(on) = canvas {
            store.set_preference_flag(preference::LABS_CANVAS, on)?;
        }
        shown(&store)
    })
    .await
}

/// `maintenance.clean_now` — the Delete now button, at the days chosen.
#[tauri::command]
#[specta::specta]
pub async fn maintenance_clean_now() -> Result<Maintenance, RpcError> {
    crate::off_main::blocking(|| {
        let store = store()?;
        clean_now(&store)?;
        shown(&store)
    })
    .await
}

/// `maintenance.project_agent` — turns the project agent on for one project,
/// where it learns its project if it never did, or off.
#[tauri::command]
#[specta::specta]
pub async fn maintenance_project_agent(
    app: tauri::AppHandle,
    project_id: String,
    on: bool,
) -> Result<Maintenance, RpcError> {
    crate::off_main::blocking(move || {
        let store = store()?;
        let projects = crate::projects::project_list_now()?.projects;
        crate::project_onboarding::switch(&app, &store, &projects, &project_id, on)
            .map_err(|why| RpcError::new(ErrorCode::Conflict, why))?;
        shown(&store)
    })
    .await
}

/// `maintenance.accept_facts` — one project's facts resting only on files
/// enter its memory without the person, or wait for them again.
#[tauri::command]
#[specta::specta]
pub async fn maintenance_accept_facts(
    project_id: String,
    on: bool,
) -> Result<Maintenance, RpcError> {
    crate::off_main::blocking(move || {
        let store = store()?;
        crate::project_onboarding::accept_file_facts(&store, &project_id, on)
            .map_err(RpcError::internal)?;
        shown(&store)
    })
    .await
}

/// Power saver as it was left, and the app the probes ask about the window.
pub(crate) fn restore(app: &tauri::AppHandle, store: &Store) {
    let _ = APP.set(app.clone());
    SAVER.store(
        flag(store, preference::MAINTENANCE_POWER_SAVER),
        Ordering::Relaxed,
    );
}

/// How long a probe waits before its next look: `every`, or longer while
/// power is saved and nobody is at the window or the machine is on battery.
pub(crate) fn stretch(every: Duration) -> Duration {
    if !SAVER.load(Ordering::Relaxed) {
        return every;
    }
    slower(every, away() || on_battery_in(Path::new(POWER_SUPPLY)))
}

pub(crate) fn slower(every: Duration, saving: bool) -> Duration {
    if saving {
        every * SLOWER
    } else {
        every
    }
}

fn away() -> bool {
    APP.get()
        .and_then(|app| app.try_state::<crate::error_sender::Presence>())
        .is_some_and(|presence| presence.away(devpit_core::reports::now()))
}

/// Whether a battery under `dir` (Linux's power supply class) is discharging.
pub(crate) fn on_battery_in(dir: &Path) -> bool {
    let Ok(supplies) = std::fs::read_dir(dir) else {
        return false;
    };
    supplies.flatten().any(|supply| {
        let said =
            |name: &str| std::fs::read_to_string(supply.path().join(name)).unwrap_or_default();
        said("type").trim() == "Battery" && said("status").trim() == "Discharging"
    })
}

/// What is older than `cutoff`: files in `logs`, and in each `<group>` under
/// `trees` the runs (folders named as Devflow names them) but the newest and,
/// for each file any of them holds, the newest run holding it: the gate, the
/// card and each stage read their own file from the newest run that has it,
/// so a card still open keeps them however old. Links are never followed,
/// and a folder that is not a run is never touched.
pub(crate) fn stale(logs: &Path, trees: &[PathBuf], cutoff: SystemTime) -> Vec<PathBuf> {
    let before = cutoff
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let mut found: Vec<PathBuf> = inside(logs, false)
        .into_iter()
        .filter(|path| {
            std::fs::symlink_metadata(path)
                .and_then(|meta| meta.modified())
                .is_ok_and(|at| at < cutoff)
        })
        .collect();
    for tree in trees {
        for group in inside(tree, true) {
            let runs: Vec<String> = inside(&group, true)
                .iter()
                .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
                .filter(|name| runs::seconds_of(name).is_some())
                .collect();
            let mut held: Vec<String> = Vec::new();
            for run in &runs {
                for file in inside(&group.join(run), false) {
                    let name = file.file_name().and_then(|name| name.to_str());
                    if let Some(name) = name.filter(|name| !held.iter().any(|one| one == name)) {
                        held.push(name.to_owned());
                    }
                }
            }
            let mut kept: Vec<String> = held
                .iter()
                .filter_map(|file| runs::newest_with(&group, file))
                .collect();
            kept.extend(
                runs.iter()
                    .max_by_key(|name| (runs::seconds_of(name), (*name).clone()))
                    .cloned(),
            );
            found.extend(
                runs.into_iter()
                    .filter(|name| !kept.contains(name))
                    .filter(|name| runs::seconds_of(name).is_some_and(|at| at < before))
                    .map(|name| group.join(name)),
            );
        }
    }
    found
}

/// The folders (`dirs`) or the files directly in `dir`, never a link; nothing
/// when `dir` is a link itself.
fn inside(dir: &Path, dirs: bool) -> Vec<PathBuf> {
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_type()
                .is_ok_and(|kind| !kind.is_symlink() && kind.is_dir() == dirs)
        })
        .map(|entry| entry.path())
        .collect()
}

/// Deletes what [`stale`] found, each resolved through links and checked to
/// be under the folder (of `bases`) it was found in.
pub(crate) fn delete(bases: &[PathBuf], paths: &[PathBuf], at: f64) -> CleanRun {
    let mut run = CleanRun {
        at,
        ..CleanRun::default()
    };
    let bases: Vec<PathBuf> = bases
        .iter()
        .filter(|base| {
            !std::fs::symlink_metadata(base).is_ok_and(|meta| meta.file_type().is_symlink())
        })
        .filter_map(|base| std::fs::canonicalize(base).ok())
        .collect();
    for path in paths {
        let under = std::fs::canonicalize(path).is_ok_and(|real| {
            bases
                .iter()
                .any(|base| real.starts_with(base) && &real != base)
        });
        if !under {
            continue;
        }
        let freed = if path.is_dir() {
            crate::worktree_sweep::drop_dir(path)
        } else {
            let bytes = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
            std::fs::remove_file(path).ok().map(|()| bytes)
        };
        if let Some(bytes) = freed {
            run.removed += 1;
            run.freed_bytes += bytes as f64;
        }
    }
    run
}

fn clean_now(store: &Store) -> Result<(), RpcError> {
    let days = keep_days(store);
    if days == 0 {
        return Ok(());
    }
    let _one = CLEANING.lock().unwrap_or_else(|held| held.into_inner());
    let root = Store::root()?;
    let mut trees = Vec::new();
    for project in store.projects()? {
        if let Ok(home) = ProjectHome::of(store, &root, &project.id) {
            trees.push(home.devflow_runs());
            trees.push(home.devflow_reviews());
        }
    }
    let cutoff = SystemTime::now() - Duration::from_secs(u64::from(days) * 24 * 60 * 60);
    let logs = devpit_core::reports::ErrorLog::at(&root)
        .path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("logs"));
    let found = stale(&logs, &trees, cutoff);
    trees.push(logs);
    let run = delete(&trees, &found, devpit_core::reports::now() as f64);
    let json = serde_json::to_string(&run).map_err(|err| RpcError::internal(err.to_string()))?;
    store.set_preference(preference::MAINTENANCE_CLEAN_LAST, &json)?;
    Ok(())
}

/// Cleans a while after start, then a few times a day, when days are set.
pub(crate) fn watch() {
    let _ = std::thread::Builder::new()
        .name("maintenance".to_owned())
        .spawn(|| {
            std::thread::sleep(FIRST_AFTER);
            loop {
                if let Err(err) = store().and_then(|store| clean_now(&store)) {
                    devpit_core::reports::background("maintenance", &err.message);
                }
                std::thread::sleep(EVERY);
            }
        });
}

#[cfg(test)]
#[path = "maintenance_tests.rs"]
mod tests;
