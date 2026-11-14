use super::*;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("AGENTS.md"),
        "# Working in this repository\n\nRules.\n",
    )
    .expect("agents");
    std::fs::create_dir_all(dir.path().join("docs/adr")).expect("adr dir");
    std::fs::write(
        dir.path().join("docs/adr/0001-sync.md"),
        "# 1. Sync the person's workspace\n",
    )
    .expect("adr");
    dir
}

fn read(root: &Path, file: &str) -> String {
    std::fs::read_to_string(root.join(DIR).join(file)).unwrap_or_default()
}

#[test]
fn an_entry_carries_its_date_and_its_source() {
    let found = entry("- 2026-11-13 · AGENTS.md · Everything versioned is in English.")
        .expect("a bullet")
        .expect("an entry");
    assert_eq!(found.day, "2026-11-13");
    assert_eq!(found.source, "AGENTS.md");
    assert_eq!(found.text, "Everything versioned is in English.");

    assert!(entry("Each entry is a bullet.").is_none());
    assert!(entry("- Everything versioned is in English.")
        .expect("a bullet")
        .is_err());
    assert!(entry("- 13/11/2026 · AGENTS.md · English.")
        .expect("a bullet")
        .is_err());
    assert!(entry("- 2026-11-13 ·  · English.")
        .expect("a bullet")
        .is_err());
}

#[test]
fn the_first_wake_writes_three_files_whose_every_entry_has_a_date_and_a_source() {
    let dir = project();
    assert!(seed(dir.path(), "2026-11-13").expect("seeded"));
    for file in FILES {
        let text = read(dir.path(), file);
        assert!(text.starts_with("# "), "{file}: {text}");
        for line in text.lines().filter(|line| line.starts_with("- ")) {
            entry(line)
                .expect("a bullet")
                .unwrap_or_else(|why| panic!("{file}: {line}: {why}"));
        }
    }
    let facts = read(dir.path(), "facts.md");
    assert!(facts.contains("- 2026-11-13 · AGENTS.md · "), "{facts}");
    let decisions = read(dir.path(), "decisions.md");
    assert!(
        decisions.contains("- 2026-11-13 · docs/adr/0001-sync.md · 1. Sync the person's workspace"),
        "{decisions}"
    );
}

#[test]
fn memory_already_kept_is_never_written_over() {
    let dir = project();
    seed(dir.path(), "2026-11-13").expect("seeded");
    let mine = "# Facts\n\n- 2026-11-01 · PR #9 · The person's own line.\n";
    std::fs::write(dir.path().join(DIR).join("facts.md"), mine).expect("edited");
    assert!(!seed(dir.path(), "2026-12-01").expect("seeded again"));
    assert_eq!(read(dir.path(), "facts.md"), mine);
}

#[test]
fn a_project_without_memory_is_left_as_it_is() {
    let dir = project();
    assert_eq!(summary(dir.path()), None);
    assert!(!dir.path().join(".devpit").exists());
}

#[test]
fn the_summary_is_the_newest_entries_and_says_what_it_left_out() {
    let dir = project();
    seed(dir.path(), "2026-11-13").expect("seeded");
    let mut facts = "# Facts\n\n- not an entry at all\n".to_owned();
    for day in 1..=(SHOWN + 2) {
        facts.push_str(&format!("- 2026-10-{day:02} · card #{day} · Fact {day}.\n"));
    }
    std::fs::write(dir.path().join(DIR).join("facts.md"), facts).expect("facts");
    let summary = summary(dir.path()).expect("a summary");
    assert!(summary.contains("Fact 22."), "{summary}");
    assert!(!summary.contains("Fact 1."), "{summary}");
    assert!(summary.contains("2 older"), "{summary}");
    assert!(summary.contains("1 line"), "{summary}");
    assert!(
        summary.contains("1. Sync the person's workspace"),
        "{summary}"
    );
}

