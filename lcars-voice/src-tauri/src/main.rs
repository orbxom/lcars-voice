//! LCARS Voice - Desktop voice recorder and transcriber.
//!
//! A Tauri v2 application that records audio via arecord, transcribes it
//! using OpenAI Whisper, and copies results to the clipboard.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod database;
mod recording;
mod transcription;

use database::{Database, Transcription};
use recording::Recorder;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_store::StoreExt;

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

struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
    venv_path: PathBuf,
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

fn handle_start_recording(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.start()?;
    state.is_recording.store(true, Ordering::SeqCst);
    let _ = app.emit("recording-started", ());
    send_notification(app, "LCARS Voice", "Recording started");

    // Update tray icon to recording state
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

        let audio_path = match state.recorder.lock() {
            Ok(mut recorder) => recorder.stop(),
            Err(e) => {
                let _ = app_clone.emit("transcription-error", format!("Lock error: {}", e));
                return;
            }
        };

        match audio_path {
            Ok(path) => {
                let _ = app_clone.emit("transcribing", ());
                let resource_dir = app_clone.path().resource_dir().ok();
                let result = transcription::transcribe(
                    &path,
                    &model,
                    &state.venv_path,
                    resource_dir.as_deref(),
                );
                match result {
                    Ok(text) => {
                        if let Ok(db) = state.db.lock() {
                            let _ = db.add_transcription(&text, None, &model);
                        }
                        let preview = truncate_preview(&text, 50);
                        send_notification(&app_clone, "LCARS Voice", &preview);
                        let _ = app_clone.emit("transcription-complete", text);
                    }
                    Err(e) => {
                        send_notification(&app_clone, "LCARS Voice", &format!("Error: {}", e));
                        let _ = app_clone.emit("transcription-error", e);
                    }
                }
            }
            Err(e) => {
                send_notification(&app_clone, "LCARS Voice", &format!("Error: {}", e));
                let _ = app_clone.emit("transcription-error", e);
            }
        }
    });
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle) -> Result<(), String> {
    handle_start_recording(&app)
}

#[tauri::command]
async fn transcribe_audio(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    audio_path: String,
) -> Result<String, String> {
    let path_str = audio_path.clone();
    let venv = state.venv_path.clone();
    let model = get_current_model(&app);
    let resource_dir = app.path().resource_dir().ok();

    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&path_str);
        transcription::transcribe(path, &model, &venv, resource_dir.as_deref())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
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

fn send_notification(_app: &tauri::AppHandle, title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        match std::process::Command::new("notify-send")
            .arg(&title)
            .arg(&body)
            .status()
        {
            Ok(status) if status.success() => {
                eprintln!("[LCARS] notification: Sent '{}' - '{}'", title, body)
            }
            Ok(status) => {
                eprintln!(
                    "[LCARS] notification: notify-send failed with status: {}",
                    status
                )
            }
            Err(e) => eprintln!("[LCARS] notification: Failed to run notify-send: {:?}", e),
        }
    });
}

#[tauri::command]
fn stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    handle_stop_and_transcribe(&app);
    Ok(())
}

fn main() {
    eprintln!("[LCARS] Application starting");
    let db = Database::new().expect("Failed to initialize database");
    let recorder = Recorder::new();
    let venv_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-to-text-env");

    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(recorder),
        is_recording: AtomicBool::new(false),
        venv_path,
    };

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
                        handle_stop_and_transcribe(app);
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
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_history,
            search_history,
            add_transcription,
            start_recording,
            stop_recording,
            transcribe_audio,
            get_whisper_model,
            set_whisper_model
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

            // Set up file-based toggle watcher for external control
            let toggle_file = std::path::PathBuf::from("/tmp/lcars-voice-toggle");
            // Clean up any existing toggle file
            let _ = std::fs::remove_file(&toggle_file);

            let app_handle = app.handle().clone();
            let toggle_path = toggle_file.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if toggle_path.exists() {
                    let _ = std::fs::remove_file(&toggle_path);
                    eprintln!("[LCARS] toggle: File detected, toggling recording");

                    let state = app_handle.state::<AppState>();
                    let was_recording = state.is_recording.load(Ordering::SeqCst);

                    if was_recording {
                        handle_stop_and_transcribe(&app_handle);
                    } else {
                        if let Err(e) = handle_start_recording(&app_handle) {
                            eprintln!("[LCARS] toggle: Failed to start recording: {}", e);
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
}
