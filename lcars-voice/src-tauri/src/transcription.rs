//! Native whisper-rs transcription replacing the Python subprocess bridge.

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: String,
}

/// Transcribes audio data using a pre-loaded WhisperContext.
///
/// `audio_data` must be f32 PCM samples at 16kHz mono.
pub fn transcribe(
    ctx: &WhisperContext,
    audio_data: &[f32],
    model_name: &str,
) -> Result<TranscriptionResult, String> {
    eprintln!(
        "[LCARS] transcription: model={}, samples={}",
        model_name,
        audio_data.len()
    );

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);


    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create whisper state: {}", e))?;

    state
        .full(params, audio_data)
        .map_err(|e| format!("Whisper inference failed: {}", e))?;

    let num_segments = state.full_n_segments();

    let mut text = String::new();
    for i in 0..num_segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(s) = segment.to_str() {
                text.push_str(s);
            }
        }
    }

    let text = text.trim().to_string();

    eprintln!("[LCARS] transcription: Success, {} chars", text.len());

    Ok(TranscriptionResult {
        text,
        language: "en".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcription_result_serialize() {
        let result = TranscriptionResult {
            text: "hello world".to_string(),
            language: "en".to_string(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: TranscriptionResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.text, "hello world");
        assert_eq!(deserialized.language, "en");
    }

    #[test]
    fn test_transcription_result_fields() {
        let result = TranscriptionResult {
            text: "test transcription".to_string(),
            language: "fr".to_string(),
        };
        assert_eq!(result.text, "test transcription");
        assert_eq!(result.language, "fr");
    }

    #[test]
    #[ignore]
    fn test_transcribe_silence() {
        // Integration test: requires the base model to be downloaded
        let model_file = crate::model_manager::model_path("base");
        if !model_file.exists() {
            eprintln!(
                "[LCARS] test_transcribe_silence: skipping, model not found at {:?}",
                model_file
            );
            return;
        }

        let mut ctx_params = whisper_rs::WhisperContextParameters::default();
        ctx_params.use_gpu(true);
        ctx_params.flash_attn(true);
        let ctx = WhisperContext::new_with_params(
            model_file.to_str().unwrap(),
            ctx_params,
        )
        .expect("failed to load model");

        // 2 seconds of silence at 16kHz
        let silence = vec![0.0f32; 16000 * 2];
        let result = transcribe(&ctx, &silence, "base").expect("transcription should succeed");
        // Silence should produce very short or empty text
        assert!(
            result.text.len() < 100,
            "Silence transcription should be short, got {} chars: '{}'",
            result.text.len(),
            result.text
        );
    }
}
