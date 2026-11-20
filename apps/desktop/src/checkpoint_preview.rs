//! `checkpoint.preview` — what a step would run, before anybody runs it.
//!
//! A check is somebody else's command against your machine and your checkout.
//! Offering it behind a button that says only "Run" asks for trust the screen
//! has not earned: the same word can be `pnpm test` and can be a deploy.
//!
//! So the command, the directory, the environment devpit declares and the
//! timeout are all said first. Nothing here starts anything — running is
//! `card.play`, which already runs the card's lane where it stands and does
//! not move the card.

use devpit_core::Store;
use devpit_rpc::{ErrorCode, RpcError, Step, StepKind, WouldRun};
use devpit_steps::CONTEXT_KEYS;

/// What running this step on this card would do.
pub(crate) fn previewed(store: &Store, card_id: &str, step_id: &str) -> Result<WouldRun, RpcError> {
    let card = store
        .card(card_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such card"))?;
    let row = store
        .step(step_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such step"))?;

    let step = Step {
        id: row.id,
        kind: crate::board::kind_of(&row.kind),
        name: row.name,
        config: row.config,
        irreversible: row.irreversible,
    };

    // The manifest is what a command step actually runs. An agent or a session
    // step has none, and saying so is better than showing an empty command
    // line that reads like one that does nothing.
    let manifest = (step.kind == StepKind::Command)
        .then(|| devpit_steps::validate(&step.config).ok())
        .flatten();

    let in_a_worktree = crate::checkout::needs_worktree(step.kind, &step.config);
    let agent = if step.kind == StepKind::Agent {
        let config =
            crate::steps::agent_config::readable(&step.config).map_err(RpcError::internal)?;
        match config.profile.as_deref() {
            Some(profile) => {
                let sources = crate::run_start::Sources::read(store)?;
                Some(sources.preview(profile, config.model.as_deref()))
            }
            None => None,
        }
    } else {
        None
    };
    Ok(WouldRun {
        step_id: step.id,
        step_name: step.name,
        irreversible: step.irreversible,
        command: manifest.as_ref().map(|it| it.command.clone()),
        // Where it would run, not where it ran: the checkout may not exist
        // yet, and this must not make one just to answer a question.
        in_directory: if in_a_worktree {
            card.worktree_path
        } else {
            // `project_of_card` answers with the project's path, which is
            // where a step that wants no checkout of its own runs.
            store.project_of_card(card_id)?
        },
        in_a_worktree,
        // Names, never values — the same rule the run's own snapshot follows.
        declared_env: CONTEXT_KEYS
            .iter()
            .map(|key| format!("DEVPIT_{}", to_shout(key)))
            .collect(),
        timeout_seconds: manifest
            .and_then(|it| it.timeout_seconds)
            .map(|seconds| seconds as f64),
        profile: agent.as_ref().map(|agent| agent.profile.clone()),
        model: agent.as_ref().and_then(|agent| agent.model.clone()),
        check: agent.map(|agent| agent.check),
    })
}

/// `projectPath` as a shell reads it. Mirrors `Context::environment`.
fn to_shout(key: &str) -> String {
    let mut shouted = String::with_capacity(key.len() + 2);
    for letter in key.chars() {
        if letter.is_ascii_uppercase() {
            shouted.push('_');
        }
        shouted.push(letter.to_ascii_uppercase());
    }
    shouted
}

#[tauri::command]
#[specta::specta]
pub async fn checkpoint_preview(card_id: String, step_id: String) -> Result<WouldRun, RpcError> {
    crate::off_main::blocking(move || checkpoint_preview_now(card_id, step_id)).await
}

/// [`checkpoint_preview`], on the calling thread.
pub(crate) fn checkpoint_preview_now(
    card_id: String,
    step_id: String,
) -> Result<WouldRun, RpcError> {
    previewed(&crate::board::store()?, &card_id, &step_id)
}

#[cfg(test)]
#[path = "checkpoint_preview_tests.rs"]
mod tests;
