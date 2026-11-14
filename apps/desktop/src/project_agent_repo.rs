//! What a project agent takes from its repository, which it no longer loads
//! itself: it starts with the person's own settings alone. The repository's
//! deny rules still bind it, and its own instructions still reach it; its allow
//! rules never do.

use std::io::Read;
use std::path::Path;

/// The repository's Claude settings whose deny rules bind its agent.
const SETTINGS: [&str; 2] = [".claude/settings.json", ".claude/settings.local.json"];
/// The project's own instructions, handed over in the agent's first message.
const INSTRUCTIONS: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];
/// More than a settings file needs: a longer one is not read.
const SETTINGS_BYTES: u64 = 256 * 1024;
/// How much of each instructions file is handed over.
const INSTRUCTIONS_BYTES: usize = 32 * 1024;
/// The most deny rules taken, and the longest one.
const RULES: usize = 200;
const RULE_CHARS: usize = 500;

/// A regular file of the project, resolved inside its root through its links,
/// with at most `limit` bytes read; `None` when there is none or it leads out.
/// Whether more was there comes beside it.
fn read(root: &Path, relative: &str, limit: u64) -> Option<(String, bool)> {
    let at = devpit_core::tree::resolve(root, relative).ok()?;
    if !at.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&at)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let more = bytes.len() as u64 > limit;
    bytes.truncate(limit as usize);
    Some((String::from_utf8_lossy(&bytes).into_owned(), more))
}

/// The repository's deny rules, from its settings at its root, each once.
/// A file that is too long or not JSON adds none.
pub(crate) fn denied(root: &Path) -> Vec<String> {
    let mut rules: Vec<String> = Vec::new();
    for file in SETTINGS {
        let Some((text, false)) = read(root, file, SETTINGS_BYTES) else {
            continue;
        };
        let Ok(settings) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let deny = settings
            .pointer("/permissions/deny")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|rule| !rule.is_empty() && rule.len() <= RULE_CHARS && !rule.contains('\n'));
        for rule in deny {
            if rules.len() < RULES && !rules.iter().any(|one| one == rule) {
                rules.push(rule.to_owned());
            }
        }
    }
    rules
}

/// The project's instructions as the agent's first message carries them:
/// each file once, marked, and cut when long. Empty when it has none.
pub(crate) fn instructions(root: &Path) -> String {
    let mut seen = Vec::new();
    let mut out = String::new();
    for file in INSTRUCTIONS {
        let Some((text, more)) = read(root, file, INSTRUCTIONS_BYTES as u64) else {
            continue;
        };
        // CLAUDE.md is often a link to AGENTS.md: the same file goes once.
        let real = root.join(file).canonicalize().ok();
        if seen.contains(&real) {
            continue;
        }
        seen.push(real);
        let cut = if more {
            format!(
                "\n(cut at {} KiB: read {file} for the rest)",
                INSTRUCTIONS_BYTES / 1024
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            "<project-instructions file=\"{file}\">\n{}{cut}\n</project-instructions>\n",
            text.trim_end()
        ));
    }
    if out.is_empty() {
        return out;
    }
    format!(
        "These are the project's instructions, from its own files at its root: follow them as \
         this project's rules, as you would had you loaded them yourself.\n\n{out}"
    )
}

#[cfg(all(test, unix))]
#[path = "project_agent_repo_tests.rs"]
mod tests;
