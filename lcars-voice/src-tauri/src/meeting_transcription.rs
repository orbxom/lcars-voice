//! Meeting transcription pipeline — pure Rust port of the Python meeting-transcripts tool.
//!
//! Provides WAV decoding, hallucination filtering, speaker diarization assignment,
//! segment merging, and transcript formatting.

use std::io::Cursor;

/// A single Whisper transcription segment with timing, text, and optional speaker.
#[derive(Debug, Clone)]
pub struct WhisperSegment {
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
    pub no_speech_prob: f32,
    pub speaker: Option<String>,
}

/// A speaker turn from pyannote diarization output.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SpeakerTurn {
    pub start: f64,
    pub end: f64,
    pub speaker: String,
}

/// Embedded Python script for running pyannote speaker diarization.
///
/// Includes a ProgressHook that writes JSON progress messages to stderr,
/// allowing the Rust side to parse and emit real-time progress events.
const DIARIZE_SCRIPT: &str = r#"
import json, sys, os, torch

class ProgressHook:
    def __enter__(self):
        return self
    def __exit__(self, *args):
        pass
    def __call__(self, step_name, step_artifact=None, file=None,
                 total=None, completed=None):
        msg = {"step": step_name}
        if total is not None and completed is not None:
            msg["completed"] = completed
            msg["total"] = total
        print(json.dumps(msg), file=sys.stderr, flush=True)

from pyannote.audio import Pipeline
pipeline = Pipeline.from_pretrained(
    "pyannote/speaker-diarization-3.1",
    token=os.environ.get("HF_TOKEN"))
if torch.cuda.is_available():
    pipeline.to(torch.device("cuda"))
with ProgressHook() as hook:
    result = pipeline(sys.argv[1], hook=hook)
turns = [{"start": t.start, "end": t.end, "speaker": s}
         for t, _, s in result.itertracks(yield_label=True)]
json.dump(turns, sys.stdout)
"#;

/// Decode WAV bytes into f32 PCM samples.
///
/// Handles mono and stereo (stereo is downmixed by averaging channels).
pub fn decode_wav_blob(wav_bytes: &[u8]) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::new(Cursor::new(wav_bytes))
        .map_err(|e| format!("Failed to read WAV: {}", e))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;

    let i16_samples: Vec<i16> = reader
        .into_samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read WAV samples: {}", e))?;

    if channels == 1 {
        Ok(i16_samples
            .iter()
            .map(|&s| s as f32 / i16::MAX as f32)
            .collect())
    } else {
        // Downmix stereo (or multi-channel) by averaging every `channels` samples
        Ok(i16_samples
            .chunks(channels)
            .map(|chunk| {
                let sum: f32 = chunk.iter().map(|&s| s as f32).sum();
                (sum / channels as f32) / i16::MAX as f32
            })
            .collect())
    }
}

/// Compute the ratio of non-Latin characters to total non-whitespace characters.
///
/// A char is "non-Latin" if its codepoint > 0x024F and NOT in the Latin Extended
/// Additional range 0x1E00..=0x1EFF.
pub fn non_latin_ratio(text: &str) -> f64 {
    let non_ws_chars: Vec<char> = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if non_ws_chars.is_empty() {
        return 0.0;
    }
    let non_latin_count = non_ws_chars
        .iter()
        .filter(|&&ch| {
            let cp = ch as u32;
            cp > 0x024F && !(0x1E00..=0x1EFF).contains(&cp)
        })
        .count();
    non_latin_count as f64 / non_ws_chars.len() as f64
}

/// Filter out hallucinated segments.
///
/// Removes segments with no_speech_prob > 0.6 or non_latin_ratio > 0.3.
pub fn filter_hallucinations(segments: Vec<WhisperSegment>) -> Vec<WhisperSegment> {
    let initial_count = segments.len();
    let mut no_speech_removed = 0usize;
    let mut non_latin_removed = 0usize;

    let result: Vec<WhisperSegment> = segments
        .into_iter()
        .filter(|seg| {
            if seg.no_speech_prob > 0.6 {
                no_speech_removed += 1;
                return false;
            }
            if non_latin_ratio(&seg.text) > 0.3 {
                non_latin_removed += 1;
                return false;
            }
            true
        })
        .collect();

    let total_removed = no_speech_removed + non_latin_removed;
    if total_removed > 0 {
        log::info!(
            "filter_hallucinations: removed {}/{} segments ({} no-speech, {} non-Latin)",
            total_removed, initial_count, no_speech_removed, non_latin_removed
        );
    }

    result
}

