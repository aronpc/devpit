use std::collections::HashMap;

use super::*;

const NAME: &str = "0123456789abcdef0123456789abcdef.devpit.localhost";

fn place() -> Place {
    Place {
        port: 41234,
        name: NAME.to_owned(),
    }
}
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const SESSION: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn asked(method: &str, path: &str, headers: &[(&str, &str)]) -> Request {
    Request {
        method: method.to_owned(),
        path: path.to_owned(),
        query: String::new(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<HashMap<_, _>>(),
        body: Vec::new(),
    }
}

/// What the door says of `request` asked with `query`, `codes` aside.
fn at_door_with(request: &Request, query: &str, codes: &Codes) -> Door {
    let place = place();
    let mut request = request.clone();
    request.query = query.to_owned();
    door(
        &request,
        &Keys {
            place: &place,
            token: TOKEN,
            codes,
            session: SESSION,
            ticket: "ticket",
            ours: &|| true,
        },
    )
}

fn at_door(request: &Request, query: &str) -> Door {
    at_door_with(request, query, &Codes::new())
}

fn signed_in(path: &str) -> Request {
    let cookie = format!("theme=dark; {COOKIE}={SESSION}");
    asked(
        "POST",
        path,
        &[("host", "127.0.0.1:41234"), ("cookie", &cookie)],
    )
}

#[test]
fn without_the_cookie_or_with_a_wrong_one_the_door_says_401() {
    let bare = asked("POST", "/rpc/project_list", &[("host", "127.0.0.1:41234")]);
    assert_eq!(at_door(&bare, ""), Door::Unauthorized);
    let wrong = asked(
        "POST",
        "/rpc/project_list",
        &[("host", "127.0.0.1:41234"), ("cookie", "devpit_local=nope")],
    );
    assert_eq!(at_door(&wrong, ""), Door::Unauthorized);
    assert_eq!(at_door(&signed_in("/rpc/project_list"), ""), Door::In);
    let planted = format!("{COOKIE}=attacker; {COOKIE}={SESSION}");
    let shadowed = asked(
        "POST",
        "/rpc/project_list",
        &[("host", "127.0.0.1:41234"), ("cookie", planted.as_str())],
    );
    assert_eq!(
        at_door(&shadowed, ""),
        Door::In,
        "a cookie planted first under the same name does not sign the browser out"
    );
    let token_as_cookie = asked(
        "POST",
        "/rpc/project_list",
        &[
            ("host", "127.0.0.1:41234"),
            ("cookie", &format!("{COOKIE}={TOKEN}")),
        ],
    );
    assert_eq!(
        at_door(&token_as_cookie, ""),
        Door::Unauthorized,
        "the cookie is this run's session, never the token in the home"
    );
}

/// DNS rebinding: a page on another name that resolves to 127.0.0.1 sends
/// its own name as the Host, or its own origin; both are refused before the
/// cookie is even looked at.
#[test]
fn a_foreign_host_or_origin_is_refused_whatever_it_carries() {
    let cookie = format!("{COOKIE}={SESSION}");
    for (host, origin) in [
        ("evil.example:41234", None),
        ("127.0.0.1:9999", None),
        ("127.0.0.1", None),
        ("127.0.0.1:41234", Some("http://evil.example")),
        ("127.0.0.1:41234", Some("http://127.0.0.1:9999")),
        ("localhost:41234", Some("https://localhost:41234")),
        ("devpit.localhost:41234", None),
        ("evil.devpit.localhost:41234", None),
        (
            "0123456789abcdef0123456789abcdef.devpit.localhost:9999",
            None,
        ),
        (
            "0123456789abcdef0123456789abcdef.devpit.localhost:41234",
            Some("http://0123456789abcdef0123456789abcdef.devpit.localhost:9999"),
        ),
    ] {
        let mut headers = vec![("host", host), ("cookie", cookie.as_str())];
        if let Some(origin) = origin {
            headers.push(("origin", origin));
        }
        let request = asked("POST", "/rpc/project_list", &headers);
        assert_eq!(at_door(&request, ""), Door::Foreign, "{host} {origin:?}");
    }
    let named = asked(
        "GET",
        "/events",
        &[
            ("host", "localhost:41234"),
            ("origin", "http://localhost:41234"),
            ("cookie", cookie.as_str()),
        ],
    );
    assert_eq!(at_door(&named, ""), Door::In);
}

/// The token is traded only under this install's own random name, so the
/// cookie is kept for a name no other loopback server, on any port, answers.
#[test]
fn the_token_is_traded_for_the_cookie_once_and_only_when_it_is_right() {
    for host in ["127.0.0.1:41234", "localhost:41234"] {
        let elsewhere = asked("GET", "/auth", &[("host", host)]);
        assert_eq!(
            at_door(&elsewhere, &format!("t={TOKEN}")),
            Door::Unauthorized,
            "{host}"
        );
    }
    let host = format!("{NAME}:41234");
    let auth = asked("GET", "/auth", &[("host", host.as_str())]);
    assert_eq!(at_door(&auth, &format!("t={TOKEN}")), Door::Admit);
    assert_eq!(at_door(&auth, "t=wrong"), Door::Unauthorized);
    assert_eq!(at_door(&auth, ""), Door::Unauthorized);
    assert_eq!(at_door(&auth, "x=1"), Door::Unauthorized);
}

#[test]
fn a_stream_or_the_desktops_own_ui_is_not_run_from_a_browser() {
    for command in [
        "chat_send",
        "chat_rejoin",
        "session_attach",
        "browser_open",
        "island_open_chat",
        "island_pending_pane",
        "tray_read",
        "path_reveal",
        "url_open",
        "apps_open",
        "media_open",
        "mcp_app_open",
        "plugin:dialog|open",
    ] {
        assert!(desktop_only(command), "{command}");
    }
    for command in ["project_list", "chat_history", "board_get", "chat_head"] {
        assert!(!desktop_only(command), "{command}");
    }
}

#[cfg(unix)]
#[test]
fn the_token_is_made_once_owner_only_and_kept() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().expect("home");
    let first = token_in(home.path()).expect("made");
    assert_eq!(first.len(), 64);
    let mode = std::fs::metadata(home.path().join(TOKEN_FILE))
        .expect("kept")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(token_in(home.path()).expect("read"), first);
}

