//! devflow behind a process boundary: the `devflow` program found where the
//! person has it, its version checked, and one `devflow host` session per
//! call — JSON-RPC 2.0, a line each way on stdio — with devflow's own
//! requests for what only devpit has answered here while it works.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{mpsc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde_json::{json, Value};

/// The one protocol devpit speaks; devflow lists the ones it does.
pub(crate) const PROTOCOL: u64 = 1;

/// The first devflow that serves `host` in devpit's words.
pub(crate) const LEAST: &str = "0.5.0";

/// A run's answer is JSON a reviewer wrote into; far above any seen.
const MOST_LINE: u64 = 64 * 1024 * 1024;

/// What `version --json` is given: it reads nothing and reaches nothing.
const VERSION_WAITS: Duration = Duration::from_secs(10);

/// The tail of devflow's stderr kept for a failure's message.
const MOST_STDERR: usize = 4 * 1024;

/// Why devpit cannot hand devflow a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unusable {
    /// No `devflow` where devpit looks.
    Missing,
    /// One older than [`LEAST`], or one that does not speak [`PROTOCOL`].
    Old { version: String },
    /// One that did not say its version.
    Unreadable { why: String },
}

impl Unusable {
    /// What the person and the agent read: what is wrong and what to do.
    pub(crate) fn said(&self) -> String {
        let wanted = format!(
            "Devflow needs the devflow program, {LEAST} or later: install or update it \
             on the Devflow screen"
        );
        match self {
            Unusable::Missing => format!("{wanted}. devflow is not installed yet."),
            Unusable::Old { version } => format!("{wanted}. This one is {version}."),
            Unusable::Unreadable { why } => {
                format!("{wanted}. The devflow found did not say its version: {why}.")
            }
        }
    }
}

/// Where devpit looks for `devflow`, first first: the override a test or a
/// development build sets, the one devpit installs, then the person's PATH.
pub(crate) fn candidates(
    chosen: Option<OsString>,
    installed: Option<PathBuf>,
    search_path: &OsStr,
) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = chosen.into_iter().map(PathBuf::from).collect();
    all.extend(installed.filter(|bin| bin.is_file()));
    all.extend(devpit_pty::host_env::found_on(
        search_path,
        OsStr::new("devflow"),
    ));
    all
}

/// The version `version --json` said, when it is [`LEAST`] or later and
/// speaks [`PROTOCOL`].
pub(crate) fn usable(said: &str) -> Result<String, Unusable> {
    let unreadable = |why: &str| Unusable::Unreadable {
        why: why.to_owned(),
    };
    let said: Value = serde_json::from_str(said.trim()).map_err(|_| unreadable("not JSON"))?;
    let version = said["version"]
        .as_str()
        .ok_or_else(|| unreadable("no version"))?
        .to_owned();
    let parsed = semver::Version::parse(version.trim_start_matches('v'))
        .map_err(|_| unreadable("not a version"))?;
    let least = semver::Version::parse(LEAST).expect("LEAST is a version");
    let speaks = said["protocols"]
        .as_array()
        .is_some_and(|all| all.iter().any(|one| one.as_u64() == Some(PROTOCOL)));
    if parsed < least || !speaks {
        return Err(Unusable::Old { version });
    }
    Ok(version)
}

/// The devflow devpit hands its calls to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Devflow {
    pub path: PathBuf,
    pub version: String,
}

/// The first candidate that answers with a version devpit speaks; else why
/// the first one found cannot be used.
pub(crate) fn found_in(
    candidates: &[PathBuf],
    ask: &dyn Fn(&Path) -> Result<String, String>,
) -> Result<Devflow, Unusable> {
    let mut first = None;
    for path in candidates {
        let answer = ask(path)
            .map_err(|why| Unusable::Unreadable { why })
            .and_then(|said| usable(&said));
        match answer {
            Ok(version) => {
                return Ok(Devflow {
                    path: path.clone(),
                    version,
                })
            }
            Err(why) => {
                first.get_or_insert(why);
            }
        }
    }
    Err(first.unwrap_or(Unusable::Missing))
}

