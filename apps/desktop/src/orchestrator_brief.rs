//! An orchestrator's instructions for a CLI other than Claude Code, from one
//! source: its `CLAUDE.md` and what that imports. Gemini reads `GEMINI.md`,
//! which imports `CLAUDE.md` the way Claude does; Codex and the ACP agents
//! read `AGENTS.md`, which imports nothing, so devpit writes it out whole.
//! The preferences a Claude hook hands the model with each message go at the
//! head of the message for a CLI without that hook.

use std::io::Read;
use std::path::Path;

/// The file devpit writes from [`SOURCE`], rewritten whenever it differs.
pub(crate) const AGENTS: &str = "AGENTS.md";
/// The one the person edits.
const SOURCE: &str = "CLAUDE.md";
/// Most of one file that is read: a brief, never a document.
const LARGEST: u64 = 256 * 1024;
/// How many imports deep are followed, as a guard against a loop.
const DEPTH: usize = 4;

/// The first line of [`AGENTS`], saying where to edit it.
const WRITTEN: &str =
    "<!-- written by devpit from CLAUDE.md: edit that file, this one is rewritten -->";

fn read(path: &Path) -> Option<String> {
    let mut text = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(LARGEST)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// `text`, read in `dir`, with each line that is only `@path` replaced by
/// that file when it is a file of `folder`; any other import is left as it is.
pub(crate) fn flattened(folder: &Path, dir: &Path, text: &str, depth: usize) -> String {
    let Ok(root) = folder.canonicalize() else {
        return text.to_owned();
    };
    text.split_inclusive('\n')
        .map(|line| {
            let named = line
                .trim()
                .strip_prefix('@')
                .filter(|path| !path.is_empty());
            let found = named
                .filter(|_| depth < DEPTH)
                .and_then(|path| dir.join(path).canonicalize().ok())
                .filter(|path| path.starts_with(&root) && path.is_file());
            match found.and_then(|path| Some((read(&path)?, path))) {
                Some((inner, path)) => {
                    let at = path.parent().unwrap_or(&root).to_path_buf();
                    let inner = flattened(folder, &at, &inner, depth + 1);
                    match inner.ends_with('\n') {
                        true => inner,
                        false => format!("{inner}\n"),
                    }
                }
                None => line.to_owned(),
            }
        })
        .collect()
}

/// `AGENTS.md` written from `CLAUDE.md` when it differs, and only as a file of
/// `folder`: nothing when there is no `CLAUDE.md` to write it from.
pub(crate) fn write_agents(folder: &Path) -> std::io::Result<()> {
    let Some(source) = read(&folder.join(SOURCE)) else {
        return Ok(());
    };
    let text = format!("{WRITTEN}\n{}", flattened(folder, folder, &source, 0));
    let path = folder.join(AGENTS);
    if path.symlink_metadata().is_ok() {
        crate::assistant::inside(folder, &path)?;
    }
    if read(&path).as_deref() != Some(text.as_str()) {
        std::fs::write(&path, text)?;
    }
    Ok(())
}

/// `prompt` for a turn in `project_id` under `profile_id`: in an
/// orchestrator's chat on a CLI without Claude's prompt hook, `AGENTS.md`
/// brought up to date and the person's preferences put at its head.
pub(crate) fn headed(project_id: &str, profile_id: &str, prompt: &str) -> String {
    let Some(folder) = hookless_orchestrator(project_id, profile_id) else {
        return prompt.to_owned();
    };
    let _ = write_agents(&folder);
    let under = devpit_core::Store::root().unwrap_or_default();
    with_preferences(
        &crate::orchestrator_view::shown_now(&folder, &under),
        prompt,
    )
}

/// `prompt` under `preferences`, each its own paragraph; as it was with none.
pub(crate) fn with_preferences(preferences: &str, prompt: &str) -> String {
    match preferences.trim() {
        "" => prompt.to_owned(),
        shown => format!("{shown}\n\n{prompt}"),
    }
}

/// The folder of `project_id` when it is an orchestrator and `profile_id`
/// runs a driver with no prompt hook.
fn hookless_orchestrator(project_id: &str, profile_id: &str) -> Option<std::path::PathBuf> {
    let store = crate::projects::store().ok()?;
    let driver = crate::agent_profiles::all(&store)
        .ok()?
        .into_iter()
        .find(|one| one.id == profile_id)?
        .driver;
    if driver == "claude" {
        return None;
    }
    let (_, root) = crate::projects::locate(&store, project_id).ok()?;
    let home = devpit_core::Store::root().ok()?.canonicalize().ok()?;
    devpit_core::home::orchestrator_of(&store, &home, &root)?;
    Some(root)
}

#[cfg(test)]
#[path = "orchestrator_brief_tests.rs"]
mod tests;
