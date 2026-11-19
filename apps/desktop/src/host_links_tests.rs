use std::time::Duration;

use super::*;
use crate::fake_host::FakeHost;
use devpit_rpc::HostEnvironment;

const PATIENCE: Duration = Duration::from_secs(10);

/// A store with one project and two hosts, one of them production.
fn store_in(dir: &tempfile::TempDir) -> (Store, String) {
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    for (id, name, environment) in [
        ("host_1", "staging", "staging"),
        ("host_2", "web prod", "production"),
    ] {
        store
            .save_host(&HostRow {
                id: id.to_owned(),
                name: name.to_owned(),
                target: id.to_owned(),
                user: None,
                port: None,
                identity_file: None,
                work_root: "/srv".to_owned(),
                environment: environment.to_owned(),
            })
            .expect("host");
    }
    (store, project)
}

fn linked(
    fake: &FakeHost,
    store: &Store,
    project: &str,
    host: &str,
    path: &str,
) -> Result<HostLinks, RpcError> {
    let envs = fake.envs();
    let envs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    linked_with(
        &fake.ssh(),
        &envs,
        PATIENCE,
        store,
        fake.dir.path(),
        project,
        host,
        path,
    )
}

/// A link is kept once its path is a folder on the host, from `/` or `~`.
#[test]
fn a_link_is_kept_when_its_folder_is_on_the_host() {
    let fake = FakeHost::new();
    let folder = fake.folder("apps/demo");
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = store_in(&dir);

    linked(
        &fake,
        &store,
        &project,
        "host_1",
        &folder.display().to_string(),
    )
    .expect("absolute");
    let links = linked(&fake, &store, &project, "host_2", "~/apps/demo")
        .expect("from home")
        .links;

    assert_eq!(links.len(), 2);
    assert_eq!(links[1].host_name, "web prod");
    assert_eq!(links[1].environment, HostEnvironment::Production);
    assert_eq!(links[1].path, "~/apps/demo");
    assert_eq!(links[0].environment, HostEnvironment::Staging);
}

/// A path with no folder on the host is refused, and the reason names both.
#[test]
fn a_path_with_no_folder_on_the_host_is_refused() {
    let fake = FakeHost::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = store_in(&dir);

    let refused = linked(&fake, &store, &project, "host_1", "~/nowhere").expect_err("linked");

    assert_eq!(refused.code, ErrorCode::NotFound);
    assert!(refused.message.contains("~/nowhere"), "{}", refused.message);
    assert!(refused.message.contains("staging"), "{}", refused.message);
    assert!(listed(&store, &project).expect("listed").links.is_empty());
}

/// A path the host's shell would read as more than a path is refused before
/// anything runs there.
#[test]
fn a_path_with_shell_in_it_is_refused_before_ssh_runs() {
    let fake = FakeHost::new();
    fake.program(&fake.ssh(), "touch \"$HOME/ran\"; exit 0");
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = store_in(&dir);

    let refused = linked(&fake, &store, &project, "host_1", "/srv/$(id)").expect_err("linked");

    assert_eq!(refused.code, ErrorCode::Invalid);
    assert!(!fake.home().join("ran").exists(), "ssh ran");
}

/// When ssh cannot reach the host, nothing is linked and the reason is ssh's.
#[test]
fn a_host_ssh_cannot_reach_links_nothing() {
    let fake = FakeHost::new();
    fake.program(
        &fake.ssh(),
        "echo 'Permission denied (publickey).' >&2\nexit 255",
    );
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = store_in(&dir);

    let refused = linked(&fake, &store, &project, "host_1", "/srv").expect_err("linked");

    assert!(
        refused.message.contains("Permission denied"),
        "{}",
        refused.message
    );
    assert!(listed(&store, &project).expect("listed").links.is_empty());
}

/// Unlinking leaves the other links, and the host itself.
#[test]
fn unlinking_one_host_keeps_the_rest() {
    let fake = FakeHost::new();
    let folder = fake.folder("demo").display().to_string();
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = store_in(&dir);
    linked(&fake, &store, &project, "host_1", &folder).expect("linked");
    linked(&fake, &store, &project, "host_2", &folder).expect("linked");

    let left = unlinked(&store, &project, "host_1")
        .expect("unlinked")
        .links;

    assert_eq!(left.len(), 1);
    assert_eq!(left[0].host_id, "host_2");
    assert_eq!(store.hosts().expect("hosts").len(), 2);
}
