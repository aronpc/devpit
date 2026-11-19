use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

fn host(environment: &str) -> HostRow {
    HostRow {
        id: "host_1".to_owned(),
        name: "web prod".to_owned(),
        target: "web-01".to_owned(),
        user: None,
        port: None,
        identity_file: None,
        work_root: "/srv".to_owned(),
        environment: environment.to_owned(),
    }
}

/// A store with one project linked to a production host and a staging one.
fn linked(dir: &tempfile::TempDir) -> (Store, String) {
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.save_host(&host("production")).expect("host");
    store
        .save_host(&HostRow {
            id: "host_2".to_owned(),
            name: "staging".to_owned(),
            environment: "staging".to_owned(),
            ..host("staging")
        })
        .expect("host");
    store
        .link_host(&project, "host_1", "/srv/site")
        .expect("link");
    store.link_host(&project, "host_2", "~/site").expect("link");
    (store, project)
}

/// A start on production waits for the person's yes, passed on as
/// confirm_production; anywhere else it needs none.
#[test]
fn a_start_on_production_needs_the_person_s_yes() {
    let refused = production_refusal(&host("production"), false).expect("refused");
    assert!(refused.contains("confirm_production"), "{refused}");
    assert_eq!(production_refusal(&host("production"), true), None);
    assert_eq!(production_refusal(&host("staging"), false), None);
}

/// A session on production is told so, first, with the rules for it; any
/// session on a host is told where it runs and that devpit's tools are not
/// there.
#[test]
fn a_session_on_a_host_is_told_where_it_runs_and_production_s_rules() {
    let on_prod = briefed_for(&host("production"), "/srv/site", "Fix the invoice total.");
    assert!(
        on_prod.starts_with("## This session runs on production"),
        "{on_prod}"
    );
    assert!(on_prod.contains("web prod"), "{on_prod}");
    assert!(on_prod.contains("reversible"), "{on_prod}");
    assert!(on_prod.ends_with("Fix the invoice total."), "{on_prod}");

    let on_staging = briefed_for(&host("staging"), "~/site", "Fix it.");
    assert!(!on_staging.contains("production"), "{on_staging}");
    assert!(on_staging.contains("~/site"), "{on_staging}");
    assert!(on_staging.contains("devpit's tools"), "{on_staging}");
}

/// The line typed on the host starts the CLI by its plain name, every word
/// quoted for the host's shell, fish's included.
#[test]
fn the_launch_line_is_the_cli_by_name_with_every_word_quoted() {
    let line =
        launch_line("claude", "fix-it", "don't touch \\ the db", Some("opus")).expect("a line");

    assert!(
        line.starts_with("claude --name fix-it --model opus "),
        "{line}"
    );
    assert!(
        !line.contains("\\'"),
        "a quote fish would read as escaped: {line}"
    );
    assert!(launch_line("claude", "bad\nname", "x", None).is_err());
}

/// Typed at a shell on the host, the line runs the CLI with the brief whole.
#[test]
fn the_launch_line_runs_the_cli_on_the_host_with_the_brief_whole() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    let said = fake.dir.path().join("said");
    // Where a login there finds its own programs: its profile says so.
    let own = fake.folder(".local/bin");
    fake.program(
        &own.join("claude"),
        &format!("printf '%s\\n' \"$@\" > '{}'", said.display()),
    );
    std::fs::write(
        fake.home().join(".profile"),
        "PATH=\"$HOME/.local/bin:$PATH\"\n",
    )
    .expect("profile");
    fake.folder("site");
    let reach = crate::host_panes::Reach {
        ssh: fake.ssh(),
        sockets: fake.dir.path().to_path_buf(),
    };
    let session = crate::host_panes::session_of("prj_1", "leaf_1");
    crate::host_panes::started(&reach, &host("staging"), "~/site", &session).expect("started");
    let brief = "Fix it: don't `rm` $HOME; a\\b";

    reach
        .server(&host("staging"))
        .send_keys(
            &format!("{session}:"),
            &launch_line("claude", "fix-it", brief, None).expect("line"),
        )
        .expect("typed");

    let ran = (0..100).find_map(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::read_to_string(&said)
            .ok()
            .filter(|one| !one.is_empty())
    });
    let screen = reach
        .server(&host("staging"))
        .capture_pane(&format!("{session}:"))
        .unwrap_or_default();
    assert_eq!(
        ran.as_deref(),
        Some(format!("--name\nfix-it\n{brief}\n").as_str()),
        "{screen}"
    );
}

/// A host is named by its name or id, among the project's links only.
#[test]
fn a_host_is_named_among_the_project_s_links() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = linked(&dir);

    let (found, path) = linked_host(&store, &project, "Staging").expect("by name");
    assert_eq!((found.id.as_str(), path.as_str()), ("host_2", "~/site"));
    assert_eq!(
        linked_host(&store, &project, "host_1").expect("by id").1,
        "/srv/site"
    );
    store.unlink_host(&project, "host_2").expect("unlinked");
    let refused = linked_host(&store, &project, "staging").expect_err("unlinked");
    assert!(refused.contains("not linked"), "{refused}");
}

/// devpit_sessions says, of each session in a terminal on a host, which host
/// and which environment; a session here is listed as it was.
#[test]
fn a_session_on_a_host_is_listed_with_its_host_and_environment() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = linked(&dir);
    let layout = |leaf: &str, target: String| devpit_rpc::SessionLayout {
        project_id: project.clone(),
        focused_id: leaf.to_owned(),
        tree: devpit_rpc::LayoutNode::leaf(leaf, target),
    };
    crate::sessions::persist(
        &store,
        "tab_a",
        &layout("leaf_a", crate::host_panes::target("host_1", "devpit_s")),
    )
    .expect("kept");
    crate::sessions::persist(
        &store,
        "tab_b",
        &layout("leaf_b", "devpit_p:leaf_b".to_owned()),
    )
    .expect("kept");
    let here = json!({ "name": "local", "pane": { "projectId": project, "paneId": "leaf_b" } });
    let there = json!({ "name": "remote", "pane": { "projectId": project, "paneId": "leaf_a" } });

    // As devpit_sessions answers: the list itself.
    let marked = marked_in(&store, json!([here.clone(), there]));

    assert_eq!(marked[0], here, "a session here changed");
    assert_eq!(marked[1]["host"], "web prod");
    assert_eq!(marked[1]["environment"], "production");
}

/// devpit_projects lists a project's links: host, environment and path.
#[test]
fn a_project_lists_its_hosts_with_their_environments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = linked(&dir);

    let hosts = hosts_of(&store, &project);

    assert_eq!(
        hosts,
        vec![
            json!({ "host": "staging", "environment": "staging", "path": "~/site" }),
            json!({ "host": "web prod", "environment": "production", "path": "/srv/site" }),
        ]
    );
}
