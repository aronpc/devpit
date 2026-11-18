//! A voice message, made into words before it is sent.
//!
//! By a whisper on this machine when there is one — whisper.cpp, or the
//! `whisper` and `whisper-ctranslate2` commands — and by a service only when
//! the person chose one and gave it a key. Nothing leaves the machine on its
//! own: the engine is local unless set, and without one the recording is
//! sent as it is, a file the agent can open.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use devpit_core::{preference, Store};
use devpit_rpc::{ErrorCode, RpcError, Transcribing, Transcript};

/// The longest a local transcription may take before it is given up.
const AT_MOST: Duration = Duration::from_secs(180);

/// The most of a reply read from a service.
const MOST_REPLY: usize = 256 * 1024;

const OPENAI: &str = "https://api.openai.com/v1";
const API_MODEL: &str = "gpt-4o-mini-transcribe";
const WHISPER_MODEL: &str = "base";

/// The whispers looked for, in order, and whether each reads a model file.
const LOCAL: [(&str, bool); 4] = [
    ("whisper-cli", true),
    ("whisper-cpp", true),
    ("whisper-ctranslate2", false),
    ("whisper", false),
];

/// The first of `LOCAL` on the person's PATH.
fn local_whisper() -> Option<(&'static str, bool, PathBuf)> {
    LOCAL
        .iter()
        .find_map(|(name, file)| on_path(name).map(|path| (*name, *file, path)))
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = devpit_pty::login_path::search_path();
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.cmd")]
    } else {
        vec![name.to_owned()]
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |one| dir.join(one)))
        .find(|candidate| candidate.is_file())
}

fn key_file() -> Result<PathBuf, RpcError> {
    Ok(Store::root()?.join("transcribe-key"))
}

fn read_settings(store: &Store) -> Result<Transcribing, RpcError> {
    let read =
        |key: &str| -> Result<String, RpcError> { Ok(store.preference(key)?.unwrap_or_default()) };
    let engine = read(preference::TRANSCRIBE)?;
    Ok(Transcribing {
        engine: if engine.is_empty() {
            "local".to_owned()
        } else {
            engine
        },
        model: read(preference::TRANSCRIBE_MODEL)?,
        language: read(preference::TRANSCRIBE_LANGUAGE)?,
        url: read(preference::TRANSCRIBE_URL)?,
        key_set: key_file()?.is_file(),
        local: local_whisper().map(|(name, _, _)| name.to_owned()),
    })
}

/// Whether a recording would be heard here: a whisper with the model file it
/// needs, or a service with its key. The Remote offers its mic on this.
pub(crate) fn ready(settings: &Transcribing, is_file: impl Fn(&str) -> bool) -> bool {
    match settings.engine.as_str() {
        "api" => settings.key_set,
        "local" => settings.local.as_deref().is_some_and(|name| {
            let needs_file = LOCAL.iter().any(|(one, file)| *one == name && *file);
            !needs_file || is_file(&settings.model)
        }),
        _ => false,
    }
}

/// [`ready`], for this machine as Settings left it.
pub(crate) fn voice_ready() -> bool {
    let Ok(store) = Store::open_default() else {
        return false;
    };
    read_settings(&store).is_ok_and(|settings| ready(&settings, |model| Path::new(model).is_file()))
}

/// `transcribe.read` — how voice messages become words here.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_read() -> Result<Transcribing, RpcError> {
    crate::off_main::blocking(|| read_settings(&Store::open_default()?)).await
}

