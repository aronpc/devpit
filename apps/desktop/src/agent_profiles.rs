//! The agent profiles a person declared, and the list they end up seeing.
//!
//! A profile is environment, program and arguments — the shape five of these
//! already had as shell functions before there was anywhere to put them. What
//! this module owns is the round trip: where they are stored, what is refused
//! on the way in, and how a stored one becomes a row with a resolved path.
//!
//! Stored as one JSON preference rather than a table, for the same reason the
//! "Open in" apps are: it is a short list edited by hand, and a table would be
//! a migration for every field somebody wants next. The cost is honest — there
//! is no foreign key, so a step naming a profile that was deleted holds a
//! string pointing at nothing, which is why deleting asks first.

use devpit_agentcli::profile::{profiles, Base};
use devpit_core::store::preference;
use devpit_core::Store;
use devpit_rpc::{Declared, ErrorCode, Profile, RpcError, SignedIn};

/// What a base agent lends, looked up in the agent catalogue.
fn base_of(id: &str) -> Option<Base> {
    devpit_pty::agents::known(id).map(|agent| Base {
        program: agent.launch.to_owned(),
        driver: agent.driver.to_owned(),
    })
}

fn stored(store: &Store) -> Result<Vec<Declared>, RpcError> {
    // A preference that cannot be parsed reads as an empty list: it is one row
    // in a settings table, and refusing to draw the pane over it would make a
    // typo unrecoverable from inside the app.
    Ok(editable(store)?.unwrap_or_default())
}

/// The stored list, or `None` when it cannot be parsed. Saving over a list
/// that could not be read would write every other profile out of existence.
fn editable(store: &Store) -> Result<Option<Vec<Declared>>, RpcError> {
    let raw = store
        .preference(preference::AGENT_PROFILES)?
        .unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(Some(Vec::new()));
    }
    let Some(list) = serde_json::from_str::<Vec<Declared>>(&raw).ok() else {
        return Ok(None);
    };
    // Also on the way out: profiles saved before `at_home` ran on save still
    // hold a literal `~`, and re-saving each one is not something to ask.
    let Ok(home) = crate::installations::home() else {
        return Ok(Some(list));
    };
    let home = home.to_string_lossy();
    Ok(Some(
        list.into_iter()
            .map(|one| devpit_agentcli::declaring::at_home(one, &home))
            .collect(),
    ))
}

/// [`editable`], refusing to go on when the list cannot be read.
pub(crate) fn to_edit(store: &Store) -> Result<Vec<Declared>, RpcError> {
    editable(store)?.ok_or_else(|| {
        RpcError::internal(
            "the stored agent profiles cannot be read, so nothing was changed — fix or clear them first",
        )
    })
}

pub(crate) fn save(store: &Store, declared: &[Declared]) -> Result<(), RpcError> {
    let written =
        serde_json::to_string(declared).map_err(|err| RpcError::internal(err.to_string()))?;
    store.set_preference(preference::AGENT_PROFILES, &written)?;
    Ok(())
}

/// What the person called each profile, by id.
///
/// Deliberately not `all`: the sidebar asks this on a two-second timer and
/// wants two strings, while `all` resolves every command against the `PATH`.
/// That is a directory walk per profile per tick for an answer nobody read.
pub(crate) fn names(store: &Store) -> std::collections::HashMap<String, String> {
    stored(store)
        .unwrap_or_default()
        .into_iter()
        .map(|one| (one.id, one.label))
        .collect()
}

/// The profiles the person declared, as written.
///
/// Not `all`: the installations need only the environment, and `all` resolves
/// every command against the `PATH` and asks the shell about the rest.
pub(crate) fn declared(store: &Store) -> Vec<Declared> {
    stored(store).unwrap_or_default()
}

/// What the agent said of its models in its last session: the list a
/// profile with none of its own offers, its own default first, and every
/// name and refusal beside it.
pub(crate) fn reported(profile: &mut Profile, models: Vec<devpit_rpc::OfferedModel>) {
    if profile.own_models.is_empty() && !models.is_empty() {
        profile.models = std::iter::once(crate::profile_models::DEFAULT_MODEL.to_owned())
            .chain(models.iter().map(|one| one.id.clone()))
            .collect();
    }
    profile.offered = models;
}

