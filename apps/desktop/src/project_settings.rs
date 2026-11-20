//! What a project can set for itself: the model and permission mode a new
//! chat starts with, its cards' branch names, and whether blocked steps switch
//! accounts on their own. With none set, each behaves as before.

use std::collections::BTreeMap;
use std::path::Path;

use devpit_core::{preference, Store};
use devpit_rpc::{ErrorCode, RpcError};
use serde::{Deserialize, Serialize};
use specta::Type;

/// The settings a scope can hold, each unset unless chosen.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Scoped {
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub branch_pattern: Option<String>,
    pub automatic_switch: Option<bool>,
}

/// One scope's settings, beside where each applied value came from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    /// The project's own; empty when the scope is the machine.
    pub own: Scoped,
    /// From the repository's `.devpit/settings.json`.
    pub repo: Scoped,
    pub machine: Scoped,
    /// What a new chat or worktree of this scope gets.
    pub applied: Scoped,
    /// Why the repository's file was left out, when it was.
    pub repo_problem: Option<String>,
}

/// The shared defaults a repository can carry.
pub(crate) const REPO_FILE: &str = ".devpit/settings.json";
/// More than a settings file needs: anything bigger is not read.
pub(crate) const LARGEST: u64 = 64 * 1024;
/// The permission modes a chat starts in, as the composer names them.
pub(crate) const PERMISSION_MODES: [&str; 4] =
    ["manual", "acceptEdits", "bypassPermissions", "plan"];

/// The modes a repository may start chats in: those that ask at least as
/// much as Supervised.
pub(crate) const REPO_MODES: [&str; 2] = ["manual", "plan"];

/// The first set of `own`, `repo` and `machine`, setting by setting.
pub(crate) fn applied(own: &Scoped, repo: &Scoped, machine: &Scoped) -> Scoped {
    let first = |pick: fn(&Scoped) -> &Option<String>| {
        [own, repo, machine]
            .into_iter()
            .find_map(|scope| pick(scope).clone())
    };
    Scoped {
        model: first(|scope| &scope.model),
        permission_mode: first(|scope| &scope.permission_mode),
        branch_pattern: first(|scope| &scope.branch_pattern),
        automatic_switch: [own, repo, machine]
            .into_iter()
            .find_map(|scope| scope.automatic_switch),
    }
}

/// Why `scoped` cannot be kept, if it cannot.
pub(crate) fn refused(scoped: &Scoped) -> Option<String> {
    if let Some(model) = scoped
        .model
        .as_deref()
        .filter(|one| !devpit_agentcli::declaring::is_a_model(one))
    {
        return Some(format!("{model:?} is not a model name"));
    }
    if let Some(mode) = scoped
        .permission_mode
        .as_deref()
        .filter(|one| !PERMISSION_MODES.contains(one))
    {
        return Some(format!("{mode:?} is not a permission mode"));
    }
    scoped
        .branch_pattern
        .as_deref()
        .and_then(|pattern| devpit_git::pattern_allowed(pattern).err())
}

