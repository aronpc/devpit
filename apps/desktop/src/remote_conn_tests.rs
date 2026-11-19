use super::*;

/// A viewer built before ptyd reads the list it always did: a ptyd pane is
/// one more terminal in the same shape, after the tmux ones.
#[test]
fn a_ptyd_pane_is_listed_beside_tmux_ones_in_the_shape_an_old_viewer_reads() {
    let tmux = vec![devpit_tmux::Running {
        leaf_id: "leaf_t".into(),
        command: "zsh".into(),
        tty: "/dev/pts/3".into(),
        profile: String::new(),
    }];
    let ptyd = vec![devpit_rpc::PaneRunning {
        pane_id: "leaf_p".into(),
        command: "claude".into(),
        busy: true,
        agent: Some("claude".into()),
        label: "Claude Code".into(),
    }];
    let listed = RemoteOut::Projects {
        projects: vec![RemoteProject {
            id: "p".into(),
            name: "P".into(),
            group: None,
            terminals: terminals(tmux, ptyd),
        }],
    };
    assert_eq!(
        serde_json::to_string(&listed).expect("json"),
        r#"{"t":"projects","projects":[{"id":"p","name":"P","group":null,"terminals":[{"pane":"leaf_t","command":"zsh"},{"pane":"leaf_p","command":"claude"}]}]}"#
    );
}

