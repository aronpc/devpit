//! Devflow through the agent API: its state with what devpit detected, a
//! setup, a change of one field, what the MCP server offers the agent
//! standing in a project, and an active project's skills.
//!
//! The file and the rules are `devpit_devflow`'s; this adds the machine's
//! half — the agent CLIs and their sign-ins, the Decisions key — and the
//! person's click, which only the window has.

use std::path::Path;

use devflow_engine::config::Agent;
use devflow_engine::detect::{self, Cli, Machine};
use devflow_engine::folder::{self, State};
use devflow_engine::layers::{self, Home, Layered};
use devflow_engine::preset::{self, Preset};
use devflow_engine::{proposal, setup, Ask, Proposal};
use devpit_core::locale::Locale;
use devpit_core::{preference, Store};
use devpit_rpc::{Credentials, Project};
use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::devflow_asks::{self as asks, globally_on, made};
use crate::devflow_dialect::{dialect_in, file, layout, DEVPIT};

pub(crate) const METHODS: [&str; 14] = [
    "devflow_offer",
    "devflow_status",
    "devflow_setup",
    "devflow_set",
    "devflow_skills",
    "devflow_skill",
    "devflow_brief",
    "devflow_plan",
    "devflow_evidence",
    "devflow_review",
    "devflow_publish",
    "devflow_gate",
    "devflow_session",
    "devflow_stage",
];

/// The stages a method of a stage's own belongs to: it answers where one of
/// them is on, as the MCP server lists it. The context stage writes its
/// summary onto the brief, so the brief's tool is its too.
pub(crate) fn stage_of(method: &str) -> Option<&'static [&'static str]> {
    match method {
        "devflow_brief" => Some(&["briefing", "context"]),
        "devflow_plan" => Some(&["plan"]),
        "devflow_evidence" => Some(&["execution"]),
        "devflow_review" => Some(&["review"]),
        "devflow_gate" => Some(&["completion"]),
        _ => None,
    }
}

/// What the MCP server lists for a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Offered {
    /// Devflow off here — no devflow.yaml, `enabled: false`, or switched off
    /// in Settings: the list devpit had before it.
    Off,
    /// The three tools that set it up, where a file waits for the person's
    /// click or does not read.
    Setup,
    /// Those, and the tools of the stages switched on.
    Active(Vec<&'static str>),
}

/// The offer, from the global switch, the file, and the person's click. A
/// project is off until the person activates it on the Devflow screen.
pub(crate) fn offered(globally: bool, state: &State, switched_on: bool) -> Offered {
    match state {
        _ if !globally => Offered::Off,
        State::Valid(_, config) if config.enabled && switched_on => {
            Offered::Active(config.stages_on())
        }
        State::Valid(_, config) if config.enabled => Offered::Setup,
        State::Invalid(..) => Offered::Setup,
        State::Missing | State::Valid(..) => Offered::Off,
    }
}

/// The word `devpit_devflow_status` says for `state`.
pub(crate) fn state_word(state: &State, active: bool) -> &'static str {
    match state {
        State::Missing => "no file",
        State::Invalid(..) => "invalid",
        State::Valid(..) if active => "active",
        State::Valid(..) => "disabled",
    }
}