/// Assign speaker labels to segments based on diarization turns.
///
/// Maps raw speaker IDs (e.g. SPEAKER_00) to friendly labels (Speaker 1, Speaker 2, ...).
/// Segments with no diarization overlap are assigned the nearest known speaker.
pub fn assign_speakers(segments: &mut [WhisperSegment], turns: &[SpeakerTurn]) {
    // Build sorted unique speaker list and map to "Speaker 1", "Speaker 2", etc.
    let mut unique_speakers: Vec<String> = Vec::new();
    for turn in turns {
        if !unique_speakers.contains(&turn.speaker) {
            unique_speakers.push(turn.speaker.clone());
        }
    }
    unique_speakers.sort();

    let speaker_label = |raw: &str| -> String {
        match unique_speakers.iter().position(|s| s == raw) {
            Some(idx) => format!("Speaker {}", idx + 1),
            None => "Unknown".to_string(),
        }
    };

    // First pass: assign by maximum temporal overlap
    for seg in segments.iter_mut() {
        let mut best_overlap = 0.0f64;
        let mut best_speaker: Option<String> = None;

        for turn in turns {
            let overlap = (seg.end_sec.min(turn.end) - seg.start_sec.max(turn.start)).max(0.0);
            if overlap > best_overlap {
                best_overlap = overlap;
                best_speaker = Some(speaker_label(&turn.speaker));
            }
        }

        seg.speaker = if best_overlap > 0.0 {
            best_speaker
        } else {
            Some("Unknown".to_string())
        };
    }

    // Second pass: assign "Unknown" segments to nearest known speaker
    // Try previous first, then next
    let len = segments.len();
    for i in 0..len {
        if segments[i].speaker.as_deref() == Some("Unknown") {
            // Look backwards for a known speaker
            let mut found = false;
            for j in (0..i).rev() {
                if segments[j].speaker.as_deref() != Some("Unknown") {
                    let prev_speaker = segments[j].speaker.clone();
                    segments[i].speaker = prev_speaker;
                    found = true;
                    break;
                }
            }
            if !found {
                // Look forwards for a known speaker
                for j in (i + 1)..len {
                    if segments[j].speaker.as_deref() != Some("Unknown") {
                        let next_speaker = segments[j].speaker.clone();
                        segments[i].speaker = next_speaker;
                        break;
                    }
                }
            }
        }
    }
}

/// Merge adjacent segments that share the same speaker.
///
/// Text is space-joined and trimmed; time range spans first start to last end.
pub fn merge_consecutive_speakers(segments: Vec<WhisperSegment>) -> Vec<WhisperSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<WhisperSegment> = Vec::new();

    for seg in segments {
        let should_merge = result
            .last()
            .map(|prev: &WhisperSegment| prev.speaker == seg.speaker)
            .unwrap_or(false);

        if should_merge {
            let prev = result.last_mut().unwrap();
            prev.end_sec = seg.end_sec;
            let trimmed = seg.text.trim();
            if !trimmed.is_empty() {
                prev.text.push(' ');
                prev.text.push_str(trimmed);
            }
        } else {
            // Start a new merged segment with trimmed text
            let mut new_seg = seg;
            new_seg.text = new_seg.text.trim().to_string();
            result.push(new_seg);
        }
    }

    result
}

