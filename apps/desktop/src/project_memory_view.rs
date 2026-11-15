//! The project agent's page: its memory as its files keep it, what waits for
//! the person, and the two buttons that approve every proposal of the agent's
//! own read and have it read the project again.

use std::path::Path;

use devpit_rpc::{LessonProposal, MemoryEntry, MemoryProposal, Project, ProjectMemory, RpcError};
use tauri::{AppHandle, Emitter};

use crate::project_memory::{self, Entry};

fn shown(one: &Entry) -> MemoryEntry {
    MemoryEntry {
        day: one.day.clone(),
        source: one.source.clone(),
        text: one.text.clone(),
        files: one.files().into_iter().map(str::to_owned).collect(),
        cards: one.cards().into_iter().map(str::to_owned).collect(),
        auto: one.auto(),
    }
}

/// A waiting proposal of `project`, as the page and the orchestrator read it.
pub(crate) fn proposal_of(project: &Project, one: &Entry) -> LessonProposal {
    let (file, text, replaces) = project_memory::aim(&one.text);
    LessonProposal {
        id: project_memory::id_of(one),
        project_id: project.id.clone(),
        project: project.name.clone(),
        day: one.day.clone(),
        source: one.source.clone(),
        file: file.to_owned(),
        text: text.to_owned(),
        replaces: replaces.map(str::to_owned),
        onboarding: one.onboarding(),
        by: project_memory::proposer(one),
        summary: project_memory::plain(&one.text),
    }
}

/// `project`'s memory and what waits for the person, as its page shows them,
/// `on` when the person turned its agent on.
pub(crate) fn memory_of(project: &Project, on: bool) -> ProjectMemory {
    let root = Path::new(&project.root_path);
    let kept = |file: &str| -> Vec<MemoryEntry> {
        project_memory::kept(root, file).iter().map(shown).collect()
    };
    let proposals = project_memory::proposals(root)
        .iter()
        .map(|one| MemoryProposal {
            proposal: proposal_of(project, one),
            files: one.files().into_iter().map(str::to_owned).collect(),
            cards: one.cards().into_iter().map(str::to_owned).collect(),
        })
        .collect();
    ProjectMemory {
        project_id: project.id.clone(),
        kept: root.join(project_memory::DIR).is_dir(),
        facts: kept("facts.md"),
        lessons: kept("lessons.md"),
        decisions: kept("decisions.md"),
        proposals,
        learning: crate::project_onboarding::learning(&project.id),
        learned: project_memory::learned_on(root),
        on,
    }
}

fn project(project_id: &str) -> Result<Project, RpcError> {
    let listed = crate::projects::project_list_now()?;
    crate::project_agent::sidebar_project(&listed.projects, project_id)
}

/// [`memory_of`], read with whether its agent is on.
fn page(project: &Project) -> Result<ProjectMemory, RpcError> {
    let store = crate::projects::store()?;
    Ok(memory_of(
        project,
        crate::project_onboarding::on_for(&store, project),
    ))
}

/// `project_agent.memory` — the project agent's page.
#[tauri::command]
#[specta::specta]
pub async fn project_agent_memory(project_id: String) -> Result<ProjectMemory, RpcError> {
    crate::off_main::blocking(move || page(&project(&project_id)?)).await
}

/// `project_agent.approve_learned` — approves every proposal of the agent's
/// own read of the project: they rest only on the repository's files.
#[tauri::command]
#[specta::specta]
pub async fn project_agent_approve_learned(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectMemory, RpcError> {
    crate::off_main::blocking(move || {
        let project = project(&project_id)?;
        project_memory::approve_onboarding(
            Path::new(&project.root_path),
            &crate::project_agent::today(),
        )
        .map_err(RpcError::internal)?;
        let _ = app.emit("lessons:proposed", &project.id);
        page(&project)
    })
    .await
}

/// `project_agent.undo_accepted` — takes back a fact accepted without the
/// person: it waits for their decision again.
#[tauri::command]
#[specta::specta]
pub async fn project_agent_undo_accepted(
    app: AppHandle,
    project_id: String,
    text: String,
) -> Result<ProjectMemory, RpcError> {
    crate::off_main::blocking(move || {
        let project = project(&project_id)?;
        project_memory::undo_accepted(Path::new(&project.root_path), &text)
            .map_err(|why| RpcError::new(devpit_rpc::ErrorCode::NotFound, why))?;
        let _ = app.emit("lessons:proposed", &project.id);
        page(&project)
    })
    .await
}

/// `project_agent.relearn` — has the agent read its project again, in the
/// background; what it finds replaces its earlier read's proposals.
#[tauri::command]
#[specta::specta]
pub async fn project_agent_relearn(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectMemory, RpcError> {
    crate::off_main::blocking(move || {
        let project = project(&project_id)?;
        crate::project_agent::labs_on(&crate::projects::store()?, &project)?;
        crate::project_onboarding::queue(&app, vec![project.id.clone()]);
        page(&project)
    })
    .await
}

#[cfg(test)]
#[path = "project_memory_view_tests.rs"]
mod tests;
