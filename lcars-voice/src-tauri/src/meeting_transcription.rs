//! Meeting transcription pipeline — pure Rust port of the Python meeting-transcripts tool.
//!
//! Provides WAV decoding, hallucination filtering, speaker diarization assignment,
//! segment merging, and transcript formatting.

use std::collections::{HashSet, VecDeque};
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

/// Reason diarization failed — used to display actionable messages to the user.
#[derive(Debug, Clone)]
pub enum DiarizeError {
    /// Could not create or write the temp WAV file.
    TempFile(String),
    /// Python interpreter not found (spawn failed).
    PythonNotFound(String),
    /// Python script exited non-zero. Contains classified reason + raw stderr.
    ScriptFailed {
        reason: String,
        #[allow(dead_code)]
        stderr: String,
    },
    /// Script succeeded but output was not valid JSON.
    InvalidOutput(String),
}

impl std::fmt::Display for DiarizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiarizeError::TempFile(e) => write!(f, "Failed to create temp file: {}", e),
            DiarizeError::PythonNotFound(e) => write!(f, "Python not found: {}", e),
            DiarizeError::ScriptFailed { reason, .. } => write!(f, "{}", reason),
            DiarizeError::InvalidOutput(e) => write!(f, "Invalid diarization output: {}", e),
        }
    }
}

/// Classify Python stderr into a user-friendly error reason.
fn classify_diarization_error(stderr: &str) -> String {
    if stderr.contains("No module named 'pyannote'") || stderr.contains("No module named 'torch'") {
        "pyannote.audio is not installed. Install with: pip install pyannote.audio".into()
    } else if stderr.contains("HF_TOKEN") || (stderr.contains("token") && stderr.contains("401")) {
        "HF_TOKEN is missing or invalid. Set the HF_TOKEN environment variable.".into()
    } else if stderr.contains("gated repo") || stderr.contains("Access to model") {
        "Access denied to pyannote model. Accept the license at huggingface.co/pyannote/speaker-diarization-3.1 and set HF_TOKEN.".into()
    } else {
        "Diarization script failed. Check logs for details.".into()
    }
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
            msg["completed"] = int(completed)
            msg["total"] = int(total)
        print(json.dumps(msg), file=sys.stderr, flush=True)

from pyannote.audio import Pipeline
pipeline = Pipeline.from_pretrained(
    "pyannote/speaker-diarization-3.1",
    token=os.environ.get("HF_TOKEN"))
if torch.cuda.is_available():
    pipeline.to(torch.device("cuda"))
with ProgressHook() as hook:
    result = pipeline(sys.argv[1], hook=hook)
# pyannote 4.x returns object with speaker_diarization attribute;
# pyannote 3.x returns Annotation directly
if hasattr(result, 'speaker_diarization'):
    result = result.speaker_diarization
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

/// Normalize text for similarity comparison: lowercase, strip punctuation, collapse whitespace.
fn normalize_for_comparison(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
            prev_was_space = false;
        } else if !prev_was_space {
            result.push(' ');
            prev_was_space = true;
        }
    }
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

/// Compute Jaccard similarity between two texts based on word sets.
fn word_jaccard_similarity(a: &str, b: &str) -> f64 {
    let a_words: HashSet<&str> = a.split_whitespace().collect();
    let b_words: HashSet<&str> = b.split_whitespace().collect();

    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();

    intersection as f64 / union as f64
}

/// Remove near-duplicate segments that are likely whisper hallucinations.
///
/// Compares each segment against a sliding window of recent segments.
/// If the normalized text has Jaccard word similarity > 0.8 with any recent
/// segment, it is considered a hallucinated duplicate and removed.
/// Segments with fewer than 5 words are exempt from deduplication.
pub fn deduplicate_segments(segments: Vec<WhisperSegment>) -> Vec<WhisperSegment> {
    if segments.len() < 2 {
        return segments;
    }

    const WINDOW_SIZE: usize = 5;
    const SIMILARITY_THRESHOLD: f64 = 0.8;
    const MIN_WORDS: usize = 5;

    let initial_count = segments.len();
    let mut result: Vec<WhisperSegment> = Vec::with_capacity(segments.len());
    let mut recent: VecDeque<String> = VecDeque::with_capacity(WINDOW_SIZE + 1);

    for seg in segments {
        let normalized = normalize_for_comparison(&seg.text);
        let word_count = normalized.split_whitespace().count();

        let is_duplicate = word_count >= MIN_WORDS
            && recent.iter().any(|prev| {
                let prev_words = prev.split_whitespace().count();
                prev_words >= MIN_WORDS
                    && word_jaccard_similarity(prev, &normalized) > SIMILARITY_THRESHOLD
            });

        if !is_duplicate {
            result.push(seg);
        } else {
            log::debug!(
                "deduplicate_segments: removed near-duplicate: '{}'",
                &seg.text[..seg.text.len().min(80)]
            );
        }

        recent.push_back(normalized);
        if recent.len() > WINDOW_SIZE {
            recent.pop_front();
        }
    }

    let removed = initial_count - result.len();
    if removed > 0 {
        log::info!(
            "deduplicate_segments: removed {}/{} near-duplicate segments",
            removed,
            initial_count
        );
    }

    result
}