/// `transcribe.set` — the engine, its model, the language and the address.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_set(
    engine: String,
    model: String,
    language: String,
    url: String,
) -> Result<Transcribing, RpcError> {
    crate::off_main::blocking(move || {
        if !["local", "api", "off"].contains(&engine.as_str()) {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "the engine is local, api or off",
            ));
        }
        let language = language.trim().to_lowercase();
        if !language.is_empty() && !spoken(&language) {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "a language is a code such as en or pt",
            ));
        }
        let url = url.trim().trim_end_matches('/').to_owned();
        if !url.is_empty()
            && !url.starts_with("https://")
            && !url.starts_with("http://127.0.0.1")
            && !url.starts_with("http://localhost")
        {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "the address is https, or a service on this machine",
            ));
        }
        let store = Store::open_default()?;
        store.set_preference(preference::TRANSCRIBE, &engine)?;
        store.set_preference(preference::TRANSCRIBE_MODEL, model.trim())?;
        store.set_preference(preference::TRANSCRIBE_LANGUAGE, &language)?;
        store.set_preference(preference::TRANSCRIBE_URL, &url)?;
        read_settings(&store)
    })
    .await
}

/// `transcribe.key_set` — keeps the service's key in a private file, or
/// forgets it. It is never read back to the window.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_key_set(key: Option<String>) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        let file = key_file()?;
        match key
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
        {
            Some(key) if key.len() > 512 || key.chars().any(char::is_whitespace) => {
                Err(RpcError::new(ErrorCode::Invalid, "that is not a key"))
            }
            Some(key) => Ok(devpit_core::home::write_private(&file, key.as_bytes())
                .map_err(|err| RpcError::internal(err.to_string()))?),
            None => match std::fs::remove_file(&file) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                    Err(RpcError::internal(err.to_string()))
                }
                _ => Ok(()),
            },
        }
    })
    .await
}

/// A language code as whisper and the services read it: `pt`, `en`, `yue`.
pub(crate) fn spoken(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase())
}

/// A recording's name as the composer kept it: nothing that reaches out of
/// its folder.
pub(crate) fn recording(name: &str) -> bool {
    name.starts_with("voice-")
        && !name.contains(['/', '\\'])
        && !name.contains("..")
        && ["webm", "ogg", "m4a", "wav"]
            .iter()
            .any(|ext| name.ends_with(&format!(".{ext}")))
}

/// `chat.transcribe` — a recording kept by the composer, in words.
///
/// `language` is the window's guess from the system when the settings name
/// none. A recording that cannot be heard is not an error: it is sent as a
/// file, and the note says why.
#[tauri::command]
#[specta::specta]
pub async fn chat_transcribe(
    project_id: String,
    name: String,
    language: Option<String>,
) -> Result<Transcript, RpcError> {
    if project_id.contains(['/', '\\']) || project_id.starts_with('.') || !recording(&name) {
        return Err(RpcError::new(
            ErrorCode::Forbidden,
            "that is not a recording",
        ));
    }
    let audio = crate::projects::project_home(&project_id)?
        .pasted()
        .join(&name);
    heard_in(&audio, language).await
}

/// What the engine chosen in Settings hears in `audio`, as a transcript.
pub(crate) async fn heard_in(
    audio: &std::path::Path,
    language: Option<String>,
) -> Result<Transcript, RpcError> {
    let audio = audio.to_path_buf();
    let settings = crate::off_main::blocking(|| read_settings(&Store::open_default()?)).await?;
    let language = Some(settings.language.clone())
        .filter(|code| !code.is_empty())
        .or(language.map(|code| code.to_lowercase()))
        .filter(|code| spoken(code));
    let heard = match settings.engine.as_str() {
        "api" => api(&settings, &audio, language.as_deref()).await,
        "local" => {
            let model = settings.model.clone();
            crate::off_main::blocking(move || Ok(local(&audio, &model, language.as_deref())))
                .await?
        }
        _ => Err("voice messages are not transcribed — choose an engine in Settings".to_owned()),
    };
    Ok(match heard {
        Ok(text) if !text.trim().is_empty() => Transcript {
            text: Some(text.trim().to_owned()),
            note: None,
        },
        Ok(_) => Transcript {
            text: None,
            note: Some("nothing was heard in it".to_owned()),
        },
        Err(why) => Transcript {
            text: None,
            note: Some(why),
        },
    })
}

