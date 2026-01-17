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
    let python_path = venv_path.join("bin").join("python3");

    // Get the whisper wrapper script path - look relative to exe or in current dir
    let script_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|p| p.join("scripts").join("whisper-wrapper.py"))
        .unwrap_or_else(|| Path::new("scripts/whisper-wrapper.py").to_path_buf());

    let output = Command::new(&python_path)
        .args([
            script_path.to_str().ok_or("Invalid script path")?,
            audio_path.to_str().ok_or("Invalid audio path")?,
            model,
        ])
        .output()
        .map_err(|e| format!("Failed to run whisper: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Whisper process failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: TranscriptionResult = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse whisper output: {} - raw: {}", e, stdout))?;

    if let Some(error) = result.error {
        return Err(error);
    }

    result.text.ok_or_else(|| "No transcription text".to_string())
}
