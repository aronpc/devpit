//! A project's agent learns its project once the person turns it on for that
//! project, in the background and one project at a time: the agent reads the
//! repository and answers the facts and decisions it found, each resting on
//! the files that show it. Its facts enter the memory; its decisions wait for
//! the person like any proposal. "Re-learn" reads the project again. A
//! project linked while off is asked about, never turned on.

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

/// The window reads again which projects wait for the person's answer.
pub(crate) const ASKED: &str = "learning:asked";

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

/// Has `key`'s agent read its project and keep what it learned, in place of
/// what an earlier read proposed and still waits; answers how many. A fact
/// resting only on files enters the memory, the rest waits for the person.
pub(crate) fn learn(agents: &Agents, key: &str, wake: &Wake, day: &str) -> Result<usize, String> {
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
        project_memory::propose_or_accept(root, day, source, std::slice::from_ref(text), true)?;
    }
    project_memory::onboarded(root, day)?;
    Ok(found.len())
}

/// Whether the agent of the project at `root` has learned it on its own.
pub(crate) fn learned_before(root: &Path) -> bool {
    project_memory::learned_on(root).is_some()
}

/// The projects Labs left out before each was turned on: read once, to carry over.
pub(crate) fn left_out(store: &Store) -> Vec<String> {
    ids(store, preference::LABS_PROJECT_AGENT_OFF).unwrap_or_default()
}

fn ids(store: &Store, key: &str) -> Option<Vec<String>> {
    store
        .preference(key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

/// The projects the person turned on; `None` before the first start that asks.
fn chosen(store: &Store) -> Option<Vec<String>> {
    ids(store, preference::LABS_PROJECT_AGENT_ON)
}

/// Before projects were turned on one by one, a project its agent had learned
/// was on, unless Labs left it out.
fn was_on(store: &Store, project: &Project) -> bool {
    project.orchestrator.is_none()
        && learned_before(Path::new(&project.root_path))
        && !left_out(store).contains(&project.id)
}

/// The projects turned on, carried over from before when none was chosen yet.
fn turned_on(store: &Store, projects: &[Project]) -> Vec<String> {
    chosen(store).unwrap_or_else(|| {
        projects
            .iter()
            .filter(|one| was_on(store, one))
            .map(|one| one.id.clone())
            .collect()
    })
}

fn keep(store: &Store, key: &str, ids: &[String]) -> Result<(), String> {
    let ids = serde_json::to_string(ids).map_err(|err| err.to_string())?;
    store
        .set_preference(key, &ids)
        .map_err(|err| err.to_string())
}

fn choose(store: &Store, ids: &[String]) -> Result<(), String> {
    keep(store, preference::LABS_PROJECT_AGENT_ON, ids)
}

/// The projects waiting for the person to say whether they learn.
fn asking(store: &Store) -> Vec<String> {
    ids(store, preference::LABS_PROJECT_AGENT_ASKING).unwrap_or_default()
}

/// The projects never asked again: answered Not now, turned off, or left
/// out by Labs before.
fn declined(store: &Store) -> Vec<String> {
    let mut ids = ids(store, preference::LABS_PROJECT_AGENT_DECLINED).unwrap_or_default();
    ids.extend(left_out(store));
    ids
}

/// The person's answer for `project_id`: it waits no more, and off is kept
/// as their answer.
fn answered(store: &Store, project_id: &str, on: bool) -> Result<(), String> {
    let mut waiting = asking(store);
    waiting.retain(|id| id != project_id);
    keep(store, preference::LABS_PROJECT_AGENT_ASKING, &waiting)?;
    let mut said = ids(store, preference::LABS_PROJECT_AGENT_DECLINED).unwrap_or_default();
    said.retain(|id| id != project_id);
    if !on {
        said.push(project_id.to_owned());
    }
    keep(store, preference::LABS_PROJECT_AGENT_DECLINED, &said)
}

/// Asks the person about each of `linked` that is off and was never
/// answered; linking turns nothing on. Answers whether one more waits.
pub(crate) fn ask_about(
    store: &Store,
    projects: &[Project],
    linked: &[String],
) -> Result<bool, String> {
    if !crate::maintenance::project_agent_on(store) {
        return Ok(false);
    }
    let _held = CHOOSING.lock().map_err(|_| "Labs is busy".to_owned())?;
    let (on, declined, mut waiting) = (turned_on(store, projects), declined(store), asking(store));
    let before = waiting.len();
    for id in linked {
        let project = projects
            .iter()
            .any(|one| &one.id == id && one.orchestrator.is_none());
        if project && !on.contains(id) && !declined.contains(id) && !waiting.contains(id) {
            waiting.push(id.clone());
        }
    }
    if waiting.len() == before {
        return Ok(false);
    }
    keep(store, preference::LABS_PROJECT_AGENT_ASKING, &waiting)?;
    Ok(true)
}

/// Choosing and carrying over, one at a time.
static CHOOSING: Mutex<()> = Mutex::new(());

/// Turns the project agent on for `project_id`, or off. Answers whether its
/// agent reads the project now: turned on, and never learned.
pub(crate) fn turn(
    store: &Store,
    projects: &[Project],
    project_id: &str,
    on: bool,
) -> Result<bool, String> {
    let _held = CHOOSING.lock().map_err(|_| "Labs is busy".to_owned())?;
    let mut ids = turned_on(store, projects);
    ids.retain(|id| id != project_id);
    if on {
        ids.push(project_id.to_owned());
    }
    choose(store, &ids)?;
    answered(store, project_id, on)?;
    Ok(on
        && projects.iter().any(|one| {
            let root = Path::new(&one.root_path);
            one.id == project_id && root.is_dir() && !learned_before(root)
        }))
}

/// Once, on the first start that turns projects on one by one: the projects
/// learned before stay on, and what their reads left waiting is accepted,
/// each to its memory file dated `day`. Answers the projects it accepted in.
pub(crate) fn carry_over(
    store: &Store,
    projects: &[Project],
    day: &str,
) -> Result<Vec<String>, String> {
    let _held = CHOOSING.lock().map_err(|_| "Labs is busy".to_owned())?;
    if chosen(store).is_some() {
        return Ok(Vec::new());
    }
    let on = turned_on(store, projects);
    let accepted = projects
        .iter()
        .filter(|one| one.orchestrator.is_none())
        .filter(|one| {
            let root = Path::new(&one.root_path);
            project_memory::proposals(root)
                .iter()
                .any(project_memory::Entry::onboarding)
                && project_memory::approve_onboarding(root, day).is_ok()
        })
        .map(|one| one.id.clone())
        .collect();
    // Chosen last: a start that stops halfway carries over again.
    choose(store, &on)?;
    Ok(accepted)
}

/// The projects whose facts resting only on files are accepted without the person.
fn accepting(store: &Store) -> Vec<String> {
    ids(store, preference::LABS_ACCEPT_FILE_FACTS).unwrap_or_default()
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
    keep(store, preference::LABS_ACCEPT_FILE_FACTS, &ids)
}

/// Whether the project agent is on for `project`: Labs makes it available,
/// and the person turned it on for that project.
pub(crate) fn on_for(store: &Store, project: &Project) -> bool {
    crate::maintenance::project_agent_on(store)
        && chosen(store).map_or_else(|| was_on(store, project), |ids| ids.contains(&project.id))
}

/// The projects Settings → Labs lists: each linked to an orchestrator, with
/// its own switch.
pub(crate) fn labs_projects(projects: &[Project], store: &Store) -> Vec<LabsProject> {
    let (linked, on, accept, waiting) = (
        linked(store, projects),
        turned_on(store, projects),
        accepting(store),
        asking(store),
    );
    projects
        .iter()
        .filter(|one| one.orchestrator.is_none() && linked.contains(&one.id))
        .map(|one| LabsProject {
            id: one.id.clone(),
            name: one.name.clone(),
            on: on.contains(&one.id),
            accept_facts: accept.contains(&one.id),
            asked: !on.contains(&one.id) && waiting.contains(&one.id),
        })
        .collect()
}

/// Every project linked to an orchestrator.
fn linked(store: &Store, projects: &[Project]) -> Vec<String> {
    projects
        .iter()
        .filter(|one| one.orchestrator.is_some())
        .flat_map(|one| crate::orchestrator_links::linked_in(store, Path::new(&one.root_path)))
        .collect()
}

/// Carries the Labs choice over from before, once the app has started.
pub(crate) fn on_start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let (Ok(store), Ok(listed)) = (
            crate::projects::store(),
            crate::projects::project_list_now(),
        ) else {
            return;
        };
        let day = crate::project_agent::today();
        if let Ok(accepted) = carry_over(&store, &listed.projects, &day) {
            for id in accepted {
                let _ = app.emit("lessons:proposed", &id);
            }
        }
    });
}