/// One of Devflow's methods, asked by the agent standing in `cwd`, from a
/// session whose `CLAUDE_CONFIG_DIR` is `config_dir`.
pub(crate) fn respond_from(
    locale: Locale,
    app: Option<&AppHandle>,
    method: &str,
    project: &Project,
    cwd: &Path,
    config_dir: Option<&Path>,
    params: &Value,
) -> Result<Value, String> {
    let store = crate::projects::store().map_err(|err| err.message)?;
    let root = Path::new(&project.root_path);
    let state = looked(app, &store, project, root);
    if let Some(app) = app.filter(|_| is_on_with(&store, project, &state)) {
        crate::devflow_host::tend(app);
    }
    match method {
        "devflow_offer" => Ok(offer(&store, project, root, &state)),
        "devflow_status" => Ok(status(&store, project, root, &state)),
        "devflow_setup" => setup_of(app, &store, project, root, &state, params, By::Agent),
        "devflow_set" => set_of(app, &store, project, root, &state, params),
        // Answered where the stage is off too: a hook there says nothing.
        "devflow_gate" => {
            crate::devflow_gate::respond(locale, app, &store, project, &state, cwd, params)
        }
        "devflow_session" => crate::devflow_gate::session(&store, project, &state, cwd, params),
        "devflow_stage" => {
            stages_on(&store, project, &state)?;
            let State::Valid(_, config) = &state else {
                return Err("devflow.yaml does not read: devpit_devflow_status says why".to_owned());
            };
            crate::devflow_stage::respond(app, &store, project, config, cwd, params)
        }
        "devflow_skill" => {
            on_demand_here(&store, project, &state)?;
            let by_devflow = |asked| hosted(&store, project, root, "skills/get", asked);
            crate::devflow_skills::respond(method, root, &[], params, &by_devflow)
        }
        "devflow_review" if params.get("card").is_none_or(Value::is_null) => {
            on_demand_here(&store, project, &state)?;
            let asked = devflow_engine::adhoc::standalone_in(dialect_in(locale), params)?
                .ok_or("a card's review is asked with its card")?;
            let profile = crate::devflow_profile::runnable(&store, &project.id, config_dir)?;
            crate::devflow_adhoc::respond(locale, app, project, &state, asked, profile.as_ref())
        }
        "devflow_publish" => {
            on_demand_here(&store, project, &state)?;
            crate::devflow_publish::respond(project, &state, params)
        }
        "devflow_plan" if crate::devflow_plan::on_demand(params) => {
            on_demand_here(&store, project, &state)?;
            crate::devflow_plan::previewed(locale, params)
        }
        "devflow_skills" => crate::devflow_skills::respond(
            method,
            root,
            &stages_on(&store, project, &state)?,
            params,
            &|_| None,
        ),
        _ => match (stage_of(method), &state) {
            (Some(stages), State::Valid(_, config))
                if stages_on(&store, project, &state)?
                    .iter()
                    .any(|on| stages.contains(on)) =>
            {
                match method {
                    "devflow_evidence" => {
                        crate::devflow_evidence::respond(app, project, config, cwd, params)
                    }
                    "devflow_review" => {
                        let profile =
                            crate::devflow_profile::runnable(&store, &project.id, config_dir)?;
                        crate::devflow_review::respond(
                            app,
                            project,
                            config,
                            cwd,
                            params,
                            profile.as_ref(),
                        )
                    }
                    _ => crate::devflow_plan::respond(locale, app, method, project, params),
                }
            }
            (Some(stages), _) => {
                stages_on(&store, project, &state)?;
                Err(format!(
                    "Devflow's {} stage is switched off in this project's devflow.yaml.",
                    stages.join(" and ")
                ))
            }
            (None, _) => Err(format!("devpit does not answer `{method}`")),
        },
    }
}

/// Where this devpit keeps the layers a project's Devflow is read over.
pub(crate) fn home() -> Home {
    crate::devflow_dialect::pinned();
    Home::under(&Store::root().unwrap_or_default())
}

/// The project's devflow.yaml read over its layers — the person's own and
/// the teams it extends at their approved commits — which is what the flow
/// runs on. The text stays the project's own file, which a change edits.
pub(crate) fn read(root: &Path) -> State {
    crate::devflow_dialect::pinned();
    match folder::read_in(&DEVPIT, root) {
        State::Valid(text, _) => match layers::layered_in(&DEVPIT, root, &text, &home()) {
            Ok(layered) => State::Valid(text, Box::new(layered.config)),
            Err(invalid) => State::Invalid(Some(text), invalid),
        },
        other => other,
    }
}

