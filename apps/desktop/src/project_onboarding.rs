//! A project's agent learns its project on its own. With Labs on, a project
//! linked to an orchestrator gets its agent once, in the background and one
//! project at a time: the agent reads the repository and proposes the facts
//! and decisions it found, each resting on the files that show it. They wait
//! for the person like any proposal; "Re-learn" reads the project again.

use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use devpit_core::{preference, Store};
use devpit_rpc::Project;
use tauri::{AppHandle, Emitter, Manager};

use crate::maintenance::LabsProject;
use crate::project_agent::{Agents, Wake};
use crate::project_memory::{self, ONBOARDING};

/// The most entries one read proposes, and the files one rests on.
const MOST: usize = 30;
const FILES: usize = 6;
/// One line of the memory, at most.
const CHARS: usize = 1000;
/// How often a read looks again at an agent busy answering someone.
const BUSY_WAIT: Duration = Duration::from_secs(5);
/// Linked projects are looked at once the app has settled after starting.
const AFTER_START: Duration = Duration::from_secs(60);

/// The projects waiting to be learned, and whether one is being learned.
static QUEUE: Mutex<(VecDeque<String>, bool)> = Mutex::new((VecDeque::new(), false));

/// What the agent is asked to learn its project with.
pub(crate) fn learning_prompt() -> String {
    format!(
        "Learn this project, the way you read it to answer questions: its guides (AGENTS.md, \
         CLAUDE.md, the README and the docs), its ADRs, how it is built, tested and run, its \
         layout, and its conventions. Then answer with what is worth keeping in your memory: \
         between 15 and {MOST} lines, each one fact or one decision of this project, in exactly \
         this form:\n\n- fact · <files> · <what is true, in one line>\n- decision · <files> · \
         <what was decided and why, in one line>\n\n<files> are the paths, from the project's \
         root and comma-separated, of the files that show it: files you read, at most {FILES}. \
         Leave out what your memory already says and anything the project's own files do not \
         show. Answer with those lines alone."
    )
}

