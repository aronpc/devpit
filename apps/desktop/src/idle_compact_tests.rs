use std::path::{Path, PathBuf};

use devpit_agentcli::talk::Say;
use devpit_rpc::{Part, Role};

use super::*;

const OVER: u32 = 212_000;

fn rule(when: IdleWhen) -> IdleCompact {
    IdleCompact {
        when,
        ..IdleCompact::default()
    }
}

/// A stand-in for the CLI that writes down what it was started with and told,
/// then ends its turn in the session it resumed.
#[cfg(unix)]
fn stand_in(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let cli = dir.join("fake-claude");
    std::fs::write(
        &cli,
        "#!/bin/sh\n\
         printf '%s\\n' \"$*\" > \"$(dirname \"$0\")/argv\"\n\
         read -r line\n\
         printf '%s\\n' \"$line\" > \"$(dirname \"$0\")/heard\"\n\
         echo '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s-1\"}'\n\
         echo '{\"type\":\"result\",\"subtype\":\"success\",\"session_id\":\"s-1\",\"total_cost_usd\":0.5}'\n",
    )
    .expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    cli
}

fn turn<'a>(command: &'a str, cwd: &'a Path) -> Say<'a> {
    Say {
        command,
        env: &[],
        prompt: "where were we?",
        cwd,
        model: None,
        budget_usd: None,
        session_id: Some("s-1"),
        fork_at: None,
        permission: None,
        disallowed: &[],
        settings: None,
        setting_sources: None,
        mcp_config: None,
        effort: None,
        control: None,
        on_session: None,
        add_dirs: &[],
        name: None,
    }
}

/// A script written a moment ago can be "text file busy" while another test
/// forks with it open; a spawn that fails reads as no compaction.
#[cfg(unix)]
fn returned(idle: &Idle, cli: &Path, dir: &Path) -> Option<Compacted> {
    let command = cli.to_string_lossy().into_owned();
    (0..5).find_map(|_| {
        let compacted = on_return(
            idle,
            None,
            &devpit_agentcli::driver::Claude::default(),
            &turn(&command, dir),
            &tauri::ipc::Channel::new(|_| Ok(())),
        );
        if compacted.is_none() && !dir.join("heard").exists() {
            std::thread::sleep(std::time::Duration::from_millis(50));
            return None;
        }
        Some(compacted)
    })?
}

#[test]
fn never_is_the_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = devpit_core::Store::open(&dir.path().join("devpit.db")).expect("store");

    assert_eq!(IdleCompact::default().when, IdleWhen::Never);
    assert_eq!(IdleCompact::default().tokens, 200_000);
    assert_eq!(rule_of(&store, "claude"), IdleCompact::default());
}

#[test]
fn a_rule_is_kept_for_its_own_profile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = devpit_core::Store::open(&dir.path().join("devpit.db")).expect("store");
    let chosen = IdleCompact {
        when: IdleWhen::Expiry,
        tokens: 150_000,
    };

    written(&store, "work", chosen).expect("written");

    assert_eq!(rule_of(&store, "work"), chosen);
    assert_eq!(rule_of(&store, "claude").when, IdleWhen::Never);
}

/// Back after the cache expired, over the threshold: `/compact` runs on the
/// resumed session before the person's turn, and the note says at how much.
#[cfg(unix)]
#[test]
fn a_conversation_over_the_threshold_is_compacted_when_the_person_returns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = stand_in(dir.path());
    let idle = Idle {
        rule: rule(IdleWhen::Return),
        used: Some(OVER),
        idle: 301.0,
        head: dir.path().join("conv.json"),
    };

    let compacted = returned(&idle, &cli, dir.path()).expect("compacted");

    let heard = std::fs::read_to_string(dir.path().join("heard")).expect("heard");
    assert!(heard.contains("\"/compact\""), "{heard}");
    let argv = std::fs::read_to_string(dir.path().join("argv")).expect("argv");
    assert!(argv.contains("--resume s-1"), "{argv}");
    assert_eq!(compacted.session_id.as_deref(), Some("s-1"));
    assert!(matches!(compacted.note.role, Role::System));
    assert_eq!(compacted.note.parts, [Part::Compacted { tokens: OVER }]);
}

#[cfg(unix)]
#[test]
fn a_conversation_back_before_its_cache_expired_is_left_as_it_is() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = stand_in(dir.path());
    let idle = Idle {
        rule: rule(IdleWhen::Return),
        used: Some(OVER),
        idle: 60.0,
        head: dir.path().join("conv.json"),
    };

    assert!(returned(&idle, &cli, dir.path()).is_none());
    assert!(!dir.path().join("heard").exists());
}

#[cfg(unix)]
#[test]
fn a_profile_left_on_never_is_not_compacted_on_return() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = stand_in(dir.path());
    let idle = Idle {
        rule: IdleCompact::default(),
        used: Some(OVER),
        idle: 3_600.0,
        head: dir.path().join("conv.json"),
    };

    assert!(returned(&idle, &cli, dir.path()).is_none());
    assert!(!dir.path().join("heard").exists());
}

/// Just before expiry: the timer waits until a minute before the five-minute
/// cache goes, compacts then, and lets a conversation it missed go.
#[test]
fn just_before_expiry_compacts_a_minute_before_the_cache_goes() {
    let expiry = rule(IdleWhen::Expiry);

    assert_eq!(next(expiry, Some(OVER), 100.0), Next::Wait(140.0));
    assert_eq!(next(expiry, Some(OVER), 240.0), Next::Compact);
    assert_eq!(next(expiry, Some(OVER), 300.0), Next::Stop);
    assert_eq!(next(expiry, Some(150_000), 240.0), Next::Stop);
    assert_eq!(next(expiry, None, 240.0), Next::Stop);
    assert_eq!(next(rule(IdleWhen::Return), Some(OVER), 240.0), Next::Stop);
    assert_eq!(next(IdleCompact::default(), Some(OVER), 240.0), Next::Stop);
}

/// What the timer runs once it is time: the same `/compact` on the session,
/// with the head settled onto it.
#[cfg(unix)]
#[test]
fn a_compaction_settles_the_head_on_the_session_it_ended_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = stand_in(dir.path());
    let command = cli.to_string_lossy().into_owned();
    let head = dir.path().join("conv.json");
    let opening = devpit_agentcli::head::Head {
        profile: "claude".to_owned(),
        model: None,
        created_at: 0.0,
        cost_usd: 1.0,
        budget_usd: None,
        session_id: Some("s-1".to_owned()),
        permission: None,
        effort: None,
        title: None,
        rewind: Default::default(),
        cwd: None,
        context: None,
    };
    devpit_agentcli::head::write_head(&head, &opening).expect("head");

    let compacted = (0..5)
        .find_map(|_| {
            compact(
                None,
                &devpit_agentcli::driver::Claude::default(),
                &turn(&command, dir.path()),
                OVER,
                &head,
            )
            .or_else(|| {
                std::thread::sleep(std::time::Duration::from_millis(50));
                None
            })
        })
        .expect("compacted");

    assert_eq!(compacted.note.parts, [Part::Compacted { tokens: OVER }]);
    let settled = devpit_agentcli::head::read_head(&head).expect("settled");
    assert_eq!(settled.cost_usd, 1.5);
    assert_eq!(settled.rewind.anchors.len(), 0);
}
