use devpit_rpc::RunState;

use super::*;
use devpit_rpc::StepKind;

/// The run a command answers with carries the time on its row. It used to say
/// zero, and the board drew a run started in 1970.
#[test]
fn a_run_just_opened_says_when_it_started() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "")
        .expect("card");
    let step_id = store
        .create_step(&project, "command", "tests", "{}", false)
        .expect("step");
    let run_id = store.start_run(&card, &step_id, None).expect("start");
    let step = Step {
        id: step_id,
        kind: StepKind::Command,
        name: "tests".to_owned(),
        config: "{}".to_owned(),
        irreversible: false,
    };

    let run = crate::run_launch::opened(&store, &card, &run_id, &step).expect("the run");
    assert!(run.started_at > 0.0, "started_at {}", run.started_at);
    assert_eq!(run.state, RunState::Running);
    assert_eq!(run.step_name, "tests");
}