/// The facts and decisions in `answer`, each as its source (the read's mark
/// and the files it rests on) and its proposal. One resting on no file of the
/// project, or saying what `known` or an earlier line says, is left out.
pub(crate) fn proposed_in(root: &Path, answer: &str, known: &[String]) -> Vec<(String, String)> {
    let mut said: HashSet<String> = known.iter().map(|one| one.to_lowercase()).collect();
    let mut found = Vec::new();
    for line in answer.lines() {
        if found.len() == MOST {
            break;
        }
        let Some(rest) = line.trim().strip_prefix("- ") else {
            continue;
        };
        let mut parts = rest.splitn(3, " · ");
        let (Some(kind), Some(files), Some(text)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let file = match kind.trim() {
            "fact" => "facts.md",
            "decision" => "decisions.md",
            _ => continue,
        };
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let files: Vec<String> = files
            .split(',')
            .filter_map(|named| real_file(root, named))
            .take(FILES)
            .collect();
        if files.is_empty() || text.is_empty() || text.chars().count() > CHARS {
            continue;
        }
        if said.insert(text.to_lowercase()) {
            let source = format!("{ONBOARDING}, {}", files.join(", "));
            found.push((source, project_memory::aimed(file, &text, None)));
        }
    }
    found
}

/// `named` as a file of the project, from its root, when it is one.
fn real_file(root: &Path, named: &str) -> Option<String> {
    // `src/main.rs`:12 names the file.
    let named = named
        .trim()
        .trim_start_matches('`')
        .split([':', '`'])
        .next()?;
    let named = named.trim_start_matches("./");
    let named = Path::new(named)
        .strip_prefix(root)
        .map_or(named, |inside| inside.to_str().unwrap_or_default());
    if named.is_empty() || Path::new(named).is_absolute() {
        return None;
    }
    devpit_core::tree::resolve(root, named)
        .ok()
        .filter(|at| at.is_file())
        .map(|_| named.to_owned())
}

/// Has `key`'s agent read its project and propose what it learned, in place
/// of what an earlier read proposed and still waits; answers how many. With
/// `accept`, a fact resting only on files is accepted without the person.
pub(crate) fn learn(
    agents: &Agents,
    key: &str,
    wake: &Wake,
    day: &str,
    accept: bool,
) -> Result<usize, String> {
    let answer = agents.ask(key, wake, &learning_prompt())?;
    let root = wake.root;
    project_memory::drop_onboarding(root)?;
    let mut known: Vec<String> = project_memory::FILES
        .iter()
        .flat_map(|file| project_memory::kept(root, file))
        .map(|one| one.text)
        .collect();
    known.extend(
        project_memory::proposals(root)
            .iter()
            .map(|one| project_memory::aim(&one.text).1.to_owned()),
    );
    let found = proposed_in(root, &answer.text, &known);
    for (source, text) in &found {
        project_memory::propose_or_accept(root, day, source, std::slice::from_ref(text), accept)?;
    }
    project_memory::onboarded(root, day)?;
    Ok(found.len())
}

/// Whether the agent of the project at `root` has learned it on its own.
pub(crate) fn learned_before(root: &Path) -> bool {
    project_memory::learned_on(root).is_some()
}

/// The projects whose agent learns them on its own now: linked to an
/// orchestrator, not left out in Labs, and not learned yet.
pub(crate) fn to_learn(
    projects: &[Project],
    linked: &[String],
    labs: bool,
    left_out: &[String],
) -> Vec<String> {
    if !labs {
        return Vec::new();
    }
    projects
        .iter()
        .filter(|one| one.orchestrator.is_none())
        .filter(|one| linked.contains(&one.id) && !left_out.contains(&one.id))
        .filter(|one| {
            let root = Path::new(&one.root_path);
            root.is_dir() && !learned_before(root)
        })
        .map(|one| one.id.clone())
        .collect()
}

/// The projects Labs leaves out of the project agent.
pub(crate) fn left_out(store: &Store) -> Vec<String> {
    store
        .preference(preference::LABS_PROJECT_AGENT_OFF)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// The projects whose facts resting only on files are accepted without the person.
fn accepting(store: &Store) -> Vec<String> {
    store
        .preference(preference::LABS_ACCEPT_FILE_FACTS)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Whether Labs accepts `project_id`'s facts that rest only on files.
pub(crate) fn accepts_file_facts(store: &Store, project_id: &str) -> bool {
    accepting(store).iter().any(|id| id == project_id)
}

/// Accepts `project_id`'s facts resting only on files without the person, or stops.
pub(crate) fn accept_file_facts(store: &Store, project_id: &str, on: bool) -> Result<(), String> {
    let mut ids = accepting(store);
    ids.retain(|id| id != project_id);
    if on {
        ids.push(project_id.to_owned());
    }
    let ids = serde_json::to_string(&ids).map_err(|err| err.to_string())?;
    store
        .set_preference(preference::LABS_ACCEPT_FILE_FACTS, &ids)
        .map_err(|err| err.to_string())
}

/// Whether the project agent is on for `project_id`: Labs has it on, and
/// does not leave that project out.
pub(crate) fn on_for(store: &Store, project_id: &str) -> bool {
    crate::maintenance::project_agent_on(store)
        && !left_out(store).iter().any(|id| id == project_id)
}

/// Leaves `project_id` out of the project agent, or takes it back in.
pub(crate) fn leave_out(store: &Store, project_id: &str, out: bool) -> Result<(), String> {
    let mut ids = left_out(store);
    ids.retain(|id| id != project_id);
    if out {
        ids.push(project_id.to_owned());
    }
    let ids = serde_json::to_string(&ids).map_err(|err| err.to_string())?;
    store
        .set_preference(preference::LABS_PROJECT_AGENT_OFF, &ids)
        .map_err(|err| err.to_string())
}

/// The projects Settings → Labs lists: each linked to an orchestrator, with
/// its own switch.
pub(crate) fn labs_projects(projects: &[Project], store: &Store) -> Vec<LabsProject> {
    let (linked, out, accept) = (linked(projects), left_out(store), accepting(store));
    projects
        .iter()
        .filter(|one| one.orchestrator.is_none() && linked.contains(&one.id))
        .map(|one| LabsProject {
            id: one.id.clone(),
            name: one.name.clone(),
            on: !out.contains(&one.id),
            accept_facts: accept.contains(&one.id),
        })
        .collect()
}

/// Every project linked to an orchestrator.
fn linked(projects: &[Project]) -> Vec<String> {
    projects
        .iter()
        .filter(|one| one.orchestrator.is_some())
        .flat_map(|one| crate::orchestrator_links::linked(Path::new(&one.root_path)))
        .collect()
}

/// Queues the linked projects not learned yet. On start, after linking.
pub(crate) fn sweep(app: &AppHandle) {
    let Ok(store) = crate::projects::store() else {
        return;
    };
    let Ok(listed) = crate::projects::project_list_now() else {
        return;
    };
    let ids = to_learn(
        &listed.projects,
        &linked(&listed.projects),
        crate::maintenance::project_agent_on(&store),
        &left_out(&store),
    );
    queue(app, ids);
}

/// Sweeps once the app has settled after starting.
pub(crate) fn on_start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(AFTER_START);
        sweep(&app);
    });
}

