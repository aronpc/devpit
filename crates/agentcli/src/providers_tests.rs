use std::ffi::OsString;

use super::*;
use devpit_rpc::Credentials;

fn none(_: &str) -> Option<String> {
    None
}

fn row<'a>(rows: &'a [ProviderFound], id: &str) -> &'a ProviderFound {
    rows.iter().find(|one| one.id == id).expect("listed")
}

/// A fake CLI on a fake `PATH`: a script that prints `said` for `--version`.
#[cfg(unix)]
fn fake(dir: &std::path::Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join(name);
    std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).expect("script");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[test]
fn the_six_providers_are_listed_in_order() {
    let home = tempfile::tempdir().expect("home");
    let rows = detect(&OsString::new(), home.path(), &none);
    let ids: Vec<&str> = rows.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(
        ids,
        ["claude", "codex", "opencode", "cursor", "grok", "agy"]
    );
}

#[cfg(unix)]
#[test]
fn installed_clis_are_found_on_the_path_with_their_versions() {
    let home = tempfile::tempdir().expect("home");
    let bin = tempfile::tempdir().expect("bin");
    fake(bin.path(), "claude", "echo '2.1.325 (Claude Code)'");
    fake(bin.path(), "codex", "echo 'codex-cli 0.160.0'");
    fake(bin.path(), "opencode", "echo 1.17.4");
    let rows = detect(bin.path().as_os_str(), home.path(), &none);

    for (id, version) in [
        ("claude", "2.1.325"),
        ("codex", "0.160.0"),
        ("opencode", "1.17.4"),
    ] {
        let one = row(&rows, id);
        assert_eq!(one.version.as_deref(), Some(version), "{id}");
        assert_eq!(
            one.path.as_deref(),
            Some(bin.path().join(id).to_string_lossy().as_ref()),
            "{id}"
        );
    }
    assert_eq!(row(&rows, "grok").path, None);
}

#[test]
fn a_missing_cli_shows_its_install_command() {
    let home = tempfile::tempdir().expect("home");
    let empty = tempfile::tempdir().expect("empty");
    let rows = detect(empty.path().as_os_str(), home.path(), &none);
    for one in &rows {
        assert_eq!(one.path, None, "{}", one.id);
        assert_eq!(one.version, None, "{}", one.id);
        assert!(!one.install.is_empty(), "{} has no install line", one.id);
    }
    let codex = row(&rows, "codex");
    assert_eq!(codex.install, "npm install -g @openai/codex");
    assert_eq!(codex.login, "codex login");
    assert_eq!(row(&rows, "claude").login, "claude auth login");
}

#[test]
fn a_chat_is_offered_where_the_catalogue_has_a_driver() {
    let home = tempfile::tempdir().expect("home");
    let rows = detect(&OsString::new(), home.path(), &none);
    assert!(row(&rows, "claude").chat);
    for one in &rows {
        let driver = devpit_pty::agents::known(&one.id)
            .expect("in the catalogue")
            .driver;
        assert_eq!(one.chat, !driver.is_empty(), "{}", one.id);
    }
}

