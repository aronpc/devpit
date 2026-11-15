use super::*;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

/// A root with the two files the app writes when it starts listening.
fn root_listening_at(address: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        root.path().join("hook-endpoint"),
        format!("http://{address}/hook"),
    )
    .expect("endpoint");
    std::fs::write(root.path().join("hook-auth"), "x-devpit-hook: s3cret\n").expect("auth");
    root
}

#[test]
fn a_hook_is_told_its_budget_and_whether_to_answer() {
    let asked = parsed(&words("--wait 125 --echo"));
    assert_eq!(asked.wait, Duration::from_secs(125));
    assert!(asked.echo && !asked.plugin);

    let asked = parsed(&words("--wait 1.5 --plugin --from-a-newer-devpit"));
    assert_eq!(asked.wait, Duration::from_millis(1500));
    assert!(!asked.echo && asked.plugin);

    // No budget at all is never a hook that hangs the agent.
    assert_eq!(
        parsed(&words("--wait nope")).wait,
        Duration::from_millis(1500)
    );
    assert!(parsed(&words("--wait 1e9")).wait <= Duration::from_secs(600));
}

#[test]
fn a_hook_for_another_agent_says_which_in_the_query() {
    assert_eq!(
        parsed(&words("--from gemini")).from.as_deref(),
        Some("gemini")
    );
    // Only a word reaches the URL.
    assert_eq!(parsed(&words("--from a&pane=x")).from, None);
    assert_eq!(
        query_of("leaf_1", Some("gemini")),
        "?from=gemini&pane=leaf_1"
    );
    assert_eq!(query_of("", Some("gemini")), "?from=gemini");
    assert_eq!(query_of("leaf_1", None), "?pane=leaf_1");
    assert_eq!(query_of("", None), "");
}

#[test]
fn the_plugins_hook_speaks_only_inside_a_pane_and_not_beside_the_settings() {
    let plugin = parsed(&words("--plugin"));
    assert!(!speaks(&plugin, "", ""));
    assert!(speaks(&plugin, "leaf_1", ""));
    assert!(!speaks(&plugin, "leaf_1", "1"));

    let settings = parsed(&[]);
    assert!(speaks(&settings, "", ""));
    assert!(speaks(&settings, "leaf_1", "1"));
}

#[test]
fn the_route_comes_from_the_endpoint_the_app_wrote() {
    assert_eq!(
        route_of("http://127.0.0.1:4100/hook").as_deref(),
        Some("/hook")
    );
    assert_eq!(route_of("127.0.0.1:4100/hook"), None);
    assert_eq!(route_of("http://127.0.0.1:4100"), None);
}

#[test]
fn a_hook_posts_its_payload_with_the_secret_and_the_pane_and_hears_the_reply() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address").to_string();
    let root = root_listening_at(&address);

    let app = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut head = Vec::new();
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("line");
            let line = line.trim_end().to_owned();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("content-length: ") {
                length = value.parse().expect("length");
            }
            head.push(line);
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).expect("body");
        let mut stream = stream;
        let decision = r#"{"decision":"allow"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{decision}",
            decision.len()
        )
        .expect("reply");
        (head, body)
    });

    let reply = post(
        root.path(),
        br#"{"hook_event_name":"PreToolUse"}"#,
        "leaf_1",
        None,
        Duration::from_secs(5),
    );
    let (head, body) = app.join().expect("app");

    assert_eq!(head[0], "POST /hook?pane=leaf_1 HTTP/1.1");
    assert!(head.iter().any(|line| line == "x-devpit-hook: s3cret"));
    assert!(head
        .iter()
        .any(|line| line == "content-type: application/json"));
    assert_eq!(body, br#"{"hook_event_name":"PreToolUse"}"#);
    assert_eq!(
        reply.as_deref(),
        Some(br#"{"decision":"allow"}"#.as_slice())
    );
}

#[test]
fn a_devpit_that_is_gone_costs_the_hook_a_moment_not_a_hang() {
    // Bound and dropped: a port with nothing behind it.
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("address").to_string()
    };
    let root = root_listening_at(&address);
    let started = std::time::Instant::now();
    assert!(post(root.path(), b"{}", "", None, Duration::from_secs(5)).is_none());
    assert!(started.elapsed() < Duration::from_secs(2));

    let nowhere = tempfile::tempdir().expect("tempdir");
    assert!(post(nowhere.path(), b"{}", "", None, Duration::from_secs(5)).is_none());
}

/// What the Windows hook sends after a big Read: the report, not the file.
#[test]
fn a_big_report_is_cut_down_before_it_is_sent() {
    let body = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_response": { "file": { "content": "x".repeat(1024 * 1024) } },
    })
    .to_string()
    .into_bytes();
    let sent = slim(body);
    assert!(sent.len() < 1024, "{} bytes", sent.len());
    // A small one goes as it came.
    assert_eq!(slim(b"{\"a\":1}".to_vec()), b"{\"a\":1}");
}

/// The Windows hook names the chat its process runs in, as the curl line does,
/// and nothing outside one or for a value that is not one of ours.
#[test]
fn the_hook_names_the_chat_it_runs_in_and_nothing_outside_one() {
    assert_eq!(chat_line("conv_01A"), "x-devpit-chat: conv_01A\r\n");
    assert_eq!(chat_line(""), "");
    assert_eq!(chat_line("conv\r\nx-devpit-hook: guess"), "");
}