/// `transcribe.test` — a few seconds recorded in Settings, heard by the engine
/// chosen there and thrown away: the microphone, its permission and the engine,
/// diagnosed at once.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_test(
    media_type: String,
    data: String,
    language: Option<String>,
) -> Result<Transcript, RpcError> {
    let ext = crate::pasting::extension_for(&media_type)
        .filter(|_| media_type.starts_with("audio/"))
        .ok_or_else(|| RpcError::new(ErrorCode::Invalid, "that is not a recording"))?;
    let bytes = crate::pasting::decoded(&data, crate::pasting::MOST_RECORDING_BYTES)?;
    let audio = Store::root()?.join(format!("voice-test.{ext}"));
    std::fs::write(&audio, bytes).map_err(|err| RpcError::internal(err.to_string()))?;
    let heard = heard_in(&audio, language).await;
    let _ = std::fs::remove_file(&audio);
    heard
}

/// Heard by a whisper on this machine, working in a folder beside the
/// recording that goes when it is done.
fn local(audio: &Path, model: &str, language: Option<&str>) -> Result<String, String> {
    let stem = audio
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = audio.with_file_name(format!("{stem}-heard"));
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let heard = local_in(audio, &stem, &dir, model, language);
    let _ = std::fs::remove_dir_all(&dir);
    heard
}

fn local_in(
    audio: &Path,
    stem: &str,
    dir: &Path,
    model: &str,
    language: Option<&str>,
) -> Result<String, String> {
    let (name, needs_file, whisper) = local_whisper().ok_or(
        "no whisper on this machine — install whisper.cpp, or choose a service in Settings",
    )?;
    if needs_file {
        if model.is_empty() || !Path::new(model).is_file() {
            return Err(format!(
                "{name} needs its model file — choose it in Settings"
            ));
        }
        // whisper.cpp reads 16 kHz mono WAV.
        let wav = dir.join("voice.wav");
        let ffmpeg = on_path("ffmpeg").ok_or("whisper.cpp needs ffmpeg to read a recording")?;
        let mut convert = devpit_pty::host_env::command(ffmpeg);
        convert
            .args(["-nostdin", "-loglevel", "error", "-y", "-i"])
            .arg(audio)
            .args(["-ar", "16000", "-ac", "1"])
            .arg(&wav);
        run(&mut convert)?;
        let mut hear = devpit_pty::host_env::command(whisper);
        hear.arg("-m").arg(model).arg("-f").arg(&wav).args([
            "-nt",
            "-np",
            "-l",
            language.unwrap_or("auto"),
        ]);
        return run(&mut hear);
    }
    let mut hear = devpit_pty::host_env::command(whisper);
    hear.arg(audio)
        .args([
            "--model",
            if model.is_empty() {
                WHISPER_MODEL
            } else {
                model
            },
        ])
        .args(["--output_format", "txt", "--output_dir"])
        .arg(dir)
        .args(["--verbose", "False"]);
    if let Some(language) = language {
        hear.args(["--language", language]);
    }
    run(&mut hear)?;
    std::fs::read_to_string(dir.join(format!("{stem}.txt")))
        .map_err(|_| format!("{name} wrote no transcript"))
}

