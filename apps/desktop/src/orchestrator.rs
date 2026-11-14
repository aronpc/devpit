//! `orchestrator.open` — the chat that sees every project, one per profile.
//!
//! An orchestrator is a project whose folder is devpit's own, under
//! `orchestrator/<profile>`. Being a project is what gives it the chat, the
//! tabs, the files and the MCP's scope for nothing; being known by its folder
//! is what spares the store a column that would say the same thing.

use std::path::Path;

use devpit_core::Store;
use devpit_rpc::{ErrorCode, Project, RpcError};

/// devpit's half of the brief: rewritten on every opening, so an orchestrator
/// made by an older build learns how this one works. `CLAUDE.md` imports it.
const DEVPIT_BRIEF: (&str, &str) = (
    ".devpit/orchestrator.md",
    include_str!("orchestrator_devpit.md"),
);

/// What a new orchestrator's folder starts with. Written only when missing:
/// the person edits these, and a second opening must not undo that.
const SEEDED: &[(&str, &str)] = &[
    ("CLAUDE.md", include_str!("orchestrator_claude.md")),
    // Gemini follows the same imports; `AGENTS.md` is written from both.
    ("GEMINI.md", "@CLAUDE.md\n"),
    // Where it keeps what it learns: an orchestrator's notes, not a
    // repository — there is no remote to push them to, so no git either.
    ("context/projects/.keep", ""),
    ("decisions/.keep", ""),
    ("docs/.keep", ""),
    ("artifacts/.keep", ""),
];

/// `orchestrator.create` — a new orchestrator speaking as this profile.
///
/// Several may share a profile: each is its own folder, brief and notes, and
/// all of them reach the same sessions, since those belong to the account.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_create(profile_id: String, name: String) -> Result<Project, RpcError> {
    crate::off_main::blocking(move || orchestrator_create_now(profile_id, name)).await
}

/// [`orchestrator_create`], on the calling thread.
pub(crate) fn orchestrator_create_now(
    profile_id: String,
    name: String,
) -> Result<Project, RpcError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            "an orchestrator needs a name of up to 60 characters",
        ));
    }
    let store = crate::projects::store()?;
    let profile = orchestrable(&store, &profile_id)?;
    let root = Store::root().map_err(|err| RpcError::internal(err.to_string()))?;
    let folder = free_folder(&root, name)
        .ok_or_else(|| RpcError::new(ErrorCode::Invalid, "that name cannot name a folder"))?;
    seed(&folder).map_err(|err| RpcError::internal(err.to_string()))?;
    speaks_as(&store, &folder, &profile.id)?;
    let here = folder
        .canonicalize()
        .map_err(|err| RpcError::internal(err.to_string()))?;
    let made = crate::projects::project_add_now(here.display().to_string())?;
    let named = crate::project_naming::project_edit_now(
        made.id.clone(),
        name.to_owned(),
        None,
        None,
        None,
    )?;
    Ok(named
        .projects
        .into_iter()
        .find(|one| one.id == made.id)
        .unwrap_or(made))
}

/// `orchestrator.refresh` — devpit's half of an orchestrator's brief, brought
/// up to this build as it is opened.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_refresh(project_id: String) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let (_, root) = crate::projects::locate(&store, &project_id)?;
        seed(&root)
            .and_then(|()| crate::assistant::brief_in(&root, crate::assistant::on(&store, &root)))
            .map_err(|err| RpcError::internal(err.to_string()))
    })
    .await
}

/// The folders an orchestrator reads beyond its own: the projects the person
/// linked it to. Nothing for a project, which stays inside itself.
pub(crate) fn reaches(project_id: &str) -> Vec<String> {
    let Ok(listed) = crate::projects::project_list_now() else {
        return Vec::new();
    };
    let Some(here) = listed
        .projects
        .iter()
        .find(|one| one.id == project_id && one.orchestrator.is_some())
    else {
        return Vec::new();
    };
    let linked = crate::orchestrator_links::linked(Path::new(&here.root_path));
    listed
        .projects
        .iter()
        .filter(|one| linked.contains(&one.id))
        .map(|one| one.root_path.clone())
        .collect()
}

/// The name an orchestrator's chat runs under, from its folder, so it is the
/// same after every restart: its sessions report back to it by name.
pub(crate) fn address(folder: &Path) -> Option<String> {
    let name = folder.file_name()?.to_str()?;
    Some(format!("orchestrator-{name}"))
}

/// [`address`] for a chat in `project_id`, when that project is an
/// orchestrator; nothing for any other project, whose chats stay unnamed.
pub(crate) fn address_of(project_id: &str) -> Option<String> {
    let store = crate::projects::store().ok()?;
    let (_, root) = crate::projects::locate(&store, project_id).ok()?;
    let home = Store::root().ok()?.canonicalize().ok()?;
    devpit_core::home::orchestrator_of(&store, &home, &root)?;
    address(&root)
}

/// The profile, if an orchestrator can speak as it.
fn orchestrable(
    store: &devpit_core::Store,
    profile_id: &str,
) -> Result<devpit_rpc::Profile, RpcError> {
    let profile = crate::agent_profiles::all(store)?
        .into_iter()
        .find(|one| one.id == profile_id)
        .ok_or_else(|| {
            RpcError::new(
                ErrorCode::NotFound,
                format!("no profile called {profile_id}"),
            )
        })?;
    may_orchestrate(&profile).map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    Ok(profile)
}