/// Remove hallucination bursts — runs of 4+ consecutive segments with similar text.
///
/// Unlike `deduplicate_segments` which uses a similarity window for longer segments,
/// this catches the pattern where whisper gets stuck repeating a short phrase
/// (e.g. "I see.", "I don't know.") across many consecutive segments.
/// Uses Jaccard word similarity > 0.5 to detect runs, keeping only the first segment
/// of each burst.
pub fn remove_hallucination_bursts(segments: Vec<WhisperSegment>) -> Vec<WhisperSegment> {
    if segments.len() < 4 {
        return segments;
    }

    const BURST_THRESHOLD: usize = 4;
    const SIMILARITY_THRESHOLD: f64 = 0.5;

    let normalized: Vec<String> = segments.iter()
        .map(|s| normalize_for_comparison(&s.text))
        .collect();

    let mut keep = vec![true; segments.len()];
    let mut i = 0;

    while i < segments.len() {
        // Skip empty segments
        if normalized[i].is_empty() {
            i += 1;
            continue;
        }

        // Extend the run while consecutive segments are similar
        let mut run_end = i + 1;
        while run_end < segments.len() {
            if !normalized[run_end].is_empty()
                && word_jaccard_similarity(&normalized[i], &normalized[run_end]) > SIMILARITY_THRESHOLD
            {
                run_end += 1;
            } else {
                break;
            }
        }

        let run_length = run_end - i;
        if run_length >= BURST_THRESHOLD {
            // Keep only the first segment in the burst
            for k in (i + 1)..run_end {
                keep[k] = false;
            }
            log::info!(
                "remove_hallucination_bursts: removed {} segments in burst at index {} ('{}')",
                run_length - 1,
                i,
                &normalized[i][..normalized[i].len().min(40)]
            );
        }

        i = run_end;
    }

    let removed = keep.iter().filter(|&&k| !k).count();
    if removed > 0 {
        log::info!(
            "remove_hallucination_bursts: removed {}/{} burst segments total",
            removed,
            segments.len()
        );
    }

    segments
        .into_iter()
        .zip(keep.into_iter())
        .filter(|(_, k)| *k)
        .map(|(s, _)| s)
        .collect()
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

/// Emit rescaled progress for chunked meeting transcription.
///
/// Maps a per-chunk local percent (0-100) to a global percent across all chunks.
pub fn emit_chunk_progress(app: &tauri::AppHandle, chunk_idx: usize, num_chunks: usize, local_percent: i32) {
    use tauri::Emitter;
    let global_percent = if num_chunks == 0 {
        0
    } else {
        ((chunk_idx as i32 * 100 + local_percent) / num_chunks as i32).min(100)
    };
    let _ = app.emit(
        "meeting-transcription-progress",
        serde_json::json!({"stage": "transcribing", "percent": global_percent}),
    );
}

/// Single-pass meeting transcription for short audio.
///
/// Processes the entire audio buffer in one whisper inference call.
fn transcribe_meeting_audio_single(
    ctx: &whisper_rs::WhisperContext,
    audio_data: &[f32],
    model_name: &str,
    app: Option<tauri::AppHandle>,
) -> Result<Vec<WhisperSegment>, String> {
    log::info!(
        "meeting_transcription: single-pass, model={}, samples={}",
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

/// Transcribe meeting audio using whisper-rs, returning individual segments with timing.
///
/// For short audio (<= CHUNK_THRESHOLD), runs a single-pass transcription.
/// For long audio, splits into 5-minute chunks with 1-second overlap, transcribes
/// each independently, offsets timestamps, and deduplicates overlap segments.
/// When an `app` handle is provided, emits rescaled progress events during inference.
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

    if audio_data.len() <= crate::transcription::CHUNK_THRESHOLD {
        return transcribe_meeting_audio_single(ctx, audio_data, model_name, app);
    }

    let chunks = crate::transcription::compute_chunks(
        audio_data.len(),
        crate::transcription::CHUNK_SIZE,
        crate::transcription::CHUNK_OVERLAP,
    );
    let num_chunks = chunks.len();
    log::info!(
        "meeting_transcription: chunking {} samples into {} chunks",
        audio_data.len(),
        num_chunks
    );

    let overlap_sec = crate::transcription::CHUNK_OVERLAP as f64 / 16000.0;
    let mut all_segments = Vec::new();

    for (chunk_idx, (start, end)) in chunks.iter().enumerate() {
        log::info!(
            "meeting_transcription: chunk {}/{} (samples {}..{})",
            chunk_idx + 1,
            num_chunks,
            start,
            end
        );

        let chunk_audio = &audio_data[*start..*end];

        // Build params with rescaled progress callback
        let mut params = crate::transcription::build_whisper_params(500, None);
        if let Some(ref app_handle) = app {
            let app_cb = app_handle.clone();
            let ci = chunk_idx;
            let nc = num_chunks;
            params.set_progress_callback_safe(move |local_percent: i32| {
                emit_chunk_progress(&app_cb, ci, nc, local_percent);
            });
        }

        let mut state = ctx
            .create_state()
            .map_err(|e| format!("Failed to create whisper state: {}", e))?;

        state
            .full(params, chunk_audio)
            .map_err(|e| format!("Whisper inference failed on chunk {}: {}", chunk_idx, e))?;

        let num_segments = state.full_n_segments();
        let time_offset = *start as f64 / 16000.0;

        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                let start_cs = segment.start_timestamp();
                let end_cs = segment.end_timestamp();
                let seg_start = start_cs as f64 / 100.0;
                let seg_end = end_cs as f64 / 100.0;

                // For chunks after the first, skip segments starting in the overlap zone
                if chunk_idx > 0 && seg_start < overlap_sec {
                    continue;
                }

                let raw_text = segment.to_str().unwrap_or_default().to_string();
                let text = crate::transcription::detect_and_remove_repetitions(&raw_text);
                let no_speech_prob = segment.no_speech_probability();

                all_segments.push(WhisperSegment {
                    start_sec: seg_start + time_offset,
                    end_sec: seg_end + time_offset,
                    text,
                    no_speech_prob,
                    speaker: None,
                });
            }
        }
    }

    log::info!(
        "meeting_transcription: {} total segments from {} chunks",
        all_segments.len(),
        num_chunks
    );
    Ok(all_segments)
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
            Some(((completed * 40 / total) as i32).min(40))
        }
        "speaker_counting" => Some(45),
        "embeddings" => {
            let completed = v.get("completed")?.as_i64()?;
            let total = v.get("total")?.as_i64()?;
            if total == 0 {
                return Some(45);
            }
            Some(((45 + completed * 45 / total) as i32).min(90))
        }
        "discrete_diarization" => Some(95),
        _ => None,
    }
}

