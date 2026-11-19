//! Sessions an orchestrator starts on a host: a tab with one terminal there,
//! in the project's folder, the CLI typed at its shell by its plain name.
//!
//! devpit keeps every one: no registry on this machine lists it, and nothing
//! of devpit's reaches it there — its state is read from its screen, and a
//! message is typed into its terminal. A start on production waits for the
//! person's yes and briefs the session with production's rules.

use devpit_core::{HostRow, Store};
use devpit_rpc::{ErrorCode, RpcError};
use serde_json::{json, Value};

use crate::session_locale::SessionLocale;

/// What a session on production is told first.
const PRODUCTION_RULES: &str = "## This session runs on production\n\nYou are on {host}, a production host, in {path}. Make the smallest change that does the work, and a reversible one. Read before you write. Say what you will do before you do it. Nothing destructive — deleting data, a migration, restarting or stopping a service, a force push, changing access — without the person's explicit yes in this session.";

/// What any session on a host is told of where it runs.
const ON_HOST: &str = "## Where you run\n\nYou run on {host} ({environment}), in {path}, over ssh from the person's devpit. devpit's tools do not reach you there: a message from the orchestrator is typed into this terminal, and you answer here, where it reads your terminal.";

/// Whether `host` is production: an unreadable label is.
fn on_production(host: &HostRow) -> bool {
    crate::hosts::environment_of(&host.environment) == devpit_rpc::HostEnvironment::Production
}

/// Why a start on `host` waits, or nothing when it does not: production
/// waits for the person's yes, which the orchestrator passes on.
pub(crate) fn production_refusal(host: &HostRow, confirmed: bool) -> Option<String> {
    (on_production(host) && !confirmed).then(|| {
        format!(
            "{} is a production host: ask the person first, and start it with confirm_production: true once they say yes",
            host.name
        )
    })
}

/// The brief a session on `host` starts with: production's rules first, then
/// where it runs, then the orchestrator's own.
pub(crate) fn briefed_for(host: &HostRow, path: &str, prompt: &str) -> String {
    let filled = |text: &str| {
        text.replace("{host}", &host.name)
            .replace("{path}", path)
            .replace("{environment}", label_of(host))
    };
    let mut brief = String::new();
    if on_production(host) {
        brief.push_str(&filled(PRODUCTION_RULES));
        brief.push_str("\n\n");
    }
    brief.push_str(&filled(ON_HOST));
    brief.push_str("\n\n");
    brief.push_str(prompt.trim());
    brief
}

/// The line typed at the host's shell to start `cli` as `name` with `prompt`:
/// the CLI by its plain name, which the host's own login runs, and every word
/// quoted for that shell.
pub(crate) fn launch_line(
    cli: &str,
    name: &str,
    prompt: &str,
    model: Option<&str>,
) -> Result<String, RpcError> {
    crate::opening::typeable(name)?;
    crate::opening::typeable(prompt)?;
    if prompt.trim().is_empty() || prompt.chars().count() > crate::opening::LONGEST {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            format!(
                "a session is started with between 1 and {} characters of brief",
                crate::opening::LONGEST
            ),
        ));
    }
    let quoted = devpit_tmux::remote::quoted;
    let model = crate::opening::model_of(model)?;
    let known = devpit_pty::agents::known(cli);
    let program = known.map_or(cli, |one| one.launch);
    // A leading space keeps the line, brief and all, out of the shell's history.
    if cli == crate::claude_accounts::DRIVER {
        let mut words = vec![program.to_owned(), "--name".to_owned(), quoted(name)];
        if let Some(model) = model {
            words.extend(["--model".to_owned(), quoted(model)]);
        }
        words.push(quoted(prompt));
        return Ok(format!(" {}", words.join(" ")));
    }
    let flag = known.and_then(|one| one.prompt_flag).ok_or_else(|| {
        RpcError::new(
            ErrorCode::Invalid,
            format!("devpit does not know how {cli} starts with a brief: start the session on another CLI"),
        )
    })?;
    let model = model.map_or_else(String::new, |one| format!(" --model {}", quoted(one)));
    Ok(format!(
        " {program}{model} {}",
        flag.replace("{}", &quoted(prompt))
    ))
}