#[cfg(unix)]
#[test]
fn a_guide_or_an_adr_leading_out_of_the_project_is_not_read() {
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("secret.md"), "# Not this project's\n").expect("secret");
    let dir = project();
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        dir.path().join("README.md"),
    )
    .expect("guide link");
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        dir.path().join("docs/adr/0002-outside.md"),
    )
    .expect("adr link");
    assert!(seed(dir.path(), "2026-11-13").expect("seeded"));
    let memory = read(dir.path(), "facts.md") + &read(dir.path(), "decisions.md");
    assert!(!memory.contains("Not this project's"), "{memory}");
    assert!(!memory.contains("README.md"), "{memory}");
    assert!(!memory.contains("0002-outside"), "{memory}");
    assert!(
        memory.contains("1. Sync the person's workspace"),
        "{memory}"
    );
}

#[test]
fn a_heading_is_looked_for_only_near_the_top_of_a_file() {
    let dir = project();
    let late = format!("{}# Too far down\n", "\n".repeat(HEAD_BYTES + 1));
    std::fs::write(dir.path().join("docs/adr/0003-long.md"), late).expect("long adr");
    seed(dir.path(), "2026-11-13").expect("seeded");
    let decisions = read(dir.path(), "decisions.md");
    assert!(!decisions.contains("Too far down"), "{decisions}");
    assert!(
        decisions.contains("· docs/adr/0003-long.md · docs/adr/0003-long.md"),
        "{decisions}"
    );
}

#[test]
fn an_approved_lesson_is_kept_with_its_source_and_date_and_a_discarded_one_leaves_no_trace() {
    let dir = project();
    let root = dir.path();
    seed(root, "2026-11-13").expect("seeded");
    propose(
        root,
        "2026-11-13",
        "#331",
        &[
            "Wait 3 s for the queue test.".to_owned(),
            "Run make fmt.".to_owned(),
        ],
    )
    .expect("proposed");
    let waiting = proposals(root);
    assert_eq!(waiting.len(), 2);
    assert_eq!(waiting[0].source, "#331");

    let edited = "Wait 3 s for the queue test under load.";
    decide(
        root,
        "2026-11-14",
        "#331",
        "Wait 3 s for the queue test.",
        None,
        Some(edited),
    )
    .expect("approved");
    decide(root, "2026-11-14", "#331", "Run make fmt.", None, None).expect("discarded");

    assert!(
        read(root, "lessons.md")
            .contains("- 2026-11-14 · #331 · Wait 3 s for the queue test under load.\n"),
        "{}",
        read(root, "lessons.md")
    );
    for file in ["lessons.md", "proposed.md", "facts.md", "decisions.md"] {
        assert!(!read(root, file).contains("make fmt"), "{file}");
    }
    assert!(proposals(root).is_empty());
    let kept = lessons(root);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].text, edited);
    // Decided once: the same proposal is not there to decide again.
    assert!(decide(root, "2026-11-14", "#331", "Run make fmt.", None, None).is_err());
}

#[test]
fn review_rounds_and_repeated_findings_are_logged_per_card() {
    let dir = project();
    let root = dir.path();
    seed(root, "2026-11-13").expect("seeded");
    measure(root, "2026-11-13", "#331", 3, 2).expect("logged");
    measure(root, "2026-11-14", "#332", 1, 0).expect("logged");
    let lines: Vec<serde_json::Value> = read(root, "metrics.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        serde_json::json!({ "day": "2026-11-13", "card": "#331", "reviewRounds": 3, "repeatedFindings": 2 })
    );
}

fn one(source: &str) -> Entry {
    Entry {
        day: "2026-11-13".to_owned(),
        source: source.to_owned(),
        text: "Something.".to_owned(),
    }
}

#[test]
fn an_entry_names_the_files_it_rests_on_and_the_cards_it_came_from() {
    let linked = one("#12, docs/adr/0003-queue.md, PR #4, #15");
    assert_eq!(linked.files(), ["docs/adr/0003-queue.md"]);
    assert_eq!(linked.cards(), ["#12", "#15"]);
    // An entry from before files were listed still rests on its source file.
    assert_eq!(one("AGENTS.md").files(), ["AGENTS.md"]);
    assert!(one("AGENTS.md").cards().is_empty());
    assert!(one("#331").files().is_empty());
}

/// A project in git on main, its memory written and checked at its commit.
fn checked_project() -> tempfile::TempDir {
    use crate::devflow_review::tests::git;
    let dir = project();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "start"]);
    seed(root, "2026-11-13").expect("seeded");
    let first = flagged(root).expect("on a default branch");
    assert!(
        first.entries.is_empty(),
        "the first check only notes where main stands"
    );
    checked(root, &first.tip).expect("checked");
    dir
}

