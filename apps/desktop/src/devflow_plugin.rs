//! The standalone devflow's Claude Code plugin, `devflow@devflow`, in the
//! profile a project's sessions run as. devpit's own Devflow serves the same
//! tools, so the two on at once is said, with a way to switch one off.

use std::path::{Path, PathBuf};

use devpit_core::Store;
use devpit_rpc::{DevflowPlugin, DevflowPluginAction, RpcError};

/// The release devpit is built against: the engine's tag.
pub(crate) const KNOWN: &str = "0.5.0";

const KEY: &str = "devflow@devflow";
const MARKETPLACE: &str = "devflow";
const SOURCE: &str = "jholhewres/devflow";

/// The most of a Claude Code file read here.
const MOST_READ: u64 = 4 * 1024 * 1024;

/// How long one `claude` call is given: a clone of the marketplace is the slow one.
const CLAUDE_WAITS: std::time::Duration = std::time::Duration::from_secs(120);

/// What a profile's own files say of the plugin.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Read {
    pub version: Option<String>,
    pub enabled: bool,
}

fn json_at(path: &Path) -> Option<serde_json::Value> {
    use std::io::Read as _;
    let mut text = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(MOST_READ)
        .read_to_string(&mut text)
        .ok()?;
    serde_json::from_str(&text).ok()
}

/// The plugin in the Claude Code configuration directory `directory`.
pub(crate) fn read_in(directory: &Path) -> Read {
    let version =
        json_at(&directory.join("plugins").join("installed_plugins.json")).and_then(|read| {
            read["plugins"][KEY]
                .as_array()?
                .iter()
                .rev()
                .find_map(|one| one["version"].as_str().map(str::to_owned))
        });
    // Claude Code writes `true` on install; an absent entry is not on.
    let enabled = version.is_some()
        && json_at(&directory.join("settings.json"))
            .is_some_and(|settings| settings["enabledPlugins"][KEY] == true);
    Read { version, enabled }
}

/// Whether a project's agents get the devflow tools twice: from the plugin
/// and from devpit's own Devflow.
pub(crate) fn twice(read: &Read, devpit_on: bool) -> bool {
    read.enabled && devpit_on
}

/// The `claude` calls an action makes, in order, and whether a failure of
/// each stops the rest.
fn steps(action: DevflowPluginAction) -> Vec<(Vec<&'static str>, bool)> {
    let update_market = (vec!["plugin", "marketplace", "update", MARKETPLACE], true);
    match action {
        // Already added is a failure that changes nothing: the update says
        // whether the marketplace is there.
        DevflowPluginAction::Install => vec![
            (vec!["plugin", "marketplace", "add", SOURCE], false),
            update_market,
            (vec!["plugin", "install", KEY], true),
        ],
        DevflowPluginAction::Update => {
            vec![update_market, (vec!["plugin", "update", KEY], true)]
        }
        DevflowPluginAction::Remove => vec![(vec!["plugin", "uninstall", KEY], true)],
        DevflowPluginAction::Enable => vec![(vec!["plugin", "enable", KEY], true)],
        DevflowPluginAction::Disable => vec![(vec!["plugin", "disable", KEY], true)],
    }
}

/// Runs `action` with `program` against `directory`; `home`'s `.claude` is
/// the CLI's own default, reached with the variable unset.
pub(crate) fn run(
    program: &Path,
    directory: &Path,
    home: &Path,
    action: DevflowPluginAction,
) -> Result<(), RpcError> {
    for (args, stops) in steps(action) {
        let mut command = devpit_pty::host_env::command(program);
        command.args(&args);
        if directory == home.join(".claude") {
            command.env_remove("CLAUDE_CONFIG_DIR");
        } else {
            command.env("CLAUDE_CONFIG_DIR", directory);
        }
        let what = format!("`claude {}`", args.join(" "));
        let out = crate::claude_plugin::output_within(command, CLAUDE_WAITS)
            .map_err(|why| RpcError::internal(format!("{what} {why}")))?;
        if out.status.success() || !stops {
            continue;
        }
        let said = String::from_utf8_lossy(if out.stderr.is_empty() {
            &out.stdout
        } else {
            &out.stderr
        })
        .trim()
        .to_owned();
        return Err(RpcError::internal(format!("{what} failed: {said}")));
    }
    Ok(())
}

fn home() -> Result<PathBuf, RpcError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| RpcError::internal("HOME is not set"))
}

/// The configuration directory and label of the profile `project_id` runs as.
fn profile_of(store: &Store, project_id: &str, home: &Path) -> (PathBuf, Option<String>) {
    let profile = crate::devflow_profile::of(store, project_id, None);
    let inherited = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let directory =
        crate::devflow_profile::config_dir(profile.as_ref(), home, inherited.as_deref());
    (directory, profile.map(|one| one.label))
}

fn shown(store: &Store, project_id: &str) -> Result<DevflowPlugin, RpcError> {
    let project = crate::devflow_screen::project_here(store, project_id)?;
    let home = home()?;
    let (directory, profile) = profile_of(store, project_id, &home);
    let read = read_in(&directory);
    let devpit_on = crate::devflow_screen::active_here(store, &project);
    Ok(DevflowPlugin {
        project_id: project_id.to_owned(),
        directory: directory.display().to_string(),
        profile,
        twice: twice(&read, devpit_on),
        version: read.version,
        enabled: read.enabled,
        known: KNOWN.to_owned(),
    })
}

/// `devflow.plugin` — the standalone devflow's plugin in the project's profile.
#[tauri::command]
#[specta::specta]
pub async fn devflow_plugin(project_id: String) -> Result<DevflowPlugin, RpcError> {
    crate::off_main::blocking(move || shown(&crate::projects::store()?, &project_id)).await
}

/// `devflow.plugin_do` — the person's click: install, update, remove, enable
/// or disable the plugin in the project's profile.
#[tauri::command]
#[specta::specta]
pub async fn devflow_plugin_do(
    project_id: String,
    action: DevflowPluginAction,
) -> Result<DevflowPlugin, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        crate::devflow_screen::project_here(&store, &project_id)?;
        let home = home()?;
        let (directory, _) = profile_of(&store, &project_id, &home);
        run(Path::new("claude"), &directory, &home, action)?;
        shown(&store, &project_id)
    })
    .await
}

#[cfg(test)]
#[path = "devflow_plugin_tests.rs"]
mod tests;
