use super::{brief, free_folder, may_orchestrate, seed, seed_all, slug, speaks_as};

#[test]
fn a_new_orchestrator_starts_with_its_brief_and_folders() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("orchestrator").join("claude2");
    seed(&folder).expect("seeded");
    let mine = std::fs::read_to_string(folder.join("CLAUDE.md")).expect("claude.md");
    assert!(
        mine.contains("@.devpit/orchestrator.md"),
        "CLAUDE.md does not import devpit's brief"
    );
    let brief = std::fs::read_to_string(folder.join(".devpit/orchestrator.md")).expect("brief");
    assert!(brief.contains("Never move a card into a lane that runs a step"));
    // The person answers a waiting session; the orchestrator never relays it.
    assert!(brief.contains("do\n  not relay \"go on\" as a message"));
    // It drafts the person's words instead, and only their click sends them.
    assert!(brief.contains("Draft their words with `devpit_draft_reply`"));
    // Its folder is notes, organised, and never a repository.
    assert!(brief.contains("`context/projects/<project>.md`"));
    assert!(brief.contains("never initialise one or commit"));
    // Memory and the account's MCP servers, always.
    assert!(brief.contains("Use every memory tool this account has"));
    assert!(brief.contains("Use the MCP servers this account has"));
    for kept in [
        "docs",
        "artifacts",
        "context",
        "context/projects",
        "decisions",
    ] {
        assert!(folder.join(kept).is_dir(), "{kept} is missing");
    }
}

#[test]
fn opening_again_leaves_what_the_person_changed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("claude2");
    seed(&folder).expect("seeded");
    std::fs::write(folder.join("CLAUDE.md"), "mine now").expect("edit");
    seed(&folder).expect("seeded again");
    assert_eq!(
        std::fs::read_to_string(folder.join("CLAUDE.md")).expect("brief"),
        "mine now"
    );
}

#[test]
fn devpits_half_of_the_brief_follows_the_build() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("claude2");
    seed(&folder).expect("seeded");
    std::fs::write(folder.join(".devpit/orchestrator.md"), "an older build's").expect("age it");
    seed(&folder).expect("opened again");
    let brief = std::fs::read_to_string(folder.join(".devpit/orchestrator.md")).expect("brief");
    assert!(
        brief.contains("devpit_sessions"),
        "the brief was left as an older build wrote it"
    );
}

#[test]
fn a_second_orchestrator_of_one_name_gets_a_folder_of_its_own() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = free_folder(dir.path(), "Client work").expect("a folder");
    assert!(first.ends_with("orchestrator/client-work"));
    std::fs::create_dir_all(&first).expect("made");
    let second = free_folder(dir.path(), "Client work").expect("a folder");
    assert!(second.ends_with("orchestrator/client-work-1"));
    assert_eq!(slug("   "), "orchestrator");
}

#[test]
fn an_orchestrator_says_its_account_where_the_folder_is_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("orchestrator").join("work");
    seed(&folder).expect("seeded");
    let store = devpit_core::Store::open(&dir.path().join("state.db")).expect("store");
    speaks_as(&store, &folder, "01M39W5J").expect("said");
    assert_eq!(
        devpit_core::home::orchestrator_profile(&store, &folder).as_deref(),
        Some("01M39W5J")
    );
    speaks_as(&store, &folder, "claude").expect("changed");
    assert_eq!(
        devpit_core::home::orchestrator_profile(&store, &folder).as_deref(),
        Some("claude")
    );
    // Notes, not a repository: nothing to push them to.
    assert!(!folder.join(".git").exists());
}

/// The brief names the build that wrote it, so which one an orchestrator is
/// reading is one look at the file.
#[test]
fn the_brief_says_which_build_wrote_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("claude2");
    seed(&folder).expect("seeded");
    let written = std::fs::read_to_string(folder.join(".devpit/orchestrator.md")).expect("brief");
    assert_eq!(written, brief());
    assert!(written.starts_with(&format!(
        "<!-- written by devpit {} -->\n# Orchestrator",
        env!("CARGO_PKG_VERSION")
    )));
}

