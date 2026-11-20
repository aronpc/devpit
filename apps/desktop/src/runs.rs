//! Starting a step, and recording what it did.
//!
//! A run is opened before the work begins and closed when it ends, so a card
//! never has work happening with nothing on screen to show for it. When the
//! process dies mid-run the row stays `running` and says so — which is honest,
//! and better than a row that silently reports success.
//!
//! **The work happens off the command's thread.** A turn takes seconds; doing
//! it inline would freeze the drag that started it for exactly as long, and a
//! board that locks up while it thinks is a board nobody drags things onto.
//! The command returns a `running` row and the thread finishes it, announcing
//! itself on `run:changed` so the card can catch up.
//!
//! Nothing here retries. A failed run leaves the card where it is with the
//! reason on it, and the next move is a person's.

use std::sync::Arc;

use devpit_core::store::{Carried, WhoseRun};
use devpit_core::Store;
use devpit_rpc::{ErrorCode, RpcError, Run, Step, StepKind};
use tauri::AppHandle;

use crate::card_activity::{run_heard, run_reference, Doing};
use crate::in_flight::InFlight;
use crate::run_from::Asking;
use crate::{steps, working};

/// Runs a step against a card, and returns the run it opened.
pub fn start(
    app: AppHandle,
    in_flight: Arc<InFlight>,
    store: &Store,
    card_id: &str,
    step: &Step,
    from: Asking<'_>,
) -> Result<Run, RpcError> {
    start_chained(app, in_flight, store, card_id, step, from)
}

/// The same, for a run the chain is carrying on rather than a person starting.
pub fn start_chained(
    app: AppHandle,
    in_flight: Arc<InFlight>,
    store: &Store,
    card_id: &str,
    step: &Step,
    from: Asking<'_>,
) -> Result<Run, RpcError> {
    let Asking {
        came_from,
        hops,
        asked,
    } = from;
    // Nothing new once an update is about to go in. This is the single place a
    // run is born, so a card advancing on its own is refused here too — its
    // process would die with this one at the commit.
    if let Some(updating) = tauri::Manager::try_state::<crate::update::Updating>(&app) {
        if let Some(why) = crate::update::starting_refused(&updating.state()) {
            return Err(RpcError::new(ErrorCode::Busy, why.to_owned()));
        }
    }

    let begun = crate::run_start::prepare(store, card_id, step, came_from)?;
    let run_id = begun.run_id;
    // A session of its own for every run: two runs of one card are two
    // conversations, and one id for both would write the second over the first.
    let session_id = steps::fresh_session_id();
    if step.kind == StepKind::Agent && !begun.paused {
        store.set_run_session(&run_id, &session_id)?;
    }

    // Written now, never from a later event: something arriving after the fact
    // must not reattribute this run to whatever is active by then.
    // Recorded, not required: a row that could not say whose it was reads as
    // `unknown`, and that is a worse answer than the truth but a far better
    // one than refusing to do the work. The same rule the snapshot follows.
    if let Err(err) = store.record_whose_run(
        &run_id,
        &WhoseRun {
            asked,
            // The surface that asked, as a reference. Nothing looks up what it
            // names — a terminal that has closed is unavailable, and matching
            // another by name would point at work that is not this run's.
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
    // Heard in the one order everything else about the card is heard in.
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

    // Its own connection, moved in rather than shared, and the row it has to
    // close is already committed.
    let Some(own) = working::store_for_thread(store, &run_id, Store::open_default()) else {
        run_heard(&app, card_id, &run_reference(store, &run_id), Doing::Failed);
        // Rung like any run that failed with nobody watching.
        crate::notices::run_ended(&app, store, card_id, &step.name, false);
        return opened(store, card_id, &run_id, step);
    };
    let carrying = working::Carrying {
        app,
        // The chain needs the same registry the run itself is watched in.
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

/// The run as its row reads, so the time on it is the one the board shows.
fn opened(store: &Store, card_id: &str, run_id: &str, step: &Step) -> Result<Run, RpcError> {
    crate::board::runs_of(store, card_id, std::slice::from_ref(step))?
        .into_iter()
        .find(|run| run.id == run_id)
        .ok_or_else(|| RpcError::internal("the run just opened is not in the store"))
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