/// The command each declared profile runs, for the shell probe.
pub(crate) fn commands(store: &Store) -> Vec<String> {
    stored(store)
        .unwrap_or_default()
        .into_iter()
        .filter(|one| !one.command.is_empty())
        .map(|one| one.command)
        .collect()
}

/// Every profile this machine can offer: the declared ones and the discovered.
pub(crate) fn all(store: &Store) -> Result<Vec<Profile>, RpcError> {
    let off = crate::agent_choice::disabled(store);
    let mut all = profiles(
        &stored(store)?,
        base_of,
        &crate::shell_launch::shell_knows(),
    );
    let home = crate::chat::home();
    for one in &mut all {
        one.enabled = !off.contains(&one.id);
        let said = crate::slash::recall(&home, &one.id);
        reported(one, said.map(|init| init.models).unwrap_or_default());
        let models = crate::profile_models::of(store, &one.id);
        one.models
            .retain(|model| !models.models_off.contains(model));
        if models.effort.is_some() {
            one.effort_default = models.effort;
        }
    }
    Ok(all)
}

/// `agent.profiles` — the accounts this machine can talk to.
#[tauri::command]
#[specta::specta]
pub async fn agent_profiles() -> Result<Vec<Profile>, RpcError> {
    crate::off_main::blocking(agent_profiles_now).await
}

/// [`agent_profiles`], on the calling thread.
pub(crate) fn agent_profiles_now() -> Result<Vec<Profile>, RpcError> {
    all(&crate::projects::store()?)
}

/// `agent.profile_save` — writes one profile, new or edited.
///
/// The id decides which: an id already on the list is an edit in place, so a
/// rename keeps every step that names it. A blank one is minted here rather
/// than in the window, because the window can be reloaded mid-edit.
#[tauri::command]
#[specta::specta]
pub async fn agent_profile_save(declared: Declared) -> Result<Vec<Profile>, RpcError> {
    crate::off_main::blocking(move || agent_profile_save_now(declared)).await
}

/// [`agent_profile_save`], on the calling thread.
pub(crate) fn agent_profile_save_now(declared: Declared) -> Result<Vec<Profile>, RpcError> {
    devpit_agentcli::declaring::allowed(&declared, |base| base_of(base).is_some())
        .map_err(|why| RpcError::new(ErrorCode::Invalid, why.to_string()))?;
    let home = crate::installations::home()?;
    let mut declared = devpit_agentcli::declaring::at_home(declared, &home.to_string_lossy());
    // Made on save, so the profile starts in a terminal too, not only after Sign in.
    let env: Vec<(String, String)> = declared
        .env
        .iter()
        .map(|var| (var.name.clone(), var.value.clone()))
        .collect();
    devpit_agentcli::providers::prepare(&declared.base, &env)
        .map_err(|err| RpcError::internal(format!("could not make its folder: {err}")))?;

    declared.label = declared.label.trim().to_owned();
    if declared.id.trim().is_empty() {
        declared.id = ulid::Ulid::generate().to_string();
    }

    let store = crate::projects::store()?;
    upsert(&store, declared)?;
    all(&store)
}

/// Puts one profile on the stored list, in place when its id is already there.
fn upsert(store: &Store, declared: Declared) -> Result<(), RpcError> {
    let _held = devpit_core::PROFILES_WRITE
        .lock()
        .map_err(|err| RpcError::internal(err.to_string()))?;
    let mut list = to_edit(store)?;
    match list.iter_mut().find(|one| one.id == declared.id) {
        Some(existing) => *existing = declared,
        None => list.push(declared),
    }
    save(store, &list)
}

/// `agent.profile_remove` — forgets one.
///
/// Refused while a board step still names it. A step holds the id as a plain
/// string with nothing enforcing it, so a silent delete is a lane that fails
/// the next time somebody plays a card — and by then the deletion is days ago
/// and nowhere near the symptom.
#[tauri::command]
#[specta::specta]
pub async fn agent_profile_remove(id: String) -> Result<Vec<Profile>, RpcError> {
    crate::off_main::blocking(move || agent_profile_remove_now(id)).await
}

/// [`agent_profile_remove`], on the calling thread.
pub(crate) fn agent_profile_remove_now(id: String) -> Result<Vec<Profile>, RpcError> {
    let store = crate::projects::store()?;
    remove(&store, &id)?;
    all(&store)
}

