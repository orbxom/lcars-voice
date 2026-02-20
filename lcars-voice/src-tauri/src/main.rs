//! LCARS Voice - Desktop voice recorder and transcriber.
//!
//! A Tauri v2 application that records audio via cpal, transcribes it
//! using whisper-rs (native whisper.cpp), and copies results to the clipboard.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_sources;
mod database;
mod meeting;
mod model_manager;
mod recording;
mod transcription;

use audio_sources::AudioSourceInfo;
use database::{Database, Transcription};
use meeting::{MeetingSession, TimestampMark};
use recording::Recorder;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_store::StoreExt;
use whisper_rs::{WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecordingMode {
    VoiceNote,
    Meeting,
}

const VALID_WHISPER_MODELS: &[&str] = &["base", "small", "medium", "large"];

fn is_valid_whisper_model(model: &str) -> bool {
    VALID_WHISPER_MODELS.contains(&model)
}

fn get_default_whisper_model() -> &'static str {
    "base"
}

fn truncate_preview(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}...", truncated)
    } else {
        text.to_string()
    }
}

fn resolve_whisper_model(store_value: Option<String>, env_value: Option<String>) -> String {
    store_value
        .or(env_value)
        .unwrap_or_else(|| get_default_whisper_model().to_string())
}

fn resolve_recording_mode(store_value: Option<&str>) -> RecordingMode {
    match store_value {
        Some("Meeting") => RecordingMode::Meeting,
        _ => RecordingMode::VoiceNote,
    }
}

struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
    whisper_ctx: Arc<Mutex<Option<WhisperContext>>>,
    current_model_name: Mutex<String>,
    recording_mode: Mutex<RecordingMode>,
    meeting_session: Mutex<Option<MeetingSession>>,
    is_paused: AtomicBool,
}

fn ensure_whisper_context(
    app: &tauri::AppHandle,
    state: &AppState,
    model_name: &str,
) -> Result<(), String> {
    let mut ctx_guard = state.whisper_ctx.lock().map_err(|e| e.to_string())?;
    let mut current = state.current_model_name.lock().map_err(|e| e.to_string())?;

    if ctx_guard.is_none() || *current != model_name {
        if !model_manager::is_model_downloaded(model_name) {
            model_manager::download_model(app, model_name)?;
        }
        let path = model_manager::model_path(model_name);
        let path_str = path.to_str().ok_or("Invalid model path")?;
        eprintln!("[LCARS] Loading whisper model: {}", model_name);
        let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| format!("Failed to load model: {}", e))?;
        *ctx_guard = Some(ctx);
        *current = model_name.to_string();
    }
    Ok(())
}

