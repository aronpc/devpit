use devflow_engine::config::Agent;

use super::*;
use crate::devflow_review::tests::{fake_claude, git, project_at};

/// What a test reads of the person's checkout: HEAD, the branch, the status
/// and the worktrees git knows.
fn standing(root: &Path) -> Vec<String> {
    [
        &["rev-parse", "HEAD"][..],
        &["branch", "--show-current"],
        &["status", "--porcelain"],
        &["worktree", "list", "--porcelain"],
    ]
    .iter()
    .map(|args| {
        let out = std::process::Command::new("git")
            .args(*args)
            .current_dir(root)
            .output()
            .expect("git");
        String::from_utf8_lossy(&out.stdout).into_owned()
    })
    .collect()
}

/// A checkout on main with work of the person's not committed, and a
/// `feature` branch whose commit opens a link through a shell.
pub(crate) fn checkout_with_a_feature(root: &Path) -> String {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::write(root.join("app.py"), "print('hi')\n").expect("write");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "start"]);
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(root.join("app.py"), "import os\nos.system(link)\n").expect("change");
    git(root, &["commit", "-q", "-am", "open links"]);
    git(root, &["checkout", "-q", "main"]);
    std::fs::write(root.join("app.py"), "print('mine')\n").expect("unsaved");
    std::fs::write(root.join("notes.txt"), "not committed\n").expect("untracked");
    let store = crate::projects::store().expect("store");
    store.add_project(root, None).expect("project")
}

/// Devflow set up but not switched on: the review still runs on its file.
fn not_switched_on() -> State {
    let yaml = "version: 1\nenabled: false\nreview:\n  reviewers:\n    - agent: claude\n      persona: devflow-security\n";
    let config = devflow_engine::config::parse(yaml).expect("yaml");
    State::Valid(yaml.to_owned(), Box::new(config))
}

fn no_github(_: &Path, _: u64) -> Result<Pull, String> {
    Err("not asked".to_owned())
}

#[cfg(unix)]
#[test]
fn a_range_is_reviewed_without_a_card_and_the_persons_checkout_never_moves() {
    let dir = tempfile::tempdir().expect("tempdir");
    let scripts = tempfile::tempdir().expect("scripts");
    let root = dir.path().canonicalize().expect("root");
    let id = checkout_with_a_feature(&root);
    let project = project_at(&id, &root);
    let before = standing(&root);
    let claude = fake_claude(scripts.path());
    let reach = move |agent: Agent| match agent {
        Agent::Claude => Ok(claude.clone()),
        _ => Err("not installed".to_owned()),
    };
    let asked = devflow_engine::adhoc::standalone(&json!({
        "base": "main", "head": "feature", "criteria": ["links open safely"]
    }))
    .expect("asked")
    .expect("no card");

    let said = respond_with(
        devpit_core::locale::Locale::En,
        &project,
        &not_switched_on(),
        asked,
        &reach,
        &no_github,
        None,
    )
    .expect("reviewed");

    assert_eq!(said["verdict"], "request_changes", "{said}");
    assert!(said.get("card").is_none());
    let home = crate::projects::project_home(&id).expect("home");
    let folder = PathBuf::from(said["folder"].as_str().expect("folder"));
    assert!(folder.starts_with(home.devflow_reviews()), "{folder:?}");
    assert!(!folder.starts_with(home.devflow_runs()));
    assert!(folder.join(review::FINDINGS).is_file());
    let packet = std::fs::read_to_string(folder.join(review::PACKET)).expect("packet");
    assert!(packet.contains("+os.system(link)") && packet.contains("links open safely"));
    assert_eq!(standing(&root), before, "the person's checkout moved");
    let trees = std::fs::read_dir(home.review_trees()).map_or(0, Iterator::count);
    assert_eq!(trees, 0, "the detached worktree is left behind");
}

#[cfg(unix)]
#[test]
fn a_pull_request_is_read_in_a_worktree_of_devpits_and_held_to_its_body() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let id = checkout_with_a_feature(&root);
    let project = project_at(&id, &root);
    let before = standing(&root);
    let head = devpit_git::commit_of(&root, "feature").expect("feature");
    let read_at = format!("git show {head}:");
    let github = move |_: &Path, number: u64| {
        assert_eq!(number, 7);
        Ok(Pull {
            base_ref_name: "main".into(),
            head_ref_oid: head.clone(),
            title: "Open links".into(),
            body: "## Acceptance criteria\n- [ ] the first link opens\n".into(),
        })
    };
    let nobody = |_: Agent| Err("not installed".to_owned());
    let asked = devflow_engine::adhoc::standalone(&json!({ "pr": "#7" }))
        .expect("asked")
        .expect("no card");

    // No devflow.yaml and no reviewer of the person's: the session reviews.
    let said = respond_with(
        devpit_core::locale::Locale::En,
        &project,
        &State::Missing,
        asked,
        &nobody,
        &github,
        None,
    )
    .expect("reviewed");

    assert_eq!(said["reviewed"], "pr-7");
    assert!(said["pending"]
        .as_array()
        .is_some_and(|pending| !pending.is_empty()));
    let next = said["next"].as_str().expect("next");
    assert!(next.contains("pr 7") && next.contains(&read_at), "{next}");
    let packet = std::fs::read_to_string(said["packet"].as_str().expect("packet")).expect("read");
    assert!(packet.contains("the first link opens"), "{packet}");
    let home = crate::projects::project_home(&id).expect("home");
    assert!(PathBuf::from(said["folder"].as_str().expect("folder"))
        .starts_with(home.devflow_reviews().join("pr-7")));
    assert_eq!(standing(&root), before, "the person's checkout moved");
}

#[test]
fn a_review_without_a_card_is_offered_only_where_the_projects_devflow_is() {
    use crate::devflow::{offered, on_demand};
    assert!(!on_demand(&offered(false, &State::Missing, true)));
    // Off until the person activates it: no file, or one that says so.
    assert!(!on_demand(&offered(true, &State::Missing, false)));
    assert!(!on_demand(&offered(true, &not_switched_on(), false)));
    let waiting = "version: 1\nenabled: true\n";
    let waiting = State::Valid(
        waiting.to_owned(),
        Box::new(devflow_engine::config::parse(waiting).expect("yaml")),
    );
    assert!(on_demand(&offered(true, &waiting, false)));
}

#[cfg(unix)]
#[test]
fn a_path_is_reviewed_only_when_it_is_a_worktree_of_the_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let trees = tempfile::tempdir().expect("trees");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let root = dir.path().canonicalize().expect("root");
    checkout_with_a_feature(&root);
    let tree = trees.path().canonicalize().expect("trees").join("one");
    git(
        &root,
        &["worktree", "add", "-q", &tree.to_string_lossy(), "feature"],
    );

    assert_eq!(
        worktree_of(&root, &tree.to_string_lossy()),
        Ok(tree.clone())
    );
    assert!(worktree_of(&root, &elsewhere.path().to_string_lossy()).is_err());
    // A link to a worktree reads as it; a link out of the project does not.
    let link = trees.path().join("link");
    std::os::unix::fs::symlink(&tree, &link).expect("link");
    assert_eq!(worktree_of(&root, &link.to_string_lossy()), Ok(tree));
    let out = trees.path().join("out");
    std::os::unix::fs::symlink(elsewhere.path(), &out).expect("link out");
    assert!(worktree_of(&root, &out.to_string_lossy()).is_err());
}