/// What an old viewer sends about a pane is read as it always was, whoever
/// holds the pane: nothing about the wire changed for ptyd.
#[test]
fn an_old_viewers_pane_messages_are_read_unchanged() {
    let read = |text: &str| serde_json::from_str::<RemoteIn>(text).expect(text);
    assert_eq!(
        read(r#"{"t":"paneOpen","project":"p","pane":"leaf_p","cols":40,"rows":20}"#),
        RemoteIn::PaneOpen {
            project: "p".into(),
            pane: "leaf_p".into(),
            cols: 40,
            rows: 20,
            handle: None,
        }
    );
    assert_eq!(
        read(r#"{"t":"paneInput","pane":"leaf_p","b64":"bHMK"}"#),
        RemoteIn::PaneInput {
            pane: "leaf_p".into(),
            b64: "bHMK".into()
        }
    );
    assert_eq!(
        read(r#"{"t":"panePaste","pane":"leaf_p","text":"ls"}"#),
        RemoteIn::PanePaste {
            pane: "leaf_p".into(),
            text: "ls".into()
        }
    );
    assert_eq!(
        read(r#"{"t":"paneClose","pane":"leaf_p"}"#),
        RemoteIn::PaneClose {
            pane: "leaf_p".into()
        }
    );
}

/// A viewer that reads raw bytes asks for them with a handle, and an older
/// host's welcome, with no `binary`, reads as one that does not send them.
#[test]
fn a_raw_viewer_names_a_handle_and_an_old_welcome_says_no_raw_bytes() {
    let asked: RemoteIn = serde_json::from_str(
        r#"{"t":"paneOpen","project":"p","pane":"leaf_p","cols":40,"rows":20,"handle":7}"#,
    )
    .unwrap();
    assert!(matches!(
        asked,
        RemoteIn::PaneOpen {
            handle: Some(7),
            ..
        }
    ));
    let old: RemoteOut = serde_json::from_str(
        r#"{"t":"welcome","device":"phone","host":"desk","typing":true,"answering":false}"#,
    )
    .unwrap();
    assert!(matches!(old, RemoteOut::Welcome { binary: false, .. }));
}

/// The viewer asks for the stops only of a host whose welcome says it shows
/// them; an older host's welcome reads as one that does not.
#[test]
fn an_old_welcome_says_no_stops_and_a_click_reads_by_account() {
    let old: RemoteOut = serde_json::from_str(
        r#"{"t":"welcome","device":"phone","host":"desk","typing":true,"answering":false,"binary":true}"#,
    )
    .unwrap();
    assert!(matches!(old, RemoteOut::Welcome { stops: false, .. }));
    let clicked: RemoteIn = serde_json::from_str(
        r#"{"t":"stopsConfirm","stops":[
            {"profile":"p1","stop":{"name":"a","pid":null,"losing":null}},
            {"profile":"p2","stop":{"name":"b","pid":7,"losing":"it is working right now"}},
            {"profile":"p1","stop":{"name":"c","pid":null,"losing":null}}]}"#,
    )
    .unwrap();
    let RemoteIn::StopsConfirm { stops } = clicked else {
        panic!("a click on the stops");
    };
    let grouped: Vec<(String, Vec<String>)> = by_profile(&stops)
        .into_iter()
        .map(|(profile, list)| (profile, list.into_iter().map(|one| one.name).collect()))
        .collect();
    assert_eq!(
        grouped,
        [
            ("p1".to_owned(), vec!["a".to_owned(), "c".to_owned()]),
            ("p2".to_owned(), vec!["b".to_owned()])
        ]
    );
}

fn device(typing: bool) -> crate::remote_devices::Device {
    crate::remote_devices::Device {
        id: "dev_raw".into(),
        name: "phone".into(),
        token_hash: String::new(),
        key: None,
        typing,
        answering: false,
        mcp: false,
        paired_at: 0.0,
        last_seen: None,
    }
}

#[test]
fn raw_keys_from_a_device_that_may_only_watch_are_refused_and_other_kinds_ignored() {
    let mut watching = crate::remote_panes::Watching::new(11);
    let keys = crate::pane_socket::encode(1, crate::pane_socket::kind::INPUT, b"ls\r");
    assert!(matches!(
        typed_raw(&device(false), &mut watching, &keys),
        Some(RemoteOut::Failed { .. })
    ));
    let ack = crate::pane_socket::encode(1, crate::pane_socket::kind::ACK, &[0, 0, 0, 1]);
    assert!(typed_raw(&device(true), &mut watching, &ack).is_none());
    // A handle no pane was opened under types nowhere.
    assert!(typed_raw(&device(true), &mut watching, &keys).is_none());
}

/// A viewer that says nothing: each read waits as long as a real one would.
#[derive(Default)]
struct Quiet {
    said: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

impl Wire for Quiet {
    fn hear(&mut self) -> Result<Heard, ()> {
        std::thread::sleep(LISTEN);
        Ok(Heard::Nothing)
    }
    fn say(&mut self, text: &str) {
        self.said.lock().unwrap().push(text.as_bytes().to_vec());
    }
    fn say_bytes(&mut self, bytes: &[u8]) {
        self.said.lock().unwrap().push(bytes.to_vec());
    }
    fn end(&mut self) {}
}

/// Forgetting a device on the machine cuts its connection at once: its loop
/// sees the hub let it go before its next read, not when the viewer speaks.
#[test]
fn a_device_forgotten_loses_its_connection_at_once() {
    let device = "dev_forgotten_now";
    let (id, _out, heard) = crate::remote_hub::enter(device);
    let (_raw, raws) = std::sync::mpsc::channel();
    let (ended, ends) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut watching = crate::remote_panes::Watching::new(id);
        converse(
            &mut Quiet::default(),
            id,
            &mut watching,
            &heard,
            &raws,
            |_, _| None,
        );
        let _ = ended.send(());
    });
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(ends.try_recv().is_err(), "it ended before it was forgotten");
    crate::remote_hub::drop_conns(device);
    ends.recv_timeout(std::time::Duration::from_millis(500))
        .expect("the connection outlived its device");
}

/// A viewer that reads slowly: each frame sent takes a while.
struct Slow;

impl Wire for Slow {
    fn hear(&mut self) -> Result<Heard, ()> {
        std::thread::sleep(LISTEN);
        Ok(Heard::Nothing)
    }
    fn say(&mut self, _: &str) {}
    fn say_bytes(&mut self, _: &[u8]) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    fn end(&mut self) {}
}

/// A pane printing faster than the viewer reads does not keep a forgotten
/// device's connection alive: the drain lets go as soon as the hub does.
#[test]
fn a_device_forgotten_while_its_pane_floods_loses_its_connection_at_once() {
    let device = "dev_forgotten_mid_flood";
    let (id, _out, heard) = crate::remote_hub::enter(device);
    let (raw, raws) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let flood = || crate::remote_panes::Raw {
            handle: 1,
            bytes: vec![b'y'; 64],
        };
        while raw.send(flood()).is_ok() {
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    });
    let (ended, ends) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut watching = crate::remote_panes::Watching::new(id);
        converse(&mut Slow, id, &mut watching, &heard, &raws, |_, _| None);
        let _ = ended.send(());
    });
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(ends.try_recv().is_err(), "it ended before it was forgotten");
    crate::remote_hub::drop_conns(device);
    ends.recv_timeout(std::time::Duration::from_millis(500))
        .expect("still connected while its pane floods");
}

