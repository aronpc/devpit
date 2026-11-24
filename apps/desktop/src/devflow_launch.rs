//! Devflow on an agent's launch line: the plugin folder Claude Code loads for
//! that one session, or the hooks Codex takes with `-c` and the trust for
//! exactly them.
//!
//! Added only where the person switched Devflow on, when the line is made:
//! switched off, the next session starts as it did before, and the gate a
//! running session still calls answers nothing. Nothing is written into a
//! CLI's own settings, so no session outside devpit's ever hears of them.

use std::path::{Path, PathBuf};

use devflow_engine::plugin::launch::{self as plugin, Cli};
use devpit_core::Store;

/// Where devpit keeps Devflow's plugin for each CLI, in its own home.
pub(crate) fn folder(root: &Path, cli: Cli) -> PathBuf {
    root.join("devflow-plugin").join(cli.word())
}

/// Writes what differs of `files` under `base`.
pub(crate) fn written(base: &Path, files: &[(String, String)]) -> Option<()> {
    for (relative, text) in files {
        let path = base.join(relative);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        std::fs::create_dir_all(path.parent()?).ok()?;
        // Private, as the hook settings: these are commands an agent runs.
        devpit_core::home::write_private(&path, text.as_bytes()).ok()?;
    }
    Some(())
}

/// The flags that bring Devflow to `agent` started in `project_id`; none
/// where Devflow is off there, or for a CLI it has no hooks for. The
/// session's `locale` arrives with them: the plugin's own words will speak
/// it when they are translated, and no launcher asks the settings again.
pub(crate) fn flags(
    locale: crate::session_locale::SessionLocale,
    agent: &str,
    project_id: &str,
) -> Option<String> {
    let cli = Cli::of(agent)?;
    let store = crate::projects::store().ok()?;
    let row = store.project(project_id).ok()??;
    if !crate::devflow::is_on(project_id, Path::new(&row.root_path)) {
        return None;
    }
    let exe = crate::agent_reach::exe()?;
    flags_for(locale, cli, &Store::root().ok()?, exe.to_str()?)
}

/// [`flags`], once Devflow is known to be on. The plugin folder is every
/// session's, so no session's language is written into it: `locale` is held
/// for the hooks' words, which the session's own environment already names.
pub(crate) fn flags_for(
    _locale: crate::session_locale::SessionLocale,
    cli: Cli,
    root: &Path,
    exe: &str,
) -> Option<String> {
    match cli {
        Cli::Claude => {
            let base = folder(root, cli);
            written(
                &base,
                &plugin::claude_in(&crate::devflow_dialect::DEVPIT, exe),
            )?;
            let path = base.to_str()?;
            // `=`, as the MCP file's flag: a list option takes the next word too.
            (!path.contains('\'')).then(|| format!("--plugin-dir='{path}'"))
        }
        Cli::Codex => {
            // Codex installs a plugin into every session it starts, so its
            // skills are left for the person to add; the hooks ride this line.
            let _ = written(
                &folder(root, cli),
                &plugin::codex_in(&crate::devflow_dialect::DEVPIT, exe),
            );
            // Codex keys a `-c` hook's trust by a path it spells for POSIX.
            if cfg!(windows) {
                return None;
            }
            plugin::codex_flags(exe)
        }
    }
}

#[cfg(test)]
#[path = "devflow_launch_tests.rs"]
mod tests;
