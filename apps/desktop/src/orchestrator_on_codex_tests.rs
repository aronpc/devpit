//! An orchestrator on Codex runs a round through devpit's own seams, on
//! screens as Claude Code 2.1 and Codex 0.160 draw them: it starts a Claude
//! Code session and a Codex session in two projects, hears both go idle,
//! reads their screens, sends a drafted reply on the person's word and stops
//! one.

use std::collections::HashMap;

use crate::card_activity::Doing;
use crate::pane_sessions::{keeps, rows_in, Kept, Looked};
use crate::session_bus::{idle_notice, pane_now, Bus, Inbox, Pane, Seen, Watch};

const CLAUDE_WORKING: &str =
    "> fix the flaky test\n\n✻ Thinking… (12s · esc to interrupt)\n\n────\n❯ \n────\n  ? for shortcuts";
const CLAUDE_AT_PROMPT: &str =
    "> fix the flaky test\n\n● Fixed: the clock is injected.\n\n────\n❯ \n────\n  ? for shortcuts";
const CODEX_IN_A_TURN: &str =
    "› Count to 60\n• Working (6s • esc to interrupt)\n› Ask Codex to do anything";
const CODEX_AT_PROMPT: &str =
    "  done\n  Worked for 12s\n› Ask Codex to do anything\n  medium · ~/work";

fn codex_profile() -> devpit_rpc::Profile {
    devpit_rpc::Profile {
        id: "codex".to_owned(),
        label: "Codex".to_owned(),
        command: "codex".to_owned(),
        driver: "codex".to_owned(),
        path: Some("/usr/bin/codex".to_owned()),
        reach: devpit_rpc::Reach::Runnable,
        base: "codex".to_owned(),
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

fn kept(name: &str, project: &str, leaf: &str) -> Kept {
    Kept {
        profile: "codex".to_owned(),
        name: name.to_owned(),
        project_id: project.to_owned(),
        project_name: Some(project.trim_start_matches("prj_").to_owned()),
        card_id: None,
        cwd: format!("/home/ana/work/{project}"),
        pane_id: leaf.to_owned(),
    }
}

fn pane(project: &str, leaf: &str) -> Inbox {
    Inbox::Pane {
        project: project.to_owned(),
        leaf: leaf.to_owned(),
    }
}

/// What each pane shows, and what its agent's hook last said.
type Screens = HashMap<&'static str, (&'static str, Option<Doing>)>;

fn look(screens: &Screens) -> impl Fn(&Kept) -> Option<Looked> + '_ {
    |one| {
        let (screen, hook) = screens.get(one.pane_id.as_str())?;
        Some(Looked {
            agent: true,
            screen: Some((*screen).to_owned()),
            hook: *hook,
        })
    }
}

/// A look the bus keeps at each pane: the idle ones tell their watchers.
fn heard(bus: &mut Bus, screens: &Screens) -> Vec<String> {
    let mut told = Vec::new();
    for (leaf, project) in [("leaf_api", "prj_api"), ("leaf_web", "prj_web")] {
        let (screen, hook) = screens[leaf];
        let seen = Seen {
            hook,
            agent: true,
            screen: Some(screen),
        };
        if bus.settled(leaf, pane_now(&seen, None, 0.0), hook.is_some()) == Pane::Free {
            told.extend(
                bus.idle(&pane(project, leaf))
                    .into_iter()
                    .map(|one| idle_notice(&one.name, false)),
            );
        }
    }
    told
}