/// The layers as they stand, when the project's file reads.
pub(crate) fn layered(root: &Path) -> Option<Layered> {
    match folder::read_in(&DEVPIT, root) {
        State::Valid(text, _) => layers::layered_in(&DEVPIT, root, &text, &home()).ok(),
        _ => None,
    }
}

/// A change refused when the file it leaves would not read over its layers:
/// the project may not weaken what a layer locked.
fn layered_as(root: &Path, proposal: &Proposal) -> Result<(), String> {
    layers::layered_in(&DEVPIT, root, &proposal.yaml, &home())
        .map(|_| ())
        .map_err(|invalid| format!("{invalid}"))
}

/// Whether Devflow is on in `project`: a card's blockers hold it back only there.
pub(crate) fn is_on(project_id: &str, root: &Path) -> bool {
    let Ok(store) = crate::projects::store() else {
        return false;
    };
    matches!(
        offered(
            globally_on(&store),
            &read(root),
            made(&store, project_id, Ask::Enable)
        ),
        Offered::Active(_)
    )
}

/// [`is_on`], from a file already read.
fn is_on_with(store: &Store, project: &Project, state: &State) -> bool {
    matches!(
        offered(
            globally_on(store),
            state,
            made(store, &project.id, Ask::Enable)
        ),
        Offered::Active(_)
    )
}

/// The stages switched on, when Devflow is on here: nowhere else are its
/// skills given, whatever the MCP server last listed.
pub(crate) fn stages_on(
    store: &Store,
    project: &Project,
    state: &State,
) -> Result<Vec<&'static str>, String> {
    let switched_on = made(store, &project.id, Ask::Enable);
    match offered(globally_on(store), state, switched_on) {
        Offered::Active(stages) => Ok(stages),
        _ => Err("Devflow is not on in this project: devpit_devflow_status says why.".to_owned()),
    }
}

/// Whether a stage may be run on demand — a review without a card, a plan
/// without an epic, any stage's skill: wherever this project's Devflow is
/// offered at all.
pub(crate) fn on_demand(offer: &Offered) -> bool {
    !matches!(offer, Offered::Off)
}

fn on_demand_here(store: &Store, project: &Project, state: &State) -> Result<(), String> {
    let globally = globally_on(store);
    let offer = offered(globally, state, made(store, &project.id, Ask::Enable));
    if project.orchestrator.is_some() || !on_demand(&offer) {
        return Err(match globally {
            false => "Devflow is switched off in devpit's Settings.",
            true => {
                "Devflow is off in this project: the person activates it on its Devflow screen."
            }
        }
        .to_owned());
    }
    Ok(())
}

/// The file's state, raising the person's ask when it says `enabled: true`
/// and nobody here has clicked for it.
pub(crate) fn looked(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    root: &Path,
) -> State {
    let state = read(root);
    if let State::Valid(_, config) = &state {
        if config.enabled && globally_on(store) && !made(store, &project.id, Ask::Enable) {
            asks::raise(app, &project.id, &project.name, Ask::Enable, true);
        }
    }
    state
}

fn offer(store: &Store, project: &Project, root: &Path, state: &State) -> Value {
    let offer = offered(
        globally_on(store),
        state,
        made(store, &project.id, Ask::Enable),
    );
    let orchestrator = project.orchestrator.is_some();
    let (word, stages) = match offer {
        // An orchestrator's folder is its notes, not a codebase to set a flow up in.
        _ if orchestrator => ("off", Vec::new()),
        Offered::Off => ("off", Vec::new()),
        Offered::Setup => ("setup", Vec::new()),
        Offered::Active(stages) => ("active", stages),
    };
    json!({
        "offer": word,
        "stages": stages,
        "watch": root.join(file()).display().to_string(),
        // What else the server lists: an orchestrator's tools, and Decisions'.
        "orchestrator": orchestrator,
        "decisions": crate::decisions::key().is_some(),
        "assistant": orchestrator && crate::assistant::on(store, root),
    })
}

