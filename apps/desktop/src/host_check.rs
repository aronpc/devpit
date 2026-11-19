//! Settings → Hosts → Check: whether a host can run devpit's terminals.
//!
//! One ssh call runs a script that only reads: which programs a login's
//! PATH finds, their versions, and whether an agent's sign-in file is there.
//! Nothing is written on the host.

use std::path::Path;
use std::time::Duration;

use devpit_core::{HostRow, Store};
use devpit_rpc::{ErrorCode, HostCheck, HostCheckRow, RpcError};

/// How long a check waits for the host, past ssh's own connect timeout.
const PATIENCE: Duration = Duration::from_secs(25);

/// Where an agent's sign-in lives on a host, for the agents that keep one in
/// a file. Any other installed agent is reported as found.
const SIGN_INS: &[(&str, &str)] = &[
    ("claude", ".claude/.credentials.json"),
    ("codex", ".codex/auth.json"),
];

/// The script, in POSIX sh with no single quote in it, so quoting it for the
/// host's shell is one pair of quotes. The PATH a non-interactive login gets
/// often lacks the per-user folders the agent CLIs install into.
fn script() -> String {
    let agents: Vec<&str> = devpit_pty::agents::KNOWN
        .iter()
        .map(|one| one.launch)
        .collect();
    let signed: String = SIGN_INS
        .iter()
        .map(|(cli, file)| format!("{cli}) [ -s \"$HOME/{file}\" ] && s=signed || s=unsigned ;; "))
        .collect();
    format!(
        "PATH=\"$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/bin:$PATH\"
first() {{ \"$@\" 2>/dev/null | {{ IFS= read -r l; printf \"%s\" \"$l\"; }}; }}
for t in tmux git; do
  if command -v \"$t\" >/dev/null 2>&1; then
    case $t in tmux) v=$(first tmux -V) ;; *) v=$(first git --version) ;; esac
    printf \"tool\\t%s\\tok\\t%s\\n\" \"$t\" \"$v\"
  else printf \"tool\\t%s\\tmissing\\t\\n\" \"$t\"; fi
done
for c in {agents}; do
  command -v \"$c\" >/dev/null 2>&1 || continue
  case $c in {signed}*) s=found ;; esac
  printf \"cli\\t%s\\t%s\\n\" \"$c\" \"$s\"
done
",
        agents = agents.join(" "),
    )
}

/// Checks `host` with the `ssh` program and `envs` given, waiting at most
/// `patience`.
pub(crate) fn checked_with(
    ssh: &Path,
    envs: &[(&str, &str)],
    patience: Duration,
    host: &HostRow,
    sockets: &Path,
) -> HostCheck {
    let script = script();
    let mut argv = crate::host_ssh::argv(host, sockets, &["sh", "-c", &script]);
    argv[0] = ssh.display().to_string();
    let reached = crate::host_ssh::run(&argv, envs, patience).and_then(|output| {
        let code = output.status.code().unwrap_or(-1);
        let why = crate::host_ssh::last_line(&output.stderr)
            .unwrap_or_else(|| format!("it ended with {code}"));
        match code {
            0 => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
            // ssh's own failures end with 255; anything else is the host's.
            crate::host_ssh::SSH_FAILED => Err(why),
            _ => Err(format!(
                "ssh got in, but the host's shell could not run the check: {why}"
            )),
        }
    });
    let said = match reached {
        Ok(said) => said,
        Err(why) => {
            return HostCheck {
                rows: vec![line("ssh", false, why)],
            }
        }
    };
    let mut rows = vec![line("ssh", true, format!("connected to {}", host.target))];
    for tool in ["tmux", "git"] {
        rows.push(tool_row(&said, tool));
    }
    rows.push(agent_row(&said));
    HostCheck { rows }
}

fn line(what: &str, ok: bool, said: String) -> HostCheckRow {
    HostCheckRow {
        what: what.to_owned(),
        ok,
        said,
    }
}

/// The fields of each line the script printed that starts with `kind`.
fn printed<'a>(said: &'a str, kind: &str) -> Vec<Vec<&'a str>> {
    said.lines()
        .map(|one| one.split('\t').collect::<Vec<_>>())
        .filter(|fields| fields.first() == Some(&kind))
        .map(|fields| fields[1..].to_vec())
        .collect()
}

fn tool_row(said: &str, tool: &str) -> HostCheckRow {
    let why = match tool {
        "tmux" => "devpit runs a host's terminals in tmux there",
        _ => "an agent works in a git checkout",
    };
    match printed(said, "tool")
        .into_iter()
        .find(|fields| fields.first() == Some(&tool))
    {
        Some(fields) if fields.get(1) == Some(&"ok") => {
            line(tool, true, fields.get(2).unwrap_or(&"").to_string())
        }
        _ => line(tool, false, format!("not installed: {why}")),
    }
}

fn agent_row(said: &str) -> HostCheckRow {
    let found = printed(said, "cli");
    if found.is_empty() {
        return line(
            "agent CLI",
            false,
            "no agent CLI found on a login's PATH".to_owned(),
        );
    }
    let mut ok = false;
    let named: Vec<String> = found
        .iter()
        .map(|fields| {
            let cli = fields.first().copied().unwrap_or_default();
            let state = match fields.get(1).copied() {
                Some("signed") => "signed in",
                Some("unsigned") => "no sign-in found",
                _ => "installed",
            };
            ok |= state != "no sign-in found";
            format!("{cli} ({state})")
        })
        .collect();
    line("agent CLI", ok, named.join(", "))
}

/// `host.check` — the host's check, over its own connection.
#[tauri::command]
#[specta::specta]
pub async fn host_check(id: String) -> Result<HostCheck, RpcError> {
    crate::off_main::blocking(move || {
        let host = Store::open_default()?
            .hosts()?
            .into_iter()
            .find(|one| one.id == id)
            .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "that host is no longer there"))?;
        let sockets = crate::host_ssh::sockets().map_err(RpcError::internal)?;
        Ok(checked_with(
            Path::new("ssh"),
            &[],
            PATIENCE,
            &host,
            &sockets,
        ))
    })
    .await
}

#[cfg(test)]
#[path = "host_check_tests.rs"]
mod tests;
