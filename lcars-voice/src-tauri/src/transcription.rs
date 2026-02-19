//! Bridge to Python OpenAI Whisper for audio transcription.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: Option<String>,
    pub language: Option<String>,
    pub error: Option<String>,
}

pub fn transcribe(
    audio_path: &Path,
    model: &str,
    venv_path: &Path,
    resource_dir: Option<&Path>,
) -> Result<String, String> {
    eprintln!(
        "[LCARS] transcription: path={:?}, model={}",
        audio_path, model
    );

    let python_path = venv_path.join("bin").join("python3");

    // Try resource directory first (production), then fall back to dev path
    let script_path = resource_dir
        .map(|dir| dir.join("scripts").join("whisper-wrapper.py"))
        .filter(|p| p.exists())
        .or_else(|| {
            // Dev fallback: walk up from exe location
            std::env::current_exe()
                .ok()
                .and_then(|p| {
                    p.parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .map(|p| p.join("scripts").join("whisper-wrapper.py"))
                })
                .filter(|p| p.exists())
        })
        .unwrap_or_else(|| Path::new("scripts/whisper-wrapper.py").to_path_buf());

    let output = Command::new(&python_path)
        .args([
            script_path.to_str().ok_or("Invalid script path")?,
            audio_path.to_str().ok_or("Invalid audio path")?,
            model,
        ])
        .output()
        .map_err(|e| {
            eprintln!("[LCARS] transcription: Failed to run whisper: {}", e);
            format!("Failed to run whisper: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "[LCARS] transcription: Whisper failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        );
        return Err(format!(
            "Whisper process failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let result: TranscriptionResult = serde_json::from_str(&stdout).map_err(|e| {
        eprintln!("[LCARS] transcription: Failed to parse JSON: {}", e);
        format!("Failed to parse whisper output: {} - raw: {}", e, stdout)
    })?;

    if let Some(error) = result.error {
        eprintln!("[LCARS] transcription: Whisper error: {}", error);
        return Err(error);
    }

    let text = result
        .text
        .ok_or_else(|| "No transcription text".to_string())?;
    eprintln!("[LCARS] transcription: Success, {} chars", text.len());
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_success_json() {
        let json = r#"{"text": "hello world", "language": "en"}"#;
        let result: TranscriptionResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.text, Some("hello world".to_string()));
        assert_eq!(result.language, Some("en".to_string()));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_parse_error_json() {
        let json = r#"{"error": "something went wrong"}"#;
        let result: TranscriptionResult = serde_json::from_str(json).unwrap();
        assert!(result.text.is_none());
        assert_eq!(result.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_parse_missing_text() {
        let json = r#"{"language": "en"}"#;
        let result: TranscriptionResult = serde_json::from_str(json).unwrap();
        assert!(result.text.is_none());
        assert!(result.error.is_none());
    }

    #[test]
    fn test_transcribe_missing_python() {
        // Using a bogus venv path should give a clear error
        let bogus_venv = Path::new("/tmp/nonexistent-venv-lcars-test");
        let bogus_audio = Path::new("/tmp/nonexistent-audio.wav");
        let result = transcribe(bogus_audio, "base", bogus_venv, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Failed to run whisper") || err.contains("No such file"),
            "Expected error about missing python, got: {}",
            err
        );
    }
}