/// Why devpit sets no security path: what an attack may reach is the
/// person's, and devpit holds none in force.
const PERSONS_TARGETS: &str = "what an attack may reach is the person's alone to widen, and \
     devpit holds no target in force: Devflow's checks and reviews here reach none";

/// What one `devflow host` call is given, for the file and the status.
const HOST_WAITS: std::time::Duration = std::time::Duration::from_secs(30);

/// What devpit tells devflow at `initialize`: itself, the project, its words
/// and the person's switches here.
pub(crate) fn host_init(store: &Store, project: &Project, root: &Path) -> Value {
    json!({
        "protocol": crate::devflow_host::PROTOCOL,
        "host": { "name": "devpit", "version": env!("CARGO_PKG_VERSION") },
        "root": root,
        "dialect": crate::devflow_dialect::host_words(),
        "consent": {
            "enabled": made(store, &project.id, Ask::Enable),
            "enforce": made(store, &project.id, Ask::Enforce),
        },
        "runs": crate::projects::project_home(&project.id)
            .ok()
            .map(|home| home.devflow_runs()),
        "personal": {
            "skills": crate::devflow_skills::personal().ok(),
            "reviewers": devpit_core::Store::root()
                .ok()
                .map(|home| devpit_core::home::devflow_reviewers(&home)),
        },
    })
}

/// No request of devflow's is answered for the file or the status.
fn none_asked(method: &str, _: Value) -> Result<Value, String> {
    Err(format!("devpit does not answer {method} here"))
}

/// Whether devflow reads the file devpit does. devflow stands in the git
/// repository's top folder, takes a devflow.yaml at its root or in
/// `.devflow/` before `.devpit/devflow/`, and that one only as a plain file.
pub(crate) fn reads_devpits_file(root: &Path) -> bool {
    let here = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let top = devflow_engine::git::toplevel(&here)
        .ok()
        .and_then(|top| Path::new(&top).canonicalize().ok());
    let devpits = std::fs::symlink_metadata(here.join(".devpit/devflow/devflow.yaml"));
    top.is_none_or(|top| top == here)
        && devpits.is_ok_and(|meta| meta.is_file())
        && ["devflow.yaml", ".devflow/devflow.yaml"]
            .iter()
            .all(|own| std::fs::symlink_metadata(here.join(own)).is_err())
}

/// devflow's answer to `method` for `project`, when the devflow here serves
/// it and reads the same devflow.yaml devpit does.
pub(crate) fn hosted(
    store: &Store,
    project: &Project,
    root: &Path,
    method: &str,
    params: Value,
) -> Option<Result<Value, crate::devflow_host::Failed>> {
    if !reads_devpits_file(root) {
        return None;
    }
    crate::devflow_host::asked_of(
        root,
        host_init(store, project, root),
        method,
        params,
        &none_asked,
        HOST_WAITS,
    )
}

/// The targets an attack may reach: those devflow.yaml lists that the person
/// confirmed. devpit takes no such confirmation, and a file any shell can
/// write is not the person's, so none is in force.
pub(crate) fn targets_in_force(config: &devflow_engine::Config) -> Vec<String> {
    devflow_engine::security::in_force(
        &config.security.targets,
        &[],
        &devflow_engine::security::resolved_here,
    )
    .0
}

/// The fields a newer devpit wrote, left out here rather than refused.
pub(crate) fn unread_note(unread: &[devflow_engine::Invalid]) -> Option<String> {
    let named: Vec<String> = unread.iter().map(ToString::to_string).collect();
    (!named.is_empty()).then(|| {
        format!(
            "This devpit does not read {}, and runs without it: a newer devpit wrote it, \
             or it is misspelled.",
            named.join("; ")
        )
    })
}