/// Takes one profile off the stored list, unless a step still names it.
fn remove(store: &Store, id: &str) -> Result<(), RpcError> {
    let _held = devpit_core::PROFILES_WRITE
        .lock()
        .map_err(|err| RpcError::internal(err.to_string()))?;
    let used = store.steps_using_profile(id)?;
    if !used.is_empty() {
        // Named, with their boards: "a step uses it" on a machine with six
        // projects tells somebody there is a problem and not where it is.
        let named: Vec<String> = used
            .iter()
            .map(|one| format!("{} in {}", one.step, one.project))
            .collect();
        return Err(RpcError::new(
            ErrorCode::Conflict,
            format!(
                "{} still {} this profile: {}",
                if used.len() == 1 {
                    "a step".to_owned()
                } else {
                    format!("{} steps", used.len())
                },
                if used.len() == 1 { "uses" } else { "use" },
                named.join(", ")
            ),
        ));
    }

    let mut list = to_edit(store)?;
    list.retain(|one| one.id != id);
    save(store, &list)
}

/// `agent.signed_in` — whether the directory a profile names holds a sign-in.
///
/// Asked with the field's text rather than a saved profile, so the editor
/// answers for what is typed. Empty is what a child inherits: this process's
/// own `CLAUDE_CONFIG_DIR`, else `~/.claude`.
#[tauri::command]
#[specta::specta]
pub async fn agent_signed_in(dir: String) -> Result<SignedIn, RpcError> {
    crate::off_main::blocking(move || agent_signed_in_now(&dir)).await
}

fn agent_signed_in_now(dir: &str) -> Result<SignedIn, RpcError> {
    use devpit_agentcli::cli_config;
    let home = crate::installations::home()?;
    let dir = dir.trim();
    let env = if dir.is_empty() {
        Vec::new()
    } else {
        let at = devpit_agentcli::declaring::at_home_value(dir, &home.to_string_lossy());
        vec![("CLAUDE_CONFIG_DIR".to_owned(), at)]
    };
    let inherited = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let found = cli_config::config_dir_of(&home, &env, inherited.as_deref());
    Ok(SignedIn {
        state: cli_config::sign_in_at(&found),
    })
}

/// The profile this step names, as something that can be started.
pub(crate) fn runner_for(
    store: &Store,
    id: &str,
) -> Result<devpit_agentcli::running::Runner, String> {
    let found = all(store)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|one| one.id == id)
        .ok_or_else(|| format!("this step runs under a profile that no longer exists: {id}"))?;
    if let Some(why) = crate::profile_waiting::blocked(store, id) {
        return Err(why);
    }
    if !found.spawnable() {
        // A shell function has nothing to spawn, and saying so here beats a
        // process that fails to start with the operating system's word for it.
        return Err(format!(
            "{} cannot be started outside a terminal — name the program it runs",
            found.label
        ));
    }
    Ok(devpit_agentcli::running::runner(&found))
}

#[cfg(test)]
#[path = "agent_profiles_tests.rs"]
mod tests;

/// `agent.profile_read` — what a command of the person's own does, read by
/// running it in their shell with stand-ins for the agents (`shell_probe`).
/// Nothing when it starts none of them.
#[tauri::command]
#[specta::specta]
pub async fn agent_profile_read(
    command: String,
) -> Result<Option<devpit_rpc::ReadCommand>, RpcError> {
    crate::off_main::blocking(move || agent_profile_read_now(command)).await
}

/// [`agent_profile_read`], on the calling thread.
pub(crate) fn agent_profile_read_now(
    command: String,
) -> Result<Option<devpit_rpc::ReadCommand>, RpcError> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let programs: Vec<&str> = devpit_pty::agents::KNOWN
        .iter()
        .map(|one| one.launch)
        .collect();
    let probed = crate::shell_probe::probe(&shell, command.trim(), &programs)
        .map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    Ok(probed.and_then(|found| {
        let base = devpit_pty::agents::KNOWN
            .iter()
            .find(|one| one.launch == found.program)?;
        Some(devpit_rpc::ReadCommand {
            base: base.id.to_owned(),
            args: found.args,
            env: found.env,
        })
    }))
}
