use super::*;

#[test]
fn a_log_is_only_one_the_brief_names() {
    let folder = Path::new("/o");
    assert_eq!(
        log_file(folder, "sessions"),
        Some(PathBuf::from("/o/context/sessions.md"))
    );
    assert_eq!(log_file(folder, "../../.bashrc"), None);
    assert_eq!(log_file(folder, "notes/api"), None);
}

#[test]
fn an_entry_is_a_dated_bullet_with_its_lines_under_it() {
    assert_eq!(
        entry("2026-10-01", "orch-55319: handed the fix\nPR #12 open").as_deref(),
        Ok("- 2026-10-01 — orch-55319: handed the fix\n  PR #12 open\n")
    );
    assert!(entry("2026-10-01", "   ").is_err());
    assert!(entry("2026-10-01", "bad\u{1b}[2J").is_err());
}

#[test]
fn a_note_is_appended_and_what_was_there_stays() {
    let dir = tempfile::tempdir().expect("tempdir");
    note(dir.path(), "sessions", "first", Section::Log).expect("first");
    note(dir.path(), "sessions", "second", Section::Log).expect("second");
    let written = std::fs::read_to_string(dir.path().join("context/sessions.md")).expect("read");
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("— first") && lines[1].ends_with("— second"));
}

#[test]
fn the_rules_for_sessions_go_with_every_brief() {
    let text = "# Preferences\n\n- likes small commits\n\n## Rules for sessions\n\n- Never push to main.\n- Write commit messages in English.\n\n## Other\n- not this\n";
    assert_eq!(
        super::rules_in(text).as_deref(),
        Some("- Never push to main.\n- Write commit messages in English.")
    );
    assert_eq!(super::rules_in("# Preferences\n- nothing here\n"), None);
    assert_eq!(super::rules_in("## Rules for sessions\n\n"), None);

    let dir = tempfile::tempdir().expect("dir");
    let folder = dir.path().join("workspace");
    std::fs::create_dir_all(folder.join("context")).expect("context");
    std::fs::write(folder.join("context/preferences.md"), text).expect("prefs");
    let brief = super::briefed(&folder, "Fix the invoice total.", None, false);
    assert_eq!(
        brief,
        "Fix the invoice total.\n\n## Reporting back\n\nThe orchestrator that started you is `orchestrator-workspace`: SendMessage to that name to report back. It keeps the name when its chat restarts.\n\n## Rules for sessions\n\n- Never push to main.\n- Write commit messages in English."
    );
    // Too long already, it goes as it is rather than being refused.
    let long = "x".repeat(7990);
    assert_eq!(super::briefed(&folder, &long, None, false), long);
}

/// A session reports to the orchestrator by a name taken from its folder, so
/// the address in a brief still reaches it after its chat restarted.
#[test]
fn a_session_is_told_the_orchestrators_lasting_name() {
    let folder = std::path::Path::new("/home/someone/.devpit/orchestrator/claude/workspace");
    assert_eq!(
        crate::orchestrator::address(folder).as_deref(),
        Some("orchestrator-workspace")
    );
    let brief = super::briefed(folder, "Ship it.", None, false);
    assert!(brief.starts_with("Ship it.\n\n## Reporting back\n\n"));
    assert!(brief.contains("SendMessage to that name"));
    assert!(brief.contains("`orchestrator-workspace`"));
    // Nothing to brief, nothing added.
    assert_eq!(super::briefed(folder, "  ", None, false), "  ");
}

/// How an orchestrator's preferences read today: the rules in the middle,
/// the person's own notes after them, and the log at the end.
const REORGANISED: &str = "# Preferences

## Rules for sessions
- Helper scripts only on the local machine, never in `/tmp`.
- Commit messages in English, with no attribution trailers.

## Orchestrator (not for the sessions)
- 2026-10-22 — No drafts the person did not ask for.

## Access (no secrets)
- **DNS provider:** reached through the connector, never by hand.

## Per project
- **billing:** one subject at a time.

## Register (devpit_log entries land here; not for the sessions)
- 2026-11-04 — the release goes out in one piece.
";

/// How it read before: the rules last, so every entry logged after them
/// went with every brief.
const RULES_LAST: &str = "# Preferences

## Orchestrator (not for the sessions)
- 2026-10-22 — No drafts the person did not ask for.

## Rules for sessions
- Helper scripts only on the local machine, never in `/tmp`.
- Commit messages in English, with no attribution trailers.
";

const BILLING: ProjectName<'static> = ProjectName {
    id: "prj_billing",
    name: "billing",
};

fn preferences_in(text: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("dir");
    let folder = dir.path().join("workspace");
    std::fs::create_dir_all(folder.join("context")).expect("context");
    std::fs::write(folder.join("context/preferences.md"), text).expect("prefs");
    (dir, folder)
}

fn preferences(folder: &Path) -> String {
    std::fs::read_to_string(folder.join("context/preferences.md")).expect("read")
}

#[test]
fn a_preference_logged_after_the_rules_stays_out_of_them() {
    let (_dir, folder) = preferences_in(RULES_LAST);
    let rules = rules_in(RULES_LAST);
    note(&folder, "preferences", "likes short reports", Section::Log).expect("note");
    let written = preferences(&folder);
    assert_eq!(rules_in(&written), rules);
    assert!(written.starts_with(RULES_LAST));
    assert!(written.contains("trailers.\n\n## Log\n\n- "));
    assert!(written.trim_end().ends_with("— likes short reports"));
    // The next one goes to the same log, under no second heading.
    note(
        &folder,
        "preferences",
        "reads the board first",
        Section::Log,
    )
    .expect("again");
    let again = preferences(&folder);
    assert_eq!(again.matches("## Log").count(), 1);
    assert_eq!(rules_in(&again), rules);
}

