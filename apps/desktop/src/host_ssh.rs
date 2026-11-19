//! ssh to a host, from its profile. The person's own ssh does the work: its
//! config, agent and known hosts. devpit adds one connection per host, kept
//! open a while, and never a shell line it did not quote.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use devpit_core::HostRow;

/// How long ssh waits for a host before it gives up on it.
const CONNECT_SECONDS: u32 = 10;
/// How long a host's connection stays open after its last command.
const PERSIST_SECONDS: u32 = 600;

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

/// The argv that runs `remote` on `host` over its shared connection. Its
/// options come from the profile alone, and `--` ends them before the target.
pub(crate) fn argv(host: &HostRow, sockets: &Path, remote: &[&str]) -> Vec<String> {
    let mut argv: Vec<String> = ["ssh", "-o", "BatchMode=yes"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for option in [
        format!("ConnectTimeout={CONNECT_SECONDS}"),
        "ControlMaster=auto".to_owned(),
        format!("ControlPath={}/%C", sockets.display()),
        format!("ControlPersist={PERSIST_SECONDS}"),
    ] {
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
    argv.extend(["--".to_owned(), host.target.clone(), line(remote)]);
    argv
}

/// `words` as the one line ssh hands the host's shell: each word quoted, so
/// the shell gives it back as it was.
pub(crate) fn line(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| quoted(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A word in single quotes, a quote inside it closed and escaped. A word of
/// plain characters is left as it is, for a line a person can read.
pub(crate) fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "@%+=:,./_-".contains(ch));
    if plain {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// Runs `argv` with `envs` added, and gives up on it after `patience`: a
/// host that hangs past ssh's own timeout ends here. Its output is read as
/// it comes, so a long one never stalls it on a full pipe.
pub(crate) fn run(
    argv: &[String],
    envs: &[(&str, &str)],
    patience: Duration,
) -> Result<Output, String> {
    let (program, args) = argv.split_first().ok_or("nothing to run")?;
    let mut child = devpit_pty::host_env::command(Path::new(program))
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                format!("{program} is not installed on this computer")
            }
            _ => err.to_string(),
        })?;
    let drained = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = drained(
        child
            .stdout
            .take()
            .map(|one| Box::new(one) as Box<dyn Read + Send>),
    );
    let stderr = drained(
        child
            .stderr
            .take()
            .map(|one| Box::new(one) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + patience;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("no answer within {} seconds", patience.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

#[cfg(test)]
#[path = "host_ssh_tests.rs"]
mod tests;