fn status(store: &Store, project: &Project, root: &Path, state: &State) -> Value {
    let switched_on = made(store, &project.id, Ask::Enable);
    let globally = globally_on(store);
    let active = matches!(offered(globally, state, switched_on), Offered::Active(_));
    let detected = detect::detected_in(&DEVPIT, root, machine(store, &project.id));
    let waiting: Vec<&str> = asks::waiting_in(&project.id)
        .into_iter()
        .map(Ask::word)
        .collect();
    let mut said = json!({
        "state": state_word(state, active),
        "file": file(),
        "yaml": Value::Null,
        "recommended_checks": preset::checks(&detected),
        "open": detect::open(&detected),
        "detected": detected,
        "next": next_step(
            state,
            globally,
            switched_on,
            waiting.contains(&Ask::Enable.word()),
            active,
        ),
        "waiting_for_click": waiting,
        "note": Value::Null,
    });
    // The layers it extends, each applied or waiting for the person.
    if let Some(layered) = layered(root) {
        said["teams"] = json!(layered.teams);
        said["stages_from"] = json!(layered.stages_from);
    }
    let note = match state {
        State::Missing if switched_on => Some(
            "The person switched Devflow on here, but this checkout has no devflow.yaml: \
             it is on another branch, or was never committed."
                .to_owned(),
        ),
        State::Missing => {
            Some("No devflow.yaml yet: devpit_devflow_setup previews one.".to_owned())
        }
        State::Invalid(text, invalid) => {
            said["yaml"] = json!(text);
            said["error"] = json!(invalid);
            Some(format!(
                "devflow.yaml is invalid, so the Devflow tools stay off: {invalid}."
            ))
        }
        State::Valid(text, config) => {
            said["yaml"] = json!(text);
            said["config"] = config.to_json();
            let unread = devflow_engine::config::unread(text);
            said["unread"] = json!(unread);
            let waiting = (config.enabled && !switched_on).then(|| {
                "devflow.yaml says enabled: true, but the person has not switched Devflow on \
                 here yet: devpit shows them a confirmation in the app."
                    .to_owned()
            });
            [waiting, unread_note(&unread)]
                .into_iter()
                .flatten()
                .reduce(|one, other| format!("{one} {other}"))
        }
    };
    said["note"] = match (globally, note) {
        (false, _) => json!("Devflow is switched off for every project in devpit's Settings."),
        (true, note) => json!(note),
    };
    said
}

/// Where the files a setup wrote belong: a worktree branched from the default
/// branch reads devflow.yaml only once it is committed there.
const COMMIT_IT: &str = "commit .devpit/devflow/ on the default branch";

/// The step `devpit_devflow_status` says comes next in a project.
pub(crate) fn next_step(
    state: &State,
    globally: bool,
    switched_on: bool,
    click_waits: bool,
    active: bool,
) -> &'static str {
    match state {
        _ if !globally => "the person switches Devflow on in devpit's Settings",
        _ if active => "start with devpit_devflow_skill devflow-briefing",
        // Seen where the file was committed only on a feature branch.
        State::Missing if switched_on => COMMIT_IT,
        State::Missing => "devpit_devflow_setup previews one",
        State::Invalid(..) => "devpit_devflow_set fixes the field the error names",
        State::Valid(..) if click_waits => "waiting for Switch on in the app",
        State::Valid(..) => "the person activates it on the Devflow screen",
    }
}

/// Who asks for a setup.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum By {
    /// An agent, which applies only the preview the person saw.
    Agent,
    /// The person's own Create on the Devflow screen.
    Screen,
}

/// Why an agent's apply is a preview again: the setup worked out now is not
/// the one it last showed the person, if it showed one.
pub(crate) fn unseen(shown: Option<&Proposal>, now: &Proposal) -> Option<String> {
    let Some(shown) = shown else {
        return Some("the person has not seen a preview of this setup".to_owned());
    };
    let changed = now.changed_since(shown)?;
    Some(match changed.is_empty() {
        true => "the files it writes changed since the preview the person saw".to_owned(),
        false => format!(
            "it changed since the preview the person saw ({})",
            changed.join(", ")
        ),
    })
}

