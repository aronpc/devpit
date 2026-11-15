//! What the hook listener accepts, and what it refuses to read.
//!
//! Fed bytes rather than a socket. The rules worth pinning here are the ones
//! that decide how much memory a request gets to ask for, and those do not
//! need a port to exercise.

use crate::post::{read_post, MOST_BYTES, MOST_HEADERS, MOST_HEADER_BYTES, MOST_HOOK_BYTES};

/// A request, assembled the way `curl --data-binary` sends one.
fn post(body: &str) -> Vec<u8> {
    format!(
        "POST /hook HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\n\
         content-length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// The same, with headers of the caller's choosing in front of the length.
fn post_with(headers: &[&str], body: &str) -> Vec<u8> {
    let mut raw = String::from("POST /hook HTTP/1.1\r\nhost: 127.0.0.1\r\n");
    for header in headers {
        raw.push_str(header);
        raw.push_str("\r\n");
    }
    raw.push_str(&format!("content-length: {}\r\n\r\n{body}", body.len()));
    raw.into_bytes()
}

/// The secret is read whatever case the header name arrives in and whatever
/// padding sits around the value — curl writes what the file says, and the
/// file is written by hand often enough to be worth being kind about.
#[test]
fn the_secret_is_read_whatever_its_case_or_padding() {
    for header in [
        "x-devpit-hook: 6f1c",
        "X-DevPit-Hook: 6f1c",
        "X-DEVPIT-HOOK:   6f1c   ",
        "x-devpit-hook :6f1c",
    ] {
        let raw = post_with(&[header], "{}");
        assert_eq!(
            read_post(&raw[..], None).and_then(|posted| posted.secret),
            Some("6f1c".to_owned()),
            "{header}"
        );
    }
}

/// No header at all, and a header with nothing in it, are different answers:
/// the second is a post that showed something and got it wrong.
#[test]
fn a_post_with_no_secret_carries_none_and_an_empty_one_carries_empty() {
    let raw = post("{}");
    assert_eq!(
        read_post(&raw[..], None).and_then(|posted| posted.secret),
        None
    );

    let raw = post_with(&["x-devpit-hook:"], "{}");
    assert_eq!(
        read_post(&raw[..], None).and_then(|posted| posted.secret),
        Some(String::new())
    );
}

/// `read_line` grows until it finds a newline. A sender that never sends one
/// would otherwise decide how much memory this process spends — the same rule
/// the body has always had, one line above it.
#[test]
fn a_header_line_has_a_ceiling() {
    let long = format!("x-devpit-hook: {}", "a".repeat(MOST_HEADER_BYTES + 1));
    let raw = post_with(&[&long], "{}");
    assert_eq!(read_post(&raw[..], None), None);

    // One byte under it still reads.
    let short = format!("x-devpit-hook: {}", "a".repeat(64));
    let raw = post_with(&[&short], "{}");
    assert!(read_post(&raw[..], None).is_some());
}

/// And a request cannot arrive with ten thousand short headers either.
#[test]
fn a_wall_of_headers_is_refused() {
    let many: Vec<String> = (0..MOST_HEADERS + 5)
        .map(|at| format!("x-{at}: 1"))
        .collect();
    let borrowed: Vec<&str> = many.iter().map(String::as_str).collect();
    let raw = post_with(&borrowed, "{}");
    assert_eq!(read_post(&raw[..], None), None);
}

#[test]
fn a_well_formed_post_gives_up_its_body() {
    let body = r#"{"hook_event_name":"Stop","session_id":"s1"}"#;
    let raw = post(body);
    assert_eq!(
        read_post(&raw[..], None).map(|posted| posted.body),
        Some(body.to_owned())
    );
}

/// A header can arrive in any case, and `Content-Length` is how most senders
/// spell it. Matching only the lowercase form would drop every one of them.
#[test]
fn the_length_header_is_read_whatever_its_case() {
    let raw = b"POST /hook HTTP/1.1\r\nContent-Length: 2\r\n\r\nhi";
    assert_eq!(
        read_post(&raw[..], None).map(|posted| posted.body),
        Some("hi".to_owned())
    );
}

/// The rule this file exists for.
///
/// The length is a number somebody else wrote, and the next line after the
/// check allocates exactly that many bytes. Without the ceiling, one request
/// claiming a gigabyte is one gigabyte asked for.
///
/// The body is sent in full, all of it. The first version of this test sent
/// the header alone, which passes either way: with the ceiling the length is
/// refused, and without it the short body fails the read. Two paths, one
/// answer, and a test that cannot tell them apart — the same mistake this
/// file was written to stop making.
#[test]
fn a_length_past_the_ceiling_is_refused_before_anything_is_allocated() {
    let body = "x".repeat(MOST_BYTES + 1);
    let raw = format!(
        "POST /hook HTTP/1.1\r\ncontent-length: {}\r\n\r\n{body}",
        MOST_BYTES + 1
    );
    assert_eq!(read_post(raw.as_bytes(), None), None);
}

#[test]
fn a_length_at_the_ceiling_is_still_read() {
    let body = "x".repeat(MOST_BYTES);
    let raw = format!("POST /hook HTTP/1.1\r\ncontent-length: {MOST_BYTES}\r\n\r\n{body}");
    assert_eq!(
        read_post(raw.as_bytes(), None).map(|posted| posted.body),
        Some(body)
    );
}

#[test]
fn a_request_with_no_body_is_nothing_to_read() {
    let raw = b"POST /hook HTTP/1.1\r\ncontent-length: 0\r\n\r\n";
    assert_eq!(read_post(&raw[..], None), None);
}

/// A length that is not a number ends the read rather than defaulting to one.
/// Defaulting would turn a malformed header into a body of the wrong size.
#[test]
fn a_length_that_is_not_a_number_is_refused() {
    let raw = b"POST /hook HTTP/1.1\r\ncontent-length: lots\r\n\r\nhi";
    assert_eq!(read_post(&raw[..], None), None);
}

/// Connection closed mid-headers. `read_line` returns zero and the loop has
/// to stop, not spin.
#[test]
fn a_truncated_request_ends_rather_than_spins() {
    let raw = b"POST /hook HTTP/1.1\r\ncontent-length: 5\r\n";
    assert_eq!(read_post(&raw[..], None), None);
}

/// The body arrives shorter than the header promised. Reading it as far as it
/// goes would hand half a payload to the parser.
#[test]
fn a_body_shorter_than_promised_is_refused() {
    let raw = b"POST /hook HTTP/1.1\r\ncontent-length: 40\r\n\r\nshort";
    assert_eq!(read_post(&raw[..], None), None);
}

/// The pane the hook fired in, which is the whole reason the query exists:
/// an agent's own reports arrive from a process the app never spawned, and
/// this is what ties them back to the terminal somebody is looking at.
#[test]
fn the_pane_rides_in_the_query() {
    let raw = b"POST /hook?pane=leaf_01ABC HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi";
    assert_eq!(
        read_post(&raw[..], None).and_then(|posted| posted.pane),
        Some("leaf_01ABC".to_owned())
    );
}

/// A headless turn belongs to a card, not to a pane, and posts no query at
/// all. It must go on working exactly as it did.
#[test]
fn a_post_with_no_query_names_no_pane() {
    let raw = b"POST /hook HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi";
    assert_eq!(
        read_post(&raw[..], None).and_then(|posted| posted.pane),
        None
    );
}

/// This value arrives from a shell we wrote, through a process we did not,
/// and goes on to key a map and reach the screen. Anything that is not shaped
/// like one of our leaf ids is refused rather than sanitised into one.
#[test]
fn a_pane_that_is_not_shaped_like_ours_is_refused() {
    for target in [
        "/hook?pane=../../etc/passwd",
        "/hook?pane=leaf%20one",
        "/hook?pane=",
        "/hook?pane=<script>",
    ] {
        let raw = format!("POST {target} HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi");
        assert_eq!(
            read_post(raw.as_bytes(), None).and_then(|posted| posted.pane),
            None,
            "{target} was accepted"
        );
    }
}

/// The query is not always the last thing on the line, and other parameters
/// may sit beside it.
#[test]
fn the_pane_is_found_beside_other_parameters() {
    let raw = b"POST /hook?x=1&pane=leaf_two HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi";
    assert_eq!(
        read_post(&raw[..], None).and_then(|posted| posted.pane),
        Some("leaf_two".to_owned())
    );
}

/// An agent's question and a hook's report arrive on the same door and are
/// told apart by the path, before anything reads the body.
#[test]
fn a_post_to_agent_is_a_question_and_to_hook_a_report() {
    let asked = read_post(
        std::io::Cursor::new("POST /agent HTTP/1.1\r\ncontent-length: 2\r\n\r\n{}".as_bytes()),
        None,
    )
    .expect("read");
    assert!(asked.agent);
    let told = read_post(
        std::io::Cursor::new(
            "POST /hook?pane=leaf_1 HTTP/1.1\r\ncontent-length: 2\r\n\r\n{}".as_bytes(),
        ),
        None,
    )
    .expect("read");
    assert!(!told.agent);
    // A path that merely starts with the word is neither.
    let neither = read_post(
        std::io::Cursor::new("POST /agents HTTP/1.1\r\ncontent-length: 2\r\n\r\n{}".as_bytes()),
        None,
    )
    .expect("read");
    assert!(!neither.agent);
}

/// A Read of a big file reported back: past the small ceiling, but from a
/// hook that showed the secret. It is read, and cut down to what devpit uses —
/// refused, the report was lost and its step stayed running.
#[test]
fn a_big_report_from_our_hook_is_read_and_cut_down() {
    let file = "x".repeat(MOST_BYTES * 4);
    let body = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "session_id": "s1",
        "tool_name": "Read",
        "tool_input": { "file_path": "/w/big.log" },
        "tool_response": { "file": { "content": file } },
    })
    .to_string();
    let raw = post_with(&["x-devpit-hook: 6f1c"], &body);

    let posted = read_post(&raw[..], Some("6f1c")).expect("read");
    assert!(posted.body.len() < 4 * 1024, "{} bytes", posted.body.len());
    assert!(posted.body.contains("\"tool_name\":\"Read\""));
    assert!(!posted.body.contains("tool_response"));

    // Without the secret — or with the wrong one — the small ceiling holds.
    assert_eq!(read_post(&raw[..], None), None);
    let wrong = post_with(&["x-devpit-hook: 0000"], &body);
    assert_eq!(read_post(&wrong[..], Some("6f1c")), None);
    assert!(body.len() < MOST_HOOK_BYTES);
}