/// Whether an orchestrator can speak as `profile`: a CLI devpit holds a chat
/// with, since an orchestrator is a chat, and one devpit can start rather
/// than a shell function it has not read yet.
pub(crate) fn may_orchestrate(profile: &devpit_rpc::Profile) -> Result<(), String> {
    if devpit_agentcli::driver::driver_for(&profile.driver, &profile.base).is_none() {
        return Err(format!(
            "{} is terminal only here: an orchestrator is a chat, and runs on a CLI devpit holds a chat with",
            profile.label
        ));
    }
    if profile.path.is_none() {
        return Err(format!(
            "devpit has not read what {} runs yet — open it in Settings → Providers",
            profile.command
        ));
    }
    Ok(())
}

/// Which account an orchestrator speaks as, kept by its folder.
pub(crate) fn speaks_as(store: &Store, folder: &Path, profile_id: &str) -> Result<(), RpcError> {
    crate::orchestrator_links::set_in(store, folder, "profile", serde_json::json!(profile_id))
}

/// `orchestrator.account` — the account an orchestrator speaks as, changed,
/// on the same CLI or another. Its next conversation starts under that
/// account, handed over from its notes; those so far stay with the one they
/// began with.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_account(project_id: String, profile_id: String) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let profile = orchestrable(&store, &profile_id)?;
        let (_, root) = crate::projects::locate(&store, &project_id)?;
        let home = Store::root()
            .ok()
            .and_then(|home| home.canonicalize().ok())
            .unwrap_or_default();
        let Some(was) = devpit_core::home::orchestrator_of(&store, &home, &root) else {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "that project is not an orchestrator",
            ));
        };
        crate::orchestrator_handoff::switched(&store, &root, &was, &profile.id);
        speaks_as(&store, &root, &profile.id)
    })
    .await
}

/// A folder for `name` that nothing is using yet.
pub(crate) fn free_folder(root: &Path, name: &str) -> Option<std::path::PathBuf> {
    let base = slug(name);
    (0..100).find_map(|n| {
        let tried = if n == 0 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        devpit_core::home::orchestrator_dir(root, &tried).filter(|folder| !folder.exists())
    })
}

/// A folder name from a person's name for it. Only the folder is plain; the
/// name they gave is what the rail shows.
pub(crate) fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in devpit_git::lifecycle::unaccented(name)
        .chars()
        .flat_map(char::to_lowercase)
    {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let kept: String = out.trim_matches('-').chars().take(40).collect();
    let kept = kept.trim_end_matches('-');
    if kept.is_empty() {
        "orchestrator".to_owned()
    } else {
        kept.to_owned()
    }
}

/// devpit's brief as this build writes it: the text, under a line naming the
/// build — so "which brief is this orchestrator reading" is answered by
/// opening the file rather than by diffing it against a release.
pub(crate) fn brief() -> String {
    format!(
        "<!-- written by devpit {} -->\n{}",
        env!("CARGO_PKG_VERSION"),
        DEVPIT_BRIEF.1
    )
}

/// Writes devpit's brief as this build has it, and what a new orchestrator
/// starts with, leaving the rest alone.
pub(crate) fn seed(folder: &Path) -> std::io::Result<()> {
    let text = brief();
    let brief = folder.join(DEVPIT_BRIEF.0);
    if std::fs::read_to_string(&brief).ok() != Some(text.clone()) {
        std::fs::create_dir_all(brief.parent().unwrap_or(folder))?;
        std::fs::write(&brief, text)?;
    }
    for (name, text) in SEEDED {
        let path = folder.join(name);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    }
    crate::orchestrator_brief::write_agents(folder)?;
    crate::orchestrator_view::set_up(folder)
}

/// Every orchestrator's brief brought up to this build, and what could not
/// be. Opening one refreshes it too, but one nobody opens was left with an
/// older build's brief for anything else that reads its folder.
pub(crate) fn seed_all(store: &Store, root: &Path) -> Vec<(String, std::io::Error)> {
    let Ok(rows) = store.projects() else {
        return Vec::new();
    };
    rows.into_iter()
        .map(|row| std::path::PathBuf::from(row.root_path))
        .filter(|folder| devpit_core::home::orchestrator_of(store, root, folder).is_some())
        .filter_map(|folder| {
            seed(&folder)
                .and_then(|()| {
                    crate::assistant::brief_in(&folder, crate::assistant::on(store, &folder))
                })
                .err()
                .map(|err| (folder.display().to_string(), err))
        })
        .collect()
}

/// [`seed_all`] as the app starts, on its own thread. An update is always a
/// new build starting, so this is the moment that covers every orchestrator
/// before any of its chats does.
pub(crate) fn seed_all_on_start() {
    std::thread::spawn(|| {
        let (Ok(root), Ok(store)) = (Store::root(), Store::open_default()) else {
            return;
        };
        for (folder, err) in seed_all(&store, &root) {
            eprintln!("the orchestrator brief in {folder} was left as it was: {err}");
        }
    });
}

#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "orchestrator_on_codex_tests.rs"]
mod on_codex_tests;