#[test]
fn an_entry_whose_file_changed_on_the_default_branch_is_flagged() {
    use crate::devflow_review::tests::git;
    let dir = checked_project();
    let root = dir.path();
    let mut facts = read(root, "facts.md");
    facts.push_str("- 2026-11-01 · #9 · An entry from before files were listed.\n");
    std::fs::write(root.join(DIR).join("facts.md"), facts).expect("old entry");
    std::fs::write(root.join("AGENTS.md"), "# Working on devpit\n").expect("changed");
    git(root, &["commit", "-q", "-am", "rename the guide"]);

    let found = flagged(root).expect("flagged");
    let names: Vec<(&str, &str)> = found
        .entries
        .iter()
        .map(|(file, one)| (*file, one.source.as_str()))
        .collect();
    assert_eq!(names, [("facts.md", "AGENTS.md")]);
    checked(root, &found.tip).expect("checked");
    assert!(flagged(root).expect("checked again").entries.is_empty());
}

#[test]
fn a_stale_entry_is_a_proposal_and_its_file_changes_only_once_approved() {
    let dir = checked_project();
    let root = dir.path();
    let was = "A guide to read first: \"Working in this repository\".";
    let now = "A guide to read first: \"Working on devpit\".";
    propose(
        root,
        "2026-11-14",
        "AGENTS.md",
        &[aimed("facts.md", now, Some(was))],
    )
    .expect("proposed");
    let before = read(root, "facts.md");
    assert!(before.contains(was), "{before}");

    let waiting = proposals(root);
    assert_eq!(aim(&waiting[0].text), ("facts.md", now, Some(was)));
    decide(root, "2026-11-14", "AGENTS.md", now, Some(was), Some(now)).expect("approved");
    let after = read(root, "facts.md");
    assert!(
        after.contains(&format!("- 2026-11-14 · AGENTS.md · {now}\n")),
        "{after}"
    );
    assert!(!after.contains(was), "{after}");
    assert_eq!(after.lines().count(), before.lines().count());

    // A removal, proposed then approved, takes the entry away.
    propose(
        root,
        "2026-11-15",
        "AGENTS.md",
        &[aimed("facts.md", "", Some(now))],
    )
    .expect("proposed");
    decide(root, "2026-11-15", "AGENTS.md", "", Some(now), Some("")).expect("removed");
    assert!(
        !read(root, "facts.md").contains("AGENTS.md"),
        "{}",
        read(root, "facts.md")
    );
    assert!(proposals(root).is_empty());
}

#[test]
fn a_discarded_stale_proposal_leaves_the_entry_as_it_was() {
    let dir = checked_project();
    let root = dir.path();
    let was = "A guide to read first: \"Working in this repository\".";
    let before = read(root, "facts.md");
    propose(
        root,
        "2026-11-14",
        "AGENTS.md",
        &[aimed("facts.md", "", Some(was))],
    )
    .expect("proposed");
    decide(root, "2026-11-14", "AGENTS.md", "", Some(was), None).expect("discarded");
    assert_eq!(read(root, "facts.md"), before);
    assert!(proposals(root).is_empty());
}

#[test]
fn a_decision_a_card_made_names_the_card_once_approved() {
    let dir = project();
    let root = dir.path();
    seed(root, "2026-11-13").expect("seeded");
    let settled = "Proposals share one queue.";
    propose(
        root,
        "2026-11-14",
        "#332",
        &[aimed("decisions.md", settled, None)],
    )
    .expect("proposed");
    assert!(
        decided(root).is_empty(),
        "nothing is a decision before it is approved"
    );
    decide(root, "2026-11-14", "#332", settled, None, Some(settled)).expect("approved");
    assert!(
        read(root, "decisions.md").contains(&format!("- 2026-11-14 · #332 · {settled}\n")),
        "{}",
        read(root, "decisions.md")
    );
    assert!(!read(root, "lessons.md").contains(settled));
    // The ADR seeded from the project names no card; this one names its own.
    let made = decided(root);
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].cards(), ["#332"]);
    assert_eq!(made[0].text, settled);
}