/// Format segments into a readable transcript string.
///
/// If any segment has a speaker label, format as `**Speaker N:** text` blocks.
/// Otherwise, space-join all text.
pub fn format_transcript(segments: &[WhisperSegment]) -> String {
    let has_speakers = segments.iter().any(|s| s.speaker.is_some());

    if has_speakers {
        segments
            .iter()
            .map(|s| {
                let speaker = s.speaker.as_deref().unwrap_or("Unknown");
                format!("**{}:** {}", speaker, s.text)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Transcribe meeting audio using whisper-rs, returning individual segments with timing.
///
/// Similar to the voice note transcription but extracts per-segment timing data
/// and uses higher max_tokens for longer meeting recordings.
/// When an `app` handle is provided, emits progress events during inference.
pub fn transcribe_meeting_audio(
    ctx: &whisper_rs::WhisperContext,
    audio_data: &[f32],
    model_name: &str,
    app: Option<tauri::AppHandle>,
) -> Result<Vec<WhisperSegment>, String> {
    log::info!(
        "meeting_transcription: model={}, samples={}",
        model_name,
        audio_data.len()
    );

    let params = crate::transcription::build_whisper_params(500, app);

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create whisper state: {}", e))?;

    state
        .full(params, audio_data)
        .map_err(|e| format!("Whisper inference failed: {}", e))?;

    let num_segments = state.full_n_segments();
    let mut segments = Vec::new();

    for i in 0..num_segments {
        if let Some(segment) = state.get_segment(i) {
            let raw_text = segment.to_str().unwrap_or_default().to_string();
            let text = crate::transcription::detect_and_remove_repetitions(&raw_text);
            let no_speech_prob = segment.no_speech_probability();
            let start_cs = segment.start_timestamp();
            let end_cs = segment.end_timestamp();

            segments.push(WhisperSegment {
                start_sec: start_cs as f64 / 100.0,
                end_sec: end_cs as f64 / 100.0,
                text,
                no_speech_prob,
                speaker: None,
            });
        }
    }

    log::info!(
        "meeting_transcription: {} segments extracted",
        segments.len()
    );
    Ok(segments)
}

/// Build the temp file path for diarization WAV files.
///
/// Uses `$XDG_RUNTIME_DIR` (a per-user, permission-restricted directory) when
/// available, falling back to `/tmp` otherwise.
pub fn diarize_temp_path(timestamp: u128) -> String {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok();
    diarize_temp_path_with(runtime_dir.as_deref(), timestamp)
}

fn diarize_temp_path_with(runtime_dir: Option<&str>, timestamp: u128) -> String {
    let dir = runtime_dir.unwrap_or("/tmp");
    format!("{}/lcars-diarize-{}.wav", dir, timestamp)
}

/// Resolve the Python interpreter for diarization.
///
/// Priority: $PYTHON_ENV > ~/voice-to-text-env/bin/python (if exists) > python3
pub fn resolve_python() -> String {
    let python_env = std::env::var("PYTHON_ENV").ok();
    resolve_python_with(python_env.as_deref())
}

fn resolve_python_with(python_env: Option<&str>) -> String {
    if let Some(custom) = python_env {
        return custom.to_string();
    }
    if let Some(home) = dirs::home_dir() {
        let venv = home.join("voice-to-text-env/bin/python");
        if venv.exists() {
            return venv.to_string_lossy().to_string();
        }
    }
    "python3".to_string()
}

/// Parse a line of diarization progress JSON from stderr.
///
/// The pyannote ProgressHook emits JSON like `{"step":"segmentation","completed":5,"total":20}`.
/// Maps progress across pipeline steps to a weighted 0-100 overall percent:
/// - segmentation: 0-40% (based on completed/total)
/// - speaker_counting: 45% (fixed milestone)
/// - embeddings: 45-90% (based on completed/total)
/// - discrete_diarization: 95% (fixed milestone)
///
/// Returns `None` for non-JSON lines, Python warnings, empty strings, or unrecognized steps.
pub fn parse_diarization_progress(line: &str) -> Option<i32> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let step = v.get("step")?.as_str()?;

    match step {
        "segmentation" => {
            let completed = v.get("completed")?.as_i64()?;
            let total = v.get("total")?.as_i64()?;
            if total == 0 {
                return Some(0);
            }
            Some((completed * 40 / total) as i32)
        }
        "speaker_counting" => Some(45),
        "embeddings" => {
            let completed = v.get("completed")?.as_i64()?;
            let total = v.get("total")?.as_i64()?;
            if total == 0 {
                return Some(45);
            }
            Some((45 + completed * 45 / total) as i32)
        }
        "discrete_diarization" => Some(95),
        _ => None,
    }
}

/// Run pyannote speaker diarization via a Python subprocess.
///
/// Returns None if Python is not found, the script fails, or JSON parsing fails.
/// When an `app` handle is provided, emits `meeting-transcription-progress` events
/// with `{"stage": "diarizing", "percent": N}` as the Python script reports progress.
pub fn run_diarization(wav_bytes: &[u8], app: Option<tauri::AppHandle>) -> Option<Vec<SpeakerTurn>> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Command, Stdio};

    // Create a temp file with a timestamp-based name
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = diarize_temp_path(timestamp);

    // Write WAV bytes to temp file
    let mut file = match std::fs::File::create(&temp_path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("run_diarization: failed to create temp file: {}", e);
            return None;
        }
    };
    if let Err(e) = file.write_all(wav_bytes) {
        log::warn!("run_diarization: failed to write temp file: {}", e);
        let _ = std::fs::remove_file(&temp_path);
        return None;
    }
    drop(file);

    // Find Python: check PYTHON_ENV env var, fall back to system python3
    let python = resolve_python();

    // Run the diarization script with piped stdout/stderr for progress
    let mut cmd = Command::new(&python);
    cmd.arg("-c")
        .arg(DIARIZE_SCRIPT)
        .arg(&temp_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Pass HF_TOKEN if set
    if let Ok(token) = std::env::var("HF_TOKEN") {
        cmd.env("HF_TOKEN", token);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "run_diarization: failed to run Python (not found at '{}' or execution error): {}",
                python, e
            );
            let _ = std::fs::remove_file(&temp_path);
            return None;
        }
    };

    // Read stderr on a separate thread for progress reporting
    let stderr = child.stderr.take();
    let stderr_thread = std::thread::spawn(move || {
        let mut collected = String::new();
        if let Some(stderr_pipe) = stderr {
            let reader = BufReader::new(stderr_pipe);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if let Some(percent) = parse_diarization_progress(&line) {
                            if let Some(ref app) = app {
                                use tauri::Emitter;
                                let _ = app.emit(
                                    "meeting-transcription-progress",
                                    serde_json::json!({"stage": "diarizing", "percent": percent}),
                                );
                            }
                        }
                        // Collect all stderr for error reporting
                        if !collected.is_empty() {
                            collected.push('\n');
                        }
                        collected.push_str(&line);
                    }
                    Err(e) => {
                        log::warn!("run_diarization: error reading stderr: {}", e);
                    }
                }
            }
        }
        collected
    });

    // Read stdout (JSON result)
    let mut stdout_data = String::new();
    if let Some(mut stdout_pipe) = child.stdout.take() {
        if let Err(e) = stdout_pipe.read_to_string(&mut stdout_data) {
            log::warn!("run_diarization: error reading stdout: {}", e);
        }
    }

    // Wait for child to exit
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            log::warn!("run_diarization: failed to wait for child: {}", e);
            let _ = std::fs::remove_file(&temp_path);
            return None;
        }
    };

    // Collect stderr from the thread
    let stderr_output = stderr_thread.join().unwrap_or_default();

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    if !status.success() {
        log::warn!(
            "run_diarization: Python script failed (exit {}): {}",
            status, stderr_output
        );
        return None;
    }

    // Parse stdout as JSON array of SpeakerTurn
    match serde_json::from_str::<Vec<SpeakerTurn>>(&stdout_data) {
        Ok(turns) => {
            log::info!(
                "run_diarization: found {} speaker turns",
                turns.len()
            );
            Some(turns)
        }
        Err(e) => {
            log::warn!("run_diarization: failed to parse JSON: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Helper: create a WhisperSegment with defaults
    // ---------------------------------------------------------------
    fn seg(start: f64, end: f64, text: &str) -> WhisperSegment {
        WhisperSegment {
            start_sec: start,
            end_sec: end,
            text: text.to_string(),
            no_speech_prob: 0.1,
            speaker: None,
        }
    }

    fn seg_with_prob(start: f64, end: f64, text: &str, prob: f32) -> WhisperSegment {
        WhisperSegment {
            start_sec: start,
            end_sec: end,
            text: text.to_string(),
            no_speech_prob: prob,
            speaker: None,
        }
    }

    fn seg_with_speaker(start: f64, end: f64, text: &str, speaker: &str) -> WhisperSegment {
        WhisperSegment {
            start_sec: start,
            end_sec: end,
            text: text.to_string(),
            no_speech_prob: 0.1,
            speaker: Some(speaker.to_string()),
        }
    }

    /// Encode f32 samples into WAV bytes using hound (for roundtrip tests).
    fn encode_wav_bytes(samples: &[f32], channels: u16, sample_rate: u32) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut buf, spec).expect("create wav writer");
        for &s in samples {
            let i16_val = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer.write_sample(i16_val).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
        buf.into_inner()
    }

    // ===============================================================
    // WAV decode tests
    // ===============================================================

    #[test]
    fn test_decode_wav_blob_roundtrip() {
        let original: Vec<f32> = vec![0.0, 0.5, -0.5, 0.25, -0.25];
        let wav_bytes = encode_wav_bytes(&original, 1, 16000);
        let decoded = decode_wav_blob(&wav_bytes).expect("decode should succeed");

        assert_eq!(decoded.len(), original.len());
        for (orig, dec) in original.iter().zip(decoded.iter()) {
            assert!(
                (orig - dec).abs() < 0.001,
                "sample mismatch: orig={}, decoded={}",
                orig,
                dec
            );
        }
    }

    #[test]
    fn test_decode_wav_blob_invalid() {
        let garbage = b"this is not a wav file at all";
        let result = decode_wav_blob(garbage);
        assert!(result.is_err(), "garbage bytes should produce an error");
    }

    // ===============================================================
    // Hallucination filter tests
    // ===============================================================

    #[test]
    fn test_filter_high_no_speech_prob() {
        let segments = vec![seg_with_prob(0.0, 3.0, " Hello.", 0.8)];
        let result = filter_hallucinations(segments);
        assert_eq!(result.len(), 0, "segment with no_speech_prob=0.8 should be removed");
    }

    #[test]
    fn test_filter_non_latin() {
        let segments = vec![seg(0.0, 3.0, "\u{c548}\u{b155}\u{d558}\u{c138}\u{c694} \u{c138}\u{acc4}")];
        let result = filter_hallucinations(segments);
        assert_eq!(result.len(), 0, "Korean text should be filtered as hallucination");
    }

    #[test]
    fn test_filter_preserves_good_segments() {
        let segments = vec![
            seg_with_prob(0.0, 3.0, " Hello everyone.", 0.1),
            seg_with_prob(3.0, 6.0, " How are you?", 0.2),
        ];
        let result = filter_hallucinations(segments);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, " Hello everyone.");
        assert_eq!(result[1].text, " How are you?");
    }

    // ===============================================================
    // non_latin_ratio tests
    // ===============================================================

    #[test]
    fn test_non_latin_ratio_ascii() {
        assert_eq!(non_latin_ratio("Hello world"), 0.0);
    }

    #[test]
    fn test_non_latin_ratio_korean() {
        let ratio = non_latin_ratio("\u{c548}\u{b155}\u{d558}\u{c138}\u{c694} \u{c138}\u{acc4}");
        assert!(
            ratio > 0.3,
            "Korean text should have ratio > 0.3, got {}",
            ratio
        );
    }

    #[test]
    fn test_non_latin_ratio_empty() {
        assert_eq!(non_latin_ratio(""), 0.0);
    }

    #[test]
    fn test_non_latin_ratio_mixed() {
        // Mostly ASCII with accented characters (Latin Extended Additional range)
        let ratio = non_latin_ratio("Hello caf\u{00e9} na\u{00ef}ve r\u{00e9}sum\u{00e9}");
        assert!(
            ratio < 0.3,
            "Mixed ASCII/accented should have low ratio, got {}",
            ratio
        );
    }

    // ===============================================================
    // Speaker assignment tests
    // ===============================================================

    #[test]
    fn test_assign_speakers_basic() {
        let mut segments = vec![
            seg(0.0, 5.0, " Hello everyone."),
            seg(5.0, 12.0, " Let's discuss the first ticket."),
        ];
        let turns = vec![
            SpeakerTurn { start: 0.0, end: 5.0, speaker: "SPEAKER_00".to_string() },
            SpeakerTurn { start: 5.0, end: 12.0, speaker: "SPEAKER_01".to_string() },
        ];

        assign_speakers(&mut segments, &turns);

        assert_eq!(segments[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(segments[1].speaker.as_deref(), Some("Speaker 2"));
    }

    #[test]
    fn test_assign_speakers_maps_labels() {
        let mut segments = vec![
            seg(0.0, 3.0, " First."),
            seg(3.0, 6.0, " Second."),
            seg(6.0, 9.0, " Third."),
        ];
        let turns = vec![
            SpeakerTurn { start: 0.0, end: 3.0, speaker: "SPEAKER_00".to_string() },
            SpeakerTurn { start: 3.0, end: 6.0, speaker: "SPEAKER_01".to_string() },
            SpeakerTurn { start: 6.0, end: 9.0, speaker: "SPEAKER_00".to_string() },
        ];

        assign_speakers(&mut segments, &turns);

        assert_eq!(segments[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(segments[1].speaker.as_deref(), Some("Speaker 2"));
        assert_eq!(segments[2].speaker.as_deref(), Some("Speaker 1"));
    }

    #[test]
    fn test_assign_speakers_no_overlap() {
        let mut segments = vec![
            seg(0.0, 3.0, " Covered."),
            seg(10.0, 15.0, " No overlap at all."),
        ];
        let turns = vec![
            SpeakerTurn { start: 0.0, end: 3.0, speaker: "SPEAKER_00".to_string() },
        ];

        assign_speakers(&mut segments, &turns);

        assert_eq!(segments[0].speaker.as_deref(), Some("Speaker 1"));
        // No overlap => should get nearest known speaker (previous)
        assert_eq!(segments[1].speaker.as_deref(), Some("Speaker 1"));
    }

    #[test]
    fn test_assign_speakers_overlap_resolution() {
        // Speaker A: 0-4s, Speaker B: 4-10s
        // Segment spans 3-8s => 1s overlap with A, 4s overlap with B => B wins
        let mut segments = vec![seg(3.0, 8.0, " Spans both speakers.")];
        let turns = vec![
            SpeakerTurn { start: 0.0, end: 4.0, speaker: "SPEAKER_00".to_string() },
            SpeakerTurn { start: 4.0, end: 10.0, speaker: "SPEAKER_01".to_string() },
        ];

        assign_speakers(&mut segments, &turns);

        assert_eq!(segments[0].speaker.as_deref(), Some("Speaker 2"));
    }

    // ===============================================================
    // Merge consecutive tests
    // ===============================================================

    #[test]
    fn test_merge_consecutive_same_speaker() {
        let segments = vec![
            seg_with_speaker(0.0, 3.0, " Hello.", "Speaker 1"),
            seg_with_speaker(3.0, 6.0, " How are you?", "Speaker 1"),
            seg_with_speaker(6.0, 9.0, " I'm fine.", "Speaker 1"),
        ];
        let result = merge_consecutive_speakers(segments);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(result[0].start_sec, 0.0);
        assert_eq!(result[0].end_sec, 9.0);
        assert_eq!(result[0].text, "Hello. How are you? I'm fine.");
    }

    #[test]
    fn test_merge_consecutive_different_speakers() {
        let segments = vec![
            seg_with_speaker(0.0, 3.0, " Hello.", "Speaker 1"),
            seg_with_speaker(3.0, 6.0, " Hi there.", "Speaker 2"),
            seg_with_speaker(6.0, 9.0, " Good morning.", "Speaker 1"),
        ];
        let result = merge_consecutive_speakers(segments);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(result[1].speaker.as_deref(), Some("Speaker 2"));
        assert_eq!(result[2].speaker.as_deref(), Some("Speaker 1"));
    }

    // ===============================================================
    // Format tests
    // ===============================================================

    #[test]
    fn test_format_with_speakers() {
        let segments = vec![
            seg_with_speaker(0.0, 5.0, "Hello everyone.", "Speaker 1"),
            seg_with_speaker(5.0, 10.0, "Let's begin.", "Speaker 2"),
        ];
        let result = format_transcript(&segments);
        assert_eq!(
            result,
            "**Speaker 1:** Hello everyone.\n\n**Speaker 2:** Let's begin."
        );
    }

    #[test]
    fn test_format_without_speakers() {
        let segments = vec![
            seg(0.0, 5.0, "Hello everyone."),
            seg(5.0, 10.0, "Let's begin."),
        ];
        let result = format_transcript(&segments);
        assert_eq!(result, "Hello everyone. Let's begin.");
    }

    // ===============================================================
    // Unknown speaker assignment tests
    // ===============================================================

    // ===============================================================
    // Temp path security tests (Fix 1)
    // ===============================================================

    #[test]
    fn test_diarize_temp_path_uses_xdg_runtime_dir() {
        let path = diarize_temp_path_with(Some("/run/user/1000"), 123456);
        assert_eq!(path, "/run/user/1000/lcars-diarize-123456.wav");
    }

    #[test]
    fn test_diarize_temp_path_falls_back_to_tmp() {
        let path = diarize_temp_path_with(None, 789);
        assert_eq!(path, "/tmp/lcars-diarize-789.wav");
    }

    // ===============================================================
    // Python resolution tests (Fix 2)
    // ===============================================================

    #[test]
    fn test_resolve_python_default_is_python3() {
        let python = resolve_python_with(None);
        // Falls back to python3 when no venv exists at ~/voice-to-text-env/bin/python
        // (or picks up the venv if it does exist on this machine)
        let home = dirs::home_dir().unwrap();
        let venv = home.join("voice-to-text-env/bin/python");
        if venv.exists() {
            assert_eq!(python, venv.to_string_lossy().to_string());
        } else {
            assert_eq!(python, "python3");
        }
    }

    #[test]
    fn test_resolve_python_respects_env_var() {
        let python = resolve_python_with(Some("/custom/venv/bin/python"));
        assert_eq!(python, "/custom/venv/bin/python");
    }

    // ===============================================================
    // Unknown speaker assignment tests
    // ===============================================================

    #[test]
    fn test_unknown_assigned_to_nearest() {
        let mut segments = vec![
            seg(0.0, 3.0, " First."),
            seg(3.0, 6.0, " Second."),
            seg(10.0, 15.0, " No overlap."),
        ];
        let turns = vec![
            SpeakerTurn { start: 0.0, end: 3.0, speaker: "SPEAKER_00".to_string() },
            SpeakerTurn { start: 3.0, end: 6.0, speaker: "SPEAKER_01".to_string() },
        ];

        assign_speakers(&mut segments, &turns);

        assert_eq!(segments[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(segments[1].speaker.as_deref(), Some("Speaker 2"));
        // Third segment has no overlap, should get previous speaker
        assert_eq!(segments[2].speaker.as_deref(), Some("Speaker 2"));
    }

    // ===============================================================
    // Diarization progress parsing tests
    // ===============================================================

    #[test]
    fn test_parse_diarization_progress_segmentation() {
        let line = r#"{"step":"segmentation","completed":5,"total":20}"#;
        assert_eq!(parse_diarization_progress(line), Some(10)); // 5*40/20 = 10%
    }

    #[test]
    fn test_parse_diarization_progress_segmentation_zero() {
        let line = r#"{"step":"segmentation","completed":0,"total":20}"#;
        assert_eq!(parse_diarization_progress(line), Some(0)); // 0*40/20 = 0%
    }

    #[test]
    fn test_parse_diarization_progress_segmentation_complete() {
        let line = r#"{"step":"segmentation","completed":20,"total":20}"#;
        assert_eq!(parse_diarization_progress(line), Some(40)); // 20*40/20 = 40%
    }

    #[test]
    fn test_parse_diarization_progress_speaker_counting() {
        let line = r#"{"step":"speaker_counting"}"#;
        assert_eq!(parse_diarization_progress(line), Some(45));
    }

    #[test]
    fn test_parse_diarization_progress_embeddings() {
        let line = r#"{"step":"embeddings","completed":3,"total":10}"#;
        assert_eq!(parse_diarization_progress(line), Some(58)); // 45 + 3*45/10 = 58%
    }

    #[test]
    fn test_parse_diarization_progress_embeddings_complete() {
        let line = r#"{"step":"embeddings","completed":10,"total":10}"#;
        assert_eq!(parse_diarization_progress(line), Some(90)); // 45 + 10*45/10 = 90%
    }

    #[test]
    fn test_parse_diarization_progress_discrete_diarization() {
        let line = r#"{"step":"discrete_diarization"}"#;
        assert_eq!(parse_diarization_progress(line), Some(95));
    }

    #[test]
    fn test_parse_diarization_progress_non_json() {
        assert_eq!(parse_diarization_progress("some random text"), None);
    }

    #[test]
    fn test_parse_diarization_progress_python_warning() {
        assert_eq!(
            parse_diarization_progress("/usr/lib/python3/dist-packages/foo.py:42: UserWarning: blah"),
            None
        );
    }

    #[test]
    fn test_parse_diarization_progress_empty() {
        assert_eq!(parse_diarization_progress(""), None);
    }

    #[test]
    fn test_parse_diarization_progress_unrecognized_step() {
        let line = r#"{"step":"unknown_step","completed":1,"total":5}"#;
        assert_eq!(parse_diarization_progress(line), None);
    }
}
