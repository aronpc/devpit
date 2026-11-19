use devpit_rpc::{HostDraft, HostEnvironment};

use super::*;

fn store_in(dir: &tempfile::TempDir) -> Store {
    Store::open(&dir.path().join("state.db")).expect("open")
}

fn draft() -> HostDraft {
    HostDraft {
        id: None,
        name: "staging box".to_owned(),
        target: "staging-01.example.net".to_owned(),
        user: Some("deploy".to_owned()),
        port: Some(2222),
        identity_file: Some("~/.ssh/id_ed25519".to_owned()),
        work_root: "/srv/apps".to_owned(),
        environment: HostEnvironment::Staging,
    }
}

/// Saved from Settings, a profile is listed again with its environment.
#[test]
fn a_saved_profile_is_listed_with_its_environment() {
    let dir = tempfile::tempdir().expect("tempdir");

    saved(&store_in(&dir), draft()).expect("saved");

    let listed = listed(&store_in(&dir)).expect("listed").hosts;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "staging box");
    assert_eq!(listed[0].environment, HostEnvironment::Staging);
    assert!(listed[0].id.starts_with("host_"), "{}", listed[0].id);
}

/// Saving with an id edits that profile; removing it forgets it.
#[test]
fn a_profile_is_edited_by_its_id_and_removed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store_in(&dir);
    let id = saved(&store, draft()).expect("saved").hosts[0].id.clone();

    let edited = saved(
        &store,
        HostDraft {
            id: Some(id.clone()),
            environment: HostEnvironment::Production,
            ..draft()
        },
    )
    .expect("edited")
    .hosts;
    assert_eq!(edited.len(), 1);
    assert_eq!(edited[0].environment, HostEnvironment::Production);

    assert!(removed(&store, &id).expect("removed").hosts.is_empty());
}

/// Each field holds only what ssh is given for it; anything else, a flag or
/// shell syntax, is refused with the field it was in.
#[test]
fn a_target_that_is_not_an_alias_host_user_port_or_path_is_refused() {
    let refused = [
        (
            HostDraft {
                target: "box; rm -rf ~".to_owned(),
                ..draft()
            },
            "SSH target",
        ),
        (
            HostDraft {
                target: "-oProxyCommand=touch /tmp/x".to_owned(),
                ..draft()
            },
            "SSH target",
        ),
        (
            HostDraft {
                target: String::new(),
                ..draft()
            },
            "SSH target",
        ),
        (
            HostDraft {
                user: Some("deploy@box".to_owned()),
                ..draft()
            },
            "user",
        ),
        (
            HostDraft {
                port: Some(0),
                ..draft()
            },
            "port",
        ),
        (
            HostDraft {
                identity_file: Some("id_ed25519".to_owned()),
                ..draft()
            },
            "identity file",
        ),
        (
            HostDraft {
                work_root: "srv/apps".to_owned(),
                ..draft()
            },
            "work root",
        ),
        (
            HostDraft {
                work_root: "/srv/apps\nrm -rf ~".to_owned(),
                ..draft()
            },
            "work root",
        ),
        (
            HostDraft {
                work_root: "/srv/$(touch x)".to_owned(),
                ..draft()
            },
            "work root",
        ),
        (
            HostDraft {
                name: "  ".to_owned(),
                ..draft()
            },
            "name",
        ),
    ];
    for (one, field) in refused {
        let why = refusal(&one).unwrap_or_else(|| panic!("took {one:?}"));
        assert!(why.contains(field), "{field}: {why}");
    }
    assert_eq!(refusal(&draft()), None);
    assert_eq!(
        refusal(&HostDraft {
            target: "prod".to_owned(),
            user: None,
            port: None,
            identity_file: None,
            work_root: "~".to_owned(),
            ..draft()
        }),
        None,
        "an ssh config alias alone"
    );
}

/// A refused profile is not saved, and the command says why.
#[test]
fn a_refused_profile_is_not_saved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store_in(&dir);

    let refused = saved(
        &store,
        HostDraft {
            port: Some(0),
            ..draft()
        },
    )
    .expect_err("saved");

    assert_eq!(refused.code, ErrorCode::Invalid);
    assert!(listed(&store).expect("listed").hosts.is_empty());
}

/// Two profiles cannot share a name: the orchestrator names a host by it.
#[test]
fn a_second_profile_with_the_same_name_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store_in(&dir);
    saved(&store, draft()).expect("saved");

    let refused = saved(
        &store,
        HostDraft {
            name: "Staging Box".to_owned(),
            ..draft()
        },
    )
    .expect_err("saved twice");

    assert!(refused.message.contains("already"), "{}", refused.message);
}

/// The identity file is kept as the path it is: its contents are never
/// read, so they are nowhere in devpit's database.
#[test]
fn an_identity_file_is_kept_as_its_path_and_never_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = dir.path().join("id_test");
    std::fs::write(&key, "KEY-CONTENTS-THAT-MUST-STAY-PUT").expect("key");
    let path = key.display().to_string();
    let store = store_in(&dir);

    let kept = saved(
        &store,
        HostDraft {
            identity_file: Some(path.clone()),
            ..draft()
        },
    )
    .expect("saved")
    .hosts;
    drop(store);

    assert_eq!(kept[0].identity_file.as_deref(), Some(path.as_str()));
    let mut written = Vec::new();
    for entry in std::fs::read_dir(dir.path()).expect("dir").flatten() {
        if entry.file_name().to_string_lossy().starts_with("state.db") {
            written.extend(std::fs::read(entry.path()).expect("db"));
        }
    }
    let marker: &[u8] = b"KEY-CONTENTS-THAT-MUST-STAY-PUT";
    assert!(
        !written.windows(marker.len()).any(|at| at == marker),
        "the key's contents reached the database"
    );
}

/// A label it cannot read is taken as the most careful one.
#[test]
fn an_environment_it_cannot_read_is_taken_as_production() {
    assert_eq!(environment_of("prod"), HostEnvironment::Production);
    assert_eq!(environment_of("dev"), HostEnvironment::Dev);
}

/// An edit names a profile that exists; it never makes one under that id.
#[test]
fn editing_a_profile_that_is_not_there_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = store_in(&dir);

    let refused = saved(
        &store,
        HostDraft {
            id: Some("host_gone".to_owned()),
            ..draft()
        },
    )
    .expect_err("made one");

    assert_eq!(refused.code, ErrorCode::NotFound);
    assert!(listed(&store).expect("listed").hosts.is_empty());
}
