//! A session started by an orchestrator in a project's own folder, with no
//! card: a new terminal tab of that project, the account's Claude Code typed
//! into it with the name and the brief. It is visible like any tab — opened
//! over the chat, gone to, stopped — rather than running where nobody looks.

use devpit_rpc::{ErrorCode, Project, RpcError};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

/// Said when a tab was opened from here, so the window lists it for its project.
pub(crate) const TAB_OPENED: &str = "session:tab-opened";

/// The longest brief typed: a brief, not a document.
const LONGEST: usize = 8000;

/// What the one starting a session asked of it beyond its brief.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Wanted<'a> {
    /// The name to message it by.
    pub name: Option<&'a str>,
    /// The model it runs in, for this session alone (`--model`).
    pub model: Option<&'a str>,
    /// The profile it runs as, when not the orchestrator's own.
    pub profile: Option<&'a str>,
}

/// The profile a session an orchestrator speaking as `own` starts runs as:
/// `asked`, or its own. A Claude Code orchestrator's Claude Code sessions are
/// its own account's, which is where it lists them.
pub(crate) fn runs_as(own: &str, asked: Option<&str>) -> Result<String, String> {
    let Some(asked) = asked
        .map(str::trim)
        .filter(|one| !one.is_empty() && *one != own)
    else {
        return Ok(own.to_owned());
    };
    let store = crate::projects::store().map_err(|err| err.message)?;
    let profile = crate::agent_profiles::all(&store)
        .map_err(|err| err.message)?
        .into_iter()
        .find(|one| one.id == asked)
        .ok_or_else(|| format!("no profile called {asked}: Settings → Providers lists them"))?;
    may_run_as(crate::live_sessions::on_claude(own), &profile)?;
    Ok(profile.id)
}

/// Whether an orchestrator, on Claude Code or not, may start a session as
/// `profile`.
pub(crate) fn may_run_as(own_on_claude: bool, profile: &devpit_rpc::Profile) -> Result<(), String> {
    if profile.path.is_none() {
        return Err(format!(
            "devpit has not read what {} runs yet — open it in Settings → Providers",
            profile.command
        ));
    }
    if own_on_claude && cli_of(profile) == crate::claude_accounts::DRIVER {
        return Err(
            "a Claude Code orchestrator's Claude Code sessions run as its own account: leave `profile` out"
                .to_owned(),
        );
    }
    Ok(())
}

/// Whether a session `runs_as` starts for the orchestrator speaking as `own`
/// is kept by devpit, and so reaches it through devpit's bus rather than
/// Claude Code's SendMessage.
pub(crate) fn by_bus(own: &str, runs_as: &str) -> bool {
    crate::pane_sessions::keeps(
        crate::live_sessions::on_claude(own),
        base_of(runs_as) == crate::claude_accounts::DRIVER,
    )
}

/// Whether the session `runs_as` starts for the orchestrator speaking as
/// `own` is kept by devpit, and if so, remembered at its pane.
pub(crate) fn kept(own: &str, runs_as: &str, started: crate::pane_sessions::Kept) -> bool {
    let keeps = by_bus(own, runs_as);
    if keeps {
        crate::pane_sessions::began(started);
    }
    keeps
}