/// Nothing listens off loopback: the socket is bound to 127.0.0.1, and the
/// port is kept for next time.
/// A local.json from before the name (a port alone, 0644) keeps its port,
/// gains a name of this install's own and is made owner-only.
#[cfg(unix)]
#[test]
fn an_old_local_json_keeps_its_port_and_gains_a_name_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().expect("home");
    let free = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|probe| probe.local_addr())
        .expect("a free port")
        .port();
    let path = home.path().join(PORT_FILE);
    std::fs::write(&path, format!(r#"{{"port":{free}}}"#)).expect("old file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("0644");
    let (_listener, place) = bound(home.path()).expect("bound");
    assert_eq!(place.port, free);
    assert!(is_own_name(&place.name), "{}", place.name);
    let mode = std::fs::metadata(&path).expect("kept").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn the_server_binds_loopback_alone_and_keeps_its_port() {
    let home = tempfile::tempdir().expect("home");
    let (listener, place) = bound(home.path()).expect("bound");
    let address = listener.local_addr().expect("address");
    assert!(is_own_name(&place.name), "{}", place.name);
    assert!(address.ip().is_loopback(), "{address}");
    let kept: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.path().join(PORT_FILE)).expect("port kept"),
    )
    .expect("json");
    assert_eq!(kept["port"], address.port());
    assert_eq!(kept["name"], place.name.as_str());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.path().join(PORT_FILE))
            .expect("kept")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "the name is a secret");
    }
    drop(listener);
    let (_again, again) = bound(home.path()).expect("again");
    assert_eq!(again, place, "the port and the name are kept");
    let other = tempfile::tempdir().expect("other home");
    let (_other, elsewhere) = bound(other.path()).expect("other");
    assert_ne!(
        elsewhere.name, place.name,
        "each install has a name of its own"
    );
}