#[tauri::command]
fn get_history(state: State<AppState>, limit: Option<usize>) -> Result<Vec<Transcription>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_history(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn search_history(
    state: State<AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<Transcription>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.search(&query, limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_transcription(
    state: State<AppState>,
    text: String,
    duration_ms: Option<i64>,
    model: Option<String>,
) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_transcription(
        &text,
        duration_ms,
        &model.unwrap_or_else(|| "base".to_string()),
    )
    .map_err(|e| e.to_string())
}

fn handle_meeting_pause_toggle(app: &tauri::AppHandle, source: &str) {
    let state = app.state::<AppState>();
    let mut recorder = state.recorder.lock().unwrap_or_else(|e| e.into_inner());
    if recorder.is_paused() {
        match recorder.resume() {
            Ok(()) => {
                state.is_paused.store(false, Ordering::SeqCst);
                let _ = app.emit("meeting-resumed", ());
            }
            Err(e) => eprintln!("[LCARS] {}: resume failed: {}", source, e),
        }
    } else {
        match recorder.pause() {
            Ok(()) => {
                state.is_paused.store(true, Ordering::SeqCst);
                let _ = app.emit("meeting-paused", ());
            }
            Err(e) => eprintln!("[LCARS] {}: pause failed: {}", source, e),
        }
    }
}

fn handle_start_recording(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mode = *state.recording_mode.lock().map_err(|e| e.to_string())?;

    let (capture_mode, session) = if mode == RecordingMode::Meeting {
        let session = MeetingSession::new(None)?;
        eprintln!("[LCARS] Meeting recording to {:?}", session.output_dir);
        let cm = match audio_sources::find_monitor_device() {
            Ok(monitor) => recording::CaptureMode::MicAndMonitor {
                monitor_device: monitor,
            },
            Err(e) => {
                eprintln!(
                    "[LCARS] No monitor source found ({}), recording mic only",
                    e
                );
                recording::CaptureMode::MicOnly
            }
        };
        (cm, Some(session))
    } else {
        (recording::CaptureMode::MicOnly, None)
    };

    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    if let Err(e) = recorder.start(capture_mode) {
        if let Some(ref s) = session {
            let _ = std::fs::remove_dir_all(&s.output_dir);
            eprintln!("[LCARS] Cleaned up session dir after start failure");
        }
        return Err(e);
    }

    if let Some(s) = session {
        *state.meeting_session.lock().map_err(|e| e.to_string())? = Some(s);
    }

    state.is_recording.store(true, Ordering::SeqCst);
    state.is_paused.store(false, Ordering::SeqCst);
    let _ = app.emit("recording-started", ());
    send_notification(
        app,
        "LCARS Voice",
        if mode == RecordingMode::Meeting {
            "Meeting recording started"
        } else {
            "Recording started"
        },
    );

    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(recording_icon) = Image::from_bytes(include_bytes!("../icons/tray-recording.png"))
        {
            let _ = tray.set_icon(Some(recording_icon));
            let _ = tray.set_tooltip(Some("LCARS Voice - Recording"));
        }
    }

    eprintln!("[LCARS] Recording started");
    Ok(())
}

fn handle_stop_and_transcribe(app: &tauri::AppHandle) {
    eprintln!("[LCARS] Stopping recording, starting transcription");
    let state = app.state::<AppState>();
    state.is_recording.store(false, Ordering::SeqCst);
    state.is_paused.store(false, Ordering::SeqCst);

    // Update tray icon back to idle state
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(idle_icon) = Image::from_bytes(include_bytes!("../icons/tray-idle.png")) {
            let _ = tray.set_icon(Some(idle_icon));
            let _ = tray.set_tooltip(Some("LCARS Voice - Ready"));
        }
    }

    let model = get_current_model(app);
    let app_clone = app.clone();

    std::thread::spawn(move || {
        let state: tauri::State<AppState> = app_clone.state();

        // Step 1: Stop recording and get audio data
        let recording = match state.recorder.lock() {
            Ok(mut recorder) => match recorder.stop() {
                Ok(r) => r,
                Err(e) => {
                    send_notification(&app_clone, "LCARS Voice", &format!("Error: {}", e));
                    let _ = app_clone.emit("transcription-error", e);
                    return;
                }
            },
            Err(e) => {
                let _ = app_clone.emit("transcription-error", format!("Lock error: {}", e));
                return;
            }
        };

        // Check recording mode
        let mode = {
            let m = state
                .recording_mode
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *m
        };

        if mode == RecordingMode::Meeting {
            // Save meeting files
            let mut session_guard = state
                .meeting_session
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(session) = session_guard.take() {
                if let Err(e) = session.save_audio(&recording.audio_data) {
                    let _ = app_clone.emit(
                        "transcription-error",
                        format!("Failed to save audio: {}", e),
                    );
                    return;
                }
                if let Err(e) = session.save_metadata(recording.duration_ms as f64 / 1000.0) {
                    eprintln!("[LCARS] Warning: failed to save metadata: {}", e);
                }
                if let Err(e) = session.save_timestamps() {
                    eprintln!("[LCARS] Warning: failed to save timestamps: {}", e);
                }
                let output_dir = session.output_dir.to_string_lossy().to_string();
                send_notification(
                    &app_clone,
                    "LCARS Voice",
                    &format!("Meeting saved to {}", output_dir),
                );
                let _ = app_clone.emit("meeting-saved", output_dir);
            }
        } else {
            let _ = app_clone.emit("transcribing", ());

            // Step 2: Ensure whisper model is loaded (may trigger download)
            if let Err(e) = ensure_whisper_context(&app_clone, &state, &model) {
                send_notification(&app_clone, "LCARS Voice", &format!("Model error: {}", e));
                let _ = app_clone.emit("transcription-error", e);
                return;
            }

            // Step 3: Transcribe
            let ctx_guard = match state.whisper_ctx.lock() {
                Ok(g) => g,
                Err(e) => {
                    let _ = app_clone.emit("transcription-error", format!("Lock error: {}", e));
                    return;
                }
            };

            let ctx = match ctx_guard.as_ref() {
                Some(c) => c,
                None => {
                    let _ = app_clone.emit(
                        "transcription-error",
                        "Whisper context not loaded".to_string(),
                    );
                    return;
                }
            };

            match transcription::transcribe(ctx, &recording.audio_data, &model) {
                Ok(result) => {
                    // Step 4: Save to DB with real duration
                    if let Ok(db) = state.db.lock() {
                        let _ =
                            db.add_transcription(&result.text, Some(recording.duration_ms), &model);
                    }

                    // Step 5: Notify and emit
                    let preview = truncate_preview(&result.text, 50);
                    send_notification(&app_clone, "LCARS Voice", &preview);
                    let _ = app_clone.emit("transcription-complete", result.text);
                }
                Err(e) => {
                    send_notification(&app_clone, "LCARS Voice", &format!("Error: {}", e));
                    let _ = app_clone.emit("transcription-error", e);
                }
            }
        }
    });
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle) -> Result<(), String> {
    handle_start_recording(&app)
}