/// What a start on a host is asked.
pub(crate) struct Asked<'a> {
    /// The host, by name or id, among the project's links.
    pub host: &'a str,
    pub prompt: &'a str,
    pub name: Option<&'a str>,
    pub model: Option<&'a str>,
    /// The card whose work it takes, in the link's folder.
    pub card: Option<&'a str>,
    /// The person said yes to a start on production.
    pub confirmed: bool,
}

/// What a start on a host comes to before anything opens: the host, the
/// project's folder there, the session's name and the line typed there.
#[derive(Debug)]
pub(crate) struct Planned {
    pub host: HostRow,
    pub path: String,
    pub name: String,
    pub line: String,
}

/// Plans a start on a host the project is linked to, for the orchestrator in
/// `here_root` speaking as `profile`, running `runs_as`'s CLI: refused on
/// production without the person's yes, and briefed for where it runs, in the
/// language of the session being started.
pub(crate) fn planned(
    locale: SessionLocale,
    store: &Store,
    here_root: &std::path::Path,
    project: &devpit_rpc::Project,
    profile: &str,
    runs_as: &str,
    asked: &Asked,
) -> Result<Planned, String> {
    if asked.prompt.trim().is_empty() {
        return Err("a session on a host is started with a brief: say what it is to do".to_owned());
    }
    let (host, path) = linked_host(store, &project.id, asked.host)?;
    if let Some(why) = production_refusal(&host, asked.confirmed) {
        return Err(why);
    }
    let name = crate::opening::session_name(project, asked.name, profile);
    let brief = crate::orchestrator_notes::briefed_on_host(
        locale,
        here_root,
        &briefed_for(&host, &path, asked.prompt),
        Some(crate::orchestrator_notes::ProjectName {
            id: &project.id,
            name: &project.name,
        }),
    );
    let line = launch_line(
        &crate::opening::base_of(runs_as),
        &name,
        &brief,
        asked.model,
    )
    .map_err(|err| err.message)?;
    Ok(Planned {
        host,
        path,
        name,
        line,
    })
}

/// Starts a session on a host the project is linked to: [`planned`], then a
/// tab there, the line typed at its shell, and the session kept.
pub(crate) fn started(
    locale: SessionLocale,
    app: &tauri::AppHandle,
    here: &devpit_rpc::Project,
    project: &devpit_rpc::Project,
    profile: &str,
    runs_as: &str,
    asked: &Asked,
) -> Result<Value, String> {
    let said = |err: RpcError| err.message;
    let store = Store::open_default().map_err(|err| err.to_string())?;
    let root = std::path::Path::new(&here.root_path);
    let Planned {
        host,
        path,
        name,
        line,
    } = planned(locale, &store, root, project, profile, runs_as, asked)?;
    let opened = crate::host_panes::opened_tab(app, &project.id, &host.id).map_err(said)?;
    let target = crate::pane_io::target_of(&project.id, &opened.pane_id).map_err(said)?;
    let _ = crate::pane_io::settled(&target);
    // Typed as it is, with no key to clear a line first: a shell there may
    // read one as a character of the command.
    crate::pane_io::send_line(&target, &line).map_err(said)?;
    crate::pane_sessions::began(crate::pane_sessions::Kept {
        profile: profile.to_owned(),
        name: name.clone(),
        project_id: project.id.clone(),
        project_name: Some(project.name.clone()),
        card_id: asked.card.map(str::to_owned),
        cwd: path.clone(),
        pane_id: opened.pane_id,
    });
    // Written once it started: a start that failed on the way is not one.
    if asked.confirmed {
        noted_on_production(root, &host, &name);
    }
    let mut answer = json!({
        "name": name,
        "project": project.name,
        "host": host.name,
        "environment": label_of(&host),
        "cwd": path,
        "kept": true,
    });
    if asked.card.is_some() {
        answer["shares"] = json!(format!(
            "It runs in the project's own folder on {}, not a checkout of the card's: whatever else runs there shares that folder.",
            host.name
        ));
    }
    Ok(answer)
}

