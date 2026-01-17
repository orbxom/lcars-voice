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
    model: String,
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
    db.add_transcription(&text, duration_ms, &model.unwrap_or_else(|| "base".to_string()))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    println!("[LCARS] command: start_recording called");
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    let result = recorder.start();
    println!("[LCARS] command: start_recording result = {:?}", result);
    if result.is_ok() {
        state.is_recording.store(true, std::sync::atomic::Ordering::SeqCst);
        println!("[LCARS] event: Emitting 'recording-started' from command");
        let _ = app.emit("recording-started", ());
    }
    result
}

#[tauri::command]
async fn transcribe_audio(state: State<'_, AppState>, audio_path: String) -> Result<String, String> {
    let path_str = audio_path.clone();
    let venv = state.venv_path.clone();
    let model = state.model.clone();

    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&path_str);
        transcription::transcribe(path, &model, &venv)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn get_whisper_model(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let store_value = store.get("whisper_model").and_then(|v| v.as_str().map(String::from));
    let env_value = std::env::var("WHISPER_MODEL").ok();
    Ok(resolve_whisper_model(store_value, env_value))
}

#[tauri::command]
async fn set_whisper_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    if !is_valid_whisper_model(&model) {
        return Err(format!("Invalid model: {}. Valid options: {:?}", model, VALID_WHISPER_MODELS));
    }
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("whisper_model", serde_json::json!(model));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn stop_recording(app: tauri::AppHandle, state: State<AppState>) -> Result<String, String> {
    println!("[LCARS] command: stop_recording called");
    state.is_recording.store(false, std::sync::atomic::Ordering::SeqCst);

    let audio_path = {
        let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
        recorder.stop()?
    };
    println!("[LCARS] command: stop_recording path = {:?}", audio_path);

    // Emit transcribing event and start transcription in background
    println!("[LCARS] event: Emitting 'transcribing' from command");
    let _ = app.emit("transcribing", ());

    let venv = state.venv_path.clone();
    let model = state.model.clone();
    let path_clone = audio_path.clone();
    let app_clone = app.clone();
    let state_model = state.model.clone();

    std::thread::spawn(move || {
        println!("[LCARS] thread: Transcription thread started from command");
        let result = transcription::transcribe(&path_clone, &model, &venv);

        match result {
            Ok(text) => {
                println!("[LCARS] thread: Transcription successful, text length = {}", text.len());
                // Save to database
                let state: State<AppState> = app_clone.state();
                if let Ok(db) = state.db.lock() {
                    let _ = db.add_transcription(&text, None, &state_model);
                    println!("[LCARS] thread: Added transcription to history");
                }
                println!("[LCARS] event: Emitting 'transcription-complete' from command");
                let _ = app_clone.emit("transcription-complete", text);
            }
            Err(e) => {
                println!("[LCARS] thread: Transcription error = {}", e);
                println!("[LCARS] event: Emitting 'transcription-error' from command");
                let _ = app_clone.emit("transcription-error", e);
            }
        }
    });

    Ok(audio_path.to_string_lossy().to_string())
}