/// Starting the app brings every orchestrator's brief up to this build, the
/// ones nobody opens included, and leaves the person's half and every other
/// project alone.
#[test]
fn starting_brings_every_orchestrator_brief_up_to_this_build() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let store = devpit_core::Store::open(&root.join("state.db")).expect("store");

    let mut folders = Vec::new();
    for name in ["work", "home"] {
        let folder = root.join("orchestrator").join(name);
        seed(&folder).expect("seeded");
        speaks_as(&store, &folder, "claude").expect("account");
        store.add_project(&folder, None).expect("project");
        std::fs::write(folder.join(".devpit/orchestrator.md"), "an older build's").expect("age it");
        std::fs::write(folder.join("CLAUDE.md"), "mine").expect("edit");
        folders.push(folder);
    }
    let project = root.join("notes").join("api");
    std::fs::create_dir_all(project.join(".devpit")).expect("project");
    std::fs::write(project.join(".devpit/orchestrator.md"), "not ours").expect("write");
    store.add_project(&project, None).expect("project");

    assert!(seed_all(&store, root).is_empty());
    for folder in &folders {
        let written =
            std::fs::read_to_string(folder.join(".devpit/orchestrator.md")).expect("brief");
        assert_eq!(written, brief(), "{}", folder.display());
        assert_eq!(
            std::fs::read_to_string(folder.join("CLAUDE.md")).expect("mine"),
            "mine"
        );
    }
    assert_eq!(
        std::fs::read_to_string(project.join(".devpit/orchestrator.md")).expect("theirs"),
        "not ours",
        "a project that is not an orchestrator was written into"
    );
}

/// The bus is offered beside Claude Code's own messaging, never in its place:
/// this account's sessions are still reached with SendMessage.
#[test]
fn the_brief_keeps_send_message_for_this_accounts_sessions() {
    let brief = brief();
    for kept in [
        "`ListAgents` finds them, `SendMessage`",
        "`notify_when_idle` tells you once",
        "Keep `SendMessage` for this account's sessions",
    ] {
        assert!(brief.contains(kept), "{kept}");
    }
    assert!(brief.contains("`devpit_session_message`"));
    assert!(brief.contains("`devpit_session_notify_idle`"));
}

fn profile(id: &str, base: &str, driver: &str, path: Option<&str>) -> devpit_rpc::Profile {
    devpit_rpc::Profile {
        id: id.to_owned(),
        label: id.to_owned(),
        command: id.to_owned(),
        driver: driver.to_owned(),
        path: path.map(str::to_owned),
        reach: devpit_rpc::Reach::Runnable,
        base: base.to_owned(),
        args: Vec::new(),
        env: Vec::new(),
        mine: true,
        models: Vec::new(),
        own_models: Vec::new(),
        offered: Vec::new(),
        efforts: Vec::new(),
        effort_default: None,
        metered: true,
        enabled: true,
    }
}

/// Any CLI devpit holds a chat with can orchestrate; a terminal-only one is
/// refused saying so, and one devpit has not read yet is still sent to Providers.
#[test]
fn an_orchestrator_runs_on_any_cli_that_holds_a_chat() {
    for (base, driver) in [
        ("claude", "claude"),
        ("codex", "codex"),
        ("opencode", "acp"),
    ] {
        let one = profile(base, base, driver, Some("/usr/bin/x"));
        assert!(may_orchestrate(&one).is_ok(), "{base} is refused");
    }
    let gemini = profile("gemini", "gemini", "", Some("/usr/bin/gemini"));
    let refused = may_orchestrate(&gemini).expect_err("gemini holds no chat");
    assert!(refused.contains("terminal only"), "{refused}");
    let unread = profile("codex2", "codex", "codex", None);
    let refused = may_orchestrate(&unread).expect_err("not read yet");
    assert!(refused.contains("Settings → Providers"), "{refused}");
}

/// Forwarding work to another orchestrator: when it is theirs to see
/// through, that nothing runs there until the person sends it, and what an
/// error asks for — the same id again.
#[test]
fn the_brief_says_how_to_forward_to_another_orchestrator() {
    let brief = brief();
    for kept in [
        "## Forwarding to another orchestrator",
        "another orchestrator's to see through",
        "nothing runs there until the person sends it",
        "send the same request again with the same id",
    ] {
        assert!(brief.contains(kept), "{kept}");
    }
}

/// An orchestrator on a CLI without SendMessage is pointed at the bus for
/// every session, and starts sessions on any CLI.
#[test]
fn the_brief_points_an_orchestrator_on_any_cli_at_the_bus() {
    let brief = brief();
    assert!(brief.contains("or every session when you run on a CLI other than Claude Code"));
    assert!(brief.contains("`devpit_session_notify_idle` for any other"));
    assert!(brief.contains("on another with\n    `profile`"));
}
