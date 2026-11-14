use std::path::Path;

use super::{flattened, with_preferences, write_agents, AGENTS};

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read")
}

/// Claude, Gemini and Codex read one brief: `CLAUDE.md` imports devpit's
/// half, `GEMINI.md` imports `CLAUDE.md`, and `AGENTS.md` is both written out.
#[test]
fn each_cli_reads_the_orchestrators_brief_from_claude_md() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("orchestrator").join("work");
    crate::orchestrator::seed(&folder).expect("seeded");
    assert_eq!(read(&folder.join("GEMINI.md")), "@CLAUDE.md\n");
    let agents = read(&folder.join(AGENTS));
    assert!(
        agents.starts_with("<!-- written by devpit from CLAUDE.md"),
        "{agents}"
    );
    assert!(agents.contains("# Orchestrator"), "the person's half");
    assert!(agents.contains("devpit_sessions"), "devpit's half");
    assert!(
        !agents.contains("@.devpit/orchestrator.md"),
        "imports are written out"
    );

    // An edit to the one source reaches the file Codex reads.
    std::fs::write(folder.join("CLAUDE.md"), "Reply in Portuguese.\n").expect("edit");
    write_agents(&folder).expect("written");
    assert!(read(&folder.join(AGENTS)).ends_with("\nReply in Portuguese.\n"));
}

/// The chat can put files in its folder: an import is followed only to a
/// file of the folder, and never further than a few deep.
#[test]
fn an_import_out_of_the_folder_is_left_as_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("work");
    std::fs::create_dir_all(folder.join("docs")).expect("folder");
    std::fs::write(dir.path().join("secret.md"), "outside").expect("outside");
    std::fs::write(folder.join("docs/a.md"), "A\n@b.md\n").expect("a");
    std::fs::write(folder.join("docs/b.md"), "B").expect("b");
    std::fs::write(folder.join("loop.md"), "@loop.md\n").expect("loop");
    let text = "head\n@docs/a.md\n@../secret.md\n@/etc/hostname\n@loop.md\ntail";
    let flat = flattened(&folder, text);
    assert!(
        flat.starts_with("head\nA\nB\n@../secret.md\n@/etc/hostname\n"),
        "{flat}"
    );
    assert!(flat.ends_with("@loop.md\ntail"), "{flat}");
    assert!(!flat.contains("outside"));
}

/// A file put in once whatever imports it again, and imports no longer
/// followed past the size of a brief: what the chat writes in its folder
/// cannot make the brief, and devpit with it, grow without end.
#[test]
fn an_import_is_put_in_once_and_a_brief_stops_growing_at_its_size() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("work");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("loop.md"), "@loop.md\n".repeat(8)).expect("loop");
    assert_eq!(
        flattened(&folder, "@loop.md\n@loop.md\n"),
        "@loop.md\n".repeat(9)
    );

    let big = "x".repeat(100_000) + "\n";
    let mut text = String::new();
    for n in 0..10 {
        std::fs::write(folder.join(format!("a{n}.md")), &big).expect("big");
        text.push_str(&format!("@a{n}.md\n"));
    }
    let flat = flattened(&folder, &text);
    assert!(flat.len() < 400_000, "{}", flat.len());
    assert!(flat.ends_with("@a9.md\n"), "the rest are left as written");
}

/// A link in `AGENTS.md`'s place is not written through.
#[cfg(unix)]
#[test]
fn agents_md_is_not_written_through_a_link() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("work");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("CLAUDE.md"), "brief").expect("brief");
    let elsewhere = dir.path().join("elsewhere.md");
    std::fs::write(&elsewhere, "theirs").expect("theirs");
    std::os::unix::fs::symlink(&elsewhere, folder.join(AGENTS)).expect("link");
    assert!(write_agents(&folder).is_err());
    assert_eq!(read(&elsewhere), "theirs");
}

/// For a CLI the prompt hook does not reach, the preferences lead the message.
#[test]
fn the_preferences_lead_the_message_and_nothing_is_added_without_them() {
    assert_eq!(
        with_preferences("The person's preferences: short reports.\n", "status?"),
        "The person's preferences: short reports.\n\nstatus?"
    );
    assert_eq!(with_preferences("  \n", "status?"), "status?");
}