pub(crate) fn open(
    app: &AppHandle,
    project: &Project,
    profile_id: &str,
    prompt: &str,
    wanted: Wanted<'_>,
) -> Result<Value, RpcError> {
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > LONGEST {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            format!("a session is started with between 1 and {LONGEST} characters of brief"),
        ));
    }
    let name = crate::handing::unused(
        wanted
            .name
            .map(str::trim)
            .filter(|one| !one.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let short: String = ulid::Ulid::generate()
                    .to_string()
                    .chars()
                    .rev()
                    .take(4)
                    .collect();
                format!(
                    "{}-{}",
                    crate::chat_remote::remote_name(&project.name).trim_start_matches("devpit-"),
                    short.to_lowercase()
                )
            }),
        &crate::handing::live_names(profile_id),
    );
    let runs_as = wanted.profile.unwrap_or(profile_id);
    let tab_id = fresh_tab();
    let pane_id = typed_in(
        app,
        &project.id,
        &tab_id,
        std::path::Path::new(&project.root_path),
        &launched(runs_as, &project.id, &name, prompt, wanted.model)?,
        Some(runs_as),
    )?;
    let kept = kept(
        profile_id,
        runs_as,
        crate::pane_sessions::Kept {
            profile: profile_id.to_owned(),
            runs_as: runs_as.to_owned(),
            name: name.clone(),
            project_id: project.id.clone(),
            project_name: Some(project.name.clone()),
            card_id: None,
            cwd: project.root_path.clone(),
            pane_id: pane_id.clone(),
        },
    );
    let _ = app.emit(
        TAB_OPENED,
        json!({ "projectId": project.id, "tabId": tab_id, "paneId": pane_id }),
    );
    Ok(json!({
        "name": name,
        "project": project.name,
        "cwd": project.root_path,
        "kept": kept,
        "next": "It runs in a new terminal tab of that project. Message it by this name with SendMessage once it is up, and pass notify_when_idle to hear when it is done.",
    }))
}

/// A tab id nothing else has.
pub(crate) fn fresh_tab() -> String {
    format!("tab_{}", ulid::Ulid::generate().to_string().to_lowercase())
}

/// Whether text can be typed at a shell inside quotes and stay text.
///
/// A control character is a key: Ctrl-C or Ctrl-U in a brief would end the
/// quoted argument at the terminal and run what follows it as a command.
/// Lines and tabs stay inside the quotes, so they are kept.
pub(crate) fn typeable(text: &str) -> Result<(), RpcError> {
    match text
        .chars()
        .find(|ch| ch.is_control() && *ch != '\n' && *ch != '\t')
    {
        Some(ch) => Err(RpcError::new(
            ErrorCode::Invalid,
            format!(
                "a brief cannot hold control characters (found U+{:04X})",
                u32::from(ch)
            ),
        )),
        None => Ok(()),
    }
}

/// The profile's CLI, named when it is Claude Code and briefed, as one line
/// to type.
pub(crate) fn launched(
    profile_id: &str,
    project_id: &str,
    name: &str,
    prompt: &str,
    model: Option<&str>,
) -> Result<String, RpcError> {
    typeable(name)?;
    typeable(prompt)?;
    let model = match model.map(str::trim).filter(|one| !one.is_empty()) {
        Some(one)
            if one
                .chars()
                .any(|ch| ch.is_whitespace() || ch.is_control() || ch == ';') =>
        {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "a model is one word, such as opus or claude-opus-5-5",
            ))
        }
        Some(one) => format!(" --model {}", devpit_agentcli::declaring::quoted(one)),
        None => String::new(),
    };
    let base = base_of(profile_id);
    let brief = devpit_agentcli::declaring::quoted(prompt);
    // Only Claude Code takes a name; devpit keeps the name of any other.
    let rest = if base == crate::claude_accounts::DRIVER {
        format!(
            " --name {}{model} {brief}",
            devpit_agentcli::declaring::quoted(name)
        )
    } else {
        let flag = devpit_pty::agents::known(&base)
            .and_then(|one| one.prompt_flag)
            .ok_or_else(|| {
                RpcError::new(
                    ErrorCode::Invalid,
                    format!("devpit does not know how {base} starts with a brief: start the session on another CLI"),
                )
            })?;
        format!("{model} {}", flag.replace("{}", &brief))
    };
    Ok(format!(
        "{}{rest}",
        crate::shell_launch::to_start_in(profile_id, Some(project_id))?
    ))
}

