//! A picture pasted into the composer, or text too long to type, kept where
//! the agent can read it.
//!
//! Written into the devpit workspace and never into the project: a paste is
//! not a file anybody asked to commit, and a repository that grows a folder of
//! screenshots every time someone shares one is a repository being littered.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use devpit_rpc::{Attachment, ErrorCode, RpcError};

/// The most a pasted picture may be. Checked on the encoded length before it
/// is decoded, so an oversized paste is refused without ever being held whole.
pub(crate) const MOST_BYTES: usize = 8 * 1024 * 1024;

/// The longest a voice message runs: `LONGEST_MS` in `web/src/shell/recorder.ts`.
pub(crate) const LONGEST_RECORDING_SECONDS: usize = 5 * 60;

/// The size of a recording as the WAV a window makes where its webview writes
/// no compressed format the services take: a 44-byte header, then 16 kHz of
/// 16-bit mono samples (`WAV_RATE` and `wavOf` in `web/src/shell/recorder.ts`).
pub(crate) const fn wav_bytes(seconds: usize) -> usize {
    44 + 16_000 * 2 * seconds
}

/// The most a recording may be: the longest one as WAV (9.6 MB), with room to
/// spare, and still well under the 25 MB the services accept.
pub(crate) const MOST_RECORDING_BYTES: usize = 12 * 1024 * 1024;
const _: () = assert!(wav_bytes(LONGEST_RECORDING_SECONDS) <= MOST_RECORDING_BYTES);

/// The ceiling for a paste of this type.
pub(crate) fn most_for(media_type: &str) -> usize {
    if media_type.starts_with("audio/") {
        MOST_RECORDING_BYTES
    } else {
        MOST_BYTES
    }
}

/// The extension a pasted type is saved under, or nothing for a type this
/// does not take — pictures, the voice messages the composer records, and a
/// paste of text too long to sit in the composer.
/// A recording's type comes with its codec (`audio/webm;codecs=opus`).
pub(crate) fn extension_for(media_type: &str) -> Option<&'static str> {
    match media_type.split(';').next().unwrap_or_default().trim() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "audio/webm" => Some("webm"),
        "audio/ogg" => Some("ogg"),
        "audio/mp4" => Some("m4a"),
        "audio/wav" => Some("wav"),
        "text/plain" => Some("txt"),
        _ => None,
    }
}

/// Decodes a paste, refusing it before allocation when it is past `most`.
pub(crate) fn decoded(data: &str, most: usize) -> Result<Vec<u8>, RpcError> {
    // Base64 is four characters for three bytes.
    if data.len() / 4 * 3 > most {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            format!(
                "that paste is past the {} MB a paste takes",
                most / 1_048_576
            ),
        ));
    }
    BASE64
        .decode(data)
        .map_err(|_| RpcError::new(ErrorCode::Invalid, "that paste is not a picture"))
}

/// `chat.paste` — keeps a pasted picture and answers with it as an attachment.
#[tauri::command]
#[specta::specta]
pub async fn chat_paste(
    project_id: String,
    media_type: String,
    data: String,
) -> Result<Attachment, RpcError> {
    crate::off_main::blocking(move || chat_paste_now(project_id, media_type, data)).await
}

/// [`chat_paste`], on the calling thread.
pub(crate) fn chat_paste_now(
    project_id: String,
    media_type: String,
    data: String,
) -> Result<Attachment, RpcError> {
    // The id names a folder; a separator in it would name another one.
    if project_id.contains(['/', '\\']) || project_id.starts_with('.') {
        return Err(RpcError::new(ErrorCode::Forbidden, "that is not a project"));
    }
    let ext = extension_for(&media_type).ok_or_else(|| {
        RpcError::new(
            ErrorCode::Invalid,
            "only pictures, recordings and text can be pasted",
        )
    })?;
    let bytes = decoded(&data, most_for(&media_type))?;
    let dir = crate::projects::project_home(&project_id)?.pasted();
    kept_in(&dir, &media_type, ext, &bytes)
}

/// Writes a paste into `dir` under a name devpit makes, never one it was
/// given, so nothing written lands outside it.
pub(crate) fn kept_in(
    dir: &std::path::Path,
    media_type: &str,
    ext: &str,
    bytes: &[u8],
) -> Result<Attachment, RpcError> {
    std::fs::create_dir_all(dir).map_err(|err| RpcError::internal(err.to_string()))?;
    // A recording is named as one, which is what transcribing it asks for.
    let what = if media_type.starts_with("audio/") {
        "voice"
    } else {
        "pasted"
    };
    let name = format!(
        "{what}-{}.{ext}",
        ulid::Ulid::generate().to_string().to_lowercase()
    );
    let path = dir.join(&name);
    std::fs::write(&path, bytes).map_err(|err| RpcError::internal(err.to_string()))?;
    Ok(Attachment {
        name,
        // Absolute: the file is outside the project, and the agent reads `@`
        // paths as they are written.
        path: path.display().to_string(),
        kind: ext.to_owned(),
    })
}

#[cfg(test)]
#[path = "pasting_tests.rs"]
mod tests;
