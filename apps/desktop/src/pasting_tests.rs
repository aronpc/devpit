use super::*;

#[test]
fn only_pictures_recordings_and_text_are_taken() {
    assert_eq!(extension_for("image/png"), Some("png"));
    assert_eq!(extension_for("image/jpeg"), Some("jpg"));
    assert_eq!(extension_for("text/html"), None);
    assert_eq!(extension_for("application/octet-stream"), None);
    assert_eq!(extension_for("audio/webm;codecs=opus"), Some("webm"));
    assert_eq!(extension_for("audio/mp4"), Some("m4a"));
    assert_eq!(extension_for("audio/x-anything"), None);
    assert_eq!(extension_for("text/plain"), Some("txt"));
    assert_eq!(extension_for("text/plain;charset=utf-8"), Some("txt"));
}

#[test]
fn a_small_paste_decodes() {
    assert_eq!(decoded("aGk=", MOST_BYTES).expect("decoded"), b"hi");
}

/// Refused on its length, before a byte is decoded: the ceiling is what keeps
/// a paste from being held whole in memory.
#[test]
fn a_paste_past_the_ceiling_is_refused_before_decoding() {
    let huge = "A".repeat(MOST_BYTES / 3 * 4 + 8);
    let refused = decoded(&huge, MOST_BYTES).expect_err("refused");
    assert_eq!(refused.code, ErrorCode::Invalid);
}

/// The longest recording, as the WAV a window makes, is past a picture's
/// ceiling and within a recording's.
#[test]
fn the_longest_recording_fits_where_a_picture_that_size_would_not() {
    let longest = wav_bytes(LONGEST_RECORDING_SECONDS);
    assert!(longest > most_for("image/png"));
    assert!(longest <= most_for("audio/wav"));
    let encoded = "A".repeat(longest / 3 * 4);
    assert!(decoded(&encoded, most_for("audio/wav")).is_ok());
    assert!(decoded(&encoded, most_for("image/png")).is_err());
}

#[test]
fn text_that_is_not_base64_is_refused() {
    assert!(decoded("not base64 at all!", MOST_BYTES).is_err());
}

#[test]
fn a_project_id_that_is_a_path_is_refused() {
    let refused = chat_paste_now("../x".to_owned(), "image/png".to_owned(), "aGk=".to_owned())
        .expect_err("refused");
    assert_eq!(refused.code, ErrorCode::Forbidden);
}