#[test]
fn an_onboarding_proposal_rests_on_its_files_and_not_on_its_mark() {
    let found = one(&format!("{ONBOARDING}, AGENTS.md, Makefile"));
    assert_eq!(found.files(), ["AGENTS.md", "Makefile"]);
    assert!(found.onboarding());
    assert!(!one("AGENTS.md, #12").onboarding());
}

#[test]
fn approve_all_from_onboarding_writes_each_to_its_file_and_leaves_the_rest_waiting() {
    let dir = project();
    let root = dir.path();
    seed(root, "2026-11-13").expect("seed");
    let marked = format!("{ONBOARDING}, AGENTS.md");
    propose(
        root,
        "2026-11-14",
        &marked,
        &[
            aimed("facts.md", "Tests run with make test.", None),
            aimed("decisions.md", "Sync the workspace, not the machine.", None),
        ],
    )
    .expect("onboarding");
    propose(
        root,
        "2026-11-14",
        "#12",
        &["Wait for the queue.".to_owned()],
    )
    .expect("lesson");

    assert_eq!(approve_onboarding(root, "2026-11-15").expect("approved"), 2);
    assert!(
        read(root, "facts.md").contains("- 2026-11-15 · AGENTS.md · Tests run with make test.\n"),
        "{}",
        read(root, "facts.md")
    );
    assert!(read(root, "decisions.md")
        .contains("- 2026-11-15 · AGENTS.md · Sync the workspace, not the machine.\n"));
    let waiting = proposals(root);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].source, "#12");
}

#[test]
fn learning_again_replaces_the_onboarding_proposals_still_waiting() {
    let dir = project();
    let root = dir.path();
    seed(root, "2026-11-13").expect("seed");
    let marked = format!("{ONBOARDING}, AGENTS.md");
    propose(
        root,
        "2026-11-14",
        &marked,
        &[aimed("facts.md", "Old.", None)],
    )
    .expect("old");
    propose(
        root,
        "2026-11-14",
        "#12",
        &["Wait for the queue.".to_owned()],
    )
    .expect("lesson");
    assert_eq!(drop_onboarding(root).expect("dropped"), 1);
    let waiting = proposals(root);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].source, "#12");
}

/// The memory as v0.1.50 wrote it on a first wake, byte for byte.
const KEPT: [(&str, &str); 4] = [
    ("facts.md", "# Facts\n\nEach entry: `- YYYY-MM-DD · source · what it says`. The source is a file, a card (#12) or a PR (PR #3).\n\n- 2026-11-14 · AGENTS.md · A guide to read first: \"Working in this repository\".\n- 2026-11-14 · README.md · A guide to read first: \"README.md\".\n"),
    ("lessons.md", "# Lessons\n\nEach entry: `- YYYY-MM-DD · source · what it says`. The source is a file, a card (#12) or a PR (PR #3).\n\n"),
    ("decisions.md", "# Decisions\n\nEach entry: `- YYYY-MM-DD · source · what it says`. The source is a file, a card (#12) or a PR (PR #3).\n\n- 2026-11-14 · docs/adr/0001-sync.md · ADR 0001 — Sync the person's workspace, not their machine\n"),
    ("checked", "d85c74b14145f97da79c099d3e86c8d7946b0457\n"),
];

#[test]
fn a_memory_kept_before_onboarding_opens_unchanged() {
    let dir = project();
    let root = dir.path();
    std::fs::create_dir_all(root.join(DIR)).expect("dir");
    for (file, text) in KEPT {
        std::fs::write(root.join(DIR).join(file), text).expect("kept");
    }
    assert!(!seed(root, "2026-11-15").expect("seed"));
    let facts = kept(root, "facts.md");
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].files(), ["AGENTS.md"]);
    assert_eq!(kept(root, "lessons.md").len(), 0);
    assert_eq!(
        kept(root, "decisions.md")[0].files(),
        ["docs/adr/0001-sync.md"]
    );
    // Onboarding only proposes: what was kept stays byte for byte.
    propose(
        root,
        "2026-11-15",
        &format!("{ONBOARDING}, AGENTS.md"),
        &[aimed("facts.md", "Tests run with make test.", None)],
    )
    .expect("proposed");
    onboarded(root, "2026-11-15").expect("onboarded");
    for (file, text) in KEPT {
        assert_eq!(read(root, file), text, "{file} changed");
    }
}