#[test]
fn an_orchestrator_on_codex_runs_a_claude_and_a_codex_session_to_the_end() {
    // Created on Codex.
    assert_eq!(
        crate::orchestrator::may_orchestrate(&codex_profile()),
        Ok(())
    );

    // It starts a Claude Code session in one project and a Codex one in another:
    // each briefed as its CLI takes it, and both kept by devpit, since an
    // orchestrator on Codex has no Claude Code registry of its own to read.
    let claude = crate::opening::launched(
        crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::En),
        "claude",
        "prj_api",
        "api-fix",
        "fix the test",
        None,
    )
    .expect("claude");
    assert!(claude.contains(" --name 'api-fix' "), "{claude}");
    let codex = crate::opening::launched(
        crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::En),
        "codex",
        "prj_web",
        "web-fix",
        "ship the page",
        None,
    )
    .expect("codex");
    assert!(
        codex.ends_with(" 'ship the page'") && !codex.contains("--name"),
        "{codex}"
    );
    assert!(keeps(false, true) && keeps(false, false));
    let mut held = vec![
        kept("api-fix", "prj_api", "leaf_api"),
        kept("web-fix", "prj_web", "leaf_web"),
    ];

    // Both at work, and listed by the names it gave them.
    let mut screens: Screens = HashMap::from([
        ("leaf_api", (CLAUDE_WORKING, Some(Doing::Working))),
        ("leaf_web", (CODEX_IN_A_TURN, None)),
    ]);
    let rows = rows_in(&mut held, "codex", look(&screens), 0.0);
    let listed: Vec<(&str, &str)> = rows
        .iter()
        .map(|one| (one.name.as_str(), one.status.as_str()))
        .collect();
    assert_eq!(listed, [("api-fix", "busy"), ("web-fix", "busy")]);

    // It asks to hear when each goes idle, and hears it once each.
    let mut bus = Bus::default();
    for (project, leaf, name) in [
        ("prj_api", "leaf_api", "api-fix"),
        ("prj_web", "leaf_web", "web-fix"),
    ] {
        assert!(bus.watch(Watch {
            pane: pane(project, leaf),
            orchestrator: "prj_orch".to_owned(),
            name: name.to_owned(),
        }));
    }
    assert!(heard(&mut bus, &screens).is_empty(), "nothing is idle yet");
    screens.insert("leaf_api", (CLAUDE_AT_PROMPT, Some(Doing::Done)));
    screens.insert("leaf_web", (CODEX_AT_PROMPT, None));
    let mut told = heard(&mut bus, &screens);
    // A pane no hook speaks for is idle once two looks agree.
    told.extend(heard(&mut bus, &screens));
    assert_eq!(told.len(), 2, "{told:?}");
    assert!(told
        .iter()
        .any(|one| one.contains("api-fix, which you asked to hear about, is idle now")));
    assert!(told
        .iter()
        .any(|one| one.contains("web-fix, which you asked to hear about, is idle now")));
    assert!(heard(&mut bus, &screens).is_empty(), "told twice");

    // It reads their screens: idle, no question open, at the panes it typed into.
    let rows = rows_in(&mut held, "codex", look(&screens), 0.0);
    for row in &rows {
        assert_eq!(row.status, "idle", "{}", row.name);
        assert!(row.waiting.is_none(), "{}", row.name);
    }
    let panes: Vec<&str> = rows
        .iter()
        .filter_map(|one| one.pane.as_ref().map(|pane| pane.pane_id.as_str()))
        .collect();
    assert_eq!(panes, ["leaf_api", "leaf_web"]);

    // A drafted reply waits for the person; their word, heard by devpit since
    // Codex keeps no log it reads, lets it go, and nothing else does.
    let root = tempfile::tempdir().expect("orchestrator folder");
    crate::reply_drafts::draft("prj_orch", "codex", "web-fix", "Open the PR").expect("drafted");
    assert_eq!(
        crate::reply_drafts::drafted("codex", "web-fix").as_deref(),
        Some("Open the PR")
    );
    crate::draft_sending::heard_in(root.path(), "how are they doing?");
    let said = crate::draft_sending::heard_last(root.path()).expect("heard");
    assert!(!crate::draft_sending::lets_go(&said, "web-fix", 1));
    crate::draft_sending::heard_in(
        root.path(),
        &devpit_agentcli::person::by_devpit("web-fix is idle."),
    );
    crate::draft_sending::heard_in(root.path(), "✅ web-fix");
    let said = crate::draft_sending::heard_last(root.path()).expect("heard");
    assert!(crate::draft_sending::lets_go(&said, "web-fix", 1));

    // It stops one on the person's word, and only on it.
    let running = ["api-fix", "web-fix"];
    let request = |said| crate::stop_asked::Request {
        said,
        name: "api-fix",
        pid: None,
        running: &running,
        swapped: false,
        losing: None,
        force: false,
        drafted: false,
    };
    assert!(crate::stop_asked::allowed(&request(Some(&said))).is_err());
    crate::draft_sending::heard_in(root.path(), "stop api-fix");
    let said = crate::draft_sending::heard_last(root.path()).expect("heard");
    assert_eq!(crate::stop_asked::allowed(&request(Some(&said))), Ok(()));
    crate::reply_drafts::forget("codex", "web-fix");
}