/// What `devflow version --json` prints.
fn version_of(bin: &Path) -> Result<String, String> {
    let mut command = devpit_pty::host_env::command(bin);
    command
        .args(["version", "--json"])
        .env("DEVFLOW_NO_UPDATE", "1");
    let out = crate::claude_plugin::output_within(command, VERSION_WAITS)?;
    if !out.status.success() {
        return Err(format!("it exited with {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(test))]
fn where_to_look() -> Vec<PathBuf> {
    let installed = crate::devflow_binary::bin_dir()
        .ok()
        .map(|dir| dir.join("devflow"));
    candidates(
        std::env::var_os("DEVPIT_DEVFLOW"),
        installed,
        devpit_pty::login_path::search_path(),
    )
}

// A test sees only the devflow it put there, never the machine's.
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_DEVFLOW: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn where_to_look() -> Vec<PathBuf> {
    TEST_DEVFLOW.with(|chosen| chosen.borrow().clone().into_iter().collect())
}

/// Each candidate with when it last changed: what was found is asked again
/// only once one of them is installed, updated or gone.
type Seen = Vec<(PathBuf, Option<SystemTime>)>;

fn seen_of(all: &[PathBuf]) -> Seen {
    all.iter()
        .map(|path| {
            let changed = std::fs::metadata(path).and_then(|meta| meta.modified());
            (path.clone(), changed.ok())
        })
        .collect()
}

/// The devflow on this machine, as devpit looks for it now.
pub(crate) fn devflow() -> Result<Devflow, Unusable> {
    static FOUND: Mutex<Option<(Seen, Result<Devflow, Unusable>)>> = Mutex::new(None);
    let all = where_to_look();
    let seen = seen_of(&all);
    let mut found = FOUND.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((before, answer)) = found.as_ref() {
        if *before == seen {
            return answer.clone();
        }
    }
    let answer = found_in(&all, &version_of);
    *found = Some((seen, answer.clone()));
    answer
}

/// A call devflow refused or could not make: its message is what the agent
/// or the person reads.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Failed {
    pub message: String,
    pub data: Option<Value>,
    /// devflow's JSON-RPC code; `None` when devpit could not reach it.
    pub code: Option<i64>,
}

impl Failed {
    fn said(message: impl Into<String>) -> Failed {
        Failed {
            message: message.into(),
            data: None,
            code: None,
        }
    }
}

/// devflow's code for a method it does not serve: one a later release adds.
pub(crate) const NOT_SERVED: i64 = -32601;

/// devpit's answer to one of devflow's requests, or why it has none.
pub(crate) type Answering<'a> = &'a dyn Fn(&str, Value) -> Result<Value, String>;

/// One line, up to `most` bytes; `None` at the end of the stream.
fn line_of(reader: &mut dyn BufRead, most: u64) -> Result<Option<Vec<u8>>, Failed> {
    let mut line = Vec::new();
    let read = reader
        .take(most + 1)
        .read_until(b'\n', &mut line)
        .map_err(|err| Failed::said(format!("devflow could not be read: {err}")))?;
    if read == 0 {
        return Ok(None);
    }
    if line.last() != Some(&b'\n') && read as u64 > most {
        return Err(Failed::said(format!(
            "devflow sent a line over {most} bytes"
        )));
    }
    Ok(Some(line))
}

fn sent(writer: &mut dyn Write, message: &Value) -> Result<(), Failed> {
    let mut line = serde_json::to_vec(message).expect("a Value serialises");
    line.push(b'\n');
    writer
        .write_all(&line)
        .and_then(|()| writer.flush())
        .map_err(|err| Failed::said(format!("devflow stopped listening: {err}")))
}

/// Sends `method` as request `id` and reads until its answer, answering each
/// request devflow makes meanwhile with `host`.
pub(crate) fn asked(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    id: u64,
    method: &str,
    params: Value,
    host: Answering,
    most: u64,
) -> Result<Value, Failed> {
    sent(
        writer,
        &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
    )?;
    loop {
        let Some(line) = line_of(reader, most)? else {
            return Err(Failed::said(format!(
                "devflow ended before it answered {method}"
            )));
        };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let message: Value = serde_json::from_slice(&line)
            .map_err(|_| Failed::said("devflow sent a line that is not JSON"))?;
        match (
            message.get("method").and_then(Value::as_str),
            message.get("id"),
        ) {
            // A request of devflow's own: answered before it goes on.
            (Some(asked), Some(their)) => {
                let params = message.get("params").cloned().unwrap_or(Value::Null);
                let reply = match host(asked, params) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": their, "result": result }),
                    Err(why) => json!({
                        "jsonrpc": "2.0",
                        "id": their,
                        "error": { "code": -32000, "message": why },
                    }),
                };
                sent(writer, &reply)?;
            }
            // A notice: devpit keeps none yet.
            (Some(_), None) => {}
            (None, Some(their)) if their.as_u64() == Some(id) => {
                if let Some(error) = message.get("error") {
                    return Err(Failed {
                        message: error["message"]
                            .as_str()
                            .unwrap_or("devflow refused without saying why")
                            .to_owned(),
                        data: error.get("data").cloned(),
                        code: error["code"].as_i64(),
                    });
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            _ => {}
        }
    }
}

/// `initialize`, then `method`, over one stream: what each answered.
pub(crate) fn exchange(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    initialize: Value,
    method: &str,
    params: Value,
    host: Answering,
) -> Result<(Value, Value), Failed> {
    let initialized = asked(reader, writer, 1, "initialize", initialize, host, MOST_LINE)?;
    let answer = asked(reader, writer, 2, method, params, host, MOST_LINE)?;
    Ok((initialized, answer))
}

/// The last of what a reader gives, up to `most` bytes, on a thread of its
/// own so a full pipe never stops devflow.
fn tail_of(mut pipe: impl Read + Send + 'static, most: usize) -> mpsc::Receiver<String> {
    let (send, heard) = mpsc::channel();
    std::thread::spawn(move || {
        let mut all = Vec::new();
        let mut chunk = [0u8; 8192];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 {
                break;
            }
            all.extend_from_slice(&chunk[..read]);
            if all.len() > most * 2 {
                all.drain(..all.len() - most);
            }
        }
        let start = all.len().saturating_sub(most);
        let _ = send.send(String::from_utf8_lossy(&all[start..]).trim().to_owned());
    });
    heard
}

/// Ends `child` when `done` says nothing by `waits`.
fn watched(mut child: Child, waits: Duration) -> (mpsc::Sender<()>, mpsc::Receiver<bool>) {
    let (done, finished) = mpsc::channel::<()>();
    let (said, late) = mpsc::channel();
    std::thread::spawn(move || {
        let timed_out = matches!(
            finished.recv_timeout(waits),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        devpit_steps::descendants::end_it_all(&mut child);
        let _ = said.send(timed_out);
    });
    (done, late)
}

/// One call to `devflow host` in `root`: spawned, initialised, asked, ended.
pub(crate) fn call(
    devflow: &Devflow,
    root: &Path,
    initialize: Value,
    method: &str,
    params: Value,
    host: Answering,
    waits: Duration,
) -> Result<Value, Failed> {
    let mut command = devpit_pty::host_env::command(&devflow.path);
    command
        .arg("host")
        .arg("--root")
        .arg(root)
        .current_dir(root)
        .env("DEVFLOW_NO_UPDATE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    devpit_steps::descendants::in_a_session_of_its_own(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| Failed::said(format!("devflow could not be run: {err}")))?;
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(Failed::said("devflow's pipes could not be opened"));
    };
    let errors = tail_of(stderr, MOST_STDERR);
    let (done, late) = watched(child, waits);
    let mut reader = BufReader::new(stdout);
    let answer = exchange(&mut reader, &mut stdin, initialize, method, params, host).map(
        |(initialized, answer)| {
            served_by(devflow, &initialized);
            answer
        },
    );
    drop(stdin);
    let _ = done.send(());
    let timed_out = late.recv().unwrap_or(false);
    answer.map_err(|failed| {
        let said = errors
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_default();
        match (timed_out, said.is_empty()) {
            (true, _) => Failed::said(format!(
                "devflow did not answer {method} in {} s",
                waits.as_secs()
            )),
            (false, false) if failed.code.is_none() => {
                Failed::said(format!("{}: {said}", failed.message))
            }
            _ => failed,
        }
    })
}

/// The methods each devflow said it serves at `initialize`, by binary and
/// version, so a method it lacks costs no spawn.
static SERVED: Mutex<Vec<(Devflow, Vec<String>)>> = Mutex::new(Vec::new());

fn served_by(devflow: &Devflow, initialized: &Value) {
    let methods: Vec<String> = initialized["methods"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let mut served = SERVED.lock().unwrap_or_else(PoisonError::into_inner);
    served.retain(|(one, _)| one.path != devflow.path);
    served.push((devflow.clone(), methods));
}

/// Whether `devflow` is known not to serve `method`; unknown until it was
/// asked once.
pub(crate) fn lacks(devflow: &Devflow, method: &str) -> bool {
    let served = SERVED.lock().unwrap_or_else(PoisonError::into_inner);
    served
        .iter()
        .find(|(one, _)| one == devflow)
        .is_some_and(|(_, methods)| !methods.iter().any(|one| one == method))
}

/// devflow's answer to `method` in `root` when a usable devflow serves it;
/// `None` when devpit answers on its own: no devflow here, or one that does
/// not serve the method yet.
pub(crate) fn asked_of(
    root: &Path,
    initialize: Value,
    method: &str,
    params: Value,
    host: Answering,
    waits: Duration,
) -> Option<Result<Value, Failed>> {
    let devflow = devflow().ok()?;
    if lacks(&devflow, method) {
        return None;
    }
    match call(&devflow, root, initialize, method, params, host, waits) {
        Err(failed) if failed.code == Some(NOT_SERVED) => None,
        answer => Some(answer),
    }
}

/// How often devpit asks devflow to update itself.
const UPDATE_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// What `devflow update` is given: a download and a swap.
const UPDATE_WAITS: Duration = Duration::from_secs(180);

/// What `devflow update --json` said changed, when it moved to another version.
pub(crate) fn updated(said: &str) -> Option<(String, String)> {
    let said: Value = serde_json::from_str(said.lines().last()?.trim()).ok()?;
    (said["updated"] == true).then(|| {
        (
            said["from"].as_str().unwrap_or_default().to_owned(),
            said["to"].as_str().unwrap_or_default().to_owned(),
        )
    })
}

/// Keeps devflow here for a project with Devflow on, in the background:
/// installed once when missing or too old, and asked to update itself at
/// most once a day. Each says so on the bell. `DEVFLOW_NO_UPDATE=1` does
/// neither.
pub(crate) fn tend(app: &tauri::AppHandle) {
    static INSTALLING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static LOOKED: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    // The person's way to keep devflow as it is, and a test's.
    if std::env::var_os("DEVFLOW_NO_UPDATE").is_some_and(|said| said == "1") {
        return;
    }
    let app = app.clone();
    match devflow() {
        // Another program called devflow is not replaced.
        Err(Unusable::Unreadable { .. }) => {}
        Err(unusable) if !INSTALLING.swap(true, std::sync::atomic::Ordering::SeqCst) => {
            tauri::async_runtime::spawn(async move {
                let (title, detail) = match install().await {
                    Ok(version) => (
                        "devflow installed".to_owned(),
                        format!("devpit installed devflow {version}, which Devflow runs on."),
                    ),
                    Err(why) => (
                        "devflow could not be installed".to_owned(),
                        format!("{} Installing it failed: {why}", unusable.said()),
                    ),
                };
                let kind = crate::notices::kind::DEVFLOW;
                crate::notices::ring(&app, None, kind, &title, Some(&detail), None);
            });
        }
        Ok(devflow) => {
            let mut looked = LOOKED.lock().unwrap_or_else(PoisonError::into_inner);
            if looked.is_some_and(|at| at.elapsed() < UPDATE_EVERY) {
                return;
            }
            *looked = Some(std::time::Instant::now());
            std::thread::spawn(move || {
                let mut command = devpit_pty::host_env::command(&devflow.path);
                command.args(["update", "--json"]);
                let Ok(out) = crate::claude_plugin::output_within(command, UPDATE_WAITS) else {
                    return;
                };
                if let Some((from, to)) = updated(&String::from_utf8_lossy(&out.stdout)) {
                    let detail = format!("devflow moved from {from} to {to}.");
                    let kind = crate::notices::kind::DEVFLOW;
                    crate::notices::ring(&app, None, kind, "devflow updated", Some(&detail), None);
                }
            });
        }
        Err(_) => {}
    }
}

/// devflow [`LEAST`] put where devpit installs it; the version installed.
async fn install() -> Result<String, String> {
    use crate::devflow_binary as binary;
    let platform = binary::platform(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or("devflow has no release for this machine")?;
    let scratch = devpit_core::Store::root()
        .map_err(|err| err.to_string())?
        .join("downloads")
        .join(format!("devflow-{}", ulid::Ulid::generate()));
    let bin_dir = binary::bin_dir().map_err(|err| err.message)?;
    binary::install_from(binary::RELEASES, LEAST, &platform, &bin_dir, &scratch)
        .await
        .map_err(|err| err.message)?;
    Ok(LEAST.to_owned())
}

#[cfg(test)]
#[path = "devflow_host_tests.rs"]
mod tests;