/// "Open in browser" puts the link on Chromium's command line, which any
/// user of the machine can read: it carries a one-time code, never the token.
#[test]
fn the_sign_in_link_goes_to_loopback_with_a_code() {
    assert_eq!(
        sign_in_url(&place(), "c0de"),
        format!("http://{NAME}:41234/auth?c=c0de")
    );
}

/// A command line is readable by every user of the machine: the browser is
/// handed a page by path, owner-only, and the code is inside it.
#[test]
fn the_browser_is_handed_a_page_by_path_and_never_the_code() {
    let home = tempfile::tempdir().expect("home");
    let handed = hand_over(home.path(), &place(), "c0de").expect("handed");
    assert!(
        !handed.contains("c0de"),
        "the code is on the command line: {handed}"
    );
    let page = std::fs::read_to_string(home.path().join(SIGN_IN_FILE)).expect("the page");
    assert!(page.contains(&sign_in_url(&place(), "c0de")), "{page}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.path().join(SIGN_IN_FILE))
            .expect("kept")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn a_sign_in_code_admits_once_and_only_within_its_minute() {
    let host = format!("{NAME}:41234");
    let auth = asked("GET", "/auth", &[("host", host.as_str())]);
    let codes = Codes::new();
    let code = codes.issue(Instant::now()).expect("code");
    let with = |code: &str| at_door_with(&auth, &format!("c={code}"), &codes);
    assert_eq!(with(&code), Door::Admit);
    assert_eq!(with(&code), Door::Unauthorized, "a code is good once");
    let long_ago = Instant::now()
        .checked_sub(CODE_LIFE + Duration::from_secs(1))
        .expect("an uptime past a minute");
    let stale = codes.issue(long_ago).expect("stale");
    assert_eq!(with(&stale), Door::Unauthorized, "a code is good a minute");
    assert_eq!(with(TOKEN), Door::Unauthorized, "the token is no code");
}

/// The code is on a command line wherever the browser cannot read the page:
/// another user who reads it there still cannot trade it, since only a
/// connection of this user's gets past `/auth` with a code.
#[test]
fn a_code_from_another_users_connection_admits_nobody_and_stays_good() {
    let place = place();
    let codes = Codes::new();
    let code = codes.issue(Instant::now()).expect("code");
    let mut auth = asked("GET", "/auth", &[("host", &format!("{NAME}:41234"))]);
    auth.query = format!("c={code}");
    let theirs = Keys {
        ours: &|| false,
        ..keys(&place, &codes)
    };
    assert_eq!(door(&auth, &theirs), Door::Unauthorized);
    assert_eq!(
        door(&auth, &keys(&place, &codes)),
        Door::Admit,
        "still good for ours"
    );
}

/// Read from the kernel's own table of connections: this process's own
/// connection is ours, and one from a port nobody holds is not.
#[cfg(target_os = "linux")]
#[test]
fn a_connection_from_this_user_is_told_from_one_that_is_not() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let _client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (stream, peer) = listener.accept().expect("accept");
    let local = stream.local_addr().expect("local");
    assert!(connection_is_ours(peer, local));
    let nobody = std::net::SocketAddr::from(([127, 0, 0, 1], 1));
    assert!(!connection_is_ours(nobody, local));
}

/// A snap reads no hidden folder of the home, and Ubuntu's `chromium-browser`
/// is the snap under another name: those get the link, not the page.
#[test]
fn a_snap_browser_is_handed_the_link_since_it_cannot_read_the_home() {
    assert!(confined(Path::new("/snap/bin/chromium")));
    assert!(confined(Path::new("/usr/bin/chromium-browser")));
    assert!(!confined(Path::new("/usr/bin/google-chrome")));
    assert!(!confined(Path::new("/usr/bin/chromium")));
}

#[test]
fn a_chromium_opens_it_as_an_app_and_otherwise_the_desktop_opens_the_link() {
    assert_eq!(
        opener(|name| name == "google-chrome"),
        ("google-chrome", true)
    );
    assert_eq!(opener(|name| name == "chromium"), ("chromium", true));
    assert_eq!(
        opener(|_| false),
        (link_opener(std::env::consts::OS).0, false)
    );
}

/// Without a Chromium the link goes to the desktop's own opener, which is not
/// xdg-open on macOS or Windows: there the button did nothing.
#[test]
fn each_desktop_opens_a_link_with_its_own_opener() {
    assert_eq!(link_opener("macos"), ("open", &[][..]));
    assert_eq!(link_opener("windows"), ("cmd", &["/C", "start", ""][..]));
    assert_eq!(link_opener("linux"), ("xdg-open", &[][..]));
}

fn keys<'a>(place: &'a Place, codes: &'a Codes) -> Keys<'a> {
    Keys {
        place,
        token: TOKEN,
        codes,
        session: SESSION,
        ticket: "ticket",
        ours: &|| true,
    }
}

/// A pasted picture or a saved file is a call of megabytes: it is read whole
/// past the door, one past the ceiling is refused, and a caller the door
/// turns away has no body read at all, however long it says it is.
#[test]
fn only_a_call_past_the_door_has_its_body_read() {
    let (place, codes) = (place(), Codes::new());
    let call = |cookie: &str, length: usize, body: usize| {
        format!(
            "POST /rpc/file_write HTTP/1.1\r\nhost: 127.0.0.1:41234\r\ncookie: {cookie}\r\n\
             content-length: {length}\r\n\r\n{}",
            "x".repeat(body)
        )
    };
    let read = |raw: String| admitted(&mut raw.as_bytes(), &keys(&place, &codes), |_| {});
    let stranger = read(call("devpit_local=nope", MOST_BODY, 0)).expect("a head");
    assert_eq!((stranger.0, stranger.1.body.len()), (Door::Unauthorized, 0));
    let ours = format!("{COOKIE}={SESSION}");
    let big = read(call(&ours, 2 * 1024 * 1024, 2 * 1024 * 1024)).expect("a call");
    assert_eq!((big.0, big.1.body.len()), (Door::In, 2 * 1024 * 1024));
    assert!(read(call(&ours, MOST_BODY + 1, MOST_BODY + 1)).is_none());
}

/// The query is the request line's, however few bytes the first read held.
#[test]
fn the_query_is_read_from_the_whole_request_line() {
    let (place, codes) = (place(), Codes::new());
    let raw = format!("GET /auth?x=1&t={TOKEN} HTTP/1.1\r\nhost: {NAME}:41234\r\n\r\n");
    let mut reader = BufReader::with_capacity(8, raw.as_bytes());
    let (entry, _) = admitted(&mut reader, &keys(&place, &codes), |_| {}).expect("read");
    assert_eq!(entry, Door::Admit);
}

/// A head sent a byte at a time is dropped at its deadline: a read timeout
/// alone starts over with every byte.
#[test]
fn a_head_that_trickles_in_is_dropped_at_its_deadline() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let at = listener.local_addr().expect("addr");
    let sender = std::thread::spawn(move || {
        let mut client = TcpStream::connect(at).expect("connect");
        let _ = client.write_all(b"GET / HTTP/1.1\r\n");
        for _ in 0..60 {
            if client.write_all(b"x").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let (stream, _) = listener.accept().expect("accept");
    let started = Instant::now();
    let read = read_head(BufReader::new(Deadline::new(
        stream,
        Duration::from_millis(300),
    )));
    assert!(read.is_none());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    let _ = sender.join();
}

#[test]
fn connections_past_the_most_wait_for_one_to_close() {
    let open = Arc::new(AtomicUsize::new(0));
    let first = open_one(&open, 2).expect("first");
    let _second = open_one(&open, 2).expect("second");
    assert!(open_one(&open, 2).is_none());
    drop(first);
    assert!(open_one(&open, 2).is_some());
}

#[test]
fn each_run_signs_in_with_a_new_session() {
    let (one, two) = (secret().expect("one"), secret().expect("two"));
    assert_eq!(one.len(), 64);
    assert_ne!(one, two);
}
