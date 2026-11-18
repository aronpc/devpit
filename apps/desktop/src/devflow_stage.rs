//! `devflow_stage`: one of the project's or a team's own stages. Without a
//! card it is the stage's skill, read where it is; with a card it runs the
//! stage's gate and keeps how it ended as `stage-<name>.json` in a run of the
//! card, which the completion gate then reads.
//!
//! A command runs as a check does: through the step runner, in the checkout
//! the agent stands in, the card reaching it only through the environment. A
//! command a team layer brought runs only once the person confirmed it.

use std::path::Path;
use std::time::{Duration, SystemTime};

use devflow_engine::config::{StageGate, Step};
use devflow_engine::layers::Layered;
use devflow_engine::{runs, skills, stage, Config};
use devpit_core::Store;
use devpit_rpc::{DevflowChange, Project};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::steps::context::{context_of, Checkout};

/// The longest a stage's command runs: it is a gate, not a test suite.
const LONGEST: Duration = Duration::from_secs(10 * 60);
/// The output kept of a gate that failed, from its end.
const TAIL: usize = 20;

pub(crate) fn respond(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    config: &Config,
    cwd: &Path,
    params: &Value,
) -> Result<Value, String> {
    let name = params
        .get("stage")
        .and_then(Value::as_str)
        .ok_or("pass the own stage's name as `stage`")?;
    let root = Path::new(&project.root_path);
    let layered = crate::devflow::layered(root);
    let Some(card_id) = params.get("card").and_then(Value::as_str) else {
        return skill_of(root, config, layered.as_ref(), name);
    };
    let (step, gate) = stage::gated(config, name)?;
    let on_board = store
        .project_id_of_card(card_id)
        .map_err(|err| err.to_string())?;
    let card = store
        .card(card_id)
        .map_err(|err| err.to_string())?
        .filter(|_| on_board.as_deref() == Some(project.id.as_str()))
        .ok_or_else(|| format!("no card `{card_id}` on this project's board"))?;
    let checkout = crate::devflow_evidence::checkout(project, cwd);
    // Where Decisions stands, said only by a stage it judges.
    let (passed, said, decisions) = match gate {
        StageGate::Command(command) => {
            confirmed(app, store, project, layered.as_ref(), step, command)?;
            let targets = crate::devflow::targets_in_force(config);
            let (passed, said) = ran(command, &checkout, project, &card, name, &targets);
            (passed, said, None)
        }
        StageGate::Rubric(rubric) => {
            let jev = crate::devflow_jev::Jev::of(project, config, &checkout, Some(&card.id), &[]);
            let (passed, said) = judged(jev.as_ref(), &card, name, rubric);
            let decisions = crate::devflow_jev::decisions_of(config, jev.as_ref());
            (passed, said, Some(decisions))
        }
    };
    let at = SystemTime::now();
    let path = crate::devflow_evidence::runs_of(&project.id, &checkout)?.fresh(&card.id, at)?;
    let folder = path.display();
    let file = stage::file_of(name);
    let record = stage::recorded(name, gate, passed, &runs::stamp(at), &said);
    std::fs::write(
        path.join(&file),
        serde_json::to_string_pretty(&record).unwrap_or_default(),
    )
    .map_err(|err| format!("{folder}/{file}: {err}"))?;
    if let Ok(home) = crate::projects::project_home(&project.id) {
        let run = path
            .file_name()
            .and_then(|run| run.to_str())
            .unwrap_or_default();
        let under = format!("{}/{run}", runs::attached_under(&card.id));
        runs::attach(
            &path,
            std::slice::from_ref(&file),
            &home.artifacts(),
            &under,
        );
    }
    let headline = format!(
        "{}stage {name}: {}",
        devflow_engine::plan::PREFIX,
        if passed { "passed" } else { "failed" }
    );
    let comment = match said.first() {
        Some(first) => format!("{headline}\n\n{first}"),
        None => headline,
    };
    let sayable = crate::cards::sayable(&comment).map_err(|err| err.message)?;
    store
        .add_comment(&card.id, crate::devflow_evidence::SIGNED, sayable)
        .map_err(|err| err.to_string())?;
    if let Some(app) = app {
        let _ = app.emit("board:changed", &project.id);
    }
    let mut answer = json!({
        "stage": name,
        "uses": step.uses,
        "result": if passed { "passed" } else { "failed" },
        "said": said,
        "record": format!("{folder}/{file}"),
        "next": if passed {
            "Go on to the next stage; devpit_devflow_gate reads this record at the end."
        } else {
            "Fix what the gate says, then call devpit_devflow_stage again."
        },
    });
    if let Some(decisions) = decisions {
        answer["decisions"] = decisions;
    }
    Ok(answer)
}