#[test]
fn a_preference_logged_after_other_notes_is_appended_as_before() {
    let (_dir, folder) = preferences_in(REORGANISED);
    let wrote = note(&folder, "preferences", "likes short reports", Section::Log).expect("note");
    assert_eq!(preferences(&folder), format!("{REORGANISED}{wrote}"));
}

#[test]
fn a_rule_is_written_under_the_rules_only_when_asked() {
    let (_dir, folder) = preferences_in(REORGANISED);
    note(
        &folder,
        "preferences",
        "Never push to main.",
        Section::Rules,
    )
    .expect("rule");
    let rules = rules_in(&preferences(&folder)).expect("rules");
    assert!(rules.starts_with("- Helper scripts only"));
    assert!(rules.ends_with("— Never push to main."));
    assert!(preferences(&folder).contains("trailers.\n- "));
    assert!(preferences(&folder).contains("Never push to main.\n\n## Orchestrator"));
}

#[test]
fn a_projects_rule_goes_to_that_projects_section() {
    let (_dir, folder) = preferences_in(REORGANISED);
    note(
        &folder,
        "preferences",
        "Deploy only after the gate.",
        Section::Project(BILLING),
    )
    .expect("first");
    let written = preferences(&folder);
    assert!(written.starts_with(REORGANISED));
    assert!(written.contains("\n\n## Rules for sessions: billing\n\n- "));
    note(
        &folder,
        "preferences",
        "Ask before a migration.",
        Section::Project(BILLING),
    )
    .expect("second");
    let again = preferences(&folder);
    assert_eq!(again.matches("## Rules for sessions: billing").count(), 1);
    assert_eq!(rules_in(&again), rules_in(REORGANISED));
    // A section named by the project's id is the same project's.
    let by_id = REORGANISED.replace("## Per project", "## Rules for sessions: prj_billing");
    let (_other, folder) = preferences_in(&by_id);
    note(
        &folder,
        "preferences",
        "Ask before a migration.",
        Section::Project(BILLING),
    )
    .expect("by id");
    let written = preferences(&folder);
    assert!(!written.contains("## Rules for sessions: billing"));
    assert!(written.contains("one subject at a time.\n- "));
}

#[test]
fn the_sessions_log_takes_no_section() {
    let dir = tempfile::tempdir().expect("dir");
    assert!(note(dir.path(), "sessions", "x", Section::Rules).is_err());
    assert!(note(dir.path(), "sessions", "x", Section::Project(BILLING)).is_err());
}

const REPORTING: &str = "## Reporting back\n\nThe orchestrator that started you is `orchestrator-workspace`: SendMessage to that name to report back. It keeps the name when its chat restarts.";
const GENERAL: &str = "## Rules for sessions\n\n- Helper scripts only on the local machine, never in `/tmp`.\n- Commit messages in English, with no attribution trailers.";

#[test]
fn the_brief_takes_the_rules_up_to_the_next_heading() {
    for text in [REORGANISED, RULES_LAST] {
        let (_dir, folder) = preferences_in(text);
        assert_eq!(
            briefed(&folder, "Go.", Some(BILLING), false),
            format!("Go.\n\n{REPORTING}\n\n{GENERAL}")
        );
    }
    // What was logged after the rules last stays out of the brief.
    let (_dir, folder) = preferences_in(RULES_LAST);
    note(
        &folder,
        "preferences",
        "the AWS account is the shared one",
        Section::Log,
    )
    .expect("note");
    assert_eq!(
        briefed(&folder, "Go.", Some(BILLING), false),
        format!("Go.\n\n{REPORTING}\n\n{GENERAL}")
    );
}

#[test]
fn a_session_gets_its_own_projects_rules_and_no_one_elses() {
    let text = format!(
        "{RULES_LAST}\n## Rules for sessions: billing\n- Deploy only after the gate.\n\n## Rules for sessions: shop, prj_billing\n- Ask before a migration.\n\n## Rules for sessions: shop\n- The shop's DNS is the person's.\n"
    );
    let (_dir, folder) = preferences_in(&text);
    let billing = briefed(&folder, "Go.", Some(BILLING), false);
    assert_eq!(
        billing,
        format!("Go.\n\n{REPORTING}\n\n{GENERAL}\n\n## Rules for sessions: billing\n\n- Deploy only after the gate.\n\n- Ask before a migration.")
    );
    let shop = ProjectName {
        id: "prj_shop",
        name: "Shop",
    };
    let other = briefed(&folder, "Go.", Some(shop), false);
    assert!(!other.contains("Deploy only after the gate."));
    assert!(other.ends_with("## Rules for sessions: Shop\n\n- Ask before a migration.\n\n- The shop's DNS is the person's."));
    // Rules of a project and none general: the project's alone.
    let (_only, folder) =
        preferences_in("## Rules for sessions: billing\n- Deploy only after the gate.\n");
    assert_eq!(
        briefed(&folder, "Go.", Some(BILLING), false),
        format!(
            "Go.\n\n{REPORTING}\n\n## Rules for sessions: billing\n\n- Deploy only after the gate."
        )
    );
    assert_eq!(
        briefed(&folder, "Go.", None, false),
        format!("Go.\n\n{REPORTING}")
    );
}

/// A session on a CLI without SendMessage is told devpit's own tool instead.
#[test]
fn a_session_without_send_message_reports_through_devpit() {
    let claude = super::report_line("orchestrator-work", true);
    assert!(
        claude.contains("SendMessage to that name to report back"),
        "{claude}"
    );
    let other = super::report_line("orchestrator-work", false);
    assert!(other.contains("`orchestrator-work`: message it with devpit_session_message, `to` left out, to report back"), "{other}");
    assert!(!other.contains("SendMessage"), "{other}");
}