/// What an applied setup's note says, from what it wrote and what waits.
pub(crate) fn applied_note(enabled: bool, switched_on: bool, waiting: &[&str]) -> String {
    let on = match (enabled, switched_on) {
        (true, true) => "Devflow is on here.",
        (true, false) => {
            "devflow.yaml says enabled: true, but Devflow stays off until the person \
             clicks the confirmation devpit now shows in the app."
        }
        (false, _) => {
            "Devflow is still off until the person clicks the confirmation devpit now \
             shows in the app."
        }
    };
    let enforce = if waiting.contains(&Ask::Enforce.word()) {
        " Decisions stays in shadow until the person clicks for enforce."
    } else {
        ""
    };
    format!(
        "Written. devpit never commits: these files are the person's to commit — {COMMIT_IT}, \
         or a worktree started from it finds no setup. {on}{enforce}"
    )
}

pub(crate) fn setup_of(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    root: &Path,
    state: &State,
    params: &Value,
    by: By,
) -> Result<Value, String> {
    let default = asks::preset(store);
    let word = params
        .get("preset")
        .and_then(Value::as_str)
        .unwrap_or(default.word());
    let preset = Preset::of(word)
        .ok_or_else(|| format!("`{word}` is not a preset: recommended or custom"))?;
    let set: Map<String, Value> = match params.get("set") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(set)) => set.clone(),
        Some(_) => return Err("`set` is an object of dotted path to value".to_owned()),
    };
    if let Some(path) = set
        .keys()
        .find(|path| devflow_engine::security::persons_only(path))
    {
        return Err(format!("`{path}` is not set here: {}", PERSONS_TARGETS));
    }
    let detected = detect::detected_in(&DEVPIT, root, machine(store, &project.id));
    let proposal = proposal::setup_in(&layout(), state, preset, &set, &detected)
        .map_err(|why| why.to_string())?;
    layered_as(root, &proposal)?;
    let asked = params.get("apply").and_then(Value::as_bool) == Some(true);
    // The person's yes was to a preview: a setup that moved since then (a
    // click, a key saved) is shown again rather than written.
    let moved = match (by, asked) {
        (By::Agent, true) => unseen(asks::last_preview(&project.id).as_ref(), &proposal),
        _ => None,
    };
    let apply = asked && moved.is_none();
    if by == By::Agent {
        asks::previewed(&project.id, (!apply).then_some(&proposal));
    }
    let said = |check: &devflow_engine::config::Check| {
        let script = proposal.scripts.iter().find(|one| one.name == check.name);
        json!({
            "name": check.name,
            "run": check.run,
            "command": script.map(|one| one.command.as_str()),
            "source": script.and_then(|one| one.source.as_deref()),
            "report": check.report,
        })
    };
    let execution = &proposal.config.stages.execution;
    let checks: Vec<Value> = execution.checks.iter().map(said).collect();
    let release: Vec<Value> = execution.release.iter().map(said).collect();
    let planned = setup::with_unused_in(&DEVPIT, root, &proposal.config, proposal.files.clone());
    let files = if apply {
        setup::write(root, &planned)?
    } else {
        setup::preview(root, &planned)
    };
    let waiting = if apply {
        settled(app, store, project, &proposal, true)
    } else {
        Vec::new()
    };
    Ok(json!({
        "applied": apply,
        "preset": word,
        "yaml": proposal.yaml,
        "checks": checks,
        "release": release,
        "files": files,
        "open": detect::open(&detected),
        "unavailable": detected
            .every()
            .filter_map(|(one, kind)| {
                let why = one.unavailable.as_deref()?;
                Some(json!({ "kind": kind, "command": one.command, "source": one.source, "why": why }))
            })
            .collect::<Vec<_>>(),
        "asks": proposal.asks,
        "waiting_for_click": waiting,
        "note": match (apply, moved) {
            (true, _) => applied_note(
                proposal.config.enabled,
                made(store, &project.id, Ask::Enable),
                &waiting,
            ),
            (false, Some(why)) => format!(
                "Nothing was written: {why}. This is the setup now: show it, and apply once \
                 the person says yes to it."
            ),
            (false, None) => "Nothing was written. apply: true writes these files, once the \
                person said yes to this preview."
                .to_owned(),
        },
    }))
}

