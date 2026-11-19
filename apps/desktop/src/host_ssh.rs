//! ssh to a host, from its profile. The person's own ssh does the work: its
//! config, agent and known hosts. devpit adds one connection per host, kept
//! open a while, and never a shell line it did not quote.

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use devpit_core::HostRow;

/// How long ssh waits for a host before it gives up on it.
const CONNECT_SECONDS: u32 = 10;
/// How long a host's connection stays open after its last command.
const PERSIST_SECONDS: u32 = 600;
/// How long a command waits for a host, past ssh's own connect timeout.
pub(crate) const PATIENCE: Duration = Duration::from_secs(25);

/// Reads the folder in `$1` into `p`, `~` as the login's home: the folder
/// arrives as an argument, never as part of a script.
pub(crate) const FOLDER_IN_P: &str =
    "p=$1; case $p in \"~\") p=$HOME ;; \"~/\"*) p=$HOME/${p#\"~/\"} ;; esac;";

/// Where the control sockets live: under devpit's home, short enough for a
/// unix socket's path once ssh adds its 40-character hash.
pub(crate) fn sockets() -> Result<PathBuf, String> {
    let dir = devpit_core::Store::root()
        .map_err(|err| err.to_string())?
        .join("ssh");
    std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// The longest control socket path ssh can bind, with the hash it adds and
/// the suffix it binds under first: macOS's limit, the lower of the two.
const SOCKET_MOST: usize = 104 - 1 - 40 - 18;

/// The control options for `sockets`: one shared connection per host, or none
/// when ssh could not take the path.
fn control(sockets: &Path) -> Vec<String> {
    let dir = sockets.display().to_string();
    let usable = dir.len() <= SOCKET_MOST
        && !dir
            .chars()
            .any(|ch| ch.is_whitespace() || "%\"'".contains(ch));
    match usable {
        true => vec![
            "ControlMaster=auto".to_owned(),
            format!("ControlPath={dir}/%C"),
            format!("ControlPersist={PERSIST_SECONDS}"),
        ],
        false => vec!["ControlMaster=no".to_owned(), "ControlPath=none".to_owned()],
    }
}

/// The argv that runs `remote` on `host` over its shared connection. Its
/// options come from the profile alone, and `--` ends them before the target.
/// Nothing of the person's agent, display or forwards goes to the host, and a
/// `RemoteCommand` in the person's config never replaces devpit's.
pub(crate) fn argv(host: &HostRow, sockets: &Path, remote: &[&str]) -> Vec<String> {
    let mut argv = via(host, sockets, false);
    argv.push(line(remote));
    argv
}

/// Everything before the line: what runs one line on `host`.
pub(crate) fn via(host: &HostRow, sockets: &Path, tty: bool) -> Vec<String> {
    let mut argv = vec!["ssh".to_owned()];
    if tty {
        argv.push("-t".to_owned());
    }
    let options = [
        "BatchMode=yes".to_owned(),
        format!("ConnectTimeout={CONNECT_SECONDS}"),
        "ServerAliveInterval=15".to_owned(),
        "ServerAliveCountMax=3".to_owned(),
        "ForwardAgent=no".to_owned(),
        "ForwardX11=no".to_owned(),
        "ClearAllForwardings=yes".to_owned(),
        "RemoteCommand=none".to_owned(),
    ];
    for option in options.into_iter().chain(control(sockets)) {
        argv.push("-o".to_owned());
        argv.push(option);
    }
    if let Some(port) = host.port {
        argv.extend(["-p".to_owned(), port.to_string()]);
    }
    if let Some(user) = &host.user {
        argv.extend(["-l".to_owned(), user.clone()]);
    }
    if let Some(file) = &host.identity_file {
        argv.extend(["-i".to_owned(), file.clone()]);
        argv.extend(["-o".to_owned(), "IdentitiesOnly=yes".to_owned()]);
    }
    argv.extend(["--".to_owned(), host.target.clone()]);
    argv
}

/// `words` as the one line ssh hands the host's shell: each word quoted, so
/// a POSIX shell gives it back as it was.
pub(crate) fn line(words: &[&str]) -> String {
    devpit_tmux::remote::shell_line(words)
}

/// Runs `argv` with `envs` added, and gives up on it after `patience`: a
/// host that hangs past ssh's own timeout ends here, with what it started.
/// Its output is read as it comes, so a long one never stalls it.
pub(crate) fn run(
    argv: &[String],
    envs: &[(&str, &str)],
    patience: Duration,
) -> Result<Output, String> {
    let (program, args) = argv.split_first().ok_or("nothing to run")?;
    let mut command = devpit_pty::host_env::command(Path::new(program));
    command
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let child = devpit_tmux::remote::spawned(&mut command).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!("{program} is not installed on this computer"),
        _ => err.to_string(),
    })?;
    let output = devpit_tmux::remote::within_bounded(child, patience)
        .ok_or_else(|| format!("no answer within {} seconds", patience.as_secs().max(1)))?;
    output.map_err(|err| err.to_string())
}

/// What ssh exits with when it, not the command, failed.
pub(crate) const SSH_FAILED: i32 = 255;

/// The last line said, which is where ssh and a shell put their reason.
pub(crate) fn last_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .rfind(|one| !one.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
#[path = "host_ssh_tests.rs"]
mod tests;
