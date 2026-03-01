//! Native whisper-rs transcription replacing the Python subprocess bridge.

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: String,
}

/// Build common Whisper FullParams with anti-hallucination settings.
///
/// `max_tokens` controls the maximum tokens per segment (100 for voice notes, 500 for meetings).
/// If `app` is provided, registers a progress callback that emits `meeting-transcription-progress`.
/// Automatically enables VAD (Voice Activity Detection) if the VAD model is downloaded.
pub fn build_whisper_params(max_tokens: i32, app: Option<tauri::AppHandle>) -> FullParams<'static, 'static> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    // Anti-hallucination parameters
    params.set_suppress_nst(true);
    params.set_no_context(true);
    params.set_entropy_thold(2.0);
    params.set_logprob_thold(-0.5);
    params.set_temperature_inc(0.4);
    params.set_max_tokens(max_tokens);

    // Enable VAD if the model is available (path must be set before enabling)
    if let Some(vad_path) = crate::model_manager::vad_model_path_if_available() {
        params.set_vad_model_path(Some(&vad_path));
        params.enable_vad(true);
    }

    if let Some(app_cb) = app {
        use tauri::Emitter;
        params.set_progress_callback_safe(move |percent: i32| {
            let _ = app_cb.emit(
                "meeting-transcription-progress",
                serde_json::json!({"stage": "transcribing", "percent": percent}),
            );
        });
    }

    params
}

/// Transcribes audio data using a pre-loaded WhisperContext.
///
/// `audio_data` must be f32 PCM samples at 16kHz mono.
/// If `app` is provided, emits `meeting-transcription-progress` events with percent updates.
pub fn transcribe(
    ctx: &WhisperContext,
    audio_data: &[f32],
    model_name: &str,
    app: Option<tauri::AppHandle>,
) -> Result<TranscriptionResult, String> {
    eprintln!(
        "[LCARS] transcription: model={}, samples={}",
        model_name,
        audio_data.len()
    );

    let params = build_whisper_params(100, app);

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
            if segment.no_speech_probability() > 0.8 {
                eprintln!(
                    "[LCARS] transcription: skipping segment {} (no_speech_prob={:.2})",
                    i,
                    segment.no_speech_probability()
                );
                continue;
            }
            if let Ok(s) = segment.to_str() {
                text.push_str(s);
            }
        }
    }

    let text = detect_and_remove_repetitions(&text).trim().to_string();

    eprintln!("[LCARS] transcription: Success, {} chars", text.len());

    Ok(TranscriptionResult {
        text,
        language: "en".to_string(),
    })
}