pub(crate) fn set_of(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    root: &Path,
    state: &State,
    params: &Value,
) -> Result<Value, String> {
    let by_devflow = |path: &str, value: &Value| {
        let changes = json!({ "changes": [{ "path": path, "value": value }] });
        hosted(store, project, root, "config/write", changes)
    };
    set_with(app, store, project, root, state, params, &by_devflow)
}

/// Writes devflow.yaml in place, comments and fields this devpit does not
/// read kept, when devflow here serves `config/write`: `None` when it does not.
pub(crate) type WrittenBy<'a> =
    &'a dyn Fn(&str, &Value) -> Option<Result<Value, crate::devflow_host::Failed>>;

/// [`set_of`], the file written by `written_by` when it can, else laid out
/// again here.
pub(crate) fn set_with(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    root: &Path,
    state: &State,
    params: &Value,
    written_by: WrittenBy,
) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or("pass the field as a dotted `path`, such as review.block_on")?;
    let value = params
        .get("value")
        .ok_or("pass the new `value` for the field")?;
    if devflow_engine::security::persons_only(path) {
        return Err(format!("`{path}` is not set here: {}", PERSONS_TARGETS));
    }
    let proposal =
        proposal::set_in(&layout(), state, path, value).map_err(|why| why.to_string())?;
    layered_as(root, &proposal)?;
    let apply = params.get("apply").and_then(Value::as_bool) == Some(true);
    let before = match state {
        State::Valid(_, config) => preset::value_at(&config.to_json(), path).clone(),
        _ => Value::Null,
    };
    let written = apply && !proposal.changes.is_empty();
    let planned = setup::with_unused_in(&DEVPIT, root, &proposal.config, proposal.files.clone());
    // A refusal of devflow's leaves the file as it was: laid out here instead.
    let in_place = written && matches!(written_by(path, value), Some(Ok(_)));
    let files = match (written, in_place) {
        (true, true) => {
            let yaml = file();
            let rest: Vec<_> = planned.into_iter().filter(|one| one.path != yaml).collect();
            let mut done = vec![setup::FileChange {
                path: yaml,
                change: "replace",
            }];
            done.extend(setup::write(root, &rest)?);
            done
        }
        (true, false) => setup::write(root, &planned)?,
        (false, _) => setup::preview(root, &planned),
    };
    if written {
        if !proposal.config.enabled && made(store, &project.id, Ask::Enable) {
            // Switched off: switching on again is the person's click again.
            let _ = asks::remember(store, &project.id, Ask::Enable, false);
        }
        asks::rang(app, &project.id);
    }
    let waiting = if apply {
        settled(app, store, project, &proposal, false)
    } else {
        Vec::new()
    };
    let wrote = match (apply, written) {
        (true, true) => "Written. devpit never commits: the file is the person's to commit.",
        (true, false) => "Nothing was written to the file.",
        (false, _) => "Nothing was written. apply: true writes it, once the person said yes.",
    };
    let clicked = match (proposal.asks.is_empty(), apply) {
        (true, _) => "",
        (false, true) => {
            " Switching Devflow on and moving Decisions to enforce are the person's click: \
             devpit now shows them the confirmation in the app."
        }
        (false, false) => {
            " Switching Devflow on and moving Decisions to enforce are never written by a \
             tool: they wait for the person's click on a confirmation in the app."
        }
    };
    let note = format!("{wrote}{clicked}");
    Ok(json!({
        "applied": written,
        "path": path,
        "before": before,
        "after": preset::value_at(&proposal.config.to_json(), path),
        "changes": proposal.changes,
        "yaml": if in_place {
            std::fs::read_to_string(root.join(file())).unwrap_or(proposal.yaml)
        } else {
            proposal.yaml
        },
        "laid_out": proposal.laid_out && !in_place,
        "files": files,
        "asks": proposal.asks,
        "waiting_for_click": waiting,
        "note": note,
    }))
}

