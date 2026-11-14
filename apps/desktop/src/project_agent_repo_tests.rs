use super::*;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".claude")).expect("claude dir");
    dir
}

#[test]
fn settings_that_lead_out_are_too_long_or_not_json_add_no_rule() {
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(
        outside.path().join("settings.json"),
        r#"{"permissions":{"deny":["Read(./outside)"]}}"#,
    )
    .expect("outside settings");
    let dir = project();
    let root = dir.path();
    std::os::unix::fs::symlink(
        outside.path().join("settings.json"),
        root.join(".claude/settings.json"),
    )
    .expect("link");
    std::fs::write(root.join(".claude/settings.local.json"), "{ not json").expect("broken");
    assert!(denied(root).is_empty(), "{:?}", denied(root));

    // Valid as far as it would be read: only its length leaves it out.
    let long = format!(
        r#"{{"permissions":{{"deny":["Read(./.env)"]}}}}{}"#,
        " ".repeat(SETTINGS_BYTES as usize)
    );
    std::fs::write(root.join(".claude/settings.local.json"), long).expect("long");
    assert!(denied(root).is_empty(), "{:?}", denied(root));
}

#[test]
fn each_deny_rule_is_taken_once_and_nothing_else() {
    let dir = project();
    let root = dir.path();
    std::fs::write(
        root.join(".claude/settings.json"),
        r#"{"permissions":{"deny":["Read(./.env)", " ", 3, "Read(./.env)"],"allow":["Bash(sh:*)"]}}"#,
    )
    .expect("settings");
    std::fs::write(
        root.join(".claude/settings.local.json"),
        r#"{"permissions":{"deny":["Read(./.env)","Grep(./secrets/**)"]}}"#,
    )
    .expect("local");
    assert_eq!(denied(root), ["Read(./.env)", "Grep(./secrets/**)"]);
}

#[test]
fn instructions_go_once_each_marked_and_cut_when_long() {
    let dir = project();
    let root = dir.path();
    assert_eq!(instructions(root), "");
    std::fs::write(root.join("AGENTS.md"), "# Working here\n\nTabs.\n").expect("agents");
    std::os::unix::fs::symlink(root.join("AGENTS.md"), root.join("CLAUDE.md")).expect("link");
    let said = instructions(root);
    assert_eq!(said.matches("Tabs.").count(), 1, "{said}");
    assert!(
        said.contains("<project-instructions file=\"CLAUDE.md\">"),
        "{said}"
    );

    std::fs::remove_file(root.join("CLAUDE.md")).expect("unlink");
    std::fs::write(root.join("CLAUDE.md"), "y".repeat(INSTRUCTIONS_BYTES + 10)).expect("long");
    let said = instructions(root);
    assert!(
        said.contains("(cut at 32 KiB: read CLAUDE.md for the rest)"),
        "cut"
    );
    assert!(
        said.contains("<project-instructions file=\"AGENTS.md\">"),
        "{said}"
    );
    assert!(said.len() < 2 * INSTRUCTIONS_BYTES + 1024);
}