/// The CLI a profile runs, by the id it is known by.
pub(crate) fn base_of(profile_id: &str) -> String {
    crate::projects::store()
        .ok()
        .and_then(|store| crate::agent_profiles::all(&store).ok())
        .and_then(|all| all.into_iter().find(|one| one.id == profile_id))
        .map_or_else(|| profile_id.to_owned(), |one| cli_of(&one).to_owned())
}

/// The CLI `profile` runs: its base, or itself for a command devpit found.
pub(crate) fn cli_of(profile: &devpit_rpc::Profile) -> &str {
    if profile.base.is_empty() {
        &profile.id
    } else {
        &profile.base
    }
}

/// Opens tab `tab_id` of the project with its pane in `cwd`, and types `line`
/// at its prompt. Answers the pane. `launch` is the profile `line` starts, kept
/// on the pane; none for a line that starts no agent of its own.
pub(crate) fn typed_in(
    app: &AppHandle,
    project_id: &str,
    tab_id: &str,
    cwd: &std::path::Path,
    line: &str,
    launch: Option<&str>,
) -> Result<String, RpcError> {
    let state = app.state::<crate::sessions::SessionState>();
    let layout = crate::sessions::ensure_at(&state, project_id, tab_id, cwd)?;
    let target = crate::pane_io::target_in(&layout, &layout.focused_id);
    let _ = crate::pane_io::settled(&target);
    // Before the CLI looks: a session stopped on the trust question is not up.
    crate::folder_trust::trust(&[cwd]);
    match launch {
        Some(launch) => type_agent(
            crate::sessions::store().ok().as_ref(),
            None,
            project_id,
            &layout.focused_id,
            &target,
            line,
            launch,
        )?,
        None => crate::pane_io::send_line(&target, line)?,
    }
    crate::trust_answer::watch(target);
    Ok(layout.focused_id)
}

/// Types an agent's launch `line` at a pane and keeps the launch on the pane:
/// what its hooks tie the session to, and what a lost pane starts again.
/// `home` is a ptyd daemon's, when not this app's own.
pub(crate) fn type_agent(
    store: Option<&devpit_core::Store>,
    home: Option<&std::path::Path>,
    project_id: &str,
    leaf_id: &str,
    target: &str,
    line: &str,
    launch: &str,
) -> Result<(), RpcError> {
    crate::pane_io::send_line_at(home, target, line)?;
    if let Some(store) = store {
        let _ = store.remember_pane_launch(project_id, leaf_id, launch);
    }
    Ok(())
}

