#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

fn main() {
    let is_recording = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(move |app| {
            let recording_state = is_recording.clone();
            let app_handle = app.handle().clone();

            // Register Super+H hotkey
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyH);
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                let was_recording = recording_state.fetch_xor(true, Ordering::SeqCst);
                if was_recording {
                    // Was recording, now stopping
                    let _ = app_handle.emit("recording-stopped", ());
                } else {
                    // Was idle, now starting
                    let _ = app_handle.emit("recording-started", ());
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