/// A host's environment as devpit reads it: an unreadable label is production.
fn label_of(host: &HostRow) -> &'static str {
    match crate::hosts::environment_of(&host.environment) {
        devpit_rpc::HostEnvironment::Dev => "dev",
        devpit_rpc::HostEnvironment::Staging => "staging",
        devpit_rpc::HostEnvironment::Production => "production",
    }
}

/// Writes a start on production in the orchestrator's sessions log, so the
/// person can see later what was started there on their yes.
pub(crate) fn noted_on_production(folder: &std::path::Path, host: &HostRow, name: &str) {
    if !on_production(host) {
        return;
    }
    let _ = crate::orchestrator_notes::note(
        folder,
        "sessions",
        &format!(
            "`{name}` started on production host {} with confirm_production, on the person's yes",
            host.name
        ),
        crate::orchestrator_notes::Section::Log,
    );
}

/// The host named `named` — its name or id — among the project's links, with
/// the project's folder there.
pub(crate) fn linked_host(
    store: &Store,
    project_id: &str,
    named: &str,
) -> Result<(HostRow, String), String> {
    let named = named.trim();
    let hosts = store.hosts().map_err(|err| err.to_string())?;
    let host = hosts
        .into_iter()
        .find(|one| one.id == named || one.name.eq_ignore_ascii_case(named))
        .ok_or_else(|| format!("no host is called {named}: Settings → Hosts lists them"))?;
    let path = store
        .host_links(project_id)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|one| one.host_id == host.id)
        .map(|one| one.path)
        .ok_or_else(|| {
            format!(
                "this project is not linked to {}: the person links it in the project's Hosts",
                host.name
            )
        })?;
    Ok((host, path))
}

/// `devpit_sessions`' answer with, for each session in a terminal on a host,
/// the host's name and environment. Every other session is as it was.
pub(crate) fn marked_in(store: &Store, mut sessions: Value) -> Value {
    let Some(rows) = sessions.as_array_mut() else {
        return sessions;
    };
    for row in rows {
        let pane = &row["pane"];
        let (Some(project), Some(leaf)) = (pane["projectId"].as_str(), pane["paneId"].as_str())
        else {
            continue;
        };
        let Some(host) = host_of_leaf(store, project, leaf) else {
            continue;
        };
        row["host"] = json!(host.name);
        row["environment"] = json!(label_of(&host));
    }
    sessions
}

/// The host a project's leaf runs on, read from the tab that holds it.
fn host_of_leaf(store: &Store, project_id: &str, leaf_id: &str) -> Option<HostRow> {
    let layout = crate::sessions::holding_in(store, project_id, leaf_id).ok()?;
    let target = crate::pane_io::target_in(&layout, leaf_id);
    let (host, _) = crate::host_panes::parsed(&target)?;
    crate::hosts::host_by_id(store, host).ok()
}

/// Whether a session's pane is a terminal on a host: what is said of its
/// folder is the host's, not this machine's.
pub(crate) fn pane_on_host(pane: Option<&devpit_rpc::LivePane>) -> bool {
    let Some(pane) = pane else { return false };
    Store::open_default()
        .ok()
        .and_then(|store| host_of_leaf(&store, &pane.project_id, &pane.pane_id))
        .is_some()
}

/// A project's links as `devpit_projects` lists them.
pub(crate) fn hosts_of(store: &Store, project_id: &str) -> Vec<Value> {
    let hosts = store.hosts().unwrap_or_default();
    store
        .host_links(project_id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|link| {
            let host = hosts.iter().find(|one| one.id == link.host_id)?;
            Some(json!({ "host": host.name, "environment": label_of(host), "path": link.path }))
        })
        .collect()
}

#[cfg(test)]
#[path = "host_sessions_tests.rs"]
mod tests;
