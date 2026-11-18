//! The devflow binary: downloaded from its GitHub release and checked against
//! the release's checksums.txt, as its own install.sh does, into
//! `~/.local/bin`. devpit's Devflow hands it the calls it serves
//! (`devflow_host`); registering it with the agent CLIs is left to
//! `devflow register`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use devpit_core::Store;
use devpit_rpc::{DevflowBinary, ErrorCode, RpcError};
use sha2::{Digest, Sha256};

use crate::devflow_plugin::KNOWN;

pub(crate) const RELEASES: &str = "https://github.com/jholhewres/devflow/releases/download";

/// Far above the 0.2.1 archives (about 3 MB) and their checksums.
const MOST_ARCHIVE: u64 = 64 * 1024 * 1024;
const MOST_SUMS: u64 = 64 * 1024;

/// What `--version` and `doctor` are given; doctor looks at every CLI.
const RUN_WAITS: Duration = Duration::from_secs(30);
const MOST_DOCTOR_LINES: usize = 40;

/// The release's name for this machine, from Rust's `OS` and `ARCH`.
pub(crate) fn platform(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    Some(format!("{os}_{arch}"))
}

pub(crate) fn archive(version: &str, platform: &str) -> String {
    format!("devflow_{version}_{platform}.tar.gz")
}

/// The checksum `sums` lists for `archive`, in sha256sum's format, binary
/// mode's `*` included.
pub(crate) fn sum_for(sums: &str, archive: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        let sum = words.next()?;
        let name = words.next()?;
        (name.strip_prefix('*').unwrap_or(name) == archive).then(|| sum.to_ascii_lowercase())
    })
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn refused(why: String) -> RpcError {
    RpcError::new(ErrorCode::Invalid, why)
}

/// `url`'s body, refused past `most` bytes.
async fn fetch(client: &reqwest::Client, url: &str, most: u64) -> Result<Vec<u8>, RpcError> {
    let unreachable = |err: reqwest::Error| {
        RpcError::new(
            ErrorCode::Conflict,
            format!("devflow could not be downloaded: is this machine online? ({err})"),
        )
    };
    let mut response = client.get(url).send().await.map_err(unreachable)?;
    if !response.status().is_success() {
        return Err(refused(format!(
            "devflow could not be downloaded: {url} answered {}",
            response.status()
        )));
    }
    let too_big = || refused(format!("{url} is larger than any devflow release file"));
    if response.content_length().is_some_and(|size| size > most) {
        return Err(too_big());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(unreachable)? {
        if body.len() as u64 + chunk.len() as u64 > most {
            return Err(too_big());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn io(err: std::io::Error) -> RpcError {
    RpcError::internal(err.to_string())
}

/// Downloads `version` for `platform` from `base`, checks it, and puts
/// `devflow` in `bin_dir`, replacing the one there only once the new one is
/// whole. `scratch` is made for the download and removed, whatever happens.
pub(crate) async fn install_from(
    base: &str,
    version: &str,
    platform: &str,
    bin_dir: &Path,
    scratch: &Path,
) -> Result<PathBuf, RpcError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("devpit/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| RpcError::internal(err.to_string()))?;
    let name = archive(version, platform);
    let at = format!("{base}/v{version}");
    let body = fetch(&client, &format!("{at}/{name}"), MOST_ARCHIVE).await?;
    let sums = fetch(&client, &format!("{at}/checksums.txt"), MOST_SUMS).await?;
    let want = sum_for(&String::from_utf8_lossy(&sums), &name)
        .ok_or_else(|| refused(format!("checksums.txt does not list {name}")))?;
    if sha256_hex(&body) != want {
        return Err(refused(format!(
            "{name} does not match its checksum: refusing to install it"
        )));
    }
    let placed = place(&body, &name, bin_dir, scratch);
    let _ = std::fs::remove_dir_all(scratch);
    placed
}

/// The verified archive's `devflow`, unpacked in `scratch` and moved into `bin_dir`.
fn place(body: &[u8], name: &str, bin_dir: &Path, scratch: &Path) -> Result<PathBuf, RpcError> {
    std::fs::create_dir_all(scratch).map_err(io)?;
    let archive = scratch.join(name);
    std::fs::write(&archive, body).map_err(io)?;
    let mut tar = devpit_pty::host_env::command("tar");
    tar.arg("xzf")
        .arg(&archive)
        .arg("-C")
        .arg(scratch)
        .arg("devflow");
    let out = crate::claude_plugin::output_within(tar, RUN_WAITS)
        .map_err(|why| RpcError::internal(format!("`tar` {why}")))?;
    let unpacked = scratch.join("devflow");
    if !out.status.success() || !unpacked.is_file() {
        return Err(refused(format!("{name} holds no devflow")));
    }
    std::fs::create_dir_all(bin_dir).map_err(io)?;
    // Copied beside the target, then renamed: a running devflow keeps its
    // file, and nothing ever sees half a binary.
    let part = bin_dir.join(".devflow.part");
    std::fs::copy(&unpacked, &part).map_err(io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755)).map_err(io)?;
    }
    let target = bin_dir.join("devflow");
    std::fs::rename(&part, &target).map_err(io)?;
    Ok(target)
}

/// What `bin` prints for `args`, when it runs at all.
fn said(bin: &Path, args: &[&str]) -> Option<String> {
    let mut command = devpit_pty::host_env::command(bin);
    command.args(args);
    let out = crate::claude_plugin::output_within(command, RUN_WAITS).ok()?;
    // doctor exits 1 while something is left to fix, and still said it.
    out.status.code()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The binary at `bin`: its version, and `devflow doctor`'s lines when asked.
pub(crate) fn state_of(bin: &Path, doctor: bool) -> DevflowBinary {
    let version = bin
        .is_file()
        .then(|| said(bin, &["--version"]))
        .flatten()
        .and_then(|text| {
            text.split_whitespace()
                .nth(1)
                .map(|word| word.trim_start_matches('v').to_owned())
        });
    let doctor = match (&version, doctor) {
        (Some(_), true) => said(bin, &["doctor"])
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(MOST_DOCTOR_LINES)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    DevflowBinary {
        path: bin.display().to_string(),
        version,
        known: KNOWN.to_owned(),
        doctor,
    }
}

pub(crate) fn bin_dir() -> Result<PathBuf, RpcError> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local").join("bin"))
        .ok_or_else(|| RpcError::internal("HOME is not set"))
}

/// `devflow.binary` — the standalone devflow in `~/.local/bin`.
#[tauri::command]
#[specta::specta]
pub async fn devflow_binary() -> Result<DevflowBinary, RpcError> {
    crate::off_main::blocking(|| Ok(state_of(&bin_dir()?.join("devflow"), false))).await
}

/// `devflow.binary_install` — the person's click, after their yes: devflow
/// `KNOWN` downloaded, checked and installed, then `devflow doctor` run.
#[tauri::command]
#[specta::specta]
pub async fn devflow_binary_install() -> Result<DevflowBinary, RpcError> {
    let platform = platform(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        refused(format!(
            "devflow has no release for {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    })?;
    let scratch = Store::root()?
        .join("downloads")
        .join(format!("devflow-{}", ulid::Ulid::generate()));
    let bin = install_from(RELEASES, KNOWN, &platform, &bin_dir()?, &scratch).await?;
    crate::off_main::blocking(move || Ok(state_of(&bin, true))).await
}

#[cfg(all(test, unix))]
#[path = "devflow_binary_tests.rs"]
mod tests;
