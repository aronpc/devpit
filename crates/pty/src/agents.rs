//! The agent CLIs, and how to tell one is running.
//!
//! One list, two uses, and that is the point: the menu that starts an agent
//! and the reader that recognises one have to agree, or starting Gemini from
//! our own menu opens a terminal the sidebar then says is running `node`.
//!
//! Recognition works on the argument vector, never on the executable's name.
//! Claude Code, Codex, Gemini and OpenCode are JavaScript, so the process is
//! `node` and the name is in `argv[1]`. Skipping the interpreter is therefore
//! most of the job, and it has to skip its *options* too: `node -e "…"` is not
//! an agent no matter what the script says, and `node --require x claude` is.
//!
//! Only a path-shaped argument counts as the program being run. Without that
//! rule `claude "compare opencode and codex"` reads its own prompt back and
//! calls itself OpenCode.

use crate::foreground::basename;

/// One agent CLI this app knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Known {
    /// Stable, and what the screen keys an icon by.
    pub id: &'static str,
    /// What a person calls it.
    pub label: &'static str,
    /// What to type to start it.
    pub launch: &'static str,
    /// The program names that mean this agent is already running. More than
    /// one where a CLI ships under several names, or wraps another.
    pub wears: &'static [&'static str],
    /// How this CLI is told to load an extra settings file, with `{}` where
    /// the path goes — or nothing, for one that has no such flag.
    ///
    /// This is what turns "an agent is open" into "the agent is waiting for
    /// you". Passed on the command line rather than written into the person's
    /// own configuration: our settings file reaches the agents this app
    /// starts and nothing else, and a menu item does not get to edit files in
    /// somebody's home directory.
    pub settings_flag: Option<&'static str>,
    /// How this CLI is told to continue a session by id, with `{}` where the id
    /// goes — or nothing, for one whose resume nobody here has checked.
    pub resume_flag: Option<&'static str>,
    /// Which driver reads its output when it is run headless, empty when none
    /// can. Only Claude Code streams a shape devpit knows how to read, so the
    /// rest are terminal-only until a driver exists for them — and saying so
    /// here beats a profile that offers a chat it cannot hold.
    pub driver: &'static str,
    /// Where its own documentation lives. The row links to it rather than
    /// explaining how to install something devpit does not ship.
    pub homepage: &'static str,
}

