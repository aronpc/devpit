use devpit_core::locale::Locale;

use super::told_words;

#[test]
fn the_tray_notice_is_said_in_english() {
    let (title, body) = told_words(Locale::En);
    assert_eq!(title, "devpit is still running");
    assert!(body.contains("tray"), "{body}");
}

#[test]
fn the_tray_notice_is_said_in_portuguese() {
    let (title, body) = told_words(Locale::PtBr);
    assert_eq!(title, "O devpit continua rodando");
    assert!(body.contains("bandeja"), "{body}");
}
