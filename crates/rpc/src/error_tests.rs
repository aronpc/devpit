//! The error's words follow the person's language; its code and its report do not.

use super::*;
use devpit_core::locale::Locale;
use devpit_core::{StoreError, TreeError};

/// `Locale::current()` is one global; the tests that move it take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn settled(locale: Locale) -> std::sync::MutexGuard<'static, ()> {
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    locale.set();
    turn
}

fn outside() -> TreeError {
    TreeError::Outside { path: "/x".into() }
}

#[test]
fn tree_errors_speak_pt_br_and_stay_identical_in_english() {
    let _turn = settled(Locale::PtBr);
    assert_eq!(outside().to_string(), "/x está fora do projeto");
    assert_eq!(
        TreeError::AlreadyExists { path: "/x".into() }.to_string(),
        "/x já existe"
    );
    Locale::En.set();
    assert_eq!(outside().to_string(), "/x is outside the project");
    assert_eq!(
        TreeError::AlreadyExists { path: "/x".into() }.to_string(),
        "/x already exists"
    );
}

#[test]
fn a_busy_store_error_is_translated_and_keeps_its_code() {
    let _turn = settled(Locale::PtBr);
    let pt = RpcError::from(StoreError::AlreadyRunning);
    assert_eq!(pt.code, ErrorCode::Busy);
    assert_eq!(pt.message, "já há uma execução em andamento neste card");
    Locale::En.set();
    let en = RpcError::from(StoreError::AlreadyRunning);
    assert_eq!(en.code, ErrorCode::Busy);
    assert_eq!(en.message, "a run is already going on this card");
}

#[test]
fn internal_stays_english_whatever_the_language() {
    let _turn = settled(Locale::PtBr);
    let err = RpcError::from(StoreError::NoDataDirectory);
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(err.message, "this platform exposes no user data directory");
    let io = std::io::Error::other("boom");
    let unreadable = TreeError::Unreadable {
        path: "/x".into(),
        source: io,
    };
    assert_eq!(unreadable.said(Locale::En), "could not read /x: boom");
}