/// Every agent, in the order a menu should offer them.
///
/// Ordered rather than sorted: the ones people reach for are at the top, and
/// alphabetical would put Aider above Claude for no reason anyone asked for.
pub const KNOWN: &[Known] = &[
    Known {
        id: "claude",
        label: "Claude Code",
        launch: "claude",
        // A second account is a function in front of the same binary, so the
        // process is still `claude`; its own name never reaches this list.
        wears: &["claude"],
        // Verified against `claude --help`: "load *additional* settings
        // from", so it adds our hooks rather than replacing what the person
        // has configured.
        settings_flag: Some("--settings {}"),
        // `--resume <id>`, measured with `claude -p --resume` on 2.1.270.
        resume_flag: Some("--resume {}"),
        driver: "claude",
        homepage: "https://code.claude.com/docs",
    },
    Known {
        id: "codex",
        label: "Codex",
        launch: "codex",
        wears: &["codex"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://github.com/openai/codex",
    },
    Known {
        id: "gemini",
        label: "Gemini",
        launch: "gemini",
        wears: &["gemini"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://github.com/google-gemini/gemini-cli",
    },
    Known {
        id: "opencode",
        label: "OpenCode",
        launch: "opencode",
        wears: &["opencode"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://opencode.ai/docs/cli/",
    },
    Known {
        id: "cursor",
        label: "Cursor",
        launch: "cursor-agent",
        wears: &["cursor-agent", "cursor"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://cursor.com/cli",
    },
    Known {
        id: "copilot",
        label: "GitHub Copilot",
        launch: "copilot",
        wears: &["copilot"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://docs.github.com/en/copilot/how-tos/set-up/install-copilot-cli",
    },
    Known {
        id: "amp",
        label: "Amp",
        launch: "amp",
        wears: &["amp"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://ampcode.com/manual#install",
    },
    Known {
        id: "droid",
        label: "Droid",
        launch: "droid",
        wears: &["droid"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://docs.factory.ai/cli/getting-started/quickstart",
    },
    Known {
        id: "grok",
        label: "Grok",
        launch: "grok",
        wears: &["grok"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://x.ai/cli",
    },
    Known {
        id: "agy",
        label: "Antigravity",
        launch: "agy",
        wears: &["agy"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://antigravity.google",
    },
    Known {
        id: "aider",
        label: "Aider",
        launch: "aider",
        wears: &["aider"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://aider.chat/docs/install.html",
    },
    Known {
        id: "goose",
        label: "Goose",
        launch: "goose",
        wears: &["goose"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://block.github.io/goose/docs/quickstart/",
    },
    Known {
        id: "crush",
        label: "Crush",
        launch: "crush",
        wears: &["crush"],
        settings_flag: None,
        resume_flag: None,
        driver: "",
        homepage: "https://github.com/charmbracelet/crush",
    },
];

/// The agent by that id, if it is one this build knows.
pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|one| one.id == id)
}

/// The interpreters whose first path-shaped argument is the real program.
const INTERPRETERS: &[&str] = &["node", "bun", "deno", "python", "python3", "ruby", "perl"];

/// Options that swallow the argument after them, so it is not the program.
const TAKES_A_VALUE: &[&str] = &["-r", "--require", "--import", "--loader", "-m"];

/// Options whose value *is* the program, so there is no script to find.
const CARRIES_THE_SOURCE: &[&str] = &["-e", "--eval", "-p", "--print", "-c"];

/// What this argument vector is running, as a program name.
///
/// The interpreter is stepped over; anything else is taken at face value.
/// `None` when an interpreter was given source on the command line, because
/// then there is no program with a name.
pub fn program_of(argv: &[String]) -> Option<String> {
    let first = argv.first()?;
    let head = first.strip_prefix('-').unwrap_or(first);
    let name = basename(head).to_ascii_lowercase();
    if !INTERPRETERS.contains(&name.as_str()) {
        return Some(name);
    }

    let mut at = 1;
    while at < argv.len() {
        let arg = &argv[at];
        if arg == "--" {
            at += 1;
            continue;
        }
        if arg.starts_with('-') {
            let flag = arg.split('=').next().unwrap_or(arg);
            if CARRIES_THE_SOURCE.contains(&flag) {
                return None;
            }
            if TAKES_A_VALUE.contains(&flag) && flag == arg {
                at += 1;
            }
            at += 1;
            continue;
        }
        // A bare word here is an argument to the interpreter's script, or a
        // prompt. Only something path-shaped is the script itself.
        if arg.contains('/') || arg.contains('\\') {
            let script = basename(arg).to_ascii_lowercase();
            return Some(strip_script_extension(&script).to_owned());
        }
        return Some(name);
    }
    Some(name)
}

/// `claude.js` is `claude`. The extension is the interpreter's, not the name's.
fn strip_script_extension(name: &str) -> &str {
    for suffix in [".js", ".mjs", ".cjs", ".py", ".rb", ".pl"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return stem;
        }
    }
    name
}

/// The agent this argument vector is, if it is one.
pub fn recognise(argv: &[String]) -> Option<&'static Known> {
    let program = program_of(argv)?;
    KNOWN.iter().find(|one| {
        one.wears.contains(&program.as_str())
            // Codex ships a per-platform binary named after its target, and
            // Grok does the same. A prefix is enough and is not ambiguous:
            // nothing else here starts with either word.
            || (one.id == "codex" && program.starts_with("codex-"))
            || (one.id == "grok" && program.starts_with("grok-"))
    })
}

/// The shells a pane can be sitting at.
///
/// Anything else in the foreground is a program the person started, which is
/// what the row exists to show.
pub const SHELLS: &[&str] = &["bash", "zsh", "fish", "sh", "dash", "ksh", "csh", "tcsh"];

/// Whether the pane is merely sitting at a prompt.
pub fn idle_shell(program: &str) -> bool {
    SHELLS.contains(&program) || windows_shell(program)
}

/// PowerShell and `cmd`, the shells a Windows pane sits at, however psmux
/// spells them.
#[cfg(windows)]
fn windows_shell(program: &str) -> bool {
    crate::pwsh::is_powershell(program)
        || program.eq_ignore_ascii_case("cmd")
        || program.eq_ignore_ascii_case("cmd.exe")
}

#[cfg(not(windows))]
fn windows_shell(_program: &str) -> bool {
    false
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