/// Gemini CLI says which agent it is in the query, and its report arrives in
/// Claude Code's words; an event devpit does not listen to says nothing.
#[test]
fn a_gemini_report_is_read_in_claude_codes_words() {
    let body =
        r#"{"hook_event_name":"AfterAgent","session_id":"g1","cwd":"/w","prompt_response":"Done"}"#;
    let raw = format!(
        "POST /hook?from=gemini&pane=leaf_1 HTTP/1.1\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    );
    let posted = read_post(raw.as_bytes(), None).expect("read");
    assert_eq!(posted.pane.as_deref(), Some("leaf_1"));
    assert!(
        posted.body.contains(r#""hook_event_name":"Stop""#),
        "{}",
        posted.body
    );

    let model = r#"{"hook_event_name":"AfterModel","session_id":"g1"}"#;
    let raw = format!(
        "POST /hook?from=gemini HTTP/1.1\r\ncontent-length: {}\r\n\r\n{model}",
        model.len()
    );
    assert_eq!(
        read_post(raw.as_bytes(), None).map(|posted| posted.body),
        Some("{}".to_owned())
    );

    // Without the word, the same body is read as it came.
    let raw = format!(
        "POST /hook HTTP/1.1\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    );
    assert_eq!(
        read_post(raw.as_bytes(), None).map(|posted| posted.body),
        Some(body.to_owned())
    );
}

/// The chat a post came from is read only when it looks like one of ours:
/// it goes on to decide which window is offered the question.
#[test]
fn the_chat_a_post_names_is_read_only_when_it_looks_like_ours() {
    let chat = |headers: &[&str]| {
        read_post(&post_with(headers, "{}")[..], None).and_then(|posted| posted.chat)
    };
    assert_eq!(
        chat(&["X-Devpit-Chat:  conv_01A "]),
        Some("conv_01A".to_owned())
    );
    assert_eq!(chat(&[]), None);
    assert_eq!(chat(&["x-devpit-chat:"]), None);
    assert_eq!(chat(&["x-devpit-chat: conv/../a"]), None);
}
