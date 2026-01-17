#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod database;
mod recording;

use database::{Database, Transcription};
use recording::Recorder;
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
fn start_recording(state: State<AppState>) -> Result<(), String> {
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.start()
}

#[tauri::command]
fn stop_recording(state: State<AppState>) -> Result<String, String> {
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    let path = recorder.stop()?;
    Ok(path.to_string_lossy().to_string())
}

fn main() {
    let db = Database::new().expect("Failed to initialize database");
    let recorder = Recorder::new();
    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(recorder),
        is_recording: AtomicBool::new(false),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_history,
            search_history,
            add_transcription,
            start_recording,
            stop_recording
        ])
        .setup(move |app| {
            // Register Super+H hotkey
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyH);
            app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, _event| {
                let state = app.state::<AppState>();
                let was_recording = state.is_recording.load(Ordering::SeqCst);

                if was_recording {
                    // Stop recording
                    if let Ok(mut recorder) = state.recorder.lock() {
                        if let Ok(audio_path) = recorder.stop() {
                            state.is_recording.store(false, Ordering::SeqCst);
                            let _ = app.emit("recording-stopped", audio_path.to_string_lossy().to_string());
                        }
                    }
                } else {
                    // Start recording
                    if let Ok(mut recorder) = state.recorder.lock() {
                        if recorder.start().is_ok() {
                            state.is_recording.store(true, Ordering::SeqCst);
                            let _ = app.emit("recording-started", ());
                        }
                    }
                }
            })?;

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
