//! Launching a run after its attempt row has been settled.

use std::sync::Arc;

use devpit_core::store::{Carried, WhoseRun};
use devpit_core::Store;
use devpit_rpc::{ErrorCode, RpcError, Run, Step, StepKind};
use tauri::AppHandle;

use crate::card_activity::{run_heard, run_reference, Doing};
use crate::in_flight::InFlight;
use crate::run_from::Asking;
use crate::{steps, working};

pub(crate) fn starting_refused(app: &AppHandle) -> Result<(), RpcError> {
    if let Some(updating) = tauri::Manager::try_state::<crate::update::Updating>(app) {
        if let Some(why) = crate::update::starting_refused(&updating.state()) {
            return Err(RpcError::new(ErrorCode::Busy, why.to_owned()));
        }
    }
    Ok(())
}

pub(crate) fn launch(
    app: AppHandle,
    in_flight: Arc<InFlight>,
    store: &Store,
    card_id: &str,
    step: &Step,
    from: Asking<'_>,
    begun: crate::run_start::BegunAgent,
) -> Result<Run, RpcError> {
    let Asking {
        came_from,
        hops,
        asked,
    } = from;
    let run_id = begun.run_id;
    let session_id = steps::fresh_session_id();
    if step.kind == StepKind::Agent && !begun.paused {
        store.set_run_session(&run_id, &session_id)?;
    }

    if let Err(err) = store.record_whose_run(
        &run_id,
        &WhoseRun {
            asked,
            asked_from: came_from.map(str::to_owned),
            carried: match step.kind {
                StepKind::Agent => Carried::Agent {
                    profile: crate::steps::agent_config::profile_of(&step.config),
                },
                StepKind::Command | StepKind::Session => Carried::Process,
            },
        },
    ) {
        devpit_core::reports::background(
            "run registration",
            &format!("could not record whose run {run_id} is: {err}"),
        );
    }
    run_heard(
        &app,
        card_id,
        &run_reference(store, &run_id),
        if begun.paused {
            Doing::Done
        } else {
            Doing::Working
        },
    );

    if begun.paused {
        return opened(store, card_id, &run_id, step);
    }
    let Some(own) = working::store_for_thread(store, &run_id, Store::open_default()) else {
        run_heard(&app, card_id, &run_reference(store, &run_id), Doing::Failed);
        crate::notices::run_ended(&app, store, card_id, &step.name, false);
        return opened(store, card_id, &run_id, step);
    };
    let carrying = working::Carrying {
        app,
        chained: Arc::clone(&in_flight),
        in_flight,
        run_id: run_id.clone(),
        card_id: card_id.to_owned(),
        session_id,
        step: step.clone(),
        fixed_agent: begun.fixed,
        hops,
    };
    std::thread::spawn(move || working::carry_out(carrying, &own));
    opened(store, card_id, &run_id, step)
}

pub(crate) fn opened(
    store: &Store,
    card_id: &str,
    run_id: &str,
    step: &Step,
) -> Result<Run, RpcError> {
    crate::board::runs_of(store, card_id, std::slice::from_ref(step))?
        .into_iter()
        .find(|run| run.id == run_id)
        .ok_or_else(|| RpcError::internal("the run just opened is not in the store"))
}
