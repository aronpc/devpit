//! `devflow_evidence`: a card's checks run in the checkout the agent stands
//! in, the run kept in devpit's folder for the project, `<card>/<time>/`, and
//! what it left attached to the card — among the project's artifacts and
//! linked from a comment, so a screenshot or a video opens on the card.
//!
//! The rules are `devflow_engine::evidence`'s and `runs`'s; this adds the
//! board, the secrets devpit knows and the window.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use devflow_engine::evidence::{self, Card, Given};
use devflow_engine::runs::{self, Runs};
use devflow_engine::Config;
use devpit_rpc::Project;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::steps::context::{context_of, Checkout};

/// Who signs the comment: devpit ran the checks, and an agent's comment is
/// never signed with this name.
pub(crate) const SIGNED: &str = "devpit";

/// What the agent's wait keeps for attaching and answering once the checks stop.
const SPARE: Duration = Duration::from_secs(30);

pub(crate) fn respond(
    app: Option<&AppHandle>,
    project: &Project,
    config: &Config,
    cwd: &Path,
    params: &Value,
) -> Result<Value, String> {
    let asked = evidence::asked_in(&crate::devflow_dialect::DEVPIT, params)?;
    let store = crate::projects::store().map_err(|err| err.message)?;
    let on_board = store
        .project_id_of_card(&asked.card)
        .map_err(|err| err.to_string())?;
    let card = store
        .card(&asked.card)
        .map_err(|err| err.to_string())?
        .filter(|_| on_board.as_deref() == Some(project.id.as_str()))
        .ok_or_else(|| format!("no card `{}` on this project's board", asked.card))?;

    let root = checkout(project, cwd);
    let branch = devpit_git::branch_at(&root).unwrap_or_default();
    let mut context = engine_context(
        context_of(
            &card,
            Some(Checkout {
                path: &root,
                branch: &branch,
            }),
            Some(&project.root_path),
        )
        .for_a_command(),
    );
    devflow_engine::security::with_targets(
        &mut context.shared,
        &crate::devflow::targets_in_force(&store, project, Path::new(&project.root_path)),
    );
    let secrets = crate::kept_out::what_devpit_gave(&store);
    let redact = |text: &str| crate::kept_out::kept_out(text, &secrets);
    let home = crate::projects::project_home(&project.id).map_err(|err| err.message)?;
    let found = evidence::gathered(&Given {
        dialect: &crate::devflow_dialect::DEVPIT,
        root: &root,
        runs: &runs_of(&project.id, &root)?,
        locks: &home.devflow_locks(),
        config,
        asked: &asked,
        card: Card {
            id: &card.id,
            title: &card.title,
            body: &card.body,
        },
        context: &context,
        budget: devpit_agentapi::client::wait_for("devflow_evidence").saturating_sub(SPARE),
        redact: &redact,
        at: SystemTime::now(),
    })?;

    let run = PathBuf::from(&found.folder);
    let artifacts = home.artifacts();
    let card_folder = runs::attached_under(&card.id);
    let under = format!("{card_folder}/{}", found.stamp);
    let attached = runs::attach(&run, &found.files(&run), &artifacts, &under);
    runs::prune(&artifacts.join(&card_folder), runs::KEPT);
    let links: Vec<(String, String)> = attached
        .names
        .iter()
        .map(|name| {
            let short = name.strip_prefix(&format!("{under}/")).unwrap_or(name);
            (short.to_owned(), artifacts.join(name).display().to_string())
        })
        .collect();
    let said = evidence::comment(&found, &links);
    let sayable = crate::cards::sayable(&said).map_err(|err| err.message)?;
    store
        .add_comment(&card.id, SIGNED, sayable)
        .map_err(|err| err.to_string())?;
    if let Some(app) = app {
        let _ = app.emit("board:changed", &project.id);
    }
    Ok(evidence::summary(&found, &attached, &under))
}

/// Where `project_id`'s runs are kept, read with what an older devpit left
/// in `checkout`.
pub(crate) fn runs_of(project_id: &str, checkout: &Path) -> Result<Runs, String> {
    let home = crate::projects::project_home(project_id).map_err(|err| err.message)?;
    Ok(Runs {
        home: home.devflow_runs(),
        checkout: checkout.to_path_buf(),
        dialect: &crate::devflow_dialect::DEVPIT,
    })
}

/// The runs of the project `card_id` is on, with its checkout's.
pub(crate) fn runs_of_card(
    store: &devpit_core::Store,
    card_id: &str,
    checkout: &Path,
) -> Option<Runs> {
    let project = store.project_id_of_card(card_id).ok()??;
    runs_of(&project, checkout).ok()
}

/// devpit's step context as the engine's runner takes it.
pub(crate) fn engine_context(devpit: devpit_steps::Context) -> devflow_engine::runner::Context {
    devflow_engine::runner::Context {
        project: devpit.project,
        project_path: devpit.project_path,
        worktree_path: devpit.worktree_path,
        branch: devpit.branch,
        base_ref: devpit.base_ref,
        item: devpit.card,
        item_title: devpit.card_title,
        report_dir: devpit.report_dir,
        shared: devpit.shared,
        locks: devpit.locks,
    }
}

/// The project's root or worktree the agent stands in — where its work is —
/// else the project's root.
pub(crate) fn checkout(project: &Project, cwd: &Path) -> PathBuf {
    let here = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    std::iter::once(project.root_path.as_str())
        .chain(project.worktrees.iter().map(|tree| tree.path.as_str()))
        .filter_map(|root| Path::new(root).canonicalize().ok())
        .filter(|root| here.starts_with(root))
        .max_by_key(|root| root.components().count())
        .unwrap_or_else(|| PathBuf::from(&project.root_path))
}

#[cfg(test)]
#[path = "devflow_evidence_tests.rs"]
mod tests;