/// An own stage's skill, from wherever `uses` points; never copied.
fn skill_of(
    root: &Path,
    config: &Config,
    layered: Option<&Layered>,
    name: &str,
) -> Result<Value, String> {
    let step = config
        .flow
        .iter()
        .find(|step| step.name() == name && step.builtin().is_none() && config.is_on(step))
        .ok_or_else(|| format!("`{name}` is not an own stage switched on in this project"))?;
    let personal = crate::devflow_skills::personal()?;
    let installed = crate::devflow::installed_folder();
    let teams = layered
        .map(|one| one.applied.as_slice())
        .unwrap_or_default();
    let found = skills::own(
        &step.uses,
        &skills::Places {
            project: root,
            personal: &personal,
            teams,
            installed: installed.as_deref(),
        },
    )?;
    let gate = step.own.as_ref().and_then(|own| own.gate.as_ref());
    Ok(json!({
        "stage": name,
        "uses": step.uses,
        "layer": found.layer,
        "file": found.file.map(|file| file.display().to_string()),
        "text": found.text,
        "approve": step.own.as_ref().and_then(|own| own.approve).map(|approve| approve.word()),
        "gate": match gate {
            Some(StageGate::Command(command)) => json!({ "command": command }),
            Some(StageGate::Rubric(rubric)) => json!({ "rubric": rubric }),
            None => Value::Null,
        },
        "next": if gate.is_some() {
            "Follow the skill, then call devpit_devflow_stage with this stage and the card to pass its gate."
        } else {
            "Follow the skill: the stage has no gate."
        },
    }))
}

/// A team layer's command runs once the person confirmed it at this commit;
/// until then the answer says it waits, and the person is asked.
fn confirmed(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    layered: Option<&Layered>,
    step: &Step,
    command: &str,
) -> Result<(), String> {
    let Some(layered) = layered else {
        return Ok(());
    };
    let Some(team) = layered
        .applied
        .iter()
        .find(|team| team.name == layered.stages_from)
    else {
        // The project's own file, or the person's own layer: theirs already.
        return Ok(());
    };
    let print = stage::fingerprint(&team.git, &team.commit, command);
    if crate::devflow_asks::command_confirmed(store, &print) {
        return Ok(());
    }
    let short = team.commit.get(..12).unwrap_or(&team.commit);
    crate::devflow_asks::raise_for(
        app,
        &project.id,
        &project.name,
        DevflowChange::Command,
        &format!("{} at {short} ({}): {command}", team.name, step.uses),
        &print,
    );
    Err(format!(
        "the stage `{}` runs a command from the {} layer that the person has not confirmed: `{command}`. devpit is asking them in the app; call again once they said yes.",
        step.name(),
        team.name
    ))
}

/// The command, run as a check runs: the card in its environment only.
fn ran(
    command: &str,
    checkout: &Path,
    project: &Project,
    card: &devpit_core::CardRow,
    name: &str,
    targets: &[String],
) -> (bool, Vec<String>) {
    let branch = devpit_git::branch_at(checkout).unwrap_or_default();
    let mut context = context_of(
        card,
        Some(Checkout {
            path: checkout,
            branch: &branch,
        }),
        Some(&project.root_path),
    )
    .for_a_command();
    context
        .shared
        .push(("DEVFLOW_STAGE".to_owned(), name.to_owned()));
    devflow_engine::security::with_targets(&mut context.shared, targets);
    // A `DEVPIT_` name is the step context's own to set.
    context
        .shared
        .retain(|(name, _)| !name.starts_with("DEVPIT_"));
    let mut output: Vec<String> = Vec::new();
    let ended = devpit_steps::run(
        command,
        checkout,
        &context,
        Some(LONGEST),
        |_| {},
        |said| {
            output.push(said.text.chars().take(300).collect());
            if output.len() > 4 * TAIL {
                output.drain(..TAIL);
            }
        },
    );
    match ended {
        Ok(ended) if ended.exit_code == Some(0) && !ended.timed_out => (true, Vec::new()),
        Ok(ended) => {
            let mut said = vec![if ended.timed_out {
                format!(
                    "`{command}` was stopped after {} min",
                    LONGEST.as_secs() / 60
                )
            } else {
                format!("`{command}` exited {}", ended.exit_code.unwrap_or(-1))
            }];
            let tail = output.len().saturating_sub(TAIL);
            said.extend(output.drain(tail..));
            (false, said)
        }
        Err(err) => (false, vec![format!("`{command}` did not run: {err}")]),
    }
}

/// A rubric gate: Decisions answers where the project uses it, and holds the
/// card only in `enforce`; without it the stage passes, as the gate does.
fn judged(
    jev: Option<&crate::devflow_jev::Jev>,
    card: &devpit_core::CardRow,
    name: &str,
    rubric: &str,
) -> (bool, Vec<String>) {
    let Some(jev) = jev else {
        return (
            true,
            vec![format!(
                "{rubric} not asked: this project does not use Decisions"
            )],
        );
    };
    let state = devflow_engine::jev::capped(&json!({
        "card": { "id": card.id, "title": card.title, "body": card.body },
        "stage": name,
    }));
    let said = jev.judge(rubric, &state);
    let holds = said.mode == devflow_engine::config::Mode::Enforce && said.outcome == "fail";
    let mut lines = vec![format!("{rubric}: {} ({})", said.outcome, said.mode.word())];
    lines.extend(said.skipped.clone());
    if said.skipped.is_none() {
        lines.extend(said.failing());
    }
    (!holds, lines)
}

#[cfg(test)]
#[path = "devflow_stage_tests.rs"]
mod tests;
