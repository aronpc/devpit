//! `runs.list` — every run in a project, a page at a time.

use devpit_core::store::project_runs::{ListedRun, RunQuery};
use devpit_core::Store;
use devpit_rpc::{ProjectRun, RpcError, Run, RunCursor, RunState, RunsPage, RunsQuery, Step};

/// Runs per page in the runs view.
const PAGE: u32 = 50;

fn word(state: RunState) -> &'static str {
    match state {
        RunState::Running => "running",
        RunState::Ok => "ok",
        RunState::Failed => "failed",
        RunState::Cancelled => "cancelled",
        RunState::Lost => "lost",
        RunState::Paused => "paused",
    }
}

/// One page of `size`. One more row than that is read, to know a next exists.
pub(crate) fn page(store: &Store, query: &RunsQuery, size: u32) -> Result<RunsPage, RpcError> {
    let steps = crate::board::steps_of(store, &query.project_id)?;
    let mut rows = store.project_runs(&RunQuery {
        project_id: &query.project_id,
        step_id: query.step_id.as_deref(),
        state: query.state.map(word),
        since: query.since.map(|at| at as i64),
        until: query.until.map(|at| at as i64),
        before: query
            .after
            .as_ref()
            .map(|after| (after.started_at as i64, after.id.as_str())),
        limit: size + 1,
    })?;
    let more = rows.len() > size as usize;
    rows.truncate(size as usize);
    let next = rows.last().filter(|_| more).map(|last| RunCursor {
        started_at: last.started_at as f64,
        id: last.id.clone(),
    });
    Ok(RunsPage {
        runs: rows.into_iter().map(|row| listed(row, &steps)).collect(),
        next,
    })
}

fn listed(row: ListedRun, steps: &[Step]) -> ProjectRun {
    ProjectRun {
        run: Run {
            step_name: steps
                .iter()
                .find(|step| step.id == row.step_id)
                .map(|step| step.name.clone())
                .unwrap_or_else(|| "a deleted step".to_owned()),
            id: row.id,
            step_id: row.step_id,
            state: crate::board::state_of(&row.state),
            output: row.output,
            exit_code: row.exit_code.map(|code| code as i32),
            cost_usd: row.cost_usd,
            duration_ms: row.duration_ms.map(|ms| ms as f64),
            started_at: row.started_at as f64,
        },
        card_id: row.card_id,
        card_title: row.card_title,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn runs_list(query: RunsQuery) -> Result<RunsPage, RpcError> {
    crate::off_main::blocking(move || runs_list_now(query)).await
}

/// [`runs_list`], on the calling thread.
pub(crate) fn runs_list_now(query: RunsQuery) -> Result<RunsPage, RpcError> {
    page(&crate::board::store()?, &query, PAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_says_whether_another_follows_and_where() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open(&dir.path().join("state.db")).expect("open");
        let root = dir.path().join("project");
        std::fs::create_dir_all(&root).expect("create");
        let project = store.add_project(&root, None).expect("project");
        store.ensure_board(&project).expect("board");
        let column = store.columns(&project).expect("columns")[0].id.clone();
        let card = store
            .create_card(&project, &column, "a card", "")
            .expect("card");
        let step = store
            .create_step(&project, "command", "tests", "{}", false)
            .expect("step");
        for _ in 0..3 {
            let run = store.start_run(&card, &step, None).expect("start");
            store
                .finish_run(&run, "lost", None, None, None, None)
                .expect("finish");
        }
        let query = RunsQuery {
            project_id: project,
            step_id: None,
            state: Some(RunState::Lost),
            since: None,
            until: None,
            after: None,
        };

        let first = page(&store, &query, 2).expect("first");
        assert_eq!(first.runs.len(), 2);
        assert_eq!(first.runs[0].run.state, RunState::Lost);
        assert_eq!(first.runs[0].run.step_name, "tests");
        let after = first.next.expect("a next page");

        let second = page(
            &store,
            &RunsQuery {
                after: Some(after),
                ..query
            },
            2,
        )
        .expect("second");
        assert_eq!(second.runs.len(), 1);
        assert!(second.next.is_none());
        assert!(!first
            .runs
            .iter()
            .any(|one| one.run.id == second.runs[0].run.id));
    }
}