/// `session.watch` — a background session, attached in a new terminal tab of
/// its project: it ran under the CLI's own supervisor, where nobody could see
/// it, and there it is watched and typed into like any other.
#[tauri::command]
#[specta::specta]
pub async fn session_watch(
    app: AppHandle,
    profile_id: String,
    project_id: String,
    job: String,
) -> Result<String, RpcError> {
    crate::off_main::blocking(move || {
        if !crate::adopting::plain(&job) {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "that is not a session's job id",
            ));
        }
        let store = crate::projects::store()?;
        let runner = crate::agent_profiles::runner_for(&store, &profile_id)
            .map_err(|why| RpcError::new(ErrorCode::NotFound, why))?;
        let root = crate::roots::root_of(&project_id, None)?;
        let line = format!(
            "{} attach {}",
            devpit_agentcli::running::line(&runner),
            devpit_agentcli::declaring::quoted(&job)
        );
        let tab_id = fresh_tab();
        // An attach starts no agent: the session runs under the CLI's supervisor.
        let pane_id = typed_in(&app, &project_id, &tab_id, &root, &line, None)?;
        let _ = app.emit(
            TAB_OPENED,
            json!({ "projectId": project_id, "tabId": tab_id, "paneId": pane_id }),
        );
        Ok(pane_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{launched, typeable};

    /// The orchestrator's choice of model goes on this session's line alone,
    /// before the brief; without one the profile's own stands.
    #[test]
    fn a_session_asked_for_in_a_model_starts_in_it() {
        let line =
            launched("claude", "prj_none", "fix-it", "the brief", Some("opus")).expect("line");
        assert!(
            line.ends_with(" --name 'fix-it' --model 'opus' 'the brief'"),
            "{line}"
        );
        let line = launched("claude", "prj_none", "fix-it", "the brief", None).expect("line");
        assert!(!line.contains("--model"), "{line}");
        assert!(launched(
            "claude",
            "prj_none",
            "fix-it",
            "the brief",
            Some("opus; rm -rf ~")
        )
        .is_err());
    }

    /// Each CLI is briefed as it takes a first message, and only Claude Code
    /// is given a name; a CLI whose start nobody checked is refused.
    #[test]
    fn a_session_on_another_cli_starts_with_its_brief_as_that_cli_takes_it() {
        let line = launched("codex", "prj_none", "fix-it", "the brief", None).expect("codex");
        assert!(
            line.starts_with("codex") && line.ends_with(" 'the brief'"),
            "{line}"
        );
        assert!(!line.contains("--name"), "{line}");
        let line =
            launched("opencode", "prj_none", "fix-it", "the brief", Some("x/y")).expect("opencode");
        assert!(
            line.ends_with(" --model 'x/y' --prompt 'the brief'"),
            "{line}"
        );
        let line = launched("gemini", "prj_none", "fix-it", "the brief", None).expect("gemini");
        assert!(line.ends_with(" -i 'the brief'"), "{line}");
        assert!(launched("aider", "prj_none", "fix-it", "the brief", None).is_err());
    }

    /// Ctrl-C and Ctrl-U are keys at a terminal: inside a typed brief they end
    /// the quoted argument and run the rest as a command.
    #[test]
    fn a_brief_with_a_control_character_is_refused() {
        assert!(typeable("fix it\x03; rm -rf ~").is_err());
        assert!(typeable("line one\x15rm -rf ~").is_err());
        assert!(typeable("two lines\nand a\ttab").is_ok());
    }

    /// An orchestrator's session, typed into a ptyd pane where no `$TMUX`
    /// says where it runs: its first hook ties it to that pane.
    #[cfg(unix)]
    #[test]
    fn a_session_typed_into_a_ptyd_pane_is_found_there_once_it_starts() {
        use devpit_agentcli::{Event, Happening};

        let daemon = crate::ptyd_link::tests::serving();
        let home = daemon.home.path();
        let store = devpit_core::Store::open(&home.join("state.db")).expect("store");
        let project = store.add_project(home, None).expect("project");
        let leaf = "leaf_typed_session";
        let pane = crate::ptyd_link::pane_number(leaf);
        let asked = devpit_ptyd::pane::Asked {
            rows: 24,
            cols: 80,
            cwd: home.to_string_lossy().into_owned(),
            argv: vec!["/bin/sh".into(), "-c".into(), "read line; sleep 30".into()],
            env: Vec::new(),
        };
        crate::ptyd_link::spawn(home, pane, &asked).expect("spawn");
        let target = crate::ptyd_link::target(leaf);

        let typed = super::type_agent(
            Some(&store),
            Some(home),
            &project,
            leaf,
            &target,
            "claude --name typed 'the brief'",
            "claude",
        );
        let started = Happening {
            session_id: "8f0c2b7e-0d4b-4c55-9d7e-3b1a2c4d5e6f".into(),
            event: Event::SessionStarted,
            cwd: home.to_string_lossy().into_owned(),
            transcript_path: None,
        };
        crate::restoring::remember(&store, leaf, &started);
        crate::ptyd_link::kill(home, pane);

        typed.expect("typed");
        assert_eq!(
            store
                .place_of_session("8f0c2b7e-0d4b-4c55-9d7e-3b1a2c4d5e6f")
                .expect("read"),
            Some((project, leaf.to_owned()))
        );
    }
}