/// Turns the project agent on for `project_id`, or off, from Labs or its
/// page; turned on, its agent learns the project if it never did.
pub(crate) fn switch(
    app: &AppHandle,
    store: &Store,
    projects: &[Project],
    project_id: &str,
    on: bool,
) -> Result<(), String> {
    if on {
        crate::project_agent::linked_profile(projects, project_id)?;
    }
    let day = crate::project_agent::today();
    for id in carry_over(store, projects, &day)? {
        let _ = app.emit("lessons:proposed", &id);
    }
    if turn(store, projects, project_id, on)? {
        queue(app, vec![project_id.to_owned()]);
    }
    let _ = app.emit("lessons:proposed", project_id);
    let _ = app.emit(ASKED, ());
    Ok(())
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
    let listed = crate::projects::project_list_now().map_err(|err| err.message)?;
    let project = listed
        .projects
        .iter()
        .find(|one| one.id == project_id && one.orchestrator.is_none())
        .ok_or("that project is gone")?;
    if !on_for(&store, project) {
        return Ok(0);
    }
    let profile = crate::project_agent::linked_profile(&listed.projects, project_id)?;
    let locale = app.state::<Agents>().locale_for(project_id);
    let (command, env, mcp) = crate::project_agent::waking(&profile, locale)?;
    let wake = Wake {
        command: &command,
        env: &env,
        root: Path::new(&project.root_path),
        name: &project.name,
        mcp_config: mcp.as_deref(),
        locale,
    };
    let agents = app.state::<Agents>();
    // Never in the middle of an answer: the person or a session asked first.
    while agents.busy(project_id) {
        std::thread::sleep(BUSY_WAIT);
    }
    learn(&agents, project_id, &wake, &crate::project_agent::today())
}

#[cfg(all(test, unix))]
#[path = "project_onboarding_tests.rs"]
mod tests;