/// Detects and removes repetitive phrases from transcription output.
///
/// Whisper can hallucinate by repeating the same phrase dozens of times.
/// This function detects consecutive n-gram repetitions and truncates them,
/// keeping only the first occurrence.
pub fn detect_and_remove_repetitions(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 4 {
        return text.to_string();
    }

    // Try n-gram sizes from largest to smallest (catch phrase loops first)
    // Upper bound: need at least 3 occurrences (1 original + max_repeats=2), capped at 50
    let max_ngram = (words.len() / 3).min(50);
    for ngram_size in (1..=max_ngram).rev() {
        let max_repeats = if ngram_size <= 2 { 4 } else { 2 };

        if words.len() < ngram_size * (max_repeats + 1) {
            continue;
        }

        let mut i = 0;
        while i + ngram_size <= words.len() {
            let ngram = &words[i..i + ngram_size];
            let mut repeat_count = 1;

            // Count consecutive repetitions of this n-gram
            let mut j = i + ngram_size;
            while j + ngram_size <= words.len() {
                if &words[j..j + ngram_size] == ngram {
                    repeat_count += 1;
                    j += ngram_size;
                } else {
                    break;
                }
            }

            if repeat_count > max_repeats {
                // Found excessive repetition - rebuild text keeping only first occurrence
                let before = &words[..i + ngram_size];
                let after = &words[j..];
                let mut result_words: Vec<&str> = before.to_vec();
                result_words.extend_from_slice(after);
                let cleaned = result_words.join(" ");
                eprintln!(
                    "[LCARS] transcription: repetition detected and removed ({} -> {} chars)",
                    text.len(),
                    cleaned.len()
                );
                // Recurse to catch any remaining repetitions
                return detect_and_remove_repetitions(&cleaned);
            }

            i += 1;
        }
    }

    text.to_string()
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
    fn test_repetition_clean_text_unchanged() {
        let input = "Hello world this is a normal transcription with no repetition";
        assert_eq!(detect_and_remove_repetitions(input), input);
    }

    #[test]
    fn test_repetition_phrase_loop_truncated() {
        let input = "Real content here. I'm going to use the code. I'm going to use the code. I'm going to use the code. I'm going to use the code.";
        let result = detect_and_remove_repetitions(input);
        // Should keep the real content and only one occurrence of the repeated phrase
        assert!(result.contains("Real content here."));
        let count = result.matches("I'm going to use the code.").count();
        assert!(
            count <= 2,
            "Expected at most 2 occurrences, got {}: '{}'",
            count,
            result
        );
    }

    #[test]
    fn test_repetition_natural_short_repeats_preserved() {
        let input = "no no I said no";
        assert_eq!(detect_and_remove_repetitions(input), input);
    }

    #[test]
    fn test_repetition_empty_input() {
        assert_eq!(detect_and_remove_repetitions(""), "");
    }

    #[test]
    fn test_repetition_single_word_excessive() {
        let input = "Hello blah blah blah blah blah blah blah world";
        let result = detect_and_remove_repetitions(input);
        let count = result.matches("blah").count();
        assert!(
            count <= 4,
            "Expected at most 4 'blah', got {}: '{}'",
            count,
            result
        );
        assert!(result.contains("Hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn test_build_whisper_params_does_not_panic() {
        // Should not panic regardless of VAD model availability
        let _params = build_whisper_params(100, None);
        let _params = build_whisper_params(500, None);
    }

    #[test]
    fn test_repetition_long_phrase_39_words() {
        // Real-world reproduction: 39-word phrase repeated 13 times
        let phrase = "so I think the key takeaway from this meeting is that we need to focus on building a better product experience for our users and making sure that the onboarding flow is as smooth as possible for everyone involved";
        let word_count = phrase.split_whitespace().count();
        assert_eq!(word_count, 39);

        let repeated = std::iter::repeat(phrase).take(13).collect::<Vec<_>>().join(" ");
        let result = detect_and_remove_repetitions(&repeated);
        let result_count = result.matches("key takeaway").count();
        assert!(
            result_count <= 2,
            "Expected at most 2 occurrences of the 39-word phrase, got {}: '{}'",
            result_count,
            &result[..result.len().min(200)]
        );
    }

    #[test]
    fn test_repetition_medium_phrase_15_words() {
        // 15-word phrase repeated 5 times (above old 8-word ceiling)
        let phrase = "the quick brown fox jumped over the lazy dog and then sat down quietly";
        let word_count = phrase.split_whitespace().count();
        assert_eq!(word_count, 14);

        let repeated = std::iter::repeat(phrase).take(5).collect::<Vec<_>>().join(" ");
        let result = detect_and_remove_repetitions(&repeated);
        let count = result.matches("quick brown fox").count();
        assert!(
            count <= 2,
            "Expected at most 2 occurrences of the 15-word phrase, got {}: '{}'",
            count,
            result
        );
    }

    #[test]
    fn test_repetition_long_phrase_with_surrounding_text() {
        // Repeats sandwiched between legitimate content
        let prefix = "This is a perfectly valid introduction to the recording.";
        let phrase = "and I think we should consider the implications of this decision for our team";
        let suffix = "That concludes the main points of our discussion today.";

        let middle = std::iter::repeat(phrase).take(8).collect::<Vec<_>>().join(" ");
        let input = format!("{} {} {}", prefix, middle, suffix);
        let result = detect_and_remove_repetitions(&input);

        assert!(
            result.contains("perfectly valid introduction"),
            "Prefix should be preserved: '{}'",
            &result[..result.len().min(200)]
        );
        assert!(
            result.contains("concludes the main points"),
            "Suffix should be preserved: '{}'",
            result
        );
        let count = result.matches("consider the implications").count();
        assert!(
            count <= 2,
            "Expected at most 2 occurrences of the repeated phrase, got {}",
            count
        );
    }

    #[test]
    fn test_repetition_exactly_9_word_phrase() {
        // Smallest n-gram that failed with the old 8-word ceiling
        let phrase = "I really think this is a very important point";
        let word_count = phrase.split_whitespace().count();
        assert_eq!(word_count, 9);

        let repeated = std::iter::repeat(phrase).take(5).collect::<Vec<_>>().join(" ");
        let result = detect_and_remove_repetitions(&repeated);
        let count = result.matches("very important point").count();
        assert!(
            count <= 2,
            "Expected at most 2 occurrences of the 9-word phrase, got {}: '{}'",
            count,
            result
        );
    }

    #[test]
    fn test_repetition_performance_large_clean_text() {
        // 2000 unique words; should be unchanged and complete quickly
        let words: Vec<String> = (0..2000).map(|i| format!("word{}", i)).collect();
        let input = words.join(" ");

        let start = std::time::Instant::now();
        let result = detect_and_remove_repetitions(&input);
        let elapsed = start.elapsed();

        assert_eq!(result, input, "Clean text should be unchanged");
        assert!(
            elapsed.as_secs() < 1,
            "Should complete in under 1 second, took {:?}",
            elapsed
        );
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
        ctx_params.use_gpu(cfg!(feature = "cuda"));
        ctx_params.flash_attn(cfg!(feature = "cuda"));
        let ctx = WhisperContext::new_with_params(model_file.to_str().unwrap(), ctx_params)
            .expect("failed to load model");

        // 2 seconds of silence at 16kHz
        let silence = vec![0.0f32; 16000 * 2];
        let result = transcribe(&ctx, &silence, "base", None).expect("transcription should succeed");
        // Silence should produce very short or empty text
        assert!(
            result.text.len() < 100,
            "Silence transcription should be short, got {} chars: '{}'",
            result.text.len(),
            result.text
        );
    }
}