fn main() {
    println!("[LCARS] main: Application starting");
    let db = Database::new().expect("Failed to initialize database");
    println!("[LCARS] main: Database initialized");
    let recorder = Recorder::new();
    println!("[LCARS] main: Recorder initialized");
    let venv_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-to-text-env");
    println!("[LCARS] main: venv_path = {:?}", venv_path);

    let model = std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "base".to_string());
    println!("[LCARS] main: Using whisper model = {}", model);

    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(recorder),
        is_recording: AtomicBool::new(false),
        venv_path,
        model,
    };

    let hotkey = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyH);
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

                    println!("[LCARS] hotkey: Ctrl+Shift+H pressed");
                    let state = app.state::<AppState>();
                    let was_recording = state.is_recording.load(Ordering::SeqCst);
                    println!("[LCARS] hotkey: was_recording = {}", was_recording);

                    if was_recording {
                        // Stop recording and transcribe
                        println!("[LCARS] hotkey: Stopping recording and transcribing");
                        state.is_recording.store(false, Ordering::SeqCst);
                        println!("[LCARS] state: is_recording set to false");
                        let app_clone = app.clone();
                        std::thread::spawn(move || {
                            println!("[LCARS] thread: Transcription thread started");
                            let state: State<AppState> = app_clone.state();

                            // Stop recording
                            let audio_path = {
                                match state.recorder.lock() {
                                    Ok(mut recorder) => recorder.stop(),
                                    Err(e) => {
                                        let _ = app_clone.emit("transcription-error", format!("Lock error: {}", e));
                                        return;
                                    }
                                }
                            };

                            match audio_path {
                                Ok(path) => {
                                    println!("[LCARS] thread: Audio path = {:?}", path);
                                    println!("[LCARS] event: Emitting 'transcribing'");
                                    let _ = app_clone.emit("transcribing", ());

                                    // Transcribe
                                    let result = transcription::transcribe(
                                        &path,
                                        &state.model,
                                        &state.venv_path,
                                    );

                                    match result {
                                        Ok(text) => {
                                            println!("[LCARS] thread: Transcription successful, text length = {}", text.len());
                                            // Add to history
                                            if let Ok(db) = state.db.lock() {
                                                let _ = db.add_transcription(&text, None, &state.model);
                                                println!("[LCARS] thread: Added transcription to history");
                                            }

                                            // Copy to clipboard (via frontend)
                                            println!("[LCARS] event: Emitting 'transcription-complete'");
                                            let _ = app_clone.emit("transcription-complete", text);
                                        }
                                        Err(e) => {
                                            println!("[LCARS] thread: Transcription error = {}", e);
                                            println!("[LCARS] event: Emitting 'transcription-error'");
                                            let _ = app_clone.emit("transcription-error", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("[LCARS] thread: Stop recording error = {}", e);
                                    println!("[LCARS] event: Emitting 'transcription-error'");
                                    let _ = app_clone.emit("transcription-error", e);
                                }
                            }
                        });
                    } else {
                        // Start recording
                        println!("[LCARS] hotkey: Starting recording");
                        if let Ok(mut recorder) = state.recorder.lock() {
                            match recorder.start() {
                                Ok(()) => {
                                    state.is_recording.store(true, Ordering::SeqCst);
                                    println!("[LCARS] state: is_recording set to true");
                                    println!("[LCARS] event: Emitting 'recording-started'");
                                    let _ = app.emit("recording-started", ());
                                }
                                Err(e) => {
                                    println!("[LCARS] hotkey: Failed to start recording = {}", e);
                                }
                            }
                        } else {
                            println!("[LCARS] hotkey: Failed to lock recorder");
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
            println!("[LCARS] setup: Registering Ctrl+Shift+H hotkey");
            match app.global_shortcut().register(hotkey) {
                Ok(_) => println!("[LCARS] setup: Hotkey registered successfully"),
                Err(e) => println!("[LCARS] setup: Failed to register hotkey = {:?}", e),
            }

            // Set up file-based toggle watcher for external control
            eprintln!("[LCARS] setup: Setting up toggle file watcher");
            let toggle_file = std::path::PathBuf::from("/tmp/lcars-voice-toggle");
            // Clean up any existing toggle file
            let _ = std::fs::remove_file(&toggle_file);

            let app_handle = app.handle().clone();
            let toggle_path = toggle_file.clone();
            std::thread::spawn(move || {
                println!("[LCARS] toggle: Watcher thread started, watching {:?}", toggle_path);
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if toggle_path.exists() {
                        let _ = std::fs::remove_file(&toggle_path);
                        println!("[LCARS] toggle: File detected - toggling recording");

                        let state = app_handle.state::<AppState>();
                        let was_recording = state.is_recording.load(Ordering::SeqCst);

                        if was_recording {
                            state.is_recording.store(false, Ordering::SeqCst);
                            let app_clone = app_handle.clone();
                            std::thread::spawn(move || {
                                let state: State<AppState> = app_clone.state();
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
                                        let result = transcription::transcribe(&path, &state.model, &state.venv_path);
                                        match result {
                                            Ok(text) => {
                                                if let Ok(db) = state.db.lock() {
                                                    let _ = db.add_transcription(&text, None, &state.model);
                                                }
                                                let _ = app_clone.emit("transcription-complete", text);
                                            }
                                            Err(e) => { let _ = app_clone.emit("transcription-error", e); }
                                        }
                                    }
                                    Err(e) => { let _ = app_clone.emit("transcription-error", e); }
                                }
                            });
                        } else {
                            if let Ok(mut recorder) = state.recorder.lock() {
                                if recorder.start().is_ok() {
                                    state.is_recording.store(true, Ordering::SeqCst);
                                    let _ = app_handle.emit("recording-started", ());
                                }
                            }
                        }
                    }
                }
            });

            // Load tray icons
            let idle_icon = Image::from_path("icons/tray-idle.png")
                .unwrap_or_else(|_| Image::from_bytes(include_bytes!("../icons/tray-idle.png")).unwrap());

            // Build tray
            let _tray = TrayIconBuilder::new()
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
}