#[cfg(unix)]
#[test]
fn a_cli_that_never_answers_has_no_version() {
    let bin = tempfile::tempdir().expect("bin");
    fake(bin.path(), "codex", "sleep 30");
    let started = std::time::Instant::now();
    let said = version_of(
        &bin.path().join("codex"),
        bin.path().as_os_str(),
        Duration::from_millis(300),
    );
    assert_eq!(said, None);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_version_is_read_from_what_the_cli_prints() {
    assert_eq!(
        version_in("2.1.325 (Claude Code)").as_deref(),
        Some("2.1.325")
    );
    assert_eq!(
        version_in("codex-cli 0.160.0\n").as_deref(),
        Some("0.160.0")
    );
    assert_eq!(version_in("\n1.17.4\n").as_deref(), Some("1.17.4"));
    assert_eq!(
        version_in("2026.08.31-4057e58").as_deref(),
        Some("2026.08.31-4057e58")
    );
    assert_eq!(version_in("agy v1.5.0").as_deref(), Some("1.5.0"));
    assert_eq!(version_in("usage: tool [options]"), None);
    assert_eq!(version_in(""), None);
}

#[test]
fn codex_is_signed_in_where_its_home_holds_a_sign_in() {
    let home = tempfile::tempdir().expect("home");
    let other = tempfile::tempdir().expect("codex home");
    let codex = supported("codex").expect("codex").keeps;
    assert_eq!(sign_in(codex, home.path(), &none), Credentials::Missing);

    std::fs::create_dir_all(home.path().join(".codex")).expect("dir");
    std::fs::write(home.path().join(".codex/auth.json"), "{}").expect("auth");
    assert_eq!(sign_in(codex, home.path(), &none), Credentials::Saved);

    // A second profile's own CODEX_HOME is read, not the default one.
    let at = other.path().to_string_lossy().into_owned();
    let own = move |name: &str| (name == "CODEX_HOME").then(|| at.clone());
    assert_eq!(sign_in(codex, home.path(), &own), Credentials::Missing);
    std::fs::write(other.path().join("auth.json"), "{}").expect("auth");
    assert_eq!(sign_in(codex, home.path(), &own), Credentials::Saved);
}

#[test]
fn opencode_is_signed_in_once_it_holds_a_provider() {
    let home = tempfile::tempdir().expect("home");
    let opencode = supported("opencode").expect("opencode").keeps;
    assert_eq!(sign_in(opencode, home.path(), &none), Credentials::Missing);
    let dir = home.path().join(".local/share/opencode");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("auth.json"), "{}").expect("auth");
    assert_eq!(sign_in(opencode, home.path(), &none), Credentials::Missing);
    std::fs::write(dir.join("auth.json"), r#"{"one":{"type":"oauth"}}"#).expect("auth");
    assert_eq!(sign_in(opencode, home.path(), &none), Credentials::Saved);
}

#[test]
fn cursor_is_signed_in_when_its_config_names_the_account() {
    let home = tempfile::tempdir().expect("home");
    let cursor = supported("cursor").expect("cursor").keeps;
    assert_eq!(sign_in(cursor, home.path(), &none), Credentials::Missing);
    let dir = home.path().join(".cursor");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("cli-config.json"), r#"{"version":1}"#).expect("config");
    assert_eq!(sign_in(cursor, home.path(), &none), Credentials::Missing);
    std::fs::write(dir.join("cli-config.json"), r#"{"authInfo":{"userId":1}}"#).expect("config");
    assert_eq!(sign_in(cursor, home.path(), &none), Credentials::Saved);
}

#[test]
fn a_sign_in_kept_out_of_reach_is_unknown() {
    let home = tempfile::tempdir().expect("home");
    for id in ["grok", "agy"] {
        let keeps = supported(id).expect("listed").keeps;
        assert_eq!(
            sign_in(keeps, home.path(), &none),
            Credentials::Unknown,
            "{id}"
        );
    }
}

#[test]
fn the_login_runs_with_the_profiles_variables_and_without_its_arguments() {
    let runner = Runner {
        program: "codex".to_owned(),
        args: vec!["--full-auto".to_owned()],
        env: vec![("CODEX_HOME".to_owned(), "/home/me/.codex-work".to_owned())],
    };
    assert_eq!(
        login_line("codex", &runner).as_deref(),
        Some("CODEX_HOME='/home/me/.codex-work' codex login")
    );
    let plain = Runner {
        program: "claude".to_owned(),
        args: Vec::new(),
        env: Vec::new(),
    };
    assert_eq!(
        login_line("claude", &plain).as_deref(),
        Some("claude auth login")
    );
    assert_eq!(login_line("elsewhere", &plain), None);
}

#[cfg(windows)]
#[test]
fn a_cmd_shim_on_the_path_is_found() {
    let bin = tempfile::tempdir().expect("bin");
    // npm writes an extensionless sh script beside the .cmd, which Windows cannot run.
    std::fs::write(bin.path().join("codex"), "#!/bin/sh").expect("sh shim");
    std::fs::write(bin.path().join("codex.cmd"), "@echo codex-cli 0.160.0").expect("shim");
    assert_eq!(
        locate("codex", bin.path().as_os_str()),
        Some(bin.path().join("codex.cmd"))
    );
}

#[cfg(unix)]
#[test]
fn a_cli_under_the_home_folder_is_shown_from_it() {
    let home = tempfile::tempdir().expect("home");
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).expect("bin");
    fake(&bin, "opencode", "echo 1.17.4");
    let rows = detect(bin.as_os_str(), home.path(), &none);
    assert_eq!(
        row(&rows, "opencode").path.as_deref(),
        Some("~/.local/bin/opencode")
    );
}

#[test]
fn a_codex_home_that_does_not_exist_yet_is_made_before_codex_starts() {
    let root = tempfile::tempdir().expect("root");
    let work = root.path().join("codex-work");
    let claude = root.path().join("claude-work");
    let env = |name: &str, at: &std::path::Path| {
        vec![(name.to_owned(), at.to_string_lossy().into_owned())]
    };

    prepare("codex", &env("CODEX_HOME", &work)).expect("made");
    assert!(work.is_dir());
    prepare("claude", &env("CLAUDE_CONFIG_DIR", &claude)).expect("left alone");
    assert!(!claude.exists());
    prepare(
        "codex",
        &[("CODEX_HOME".to_owned(), "relative/home".to_owned())],
    )
    .expect("left alone");
    assert!(!std::path::Path::new("relative/home").exists());
}
