//! The settings file that points the CLI's hooks at us.
//!
//! Written next to the state rather than into a temp file: a turn that
//! outlives the app still has a file to read, and a path that changed every
//! run would leave one on disk per card moved.

use std::path::Path;

/// devpit's own read tools, allowed without a question: they only read the
/// board the agent is working on. Writing still asks.
pub const ALLOWED_TOOLS: &[&str] = &[
    "mcp__devpit__devpit_context",
    "mcp__devpit__devpit_board",
    "mcp__devpit__devpit_card",
];

/// The settings a turn is launched with, so its hooks reach us.
pub fn settings_json(endpoint_file: &Path, auth_file: &Path) -> String {
    let allowed = format!(
        r#""permissions":{{"allow":{}}}"#,
        serde_json::to_string(ALLOWED_TOOLS).unwrap_or_else(|_| "[]".into())
    );
    // Said to every hook this session runs, so the plugin's copy of the same
    // hooks (`plugin_hooks_json`) knows these already report and stays quiet.
    let marked = format!(r#""env":{{"{HOOKED}":"1"}}"#);
    // A session devpit starts hears the account's other sessions whatever
    // mode either runs in. Without it a bypassing session held every message
    // from one that asks, and the reverse, until someone approved it by hand.
    // A message still approves nothing: that rule is the CLI's and stays.
    let inbound = r#""crossSessionInbound":"accept""#;
    format!(
        "{{\"hooks\":{},{allowed},{marked},{inbound}}}",
        hooks(endpoint_file, auth_file, "")
    )
}

/// [`settings_json`] with the advisor model a profile's sessions start with.
pub fn settings_json_advised(
    endpoint_file: &Path,
    auth_file: &Path,
    advisor: Option<&str>,
) -> String {
    let plain = settings_json(endpoint_file, auth_file);
    match advisor.and_then(|model| serde_json::to_string(model).ok()) {
        Some(model) => format!("{},\"advisorModel\":{model}}}", &plain[..plain.len() - 1]),
        None => plain,
    }
}

/// The same hooks, as the devpit plugin for Claude Code carries them.
///
/// A plugin reaches every session of the installation it is installed in —
/// one typed by hand, one started through the person's own shell function —
/// which the flags on a launch line cannot. So each hook first checks that it
/// runs inside a devpit terminal, and that the session was not also launched
/// with the flags: two copies of a hook would report every event twice.
pub fn plugin_hooks_json(endpoint_file: &Path, auth_file: &Path) -> String {
    #[cfg(not(windows))]
    let guard = format!(
        "[ -n \"${pane}\" ] && [ -z \"${HOOKED}\" ] || exit 0; ",
        pane = devpit_tmux_pane_env()
    );
    // `devpit hook` makes the same check itself: there is no shell to ask.
    #[cfg(windows)]
    let guard = " --plugin".to_owned();
    format!("{{\"hooks\":{}}}", hooks(endpoint_file, auth_file, &guard))
}

/// Set in the environment of a session launched with devpit's settings.
pub const HOOKED: &str = "DEVPIT_HOOKED";

/// The events devpit hears, each running `guard` and then its post.
///
/// `curl` rather than a helper binary: it is already on the machine, and a
/// helper would be one more thing to ship, find and keep in step.
///
/// The timeouts are the point. This runs before every tool call the agent
/// makes, so an app that has gone away has to cost it a second and a half,
/// not a hang. `--noproxy` because a proxy in the environment must not be
/// consulted for a loopback address.
fn hooks(endpoint_file: &Path, auth_file: &Path, guard: &str) -> String {
    let file = endpoint_file.display().to_string();
    let auth = auth_file.display().to_string();

    // Told and forgotten. The reply is discarded and the budget is short,
    // because these only report what happened and the agent is waiting.
    let tell = guarded(guard, &post(&file, &auth, "1.5", false, None));

    // `PreToolUse` is the one that can be answered, so its reply is printed:
    // the app either sends back a decision or sends back nothing, and nothing
    // leaves the CLI's own permission mode in charge. The budget is long
    // because on this one the answer is a person.
    let consult = guarded(guard, &post(&file, &auth, "125", true, None));

    let hooks: Vec<String> = [
        ("PreToolUse", &consult),
        // Fired only when the CLI is about to ask, so it is the one to answer
        // from somewhere else; held by the app only while a person can see the
        // question there, and answered with nothing otherwise.
        ("PermissionRequest", &consult),
        ("PostToolUse", &tell),
        // A tool that failed, which sends no `PostToolUse`.
        ("PostToolUseFailure", &tell),
        // A turn begins here, and a turn that only answers in text says
        // nothing else before its `Stop`.
        ("UserPromptSubmit", &tell),
        ("Stop", &tell),
        // The end of a turn an error cut short, which sends no `Stop`.
        ("StopFailure", &tell),
        ("SubagentStart", &tell),
        ("SubagentStop", &tell),
        ("Notification", &tell),
        // Which session a pane's agent is in, and when it has gone, so a pane
        // tmux lost can start it again on the same conversation.
        ("SessionStart", &tell),
        ("SessionEnd", &tell),
    ]
    .iter()
    .map(|(event, command)| {
        format!(
            "\"{event}\":[{{\"matcher\":\"*\",\"hooks\":[{{\"type\":\"command\",\
             \"command\":{}}}]}}]",
            serde_json::to_string(command).unwrap_or_else(|_| "\"true\"".to_owned())
        )
    })
    .collect();
    format!("{{{}}}", hooks.join(","))
}

/// A hook's line with its guard: a shell test before it, or on Windows a
/// flag after it.
#[cfg(not(windows))]
fn guarded(guard: &str, post: &str) -> String {
    format!("{guard}{post}")
}

#[cfg(windows)]
fn guarded(guard: &str, post: &str) -> String {
    format!("{post}{guard}")
}

/// The shell one hook runs.
///
/// The endpoint is read from disk on every invocation, so a session that
/// outlived a restart posts to the new port instead of into a dead one. It
/// always exits 0: a hook that fails must not fail the turn it is reporting
/// on.
///
/// The pane rides in the query string when the terminal set one. It is the
/// only thing joining an agent's own reports to the pane somebody is looking
/// at — the hook runs three processes below the shell, and the environment is
/// what reaches that far. `${VAR:+?pane=$VAR}` is POSIX and expands to
/// nothing at all when the agent was not started in one of our terminals, so
/// a headless turn posts exactly the URL it always did.
///
/// `from` names an agent other than Claude Code, whose reports the listener
/// puts into Claude Code's words (`gemini`).
#[cfg(not(windows))]
pub(crate) fn post(
    endpoint_file: &str,
    auth_file: &str,
    seconds: &str,
    echo: bool,
    from: Option<&str>,
) -> String {
    let sink = if echo { "" } else { " >/dev/null" };
    let pane = devpit_tmux_pane_env();
    let query = match from {
        None => format!("${{{pane}:+?pane=${pane}}}"),
        Some(agent) => format!("?from={agent}${{{pane}:+&pane=${pane}}}"),
    };
    // An empty header is one curl does not send: outside a chat, none arrives.
    let chat = format!(
        "-H \"{}: ${}\"",
        crate::hooks::CHAT_HEADER,
        crate::hooks::CHAT_ENV
    );
    format!(
        "E=$(cat {endpoint_file} 2>/dev/null) && [ -n \"$E\" ] && \
         curl -sS -X POST --noproxy '*' --connect-timeout 0.5 --max-time {seconds} \
         -H 'content-type: application/json' -H @'{auth_file}' {chat} --data-binary @- \
         \"$E{query}\"{sink} 2>/dev/null || true"
    )
}

/// On Windows the hook is devpit itself, `devpit hook`: there is no `cat`,
/// `curl` or POSIX shell to count on, and the binary is already installed. It
/// reads the endpoint and the secret from the same files, and adds the pane.
#[cfg(windows)]
pub(crate) fn post(
    _endpoint_file: &str,
    _auth_file: &str,
    seconds: &str,
    echo: bool,
    from: Option<&str>,
) -> String {
    // This process is devpit: the one binary that knows where its state is.
    let exe = std::env::current_exe()
        .map(|exe| exe.display().to_string())
        .unwrap_or_else(|_| "devpit".to_owned());
    let line = hook_line(&exe, seconds, echo);
    match from {
        Some(agent) => format!("{line} --from {agent}"),
        None => line,
    }
}

/// `<exe> hook --wait <seconds> [--echo]`, read alike by cmd, Git Bash and
/// PowerShell: forward slashes, and quotes only when a space needs them.
#[cfg(windows)]
pub(crate) fn hook_line(exe: &str, seconds: &str, echo: bool) -> String {
    let exe = exe.replace('\\', "/");
    let exe = if exe.contains(' ') {
        format!("\"{exe}\"")
    } else {
        exe
    };
    let echo = if echo { " --echo" } else { "" };
    format!("{exe} hook --wait {seconds}{echo}")
}

/// The variable a devpit terminal puts the pane's name in.
///
/// Named here rather than imported: this crate is the boundary around the
/// agent CLI and does not depend on the terminal one. The name is checked
/// against it by `the_pane_name_matches_what_the_terminal_sets`.
#[cfg(not(windows))]
fn devpit_tmux_pane_env() -> &'static str {
    "DEVPIT_PANE"
}

#[cfg(test)]
#[path = "hook_settings_tests.rs"]
mod tests;