/// Run pyannote speaker diarization via a Python subprocess.
///
/// Returns an error with an actionable message if diarization fails.
/// When an `app` handle is provided, emits `meeting-transcription-progress` events
/// with `{"stage": "diarizing", "percent": N}` as the Python script reports progress.
pub fn run_diarization(wav_bytes: &[u8], app: Option<tauri::AppHandle>) -> Result<Vec<SpeakerTurn>, DiarizeError> {
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
            return Err(DiarizeError::TempFile(e.to_string()));
        }
    };
    if let Err(e) = file.write_all(wav_bytes) {
        log::warn!("run_diarization: failed to write temp file: {}", e);
        let _ = std::fs::remove_file(&temp_path);
        return Err(DiarizeError::TempFile(e.to_string()));
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
            return Err(DiarizeError::PythonNotFound(format!("{}: {}", python, e)));
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
                            log::debug!("diarization progress: {}%", percent);
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
            return Err(DiarizeError::ScriptFailed {
                reason: format!("Failed to wait for Python process: {}", e),
                stderr: String::new(),
            });
        }
    };

    // Collect stderr from the thread
    let stderr_output = stderr_thread.join().unwrap_or_default();

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    if !status.success() {
        let reason = classify_diarization_error(&stderr_output);
        log::warn!(
            "run_diarization: Python script failed (exit {}): {}",
            status, stderr_output
        );
        return Err(DiarizeError::ScriptFailed {
            reason,
            stderr: stderr_output,
        });
    }

    // Parse stdout as JSON array of SpeakerTurn
    match serde_json::from_str::<Vec<SpeakerTurn>>(&stdout_data) {
        Ok(turns) => {
            log::info!(
                "run_diarization: found {} speaker turns",
                turns.len()
            );
            Ok(turns)
        }
        Err(e) => {
            log::warn!("run_diarization: failed to parse JSON: {}", e);
            Err(DiarizeError::InvalidOutput(e.to_string()))
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

    #[test]
    fn test_parse_diarization_progress_segmentation_zero_total() {
        let line = r#"{"step":"segmentation","completed":0,"total":0}"#;
        assert_eq!(parse_diarization_progress(line), Some(0));
    }

    #[test]
    fn test_parse_diarization_progress_embeddings_zero_total() {
        let line = r#"{"step":"embeddings","completed":0,"total":0}"#;
        assert_eq!(parse_diarization_progress(line), Some(45));
    }

    // ===============================================================
    // Chunk progress rescaling tests
    // ===============================================================

    #[test]
    fn test_chunk_progress_rescaling_first_chunk() {
        // chunk 0 of 3, local 50% => global = (0*100 + 50) / 3 = 16
        let global = (0i32 * 100 + 50) / 3;
        assert_eq!(global, 16);
    }

    #[test]
    fn test_chunk_progress_rescaling_middle_chunk() {
        // chunk 1 of 3, local 50% => global = (1*100 + 50) / 3 = 50
        let global = (1i32 * 100 + 50) / 3;
        assert_eq!(global, 50);
    }

    #[test]
    fn test_chunk_progress_rescaling_last_chunk_complete() {
        // chunk 2 of 3, local 100% => global = (2*100 + 100) / 3 = 100
        let global = ((2i32 * 100 + 100) / 3).min(100);
        assert_eq!(global, 100);
    }

    #[test]
    fn test_chunk_progress_rescaling_single_chunk() {
        // chunk 0 of 1, local 75% => global = (0*100 + 75) / 1 = 75
        let global = (0i32 * 100 + 75) / 1;
        assert_eq!(global, 75);
    }

    // ===============================================================
    // Overlap dedup math tests
    // ===============================================================

    #[test]
    fn test_overlap_dedup_first_chunk_keeps_all() {
        // chunk_idx=0: all segments should be kept regardless of start time
        let chunk_idx = 0;
        let overlap_sec = crate::transcription::CHUNK_OVERLAP as f64 / 16000.0;
        let seg_start = 0.5; // within overlap zone

        let should_skip = chunk_idx > 0 && seg_start < overlap_sec;
        assert!(!should_skip, "First chunk should keep all segments");
    }

    #[test]
    fn test_overlap_dedup_second_chunk_skips_overlap() {
        // chunk_idx=1: segments starting before overlap_sec should be skipped
        let chunk_idx = 1;
        let overlap_sec = crate::transcription::CHUNK_OVERLAP as f64 / 16000.0;
        assert!((overlap_sec - 1.0).abs() < 0.001, "Overlap should be ~1 second");

        // Segment at 0.5s (within overlap zone) should be skipped
        let seg_start = 0.5;
        let should_skip = chunk_idx > 0 && seg_start < overlap_sec;
        assert!(should_skip, "Segment in overlap zone should be skipped");

        // Segment at 1.5s (past overlap zone) should be kept
        let seg_start = 1.5;
        let should_skip = chunk_idx > 0 && seg_start < overlap_sec;
        assert!(!should_skip, "Segment past overlap zone should be kept");
    }

    #[test]
    fn test_overlap_dedup_timestamp_offset() {
        // chunk starting at sample 4_784_000 (after 5min - 1sec overlap)
        let chunk_start = crate::transcription::CHUNK_SIZE - crate::transcription::CHUNK_OVERLAP;
        let time_offset = chunk_start as f64 / 16000.0;

        // A segment at local 2.0s should become time_offset + 2.0
        let local_start = 2.0;
        let global_start = local_start + time_offset;

        // Expected: (4_800_000 - 16_000) / 16000 + 2.0 = 299.0 + 2.0 = 301.0
        let expected = (crate::transcription::CHUNK_SIZE - crate::transcription::CHUNK_OVERLAP) as f64 / 16000.0 + 2.0;
        assert!((global_start - expected).abs() < 0.001);
    }

    // ---------------------------------------------------------------
    // classify_diarization_error tests
    // ---------------------------------------------------------------

    #[test]
    fn test_classify_diarization_error_pyannote_missing() {
        let stderr = "ModuleNotFoundError: No module named 'pyannote'";
        assert!(classify_diarization_error(stderr).contains("pyannote.audio is not installed"));
    }

    #[test]
    fn test_classify_diarization_error_torch_missing() {
        let stderr = "ModuleNotFoundError: No module named 'torch'";
        assert!(classify_diarization_error(stderr).contains("pyannote.audio is not installed"));
    }

    #[test]
    fn test_classify_diarization_error_hf_token() {
        let stderr = "HF_TOKEN environment variable is not set";
        assert!(classify_diarization_error(stderr).contains("HF_TOKEN"));
    }

    #[test]
    fn test_classify_diarization_error_gated_model() {
        let stderr = "Access to model pyannote/speaker-diarization-3.1 is restricted";
        assert!(classify_diarization_error(stderr).contains("Accept the license"));
    }

    #[test]
    fn test_classify_diarization_error_unknown() {
        let stderr = "some random error";
        assert!(classify_diarization_error(stderr).contains("Check logs"));
    }

    // ===============================================================
    // Segment deduplication tests
    // ===============================================================

    #[test]
    fn test_deduplicate_removes_identical_segments() {
        let segments = vec![
            seg(0.0, 5.0, "I think we should focus on the product experience for our users"),
            seg(5.0, 10.0, "I think we should focus on the product experience for our users"),
            seg(10.0, 15.0, "I think we should focus on the product experience for our users"),
            seg(15.0, 20.0, "Moving on to the next topic of discussion"),
        ];
        let result = deduplicate_segments(segments);
        assert_eq!(result.len(), 2, "Should keep first occurrence and the different segment");
        assert!(result[0].text.contains("product experience"));
        assert!(result[1].text.contains("next topic"));
    }

    #[test]
    fn test_deduplicate_removes_near_identical_segments() {
        let segments = vec![
            seg(0.0, 5.0, "I think we should focus on the product experience for our users"),
            seg(5.0, 10.0, "I think we should focus on the product experience for our users."),
            seg(10.0, 15.0, "And I think we should focus on the product experience for our users"),
        ];
        let result = deduplicate_segments(segments);
        assert_eq!(result.len(), 1, "Near-duplicates with punctuation/filler differences should be removed");
    }

    #[test]
    fn test_deduplicate_preserves_different_segments() {
        let segments = vec![
            seg(0.0, 5.0, "Today we are going to discuss the roadmap for next quarter"),
            seg(5.0, 10.0, "The first item on the agenda is our hiring plan for engineering"),
            seg(10.0, 15.0, "We need to finalize the budget before the end of this week"),
        ];
        let result = deduplicate_segments(segments);
        assert_eq!(result.len(), 3, "Different segments should all be preserved");
    }

    #[test]
    fn test_deduplicate_preserves_short_segments() {
        let segments = vec![
            seg(0.0, 1.0, "Yes, right."),
            seg(1.0, 2.0, "Yes, right."),
            seg(2.0, 3.0, "Okay."),
        ];
        let result = deduplicate_segments(segments);
        assert_eq!(result.len(), 3, "Short segments (<5 words) should be exempt from dedup");
    }

    #[test]
    fn test_deduplicate_handles_empty_and_single() {
        assert_eq!(deduplicate_segments(vec![]).len(), 0);
        let single = vec![seg(0.0, 5.0, "Just one segment here.")];
        assert_eq!(deduplicate_segments(single).len(), 1);
    }

    #[test]
    fn test_deduplicate_sliding_window() {
        // A, B, C, A pattern with window_size=5 should catch the repeated A
        let segments = vec![
            seg(0.0, 5.0, "I think we should focus on the product experience for our users"),
            seg(5.0, 10.0, "The budget needs to be finalized before the quarterly review meeting"),
            seg(10.0, 15.0, "Let me check with the engineering team about their timeline estimate"),
            seg(15.0, 20.0, "I think we should focus on the product experience for our users"),
        ];
        let result = deduplicate_segments(segments);
        assert_eq!(result.len(), 3, "Repeated A after B,C should be caught within window");
    }

    #[test]
    fn test_normalize_for_comparison() {
        assert_eq!(
            normalize_for_comparison("Hello, World! How's it going?"),
            "hello world how s it going"
        );
        assert_eq!(
            normalize_for_comparison("  Multiple   spaces   here  "),
            "multiple spaces here"
        );
    }

    #[test]
    fn test_word_jaccard_similarity() {
        assert!((word_jaccard_similarity("a b c d e", "a b c d e") - 1.0).abs() < 0.001);
        assert!((word_jaccard_similarity("a b c", "d e f") - 0.0).abs() < 0.001);
        // {a,b,c,d} vs {a,b,c,e} => intersection=3, union=5 => 0.6
        assert!((word_jaccard_similarity("a b c d", "a b c e") - 0.6).abs() < 0.001);
        assert!((word_jaccard_similarity("", "") - 1.0).abs() < 0.001);
        assert!((word_jaccard_similarity("a", "") - 0.0).abs() < 0.001);
    }

    // ===============================================================
    // Hallucination burst detection tests
    // ===============================================================

    #[test]
    fn test_burst_removes_i_see_repetition() {
        let mut segments = Vec::new();
        for i in 0..10 {
            segments.push(seg(i as f64, (i + 1) as f64, "I see."));
        }
        segments.push(seg(10.0, 12.0, "Anyway, let me check something different now."));
        let result = remove_hallucination_bursts(segments);
        assert_eq!(result.len(), 2, "Should keep first 'I see' + the different segment");
        assert_eq!(result[0].text, "I see.");
        assert!(result[1].text.contains("Anyway"));
    }

    #[test]
    fn test_burst_removes_i_dont_know_repetition() {
        let mut segments = Vec::new();
        for i in 0..8 {
            let text = if i % 2 == 0 { "I don't know." } else { "I don't know. I don't know." };
            segments.push(seg(i as f64, (i + 1) as f64, text));
        }
        let result = remove_hallucination_bursts(segments);
        assert_eq!(result.len(), 1, "All variations of 'I don't know' should collapse to one");
    }

    #[test]
    fn test_burst_preserves_short_runs() {
        let segments = vec![
            seg(0.0, 1.0, "I see."),
            seg(1.0, 2.0, "I see."),
            seg(2.0, 3.0, "I see."),
            seg(3.0, 5.0, "That's interesting, tell me more about it."),
        ];
        let result = remove_hallucination_bursts(segments);
        assert_eq!(result.len(), 4, "Run of 3 is below threshold of 4, all preserved");
    }

    #[test]
    fn test_burst_handles_two_separate_bursts() {
        let mut segments = Vec::new();
        // First burst: "I see" x5
        for i in 0..5 {
            segments.push(seg(i as f64, (i + 1) as f64, "I see."));
        }
        // Legitimate segment in between
        segments.push(seg(5.0, 7.0, "Okay so moving on to the next item on our agenda"));
        // Second burst: "I don't know" x6
        for i in 7..13 {
            segments.push(seg(i as f64, (i + 1) as f64, "I don't know."));
        }
        let result = remove_hallucination_bursts(segments);
        assert_eq!(result.len(), 3, "Two bursts collapsed + one legitimate segment");
        assert_eq!(result[0].text, "I see.");
        assert!(result[1].text.contains("agenda"));
        assert_eq!(result[2].text, "I don't know.");
    }

    #[test]
    fn test_burst_preserves_varied_conversation() {
        let segments = vec![
            seg(0.0, 3.0, "I think we should focus on the database issue first"),
            seg(3.0, 6.0, "Yeah the database has been slow since the migration"),
            seg(6.0, 9.0, "Let me check the query performance metrics now"),
            seg(9.0, 12.0, "The index seems to be missing on the users table"),
        ];
        let result = remove_hallucination_bursts(segments);
        assert_eq!(result.len(), 4, "Varied conversation should be fully preserved");
    }

    #[test]
    fn test_diarize_error_display() {
        let e = DiarizeError::PythonNotFound("python3: No such file or directory".into());
        assert_eq!(e.to_string(), "Python not found: python3: No such file or directory");

        let e = DiarizeError::TempFile("permission denied".into());
        assert_eq!(e.to_string(), "Failed to create temp file: permission denied");

        let e = DiarizeError::ScriptFailed {
            reason: "pyannote.audio is not installed. Install with: pip install pyannote.audio".into(),
            stderr: "No module named 'pyannote'".into(),
        };
        assert_eq!(e.to_string(), "pyannote.audio is not installed. Install with: pip install pyannote.audio");

        let e = DiarizeError::InvalidOutput("expected value at line 1".into());
        assert_eq!(e.to_string(), "Invalid diarization output: expected value at line 1");
    }
}