/// Lets `device` go once `said` holds what `went_out` looks for, or after 5 s:
/// a fixed clock let a loaded runner drop it before the first drain.
fn forget_once<T: Send + 'static>(
    device: &'static str,
    said: std::sync::Arc<std::sync::Mutex<Vec<T>>>,
    went_out: impl Fn(&[T]) -> bool + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < until && !went_out(&said.lock().unwrap()) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        crate::remote_hub::drop_conns(device);
    })
}

/// A pane's raw bytes leave in the R3 frame under the viewer's handle.
#[test]
fn raw_output_goes_out_as_a_frame_under_its_handle() {
    let device = "dev_raw_output";
    let (id, _out, heard) = crate::remote_hub::enter(device);
    let (raw, raws) = std::sync::mpsc::channel();
    raw.send(crate::remote_panes::Raw {
        handle: 9,
        bytes: b"\x1b[1mok".to_vec(),
    })
    .unwrap();
    let wire = Quiet::default();
    let said = wire.said.clone();
    let stop = forget_once(device, said.clone(), |said| !said.is_empty());
    let mut wire = wire;
    let mut watching = crate::remote_panes::Watching::new(id);
    converse(&mut wire, id, &mut watching, &heard, &raws, |_, _| None);
    stop.join().unwrap();
    let said = said.lock().unwrap();
    let frame = said.first().expect("nothing went out");
    assert_eq!(
        crate::pane_socket::decode(frame),
        Some((9, crate::pane_socket::kind::OUTPUT, &b"\x1b[1mok"[..]))
    );
}

/// A viewer that reads slowly and keeps what it is sent.
#[derive(Default)]
struct SlowKept {
    said: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Wire for SlowKept {
    fn hear(&mut self) -> Result<Heard, ()> {
        std::thread::sleep(LISTEN);
        Ok(Heard::Nothing)
    }
    fn say(&mut self, text: &str) {
        self.said.lock().unwrap().push(text.to_owned());
    }
    fn say_bytes(&mut self, _: &[u8]) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    fn end(&mut self) {}
}

/// The hub's messages (a chat, a question) reach a viewer whose pane floods.
#[test]
fn a_message_from_the_hub_goes_out_while_a_pane_floods() {
    let device = "dev_hub_mid_flood";
    let (id, out, heard) = crate::remote_hub::enter(device);
    let (raw, raws) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let flood = || crate::remote_panes::Raw {
            handle: 1,
            bytes: vec![b'y'; 64],
        };
        while raw.send(flood()).is_ok() {
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    });
    out.send(devpit_rpc::RemoteOut::Failed {
        why: "the hub speaks".to_owned(),
    })
    .unwrap();
    let wire = SlowKept::default();
    let said = wire.said.clone();
    let stop = forget_once(device, said.clone(), |said| {
        said.iter().any(|text| text.contains("the hub speaks"))
    });
    let mut wire = wire;
    let mut watching = crate::remote_panes::Watching::new(id);
    converse(&mut wire, id, &mut watching, &heard, &raws, |_, _| None);
    stop.join().unwrap();
    let said = said.lock().unwrap();
    assert!(
        said.iter().any(|text| text.contains("the hub speaks")),
        "{said:?}"
    );
}

/// The Remote runs on another device and has no settings to read, so the
/// language the host speaks travels in the welcome, and changing it at the
/// host reaches the viewer with the next welcome.
#[test]
fn the_welcome_carries_the_hosts_language_and_follows_a_change() {
    let device = Device {
        id: "d1".into(),
        name: "phone".into(),
        token_hash: String::new(),
        key: None,
        typing: true,
        answering: false,
        mcp: false,
        paired_at: 0.0,
        last_seen: None,
    };
    let said = |locale: devpit_core::locale::Locale| {
        let _turn = crate::locale_turn::settled(locale);
        serde_json::to_value(welcome(&device)).expect("json")["locale"].clone()
    };
    assert_eq!(said(devpit_core::locale::Locale::PtBr), "pt-BR");
    assert_eq!(said(devpit_core::locale::Locale::En), "en");
    let old: RemoteOut = serde_json::from_str(
        r#"{"t":"welcome","device":"phone","host":"desk","typing":true,"answering":false}"#,
    )
    .unwrap();
    assert!(matches!(old, RemoteOut::Welcome { locale, .. } if locale.is_empty()));
}