/// Whether `project_id` waits to be learned or is being learned.
pub(crate) fn learning(project_id: &str) -> bool {
    QUEUE
        .lock()
        .is_ok_and(|held| held.0.iter().any(|id| id == project_id))
        || LEARNING
            .lock()
            .is_ok_and(|now| now.as_deref() == Some(project_id))
}

/// The project being learned now.
static LEARNING: Mutex<Option<String>> = Mutex::new(None);

/// Adds `ids` to the queue, each once, and starts the one reader if none runs.
pub(crate) fn queue(app: &AppHandle, ids: Vec<String>) {
    let now = LEARNING.lock().ok().and_then(|now| now.clone());
    let start = {
        let Ok(mut held) = QUEUE.lock() else {
            return;
        };
        added(&mut held.0, now.as_deref(), ids);
        !std::mem::replace(&mut held.1, true)
    };
    if start {
        let app = app.clone();
        std::thread::spawn(move || read_each(&app));
    }
}

/// Queues each of `ids` once, and not the one being learned now.
fn added(queued: &mut VecDeque<String>, now: Option<&str>, ids: Vec<String>) {
    for id in ids {
        if !queued.contains(&id) && now != Some(id.as_str()) {
            queued.push_back(id);
        }
    }
}

/// Learns the queued projects one at a time, each once its agent is free.
fn read_each(app: &AppHandle) {
    loop {
        let next = {
            let Ok(mut held) = QUEUE.lock() else {
                return;
            };
            let next = held.0.pop_front();
            held.1 = next.is_some();
            next
        };
        let Some(id) = next else {
            return;
        };
        if let Ok(mut now) = LEARNING.lock() {
            *now = Some(id.clone());
        }
        let _ = app.emit("lessons:proposed", &id);
        let _ = read_one(app, &id);
        if let Ok(mut now) = LEARNING.lock() {
            *now = None;
        }
        let _ = app.emit("lessons:proposed", &id);
    }
}

/// One project's read, under the profile of an orchestrator linked to it.
fn read_one(app: &AppHandle, project_id: &str) -> Result<usize, String> {
    let store = crate::projects::store().map_err(|err| err.message)?;
    if !on_for(&store, project_id) {
        return Ok(0);
    }
    let listed = crate::projects::project_list_now().map_err(|err| err.message)?;
    let project = listed
        .projects
        .iter()
        .find(|one| one.id == project_id && one.orchestrator.is_none())
        .ok_or("that project is gone")?;
    let profile = crate::project_agent::linked_profile(&listed.projects, project_id)?;
    let (command, env, mcp) = crate::project_agent::waking(&profile)?;
    let wake = Wake {
        command: &command,
        env: &env,
        root: Path::new(&project.root_path),
        name: &project.name,
        mcp_config: mcp.as_deref(),
    };
    let agents = app.state::<Agents>();
    // Never in the middle of an answer: the person or a session asked first.
    while agents.busy(project_id) {
        std::thread::sleep(BUSY_WAIT);
    }
    let accept = Store::open_default().is_ok_and(|store| accepts_file_facts(&store, project_id));
    learn(
        &agents,
        project_id,
        &wake,
        &crate::project_agent::today(),
        accept,
    )
}

#[cfg(all(test, unix))]
#[path = "project_onboarding_tests.rs"]
mod tests;
