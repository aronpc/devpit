//! Board commands: the columns, the cards, and what a move sets off.
//!
//! Moving a card into a column that has a step is what starts work. Moving it
//! into one that does not is just a move. That is the whole rule, and there is
//! no other way to start a step: no schedule, no retry, no loop that keeps
//! itself going.

use devpit_core::store::card_numbers::CardTally;
use devpit_core::Store;
use devpit_rpc::{Board, Card, Column, ErrorCode, RpcError, Run, RunState, Step, StepKind};

use crate::card_activity::{activity, background_read, snapshot};
use crate::card_sessions::card_sessions;

pub(crate) fn store() -> Result<Store, RpcError> {
    Ok(Store::open_default()?)
}

pub(crate) fn kind_of(raw: &str) -> StepKind {
    match raw {
        "session" => StepKind::Session,
        "command" => StepKind::Command,
        _ => StepKind::Agent,
    }
}

pub(crate) fn state_of(raw: &str) -> RunState {
    match raw {
        "ok" => RunState::Ok,
        "failed" => RunState::Failed,
        "lost" => RunState::Lost,
        "cancelled" => RunState::Cancelled,
        "paused" => RunState::Paused,
        _ => RunState::Running,
    }
}

pub(crate) fn steps_of(store: &Store, project_id: &str) -> Result<Vec<Step>, RpcError> {
    Ok(store
        .steps(project_id)?
        .into_iter()
        .map(|row| Step {
            id: row.id,
            kind: kind_of(&row.kind),
            name: row.name,
            config: row.config,
            irreversible: row.irreversible,
        })
        .collect())
}