/// The repository's shared defaults: none when there is no file; an error,
/// and nothing taken from it, when the file is outside the project, too big,
/// or not settings.
pub(crate) fn from_repo(root: &Path) -> Result<Scoped, String> {
    let path = root.join(REPO_FILE);
    if std::fs::symlink_metadata(&path).is_err() {
        return Ok(Scoped::default());
    }
    let (Ok(real_root), Ok(real)) = (std::fs::canonicalize(root), std::fs::canonicalize(&path))
    else {
        return Err(format!("{REPO_FILE} cannot be read"));
    };
    if !real.starts_with(&real_root) {
        return Err(format!("{REPO_FILE} leads outside the project"));
    }
    if !std::fs::metadata(&real).is_ok_and(|meta| meta.is_file()) {
        return Err(format!("{REPO_FILE} is not a file"));
    }
    // Capped on the read, not on what the size said a moment before.
    let mut text = String::new();
    std::io::Read::read_to_string(
        &mut std::io::Read::take(
            std::fs::File::open(&real).map_err(|err| err.to_string())?,
            LARGEST + 1,
        ),
        &mut text,
    )
    .map_err(|err| err.to_string())?;
    if text.len() as u64 > LARGEST {
        return Err(format!("{REPO_FILE} is bigger than {} KB", LARGEST / 1024));
    }
    let file: RepoFile =
        serde_json::from_str(&text).map_err(|err| format!("{REPO_FILE} is not valid: {err}"))?;
    let scoped = Scoped {
        model: file.model,
        permission_mode: file.permission_mode,
        branch_pattern: file.worktree.and_then(|worktree| worktree.branch_pattern),
        automatic_switch: None,
    };
    // A mode that asks less than Supervised is the person's to choose,
    // never a repository's.
    if let Some(mode) = scoped
        .permission_mode
        .as_deref()
        .filter(|mode| !REPO_MODES.contains(mode))
    {
        return Err(format!(
            "{REPO_FILE} cannot start chats in {mode}: only you choose a mode that asks less"
        ));
    }
    match refused(&scoped) {
        Some(why) => Err(format!("{REPO_FILE}: {why}")),
        None => Ok(scoped),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoFile {
    model: Option<String>,
    permission_mode: Option<String>,
    worktree: Option<RepoWorktree>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoWorktree {
    branch_pattern: Option<String>,
}

fn owned(store: &Store) -> BTreeMap<String, Scoped> {
    store
        .preference(preference::PROJECT_SETTINGS)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn machine(store: &Store) -> Scoped {
    let set = |key: &str| {
        store
            .preference(key)
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty())
    };
    Scoped {
        model: set(preference::CHAT_MODEL),
        permission_mode: set(preference::CHAT_PERMISSION),
        branch_pattern: set(preference::WORKTREE_BRANCH),
        automatic_switch: None,
    }
}

/// The settings of `project_id`, or of the machine when there is none.
pub(crate) fn read(store: &Store, project_id: Option<&str>) -> Result<ProjectSettings, RpcError> {
    let machine = machine(store);
    let Some(project_id) = project_id else {
        return Ok(ProjectSettings {
            applied: machine.clone(),
            machine,
            ..ProjectSettings::default()
        });
    };
    let project = store
        .project(project_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such project"))?;
    let own = owned(store).remove(project_id).unwrap_or_default();
    let (repo, repo_problem) = match from_repo(Path::new(&project.root_path)) {
        Ok(repo) => (repo, None),
        Err(why) => (Scoped::default(), Some(why)),
    };
    Ok(ProjectSettings {
        applied: applied(&own, &repo, &machine),
        own,
        repo,
        machine,
        repo_problem,
    })
}

/// The branch name pattern a card of `project_id` gets.
pub(crate) fn branch_pattern(store: &Store, project_id: &str) -> String {
    read(store, Some(project_id))
        .ok()
        .and_then(|settings| settings.applied.branch_pattern)
        .unwrap_or_default()
}

/// Whether a blocked step on this card may try another account.
pub(crate) fn automatic_switch(
    store: &Store,
    card_id: &str,
) -> Result<bool, devpit_core::store::StoreError> {
    let Some(project_id) = store.project_id_of_card(card_id)? else {
        return Ok(false);
    };
    Ok(owned(store)
        .get(&project_id)
        .and_then(|settings| settings.automatic_switch)
        .unwrap_or(false))
}

/// `project.settings_read` — `projectId` empty for the machine's.
#[tauri::command]
#[specta::specta]
pub async fn project_settings_read(
    project_id: Option<String>,
) -> Result<ProjectSettings, RpcError> {
    crate::off_main::blocking(move || read(&Store::open_default()?, project_id.as_deref())).await
}

/// The scope's settings with `field` set to `value`, or cleared when none.
pub(crate) fn with_field(
    mut scoped: Scoped,
    field: &str,
    value: Option<String>,
) -> Result<Scoped, String> {
    let value = value
        .map(|one| one.trim().to_owned())
        .filter(|one| !one.is_empty());
    match field {
        "model" => scoped.model = value,
        "permissionMode" => scoped.permission_mode = value,
        "branchPattern" => scoped.branch_pattern = value,
        "automaticSwitch" => {
            scoped.automatic_switch = match value.as_deref() {
                None => None,
                Some("true") => Some(true),
                Some("false") => Some(false),
                Some(_) => return Err("automaticSwitch must be true or false".to_owned()),
            }
        }
        _ => return Err(format!("{field:?} is not a setting a project chooses")),
    }
    match refused(&scoped) {
        Some(why) => Err(why),
        None => Ok(scoped),
    }
}

/// One write at a time: two settings changed at once both stay.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `project.settings_write` — one setting of a project, or of the machine,
/// changed and the others left as they are now. The machine's branch name is
/// chosen where it always was, in Settings → Worktrees.
#[tauri::command]
#[specta::specta]
pub async fn project_settings_write(
    project_id: Option<String>,
    field: String,
    value: Option<String>,
) -> Result<ProjectSettings, RpcError> {
    crate::off_main::blocking(move || {
        let invalid = |why: String| RpcError::new(ErrorCode::Invalid, why);
        let _one = WRITING.lock().unwrap_or_else(|held| held.into_inner());
        let store = Store::open_default()?;
        match project_id.as_deref() {
            None => {
                if field == "branchPattern" {
                    return Err(invalid(
                        "the branch name for every project is chosen in Settings → Worktrees"
                            .to_owned(),
                    ));
                }
                let set = with_field(machine(&store), &field, value).map_err(invalid)?;
                store.set_preference(preference::CHAT_MODEL, set.model.as_deref().unwrap_or(""))?;
                store.set_preference(
                    preference::CHAT_PERMISSION,
                    set.permission_mode.as_deref().unwrap_or(""),
                )?;
            }
            Some(id) => {
                let mut all = owned(&store);
                let set = with_field(all.remove(id).unwrap_or_default(), &field, value)
                    .map_err(invalid)?;
                if set != Scoped::default() {
                    all.insert(id.to_owned(), set);
                }
                let json = serde_json::to_string(&all)
                    .map_err(|err| RpcError::internal(err.to_string()))?;
                store.set_preference(preference::PROJECT_SETTINGS, &json)?;
            }
        }
        read(&store, project_id.as_deref())
    })
    .await
}

#[cfg(test)]
#[path = "project_settings_tests.rs"]
mod tests;
