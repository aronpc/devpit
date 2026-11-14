//! Settings → Providers: the supported CLIs found on this machine, each
//! profile's own sign-in, and "Sign in", which types the CLI's login into a
//! terminal for the person to finish.

use devpit_agentcli::providers;
use devpit_agentcli::running::{runner, Runner};
use devpit_rpc::{
    AgentLaunch, Declared, ErrorCode, Profile, ProfileSignIn, ProvidersFound, RpcError,
};

/// `providers.detect` — looks on the `PATH` and the usual install folders.
/// `again` also asks the person's shell again, so the rest of the pane sees
/// a CLI installed since devpit started.
#[tauri::command]
#[specta::specta]
pub async fn providers_detect(again: bool) -> Result<ProvidersFound, RpcError> {
    crate::off_main::blocking(move || {
        if again {
            crate::shell_launch::ask_shell_again();
        }
        let home = crate::installations::home()?;
        let path = devpit_pty::login_path::search_path_now();
        let inherited = |name: &str| std::env::var(name).ok();
        let found = providers::detect(&path, &home, &inherited);
        let store = crate::projects::store()?;
        let declared = crate::agent_profiles::declared(&store);
        Ok(ProvidersFound {
            providers: found,
            profiles: profile_sign_ins(&declared, &home, &inherited),
        })
    })
    .await
}

/// Each declared profile of a supported CLI, signed in as its own variables
/// say: a second Codex with its own `CODEX_HOME` is read there.
pub(crate) fn profile_sign_ins(
    declared: &[Declared],
    home: &std::path::Path,
    inherited: &dyn Fn(&str) -> Option<String>,
) -> Vec<ProfileSignIn> {
    declared
        .iter()
        .filter_map(|one| {
            let keeps = providers::supported(&one.base)?.keeps;
            let own = |name: &str| {
                one.env
                    .iter()
                    .find(|var| var.name == name)
                    .map(|var| var.value.clone())
                    .or_else(|| inherited(name))
            };
            Some(ProfileSignIn {
                id: one.id.clone(),
                label: one.label.clone(),
                base: one.base.clone(),
                signed_in: providers::sign_in(keeps, home, &own),
            })
        })
        .collect()
}

/// The login line for a profile or a catalogue agent, by id.
pub(crate) fn login_for(profiles: &[Profile], id: &str) -> Result<String, RpcError> {
    let (base, run) = match profiles.iter().find(|one| one.id == id && one.mine) {
        Some(profile) => (profile.base.clone(), runner(profile)),
        None => {
            let agent = devpit_pty::agents::known(id).ok_or_else(|| {
                RpcError::new(
                    ErrorCode::NotFound,
                    format!("{id} is not an agent this build knows"),
                )
            })?;
            let plain = Runner {
                program: agent.launch.to_owned(),
                args: Vec::new(),
                env: Vec::new(),
            };
            (agent.id.to_owned(), plain)
        }
    };
    providers::login_line(&base, &run).ok_or_else(|| {
        RpcError::new(
            ErrorCode::NotFound,
            format!("devpit knows no sign-in for {id}"),
        )
    })
}

/// The login for `id`, once it may run: a profile still waiting for the
/// person is refused, and the folder its CLI needs is made first.
pub(crate) fn login_ready(store: &devpit_core::Store, id: &str) -> Result<String, RpcError> {
    if let Some(why) = crate::profile_waiting::blocked(store, id) {
        return Err(RpcError::new(ErrorCode::Conflict, why));
    }
    let profiles = crate::agent_profiles::all(store)?;
    if let Some(profile) = profiles.iter().find(|one| one.id == id && one.mine) {
        providers::prepare(&profile.base, &runner(profile).env)
            .map_err(|err| RpcError::internal(format!("could not make its folder: {err}")))?;
    }
    login_for(&profiles, id)
}

/// `agent.login` — types the CLI's own login into a pane of the project,
/// with the profile's variables, for the person to finish there.
#[tauri::command]
#[specta::specta]
pub async fn agent_login(
    state: tauri::State<'_, crate::sessions::SessionState>,
    project_id: String,
    pane_id: String,
    id: String,
) -> Result<AgentLaunch, RpcError> {
    let line =
        crate::off_main::blocking(move || login_ready(&crate::projects::store()?, &id)).await?;
    let layout = crate::sessions::holding(&project_id, &pane_id)?;
    let target = crate::pane_io::target_in(&layout, &pane_id);
    let waiting = target.clone();
    let ready = tauri::async_runtime::spawn_blocking(move || crate::pane_io::settled(&waiting))
        .await
        .map_err(|err| RpcError::internal(err.to_string()))?;
    if let crate::shell_launch::Ready::Busy(command) = ready {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            format!("this terminal is running {command}"),
        ));
    }
    let lock = state.project_lock(&project_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| RpcError::internal("project session lock"))?;
    crate::pane_io::send_line(&target, &line)?;
    Ok(AgentLaunch {
        line: Some(line),
        missing: Vec::new(),
        failing: Vec::new(),
    })
}

#[cfg(test)]
#[path = "provider_scan_tests.rs"]
mod tests;