/// Raises what a written proposal leaves to the person: what it asked for,
/// and, after a setup, switching Devflow on when it is not on here yet.
/// Answers what waits.
fn settled(
    app: Option<&AppHandle>,
    store: &Store,
    project: &Project,
    proposal: &Proposal,
    after_setup: bool,
) -> Vec<&'static str> {
    let mut wanted = proposal.asks.clone();
    let on = proposal.config.enabled && made(store, &project.id, Ask::Enable);
    if after_setup && !on && !wanted.contains(&Ask::Enable) {
        wanted.push(Ask::Enable);
    }
    for ask in &wanted {
        asks::raise(app, &project.id, &project.name, *ask, false);
    }
    asks::waiting_in(&project.id)
        .into_iter()
        .map(Ask::word)
        .collect()
}

/// The reviewer CLIs on this machine, and the Decisions key, as the app
/// already knows them.
pub(crate) fn machine(store: &Store, project_id: &str) -> Machine {
    let profile = crate::devflow_profile::of(store, project_id, None);
    let knows = crate::shell_launch::shell_knows();
    let clis = Agent::ALL
        .iter()
        .map(|agent| {
            let program = devpit_pty::agents::KNOWN
                .iter()
                .find(|known| known.id == agent.word())
                .and_then(|known| known.launch.split_whitespace().next())
                .unwrap_or(agent.word());
            let installed = knows.contains(program);
            Cli {
                name: *agent,
                installed,
                signed_in: (installed && *agent == Agent::Claude)
                    .then(|| claude_signed_in(profile.as_ref()))
                    .flatten(),
            }
        })
        .collect();
    let opted_out: Vec<String> = store
        .preference(preference::DECISIONS_OPTED_OUT)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    // How the repository already works: its branches and what its commits name.
    let root = store
        .project(project_id)
        .ok()
        .flatten()
        .map(|row| std::path::PathBuf::from(row.root_path));
    let branches = root
        .as_deref()
        .and_then(|root| devpit_git::branches(root).ok())
        .map(|all| all.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    let subjects = root
        .as_deref()
        .and_then(|root| devpit_git::history(root, 200, 0).ok())
        .map(|all| all.into_iter().map(|one| one.subject).collect())
        .unwrap_or_default();
    Machine {
        clis,
        decisions_key: crate::decisions::key().is_some(),
        decisions_opted_out: opted_out.iter().any(|id| id == project_id),
        branches,
        subjects,
        installed_skills: installed_skills(),
        // What a check's shell searches, so a command found is one it finds.
        path: Some(devpit_pty::login_path::search_path().clone()),
    }
}

/// Where Claude Code keeps the skills already installed for it.
pub(crate) fn installed_folder() -> Option<std::path::PathBuf> {
    Some(devpit_agentcli::cli_config::config_dir()?.join("skills"))
}

/// The skills already installed for Claude Code, by name: an own stage can
/// use one where it is.
fn installed_skills() -> Vec<String> {
    let Some(folder) = installed_folder() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .take(200)
        .collect()
}

/// Whether Claude Code is signed in where `profile` keeps its sign-in, read
/// as the footer reads it. devpit has no such check for the other CLIs, so
/// they say nothing.
pub(crate) fn claude_signed_in(profile: Option<&devpit_rpc::Declared>) -> Option<bool> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    let inherited = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let directory = crate::devflow_profile::config_dir(profile, &home, inherited.as_deref());
    match devpit_agentcli::cli_config::sign_in_at(&directory) {
        Credentials::Saved => Some(true),
        Credentials::Missing | Credentials::Expired => Some(false),
        Credentials::Unknown => None,
    }
}

#[cfg(test)]
#[path = "devflow_tests.rs"]
mod tests;
