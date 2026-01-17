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
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

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
                // Note: Can't easily access db here, but that's okay for now
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

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_history,
            search_history,
            add_transcription,
            start_recording,
            stop_recording,
            transcribe_audio
        ])
        .setup(move |app| {
            println!("[LCARS] setup: Registering Super+H hotkey");
            // Register Super+H hotkey
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyH);
            match app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, _event| {
                println!("[LCARS] hotkey: Super+H pressed");
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
            }) {
                Ok(_) => println!("[LCARS] setup: Hotkey registered successfully"),
                Err(e) => println!("[LCARS] setup: Failed to register hotkey = {:?}", e),
            }

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
