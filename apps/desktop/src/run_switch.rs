use std::sync::Arc;

use devpit_core::store::StoreError;
use devpit_core::Store;
use devpit_rpc::{ErrorCode, RpcError, Run, RunSwitchOptions, RunSwitchProfile, Step};

pub(crate) struct Switched {
    card_id: String,
    step: Step,
    from_column: Option<String>,
    begun: crate::run_start::BegunAgent,
}

fn context(store: &Store, run_id: &str) -> Result<(String, Step, Option<String>), RpcError> {
    let (card_id, step_id, from_column) = store
        .run_context(run_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such run"))?;
    let row = store
        .step(&step_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "the run's step no longer exists"))?;
    Ok((
        card_id,
        Step {
            id: row.id,
            kind: crate::board::kind_of(&row.kind),
            name: row.name,
            config: row.config,
            irreversible: row.irreversible,
        },
        from_column,
    ))
}

fn paused_reason(store: &Store, run_id: &str) -> Result<String, RpcError> {
    let attempt = store
        .attempt_of(run_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such run"))?;
    if attempt.state != "paused" {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            "only a paused attempt can be switched away from",
        ));
    }
    Ok(attempt
        .paused_because
        .unwrap_or_else(|| "this step cannot start under its current profile".to_owned()))
}

pub(crate) fn options_with(
    store: &Store,
    run_id: &str,
    sources: &crate::run_start::Sources,
) -> Result<RunSwitchOptions, RpcError> {
    let reason = paused_reason(store, run_id)?;
    let (_, step, _) = context(store, run_id)?;
    let config = crate::steps::agent_config::readable(&step.config)
        .map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    let profiles = sources
        .profiles
        .iter()
        .filter_map(|profile| {
            let carry = sources.current(&profile.id, config.model.as_deref());
            sources.fixed(&carry).ok().map(|_| RunSwitchProfile {
                id: profile.id.clone(),
                label: profile.label.clone(),
                model: carry.requested_model,
            })
        })
        .collect();
    Ok(RunSwitchOptions { reason, profiles })
}

pub(crate) fn switch_with(
    store: &Store,
    run_id: &str,
    profile_id: &str,
    sources: &crate::run_start::Sources,
) -> Result<Switched, RpcError> {
    paused_reason(store, run_id)?;
    let (card_id, step, from_column) = context(store, run_id)?;
    let config = crate::steps::agent_config::readable(&step.config)
        .map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    let carry = sources.current(profile_id, config.model.as_deref());
    let fixed = sources
        .fixed(&carry)
        .map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    let begun = crate::run_start::switch_agent(store, run_id, from_column.as_deref(), carry, fixed)
        .map_err(|err| match err {
            StoreError::AlreadyContinued | StoreError::AlreadyRunning => {
                RpcError::new(ErrorCode::Busy, err.to_string())
            }
            StoreError::NotPaused => RpcError::new(ErrorCode::Conflict, err.to_string()),
            other => other.into(),
        })?;
    Ok(Switched {
        card_id,
        step,
        from_column,
        begun,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn run_switch_options(run_id: String) -> Result<RunSwitchOptions, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::board::store()?;
        let sources = crate::run_start::Sources::read(&store)?;
        options_with(&store, &run_id, &sources)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn run_switch(
    app: tauri::AppHandle,
    run_id: String,
    profile_id: String,
) -> Result<Run, RpcError> {
    crate::off_main::blocking(move || {
        crate::run_launch::starting_refused(&app)?;
        let store = crate::board::store()?;
        let sources = crate::run_start::Sources::read(&store)?;
        let switched = switch_with(&store, &run_id, &profile_id, &sources)?;
        let state = tauri::Manager::state::<Arc<crate::in_flight::InFlight>>(&app);
        crate::run_launch::launch(
            app.clone(),
            Arc::clone(&state),
            &store,
            &switched.card_id,
            &switched.step,
            crate::run_from::Asking::first(
                switched.from_column.as_deref(),
                devpit_core::store::Asked::Card,
            ),
            switched.begun,
        )
    })
    .await
}

#[cfg(test)]
#[path = "run_switch_tests.rs"]
mod tests;
