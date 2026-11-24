//! The guards, as a command.
//!
//! Each is written so that it fails when removed — a guard only ever seen
//! passing proves nothing. One guard per module, with the test that catches
//! the thing it exists to catch next to it.
//!
//! Runs as `cargo xtask check`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod agent_boundary;
mod catalog;
mod csp;
mod dead_controls;
mod home_paths;
mod listening;
mod naming;
mod off_main;
mod one_spelling;
mod packaging;
mod platform_window;
mod ratchet;
mod reachable;
mod release_announcement;
mod release_manifest;
mod release_notes;
mod release_workflow;
mod reseed;
mod shell_boundary;
mod tmux_survives;
mod ui_text;
mod uncalled;
mod versions;

fn main() -> ExitCode {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "check".to_owned());

    match command.as_str() {
        "check" => check(),
        "ceilings" => ceilings(),
        "controls" => controls(),
        "ui-text" => ui_text(),
        "release-manifest" => release_manifest_command(),
        "release-notes" => release_notes_command(),
        "release-announcement" => release_announcement_command(),
        other => {
            eprintln!(
                "unknown command: {other}\n\nusage: cargo xtask check | ceilings | controls | ui-text"
            );
            ExitCode::FAILURE
        }
    }
}

/// Rewrites the size ceilings from what the tree measures now.
fn ceilings() -> ExitCode {
    match reseed::reseed(&workspace_root()) {
        Ok(count) => {
            println!("ceilings: {count} files");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not write ceilings: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Rewrites the dead-control budgets from what the tree has now.
///
/// Tightening only, for the same reason as the ceilings: a command that can
/// raise a budget is the way around the guard it serves.
fn controls() -> ExitCode {
    match dead_controls::reseed(&workspace_root()) {
        Ok(count) => {
            println!("dead controls: {count} left");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not write the budgets: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Rewrites the fixed-screen-text counts from what the tree says now.
fn ui_text() -> ExitCode {
    match ui_text::reseed(&workspace_root()) {
        Ok(count) => {
            println!("fixed screen text: {count} left");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not write the counts: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask release-notes <version>` — the changelog's section for it,
/// printed, for the release page and the update card.
fn release_notes_command() -> ExitCode {
    let Some(version) = std::env::args().nth(2) else {
        eprintln!("usage: cargo xtask release-notes <version>");
        return ExitCode::FAILURE;
    };
    match release_notes::run(&workspace_root(), &version) {
        Ok(notes) => {
            println!("{notes}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask release-announcement <version> <url>` — the `#releases`
/// post for it, as a Discord webhook body. Posting is the caller's.
fn release_announcement_command() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let [version, url] = args.as_slice() else {
        eprintln!("usage: cargo xtask release-announcement <version> <url>");
        return ExitCode::FAILURE;
    };
    match release_announcement::run(&workspace_root(), version, url) {
        Ok(body) => {
            println!("{body}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask release-manifest <version> [date] [notes]`
///
/// Run by the release workflow after the overlay build, before anything is
/// published.
fn release_manifest_command() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let Some(version) = args.first() else {
        eprintln!("usage: cargo xtask release-manifest <version> [date] [notes]");
        return ExitCode::FAILURE;
    };
    let date = args.get(1).cloned().unwrap_or_default();
    let notes = args.get(2).cloned().unwrap_or_default();

    match release_manifest::run(&workspace_root(), version, &notes, &date) {
        Ok(written) => {
            for path in written {
                println!("{}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("nothing was written: {why}");
            ExitCode::FAILURE
        }
    }
}

fn check() -> ExitCode {
    let root = workspace_root();
    let mut findings = Vec::new();

    findings.extend(shell_boundary::core_does_not_know_the_shell(&root));
    findings.extend(platform_window::platform_window_matches_the_base(&root));
    findings.extend(naming::nothing_is_named_after_nothing(&root));
    findings.extend(one_spelling::every_name_has_one_spelling(&root));
    findings.extend(ratchet::files_only_get_shorter(&root));
    findings.extend(agent_boundary::only_one_crate_drives_the_agent(&root));
    findings.extend(dead_controls::a_control_either_works_or_goes(&root));
    findings.extend(home_paths::paths_come_from_home(&root));
    findings.extend(csp::the_csp_forbids_what_the_app_never_needs(&root));
    findings.extend(versions::the_version_has_one_source(&root));
    findings.extend(packaging::the_bundle_says_what_it_ships(&root));
    findings.extend(reachable::every_pane_can_be_opened(&root));
    findings.extend(release_workflow::the_release_workflow_keeps_its_promises(
        &root,
    ));
    findings.extend(tmux_survives::the_app_never_kills_the_tmux_server(&root));
    findings.extend(uncalled::a_command_has_a_caller(&root));
    findings.extend(uncalled::the_app_answers_what_the_contract_offers(&root));
    findings.extend(off_main::no_command_holds_the_window(&root));
    findings.extend(listening::nothing_listens_beyond_loopback(&root));
    findings.extend(catalog::every_catalog_keeps_up_with_english(&root));
    findings.extend(ui_text::uncatalogued_screen_text_never_grows(&root));

    if findings.is_empty() {
        println!("guards: ok");
        return ExitCode::SUCCESS;
    }

    eprintln!("\n{} violation(s):\n", findings.len());
    for finding in &findings {
        eprintln!("  {finding}");
    }
    eprintln!();
    ExitCode::FAILURE
}

/// A violation says where, not just that there was one.
///
/// "The guard failed" sends someone looking; file and line send someone
/// fixing. The difference shows when the reader is not the author.
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub what: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{} — {}", self.file.display(), self.line, self.what)
    }
}

/// The repository root, which every guard walks from.
///
/// `CARGO_MANIFEST_DIR` points at `xtask/`; the root is its parent. Deriving
/// it from the current directory would break when the command is run from
/// inside a crate.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/ has a parent")
        .to_path_buf()
}