pub(crate) fn card_of(store: &Store, id: &str, steps: &[Step]) -> Result<Card, RpcError> {
    let row = store
        .card(id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such card"))?;
    let tally = store.card_tally(id)?;
    let runs = runs_of(store, &row.id, steps)?;
    card_from(row, tally, runs)
}

/// A card from its row and what its tile counts, which a board reads for every
/// card at once.
fn card_from(
    row: devpit_core::CardRow,
    tally: CardTally,
    runs: Vec<Run>,
) -> Result<Card, RpcError> {
    Ok(Card {
        // What was heard, without asking the CLI: a card answered alone does
        // not pay for a process. `board_get` and `card_detail` add the rest.
        activity: snapshot(&row.id).activity,
        criteria: crate::card_ties::criteria_of(&row.body),
        criteria_unread: crate::card_ties::unread_of(&row.body),
        // What ties it to other cards is read for the whole board at once.
        parent_id: None,
        blocked_by: Vec::new(),
        progress: None,
        plan_waiting: 0,
        superseded_by: None,
        cut: false,
        runs,
        id: row.id,
        column_id: row.column_id,
        title: row.title,
        body: row.body,
        position: row.position as i32,
        worktree_path: row.worktree_path,
        due_at: row.due_at.map(|at| at as f64),
        due_time: row.due_time,
        cost_usd: tally.cost_usd,
        // Counted rather than carried. A board of thirty cards would otherwise
        // read thirty conversations to draw thirty badges.
        comments: tally.comments,
        pinned: tally.pinned,
        number: tally.number.and_then(|number| u32::try_from(number).ok()),
    })
}

/// Every session the agent CLI currently lists.
///
/// Read once per board rather than once per card: it costs a process, and a
/// board with twenty cards would pay for it twenty times.
///
/// A CLI that is missing answers with nothing, and every card then reports its
/// background session as gone — which is true from the board's point of view.
pub(crate) fn live_sessions(store: &Store, project_id: &str) -> Vec<devpit_agentcli::AgentSession> {
    let mut listed = Vec::new();
    for profile in store.session_profiles(project_id).unwrap_or_default() {
        let runner = match profile.as_deref() {
            None => None,
            // A profile that has been deleted since is not asked under the
            // default binary: that would be another account's answer.
            Some(id) => match crate::agent_profiles::runner_for(store, id) {
                Ok(runner) => Some(runner),
                Err(_) => continue,
            },
        };
        listed.extend(devpit_agentcli::list(runner.as_ref(), None).unwrap_or_default());
    }
    listed
}

pub(crate) fn runs_of(store: &Store, card_id: &str, steps: &[Step]) -> Result<Vec<Run>, RpcError> {
    runs_from(store.runs(card_id)?, steps)
}

fn runs_from(rows: Vec<devpit_core::RunRow>, steps: &[Step]) -> Result<Vec<Run>, RpcError> {
    Ok(rows
        .into_iter()
        .map(|row| Run {
            step_name: steps
                .iter()
                .find(|s| s.id == row.step_id)
                .map(|s| s.name.clone())
                // A step deleted after its run leaves the run standing. The
                // history of what happened does not disappear with the recipe.
                .unwrap_or_else(|| "a deleted step".to_owned()),
            id: row.id,
            step_id: row.step_id,
            state: state_of(&row.state),
            output: row.output,
            exit_code: row.exit_code.map(|c| c as i32),
            cost_usd: row.cost_usd,
            duration_ms: row.duration_ms.map(|ms| ms as f64),
            started_at: row.started_at as f64,
        })
        .collect())
}

/// `board.get` — the columns, the cards and the steps of a project.
#[tauri::command]
#[specta::specta]
pub async fn board_get(project_id: String) -> Result<Board, RpcError> {
    crate::off_main::blocking(move || board_get_now(project_id)).await
}

/// [`board_get`], on the calling thread.
pub(crate) fn board_get_now(project_id: String) -> Result<Board, RpcError> {
    board_read(project_id, true)
}

/// The board without what its cards' sessions are doing: asking every agent
/// CLI for its live sessions is what kept an agent's move waiting.
pub(crate) fn board_still_now(project_id: String) -> Result<Board, RpcError> {
    board_read(project_id, false)
}

fn board_read(project_id: String, live_too: bool) -> Result<Board, RpcError> {
    let store = store()?;
    store.ensure_board(&project_id)?;
    let steps = steps_of(&store, &project_id)?;

    let columns = store
        .columns(&project_id)?
        .into_iter()
        .map(|row| Column {
            role: devpit_core::store::column_roles::role_of(row.role.as_deref(), &row.name)
                .map(str::to_owned),
            role_chosen: row.role.is_some(),
            id: row.id,
            name: row.name,
            position: row.position as i32,
            step: row
                .step_id
                .and_then(|id| steps.iter().find(|s| s.id == id).cloned()),
            on_pass: row.on_pass,
            autonomy: row.autonomy,
        })
        .collect();

    // One listing for the whole board, not one per card.
    let live = if live_too {
        live_sessions(&store, &project_id)
    } else {
        Vec::new()
    };
    let tallies = store.card_tallies(&project_id)?;
    let mut cards = Vec::new();
    for row in store.cards(&project_id)? {
        let tally = tallies.get(&row.id).copied().unwrap_or_default();
        // Every card's runs without their output, which only the card shows.
        let runs = runs_from(store.runs_brief(&row.id)?, &steps)?;
        let mut card = card_from(row, tally, runs)?;
        if live_too {
            let sessions = card_sessions(&store, &project_id, &card.id, &live, &snapshot(&card.id));
            background_read(&card.id, &sessions);
            card.activity = activity(&sessions);
        }
        cards.push(card);
    }
    let ties = crate::card_ties::Ties::read(&store, &project_id)?;
    cards.iter_mut().for_each(|card| ties.fill(card));

    Ok(Board {
        project_id,
        columns,
        cards,
        steps,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn card_create(
    project_id: String,
    column_id: String,
    title: String,
    body: String,
) -> Result<Card, RpcError> {
    crate::off_main::blocking(move || card_create_now(project_id, column_id, title, body)).await
}

/// [`card_create`], on the calling thread.
pub(crate) fn card_create_now(
    project_id: String,
    column_id: String,
    title: String,
    body: String,
) -> Result<Card, RpcError> {
    let store = store()?;
    let id = store.create_card(&project_id, &column_id, &title, &body)?;
    let steps = steps_of(&store, &project_id)?;
    crate::card_ties::tied(&store, &project_id, card_of(&store, &id, &steps)?)
}

#[tauri::command]
#[specta::specta]
pub async fn card_update(
    project_id: String,
    card_id: String,
    title: String,
    body: String,
) -> Result<Card, RpcError> {
    crate::off_main::blocking(move || card_update_now(project_id, card_id, title, body)).await
}

/// [`card_update`], on the calling thread.
pub(crate) fn card_update_now(
    project_id: String,
    card_id: String,
    title: String,
    body: String,
) -> Result<Card, RpcError> {
    let store = store()?;
    store.update_card(&card_id, &title, &body)?;
    let steps = steps_of(&store, &project_id)?;
    crate::card_ties::tied(&store, &project_id, card_of(&store, &card_id, &steps)?)
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
