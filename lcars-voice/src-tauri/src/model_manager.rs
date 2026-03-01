//! Manages downloading and locating whisper.cpp GGML model files.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::Emitter;

const VAD_MODEL_URL: &str = "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin";
const VAD_MODEL_FILENAME: &str = "ggml-silero-v6.2.0.bin";

const MODEL_URLS: &[(&str, &str)] = &[
    (
        "base",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
    ),
    (
        "small",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
    ),
    (
        "medium",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
    ),
    (
        "large",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
    ),
];

/// Returns the directory where models are stored.
pub fn models_dir() -> PathBuf {
    crate::paths::app_data_dir().join("models")
}

/// Returns the expected path for a given model name.
pub fn model_path(model_name: &str) -> PathBuf {
    models_dir().join(format!("ggml-{}.bin", model_name))
}

/// Returns true if the model file exists on disk.
pub fn is_model_downloaded(model_name: &str) -> bool {
    model_path(model_name).exists()
}

/// Looks up the download URL for a model name.
pub fn get_model_url(model_name: &str) -> Option<&'static str> {
    MODEL_URLS
        .iter()
        .find(|(name, _)| *name == model_name)
        .map(|(_, url)| *url)
}

/// Returns the expected path for the VAD model.
pub fn vad_model_path() -> PathBuf {
    models_dir().join(VAD_MODEL_FILENAME)
}

/// Returns the download URL for the VAD model.
pub fn vad_model_url() -> &'static str {
    VAD_MODEL_URL
}

/// Returns true if the VAD model file exists on disk.
pub fn is_vad_model_downloaded() -> bool {
    vad_model_path().exists()
}

/// Returns the VAD model path as a String if the model is downloaded, None otherwise.
pub fn vad_model_path_if_available() -> Option<String> {
    let path = vad_model_path();
    if path.exists() {
        path.to_str().map(|s| s.to_string())
    } else {
        None
    }
}

/// Downloads the VAD model with progress events, returns path on success.
pub fn download_vad_model(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    download_file(app, VAD_MODEL_URL, &vad_model_path(), "vad")
}

/// Downloads a model with progress events, returns path on success.
///
/// Emits `model-download-progress` events with `{ model, percent, bytes_downloaded, total_bytes }`.
/// Downloads to a `.downloading` temp file first, then performs an atomic rename.
pub fn download_model(app: &tauri::AppHandle, model_name: &str) -> Result<PathBuf, String> {
    let url = get_model_url(model_name).ok_or_else(|| format!("Unknown model: {}", model_name))?;
    download_file(app, url, &model_path(model_name), model_name)
}

/// Internal: download a file from `url` to `dest`, emitting progress events with `model_label`.
fn download_file(
    app: &tauri::AppHandle,
    url: &str,
    dest: &std::path::Path,
    model_label: &str,
) -> Result<PathBuf, String> {
    let dir = models_dir();

    log::info!(
        "model_manager: downloading '{}' to {:?}",
        model_label, dest
    );

    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create models directory {:?}: {}", dir, e))?;

    let downloading_path = dest.with_extension("downloading");

    let response = reqwest::blocking::Client::new()
        .get(url)
        .send()
        .map_err(|e| format!("Failed to start download: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with HTTP status: {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length().unwrap_or(0);
    let mut bytes_downloaded: u64 = 0;

    let mut file = fs::File::create(&downloading_path)
        .map_err(|e| format!("Failed to create temp file {:?}: {}", downloading_path, e))?;

    let mut reader = response;
    let mut buf = vec![0u8; 8192];

    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)
            .map_err(|e| format!("Download read error: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Failed to write to temp file: {}", e))?;

        bytes_downloaded += n as u64;
        let percent = if total_bytes > 0 {
            (bytes_downloaded as f64 / total_bytes as f64 * 100.0) as u32
        } else {
            0
        };

        let _ = app.emit(
            "model-download-progress",
            serde_json::json!({
                "model": model_label,
                "percent": percent,
                "bytes_downloaded": bytes_downloaded,
                "total_bytes": total_bytes,
            }),
        );
    }

    file.flush()
        .map_err(|e| format!("Failed to flush temp file: {}", e))?;
    drop(file);

    fs::rename(&downloading_path, &dest)
        .map_err(|e| format!("Failed to rename temp file to {:?}: {}", dest, e))?;

    log::info!(
        "model_manager: download complete, {} bytes written to {:?}",
        bytes_downloaded, dest
    );

    Ok(dest.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_models_dir_contains_lcars_voice() {
        let dir = models_dir();
        let dir_str = dir.to_string_lossy();
        assert!(
            dir_str.contains("lcars-voice"),
            "models_dir should contain 'lcars-voice', got: {}",
            dir_str
        );
        assert!(
            dir_str.contains("models"),
            "models_dir should contain 'models', got: {}",
            dir_str
        );
    }

    #[test]
    fn test_model_path_base() {
        let path = model_path("base");
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("ggml-base.bin"),
            "model_path('base') should end with 'ggml-base.bin', got: {}",
            path_str
        );
    }

    #[test]
    fn test_model_path_large() {
        let path = model_path("large");
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("ggml-large.bin"),
            "model_path('large') should end with 'ggml-large.bin', got: {}",
            path_str
        );
    }

    #[test]
    fn test_model_url_valid() {
        assert!(get_model_url("base").is_some(), "base should have a URL");
        assert!(get_model_url("small").is_some(), "small should have a URL");
        assert!(
            get_model_url("medium").is_some(),
            "medium should have a URL"
        );
        assert!(get_model_url("large").is_some(), "large should have a URL");
    }

    #[test]
    fn test_model_url_invalid() {
        assert!(
            get_model_url("tiny").is_none(),
            "tiny should not have a URL"
        );
        assert!(
            get_model_url("xlarge").is_none(),
            "xlarge should not have a URL"
        );
        assert!(
            get_model_url("").is_none(),
            "empty string should not have a URL"
        );
    }

    #[test]
    fn test_vad_model_path() {
        let path = vad_model_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("ggml-silero-v6.2.0.bin"),
            "vad_model_path should end with 'ggml-silero-v6.2.0.bin', got: {}",
            path_str
        );
    }

    #[test]
    fn test_vad_model_url() {
        let url = vad_model_url();
        assert!(
            url.contains("huggingface.co"),
            "VAD model URL should contain 'huggingface.co', got: {}",
            url
        );
        assert!(
            url.contains("silero"),
            "VAD model URL should contain 'silero', got: {}",
            url
        );
    }

    #[test]
    fn test_is_model_downloaded_nonexistent() {
        // A model with a nonsensical name should not be downloaded
        assert!(
            !is_model_downloaded("nonexistent-fake-model-12345"),
            "nonexistent model should not be reported as downloaded"
        );
    }
}