/// Runs a command to its end or to `AT_MOST`, and answers with what it printed.
fn run(command: &mut std::process::Command) -> Result<String, String> {
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| err.to_string())?;
    let began = Instant::now();
    loop {
        match child.try_wait().map_err(|err| err.to_string())? {
            Some(_) => break,
            None if began.elapsed() > AT_MOST => {
                let _ = child.kill();
                return Err("the transcription took too long, and was stopped".to_owned());
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let out = child.wait_with_output().map_err(|err| err.to_string())?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let last = said
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("it failed");
        return Err(format!(
            "the transcription failed: {}",
            last.chars().take(200).collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Heard by the service the person chose, with their key.
async fn api(
    settings: &Transcribing,
    audio: &Path,
    language: Option<&str>,
) -> Result<String, String> {
    let key = std::fs::read_to_string(key_file().map_err(|err| err.message)?)
        .map_err(|_| "the service has no key — add it in Settings".to_owned())?;
    let bytes = std::fs::read(audio).map_err(|err| err.to_string())?;
    let base = if settings.url.is_empty() {
        OPENAI
    } else {
        &settings.url
    };
    let model = if settings.model.is_empty() {
        API_MODEL
    } else {
        &settings.model
    };
    let name = audio
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (boundary, body) = multipart(&name, &bytes, model, language);
    let reply = crate::account::client()
        .map_err(|err| err.message)?
        .post(format!("{base}/audio/transcriptions"))
        .bearer_auth(key.trim())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|err| format!("the service could not be reached: {err}"))?;
    let status = reply.status();
    let text = reply.text().await.map_err(|err| err.to_string())?;
    let text: String = text.chars().take(MOST_REPLY).collect();
    let read: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    if !status.is_success() {
        let why = read["error"]["message"]
            .as_str()
            .unwrap_or("no reason given");
        return Err(format!("the service refused it ({status}): {why}"));
    }
    read["text"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "the service sent no text".to_owned())
}

/// A form with the file, the model and the language, as the services read it.
pub(crate) fn multipart(
    name: &str,
    bytes: &[u8],
    model: &str,
    language: Option<&str>,
) -> (String, Vec<u8>) {
    let boundary = format!("devpit-{}", ulid::Ulid::generate());
    let mut body = Vec::new();
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\ncontent-disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    };
    field("model", model);
    if let Some(language) = language {
        field("language", language);
    }
    let kind = match name.rsplit('.').next() {
        Some("ogg") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        Some("wav") => "audio/wav",
        _ => "audio/webm",
    };
    body.extend_from_slice(
        format!("--{boundary}\r\ncontent-disposition: form-data; name=\"file\"; filename=\"{name}\"\r\ncontent-type: {kind}\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

/// whisper.cpp's base model, from its own release on Hugging Face.
const BASE_MODEL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
/// The most a model download may be: the base one is about 150 MB.
const MOST_MODEL_BYTES: u64 = 400 * 1_048_576;

/// `transcribe.model_download` — fetches whisper.cpp's base model into
/// devpit's own folder and chooses it, so the local engine works without a
/// file hunted down by hand.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_model_download() -> Result<Transcribing, RpcError> {
    let folder = Store::root()?.join("models");
    std::fs::create_dir_all(&folder).map_err(|err| RpcError::internal(err.to_string()))?;
    let path = folder.join("ggml-base.bin");
    let part = folder.join("ggml-base.bin.part");
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .user_agent(concat!("devpit/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| RpcError::internal(err.to_string()))?;
    let unreachable = |err: reqwest::Error| {
        RpcError::new(
            ErrorCode::Conflict,
            format!("the model could not be downloaded: {err}"),
        )
    };
    let mut response = client
        .get(BASE_MODEL)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(unreachable)?;
    if response
        .content_length()
        .is_some_and(|size| size > MOST_MODEL_BYTES)
    {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            "that model is larger than any whisper base model",
        ));
    }
    let mut file =
        std::fs::File::create(&part).map_err(|err| RpcError::internal(err.to_string()))?;
    let mut written: u64 = 0;
    while let Some(chunk) = response.chunk().await.map_err(unreachable)? {
        written += chunk.len() as u64;
        if written > MOST_MODEL_BYTES {
            let _ = std::fs::remove_file(&part);
            return Err(RpcError::new(
                ErrorCode::Invalid,
                "that model is larger than any whisper base model",
            ));
        }
        std::io::Write::write_all(&mut file, &chunk)
            .map_err(|err| RpcError::internal(err.to_string()))?;
    }
    drop(file);
    std::fs::rename(&part, &path).map_err(|err| RpcError::internal(err.to_string()))?;
    crate::off_main::blocking(move || {
        let store = Store::open_default()?;
        store.set_preference(preference::TRANSCRIBE_MODEL, &path.display().to_string())?;
        read_settings(&store)
    })
    .await
}

#[cfg(test)]
#[path = "transcribe_tests.rs"]
mod tests;
