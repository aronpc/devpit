use super::*;
use crate::store::HostRow;

fn host(id: &str, name: &str) -> HostRow {
    HostRow {
        id: id.to_owned(),
        name: name.to_owned(),
        target: name.to_owned(),
        user: None,
        port: None,
        identity_file: None,
        work_root: "/srv".to_owned(),
        environment: "staging".to_owned(),
    }
}

fn with_project(dir: &tempfile::TempDir) -> (Store, String) {
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.save_host(&host("host_1", "staging")).expect("host");
    store.save_host(&host("host_2", "prod")).expect("host");
    (store, project)
}

/// A project keeps one path per host, as many hosts as it is linked to.
#[test]
fn a_project_lists_its_links_with_their_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = with_project(&dir);

    store
        .link_host(&project, "host_1", "/srv/demo")
        .expect("linked");
    store
        .link_host(&project, "host_2", "~/demo")
        .expect("linked");
    store
        .link_host(&project, "host_1", "/srv/demo-2")
        .expect("moved");

    let links = store.host_links(&project).expect("links");
    assert_eq!(
        links
            .iter()
            .map(|one| (one.host_id.as_str(), one.path.as_str()))
            .collect::<Vec<_>>(),
        [("host_2", "~/demo"), ("host_1", "/srv/demo-2")],
        "by host name"
    );
}

/// Removing a host takes its links; the project stays as it was.
#[test]
fn removing_a_host_removes_its_links_and_keeps_the_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = with_project(&dir);
    store
        .link_host(&project, "host_1", "/srv/demo")
        .expect("linked");
    store
        .link_host(&project, "host_2", "/srv/demo")
        .expect("linked");

    store.remove_host("host_1").expect("removed");

    let links = store.host_links(&project).expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].host_id, "host_2");
    let kept: i64 = store
        .conn()
        .query_row("SELECT COUNT(*) FROM project_host", [], |row| row.get(0))
        .expect("count");
    assert_eq!(kept, 1, "the removed host's link is still stored");
    assert!(store.project(&project).expect("read").is_some());
}

/// Unlinking forgets that one link only.
#[test]
fn unlinking_forgets_one_link() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (store, project) = with_project(&dir);
    store
        .link_host(&project, "host_1", "/srv/demo")
        .expect("linked");
    store
        .link_host(&project, "host_2", "/srv/demo")
        .expect("linked");

    store.unlink_host(&project, "host_2").expect("unlinked");

    let links = store.host_links(&project).expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].host_id, "host_1");
}