#[tauri::command]
async fn get_whisper_model(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let store_value = store
        .get("whisper_model")
        .and_then(|v| v.as_str().map(String::from));
    let env_value = std::env::var("WHISPER_MODEL").ok();
    Ok(resolve_whisper_model(store_value, env_value))
}

#[tauri::command]
async fn set_whisper_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    if !is_valid_whisper_model(&model) {
        return Err(format!(
            "Invalid model: {}. Valid options: {:?}",
            model, VALID_WHISPER_MODELS
        ));
    }
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("whisper_model", serde_json::json!(model));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

fn get_current_model(app: &tauri::AppHandle) -> String {
    let store_value = app
        .store("settings.json")
        .ok()
        .and_then(|s| s.get("whisper_model"))
        .and_then(|v| v.as_str().map(String::from));
    let env_value = std::env::var("WHISPER_MODEL").ok();
    resolve_whisper_model(store_value, env_value)
}

fn send_notification(app: &tauri::AppHandle, title: &str, body: &str) {
    match app.notification().builder().title(title).body(body).show() {
        Ok(_) => eprintln!("[LCARS] notification: Sent '{}' - '{}'", title, body),
        Err(e) => eprintln!("[LCARS] notification: Failed: {:?}", e),
    }
}

#[tauri::command]
fn stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    handle_stop_and_transcribe(&app);
    Ok(())
}

#[tauri::command]
fn is_model_downloaded(model: String) -> bool {
    model_manager::is_model_downloaded(&model)
}

#[tauri::command]
async fn download_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        model_manager::download_model(&app, &model)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
fn get_audio_level(state: State<AppState>) -> Result<f32, String> {
    let recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    Ok(recorder.current_rms_level())
}

#[tauri::command]
fn get_recording_mode(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mode = store
        .get("recording_mode")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "VoiceNote".to_string());
    Ok(mode)
}

#[tauri::command]
fn set_recording_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Cannot change mode while recording".to_string());
    }
    match mode.as_str() {
        "VoiceNote" | "Meeting" => {}
        _ => return Err(format!("Invalid mode: {}", mode)),
    }
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("recording_mode", serde_json::json!(mode));
    store.save().map_err(|e| e.to_string())?;
    *state.recording_mode.lock().map_err(|e| e.to_string())? = if mode == "Meeting" {
        RecordingMode::Meeting
    } else {
        RecordingMode::VoiceNote
    };
    Ok(())
}

#[tauri::command]
fn pause_recording(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.pause()?;
    state.is_paused.store(true, Ordering::SeqCst);
    let _ = app.emit("meeting-paused", ());
    Ok(())
}

