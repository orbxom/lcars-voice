use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: Option<String>,
    pub language: Option<String>,
    pub error: Option<String>,
}

pub fn transcribe(audio_path: &Path, model: &str, venv_path: &Path) -> Result<String, String> {
    println!("[LCARS] transcription: transcribe() called with path = {:?}", audio_path);
    println!("[LCARS] transcription: model = {}, venv_path = {:?}", model, venv_path);

    let python_path = venv_path.join("bin").join("python3");
    println!("[LCARS] transcription: python_path = {:?}", python_path);

    // Get the whisper wrapper script path
    // In dev: exe at src-tauri/target/debug/lcars-voice, script at lcars-voice/scripts/
    // In prod: script should be bundled as resource
    let script_path = std::env::current_exe()
        .ok()
        .and_then(|p| {
            // Go up from exe to find project root
            p.parent() // target/debug
                .and_then(|p| p.parent()) // target
                .and_then(|p| p.parent()) // src-tauri
                .and_then(|p| p.parent()) // lcars-voice
                .map(|p| p.join("scripts").join("whisper-wrapper.py"))
        })
        .unwrap_or_else(|| Path::new("scripts/whisper-wrapper.py").to_path_buf());
    println!("[LCARS] transcription: script_path = {:?}", script_path);

    println!("[LCARS] transcription: Running python command...");
    let output = Command::new(&python_path)
        .args([
            script_path.to_str().ok_or("Invalid script path")?,
            audio_path.to_str().ok_or("Invalid audio path")?,
            model,
        ])
        .output()
        .map_err(|e| {
            println!("[LCARS] transcription: Failed to run whisper: {}", e);
            format!("Failed to run whisper: {}", e)
        })?;

    println!("[LCARS] transcription: Command completed, status = {:?}", output.status);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[LCARS] transcription: Process failed, stderr = {}", stderr);
        return Err(format!(
            "Whisper process failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("[LCARS] transcription: stdout = {}", stdout);

    let result: TranscriptionResult = serde_json::from_str(&stdout)
        .map_err(|e| {
            println!("[LCARS] transcription: Failed to parse JSON: {}", e);
            format!("Failed to parse whisper output: {} - raw: {}", e, stdout)
        })?;

    if let Some(error) = result.error {
        println!("[LCARS] transcription: Whisper returned error: {}", error);
        return Err(error);
    }

    let text = result.text.ok_or_else(|| "No transcription text".to_string())?;
    println!("[LCARS] transcription: Success, text length = {}", text.len());
    Ok(text)
}