#[tauri::command]
fn resume_recording(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.resume()?;
    state.is_paused.store(false, Ordering::SeqCst);
    let _ = app.emit("meeting-resumed", ());
    Ok(())
}

#[tauri::command]
fn add_timestamp_mark(
    state: State<AppState>,
    ticket: Option<String>,
    note: Option<String>,
) -> Result<TimestampMark, String> {
    let recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    let elapsed = recorder.elapsed_seconds() as u64;
    drop(recorder);

    let mut session = state.meeting_session.lock().map_err(|e| e.to_string())?;
    let session = session.as_mut().ok_or("No active meeting session")?;
    let mark = session.timestamps.add_mark(elapsed, ticket, note);
    Ok(mark)
}

#[tauri::command]
fn get_timestamp_marks(state: State<AppState>) -> Result<Vec<TimestampMark>, String> {
    let session = state.meeting_session.lock().map_err(|e| e.to_string())?;
    match session.as_ref() {
        Some(s) => Ok(s.timestamps.get_marks().to_vec()),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
fn list_audio_sources() -> Vec<AudioSourceInfo> {
    audio_sources::enumerate_sources()
}

#[tauri::command]
fn get_elapsed_time(state: State<AppState>) -> Result<f64, String> {
    let recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    Ok(recorder.elapsed_seconds())
}

fn main() {
    eprintln!("[LCARS] Application starting");
    let db = Database::new().expect("Failed to initialize database");
    let recorder = Recorder::new();

    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(recorder),
        is_recording: AtomicBool::new(false),
        whisper_ctx: Arc::new(Mutex::new(None)),
        current_model_name: Mutex::new(String::new()),
        recording_mode: Mutex::new(RecordingMode::VoiceNote),
        meeting_session: Mutex::new(None),
        is_paused: AtomicBool::new(false),
    };

    let socket_path = dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("lcars-voice.sock");
    let socket_path_for_setup = socket_path.clone();

    let hotkey = Shortcut::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::KeyH);
    let hotkey_for_handler = hotkey.clone();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    // Only handle key press, not release
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }

                    if shortcut != &hotkey_for_handler {
                        return;
                    }

                    eprintln!("[LCARS] hotkey: Super+Alt+H pressed");
                    let state = app.state::<AppState>();
                    let was_recording = state.is_recording.load(Ordering::SeqCst);

                    if was_recording {
                        let mode = {
                            let m = state
                                .recording_mode
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            *m
                        };

                        if mode == RecordingMode::Meeting {
                            handle_meeting_pause_toggle(app, "hotkey");
                        } else {
                            handle_stop_and_transcribe(app);
                        }
                    } else {
                        if let Err(e) = handle_start_recording(app) {
                            eprintln!("[LCARS] hotkey: Failed to start recording: {}", e);
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_history,
            search_history,
            add_transcription,
            start_recording,
            stop_recording,
            get_whisper_model,
            set_whisper_model,
            is_model_downloaded,
            download_model,
            get_audio_level,
            get_recording_mode,
            set_recording_mode,
            pause_recording,
            resume_recording,
            add_timestamp_mark,
            get_timestamp_marks,
            list_audio_sources,
            get_elapsed_time,
        ])
        .setup(move |app| {
            // Set window icon
            if let Some(window) = app.get_webview_window("main") {
                let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
                    .expect("Failed to load window icon");
                let _ = window.set_icon(icon);
            }

            match app.global_shortcut().register(hotkey) {
                Ok(_) => eprintln!("[LCARS] setup: Hotkey Super+Alt+H registered"),
                Err(e) => eprintln!("[LCARS] setup: Failed to register hotkey: {:?}", e),
            }

            // Set up Unix socket toggle listener for external control
            let socket_path = socket_path_for_setup;
            // Clean up stale socket
            let _ = std::fs::remove_file(&socket_path);

            let app_handle = app.handle().clone();

            // Sync recording mode from persisted store
            {
                let state = app.state::<AppState>();
                if let Ok(store) = app.store("settings.json") {
                    let store_mode = store
                        .get("recording_mode")
                        .and_then(|v| v.as_str().map(String::from));
                    let mode = resolve_recording_mode(store_mode.as_deref());
                    *state
                        .recording_mode
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = mode;
                    eprintln!("[LCARS] setup: Recording mode synced to {:?}", mode);
                }
            }

            tauri::async_runtime::spawn(async move {
                let listener = match tokio::net::UnixListener::bind(&socket_path) {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[LCARS] toggle: Failed to bind socket: {}", e);
                        return;
                    }
                };
                eprintln!("[LCARS] toggle: Listening on {:?}", socket_path);
                loop {
                    if let Ok((mut stream, _)) = listener.accept().await {
                        use tokio::io::AsyncReadExt;
                        let mut buf = [0u8; 64];
                        if let Ok(n) = stream.read(&mut buf).await {
                            let msg = String::from_utf8_lossy(&buf[..n]);
                            if msg.trim() == "toggle" {
                                eprintln!("[LCARS] toggle: Socket command received");
                                let state = app_handle.state::<AppState>();
                                let was_recording = state.is_recording.load(Ordering::SeqCst);
                                if was_recording {
                                    let mode = {
                                        let m = state
                                            .recording_mode
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner());
                                        *m
                                    };
                                    if mode == RecordingMode::Meeting {
                                        handle_meeting_pause_toggle(&app_handle, "toggle");
                                    } else {
                                        handle_stop_and_transcribe(&app_handle);
                                    }
                                } else {
                                    if let Err(e) = handle_start_recording(&app_handle) {
                                        eprintln!("[LCARS] toggle: Failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            });

            // Load tray icons
            let idle_icon = Image::from_path("icons/tray-idle.png").unwrap_or_else(|_| {
                Image::from_bytes(include_bytes!("../icons/tray-idle.png")).unwrap()
            });

            // Build tray
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(idle_icon)
                .tooltip("LCARS Voice - Ready")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Clean up socket on normal exit (SIGKILL/crashes handled by toggle script timeout)
    eprintln!("[LCARS] Cleaning up socket on exit");
    let _ = std::fs::remove_file(&socket_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_whisper_models() {
        // Valid models should be accepted
        assert!(is_valid_whisper_model("base"));
        assert!(is_valid_whisper_model("small"));
        assert!(is_valid_whisper_model("medium"));
        assert!(is_valid_whisper_model("large"));
    }

    #[test]
    fn test_invalid_whisper_models() {
        // Invalid models should be rejected
        assert!(!is_valid_whisper_model("tiny"));
        assert!(!is_valid_whisper_model("xlarge"));
        assert!(!is_valid_whisper_model(""));
        assert!(!is_valid_whisper_model("BASE")); // case sensitive
    }

    #[test]
    fn test_default_whisper_model() {
        assert_eq!(get_default_whisper_model(), "base");
    }

    #[test]
    fn test_model_fallback_chain() {
        // When no store value and no env var, should return "base"
        let model = resolve_whisper_model(None, None);
        assert_eq!(model, "base");

        // When store has value, use it
        let model = resolve_whisper_model(Some("medium".to_string()), None);
        assert_eq!(model, "medium");

        // When store is empty but env var set, use env var
        let model = resolve_whisper_model(None, Some("large".to_string()));
        assert_eq!(model, "large");

        // Store takes precedence over env var
        let model = resolve_whisper_model(Some("small".to_string()), Some("large".to_string()));
        assert_eq!(model, "small");
    }

    #[test]
    fn test_truncate_preview_short_text() {
        assert_eq!(truncate_preview("hello", 50), "hello");
    }

    #[test]
    fn test_truncate_preview_exact_length() {
        let text = "a".repeat(50);
        assert_eq!(truncate_preview(&text, 50), text);
    }

    #[test]
    fn test_truncate_preview_long_text() {
        let text = "a".repeat(100);
        let expected = format!("{}...", "a".repeat(50));
        assert_eq!(truncate_preview(&text, 50), expected);
    }

    #[test]
    fn test_truncate_preview_unicode() {
        // Each character is multi-byte but should be counted as 1 char
        let text = "日本語のテスト文字列はこちらです。これは五十文字以上のテストです。もっと長いテキストが必要です。";
        let result = truncate_preview(text, 10);
        assert!(result.ends_with("..."));
        // Should be 10 chars + "..."
        assert_eq!(result.chars().count(), 13); // 10 + 3 for "..."
    }

    #[test]
    fn test_truncate_preview_emoji() {
        let text = "🎤🎤🎤🎤🎤🎤🎤🎤🎤🎤🎤"; // 11 emoji
        let result = truncate_preview(text, 5);
        assert_eq!(result, "🎤🎤🎤🎤🎤...");
    }

    // Phase 3 tests: model readiness and AppState initialization

    #[test]
    fn test_nonexistent_model_not_downloaded() {
        // ensure_whisper_context relies on is_model_downloaded;
        // a nonexistent model name should not be considered downloaded
        assert!(!model_manager::is_model_downloaded(
            "nonexistent-test-model"
        ));
    }

    #[test]
    fn test_model_path_for_valid_models() {
        // Verify model_path returns sensible paths for all valid models
        for model in VALID_WHISPER_MODELS {
            let path = model_manager::model_path(model);
            let path_str = path.to_string_lossy();
            assert!(
                path_str.contains(&format!("ggml-{}.bin", model)),
                "model_path('{}') should contain 'ggml-{}.bin', got: {}",
                model,
                model,
                path_str
            );
        }
    }

    #[test]
    fn test_app_state_initial_whisper_ctx_is_none() {
        // Verify AppState initializes with no whisper context loaded
        let db = Database::new().expect("Failed to create test db");
        let state = AppState {
            db: Mutex::new(db),
            recorder: Mutex::new(Recorder::new()),
            is_recording: AtomicBool::new(false),
            whisper_ctx: Arc::new(Mutex::new(None)),
            current_model_name: Mutex::new(String::new()),
            recording_mode: Mutex::new(RecordingMode::VoiceNote),
            meeting_session: Mutex::new(None),
            is_paused: AtomicBool::new(false),
        };
        assert!(state.whisper_ctx.lock().unwrap().is_none());
        assert_eq!(*state.current_model_name.lock().unwrap(), "");
        assert!(!state.is_recording.load(Ordering::SeqCst));
    }

    #[test]
    fn test_app_state_recording_flag_toggle() {
        let db = Database::new().expect("Failed to create test db");
        let state = AppState {
            db: Mutex::new(db),
            recorder: Mutex::new(Recorder::new()),
            is_recording: AtomicBool::new(false),
            whisper_ctx: Arc::new(Mutex::new(None)),
            current_model_name: Mutex::new(String::new()),
            recording_mode: Mutex::new(RecordingMode::VoiceNote),
            meeting_session: Mutex::new(None),
            is_paused: AtomicBool::new(false),
        };
        assert!(!state.is_recording.load(Ordering::SeqCst));
        state.is_recording.store(true, Ordering::SeqCst);
        assert!(state.is_recording.load(Ordering::SeqCst));
        state.is_recording.store(false, Ordering::SeqCst);
        assert!(!state.is_recording.load(Ordering::SeqCst));
    }

    #[test]
    fn test_resolve_recording_mode_none() {
        assert_eq!(resolve_recording_mode(None), RecordingMode::VoiceNote);
    }
    #[test]
    fn test_resolve_recording_mode_voice_note() {
        assert_eq!(
            resolve_recording_mode(Some("VoiceNote")),
            RecordingMode::VoiceNote
        );
    }
    #[test]
    fn test_resolve_recording_mode_meeting() {
        assert_eq!(
            resolve_recording_mode(Some("Meeting")),
            RecordingMode::Meeting
        );
    }
    #[test]
    fn test_resolve_recording_mode_invalid() {
        assert_eq!(
            resolve_recording_mode(Some("Invalid")),
            RecordingMode::VoiceNote
        );
    }

    #[test]
    fn test_all_valid_models_have_download_urls() {
        // Every model in VALID_WHISPER_MODELS should have a download URL
        for model in VALID_WHISPER_MODELS {
            assert!(
                model_manager::get_model_url(model).is_some(),
                "Valid model '{}' should have a download URL",
                model
            );
        }
    }
}
