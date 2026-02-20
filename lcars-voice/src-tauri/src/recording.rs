//! Audio recording via cpal (cross-platform) with rubato resampling.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

pub enum CaptureMode {
    MicOnly,
    MicAndMonitor { monitor_device: cpal::Device },
}

pub struct Recorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_active: Arc<AtomicBool>,
    rms_level: Arc<AtomicU32>,
    device_sample_rate: u32,
    device_channels: u16,
    start_time: Option<Instant>,

    // Dual-stream fields
    monitor_stream: Option<cpal::Stream>,
    monitor_buffer: Arc<Mutex<Vec<f32>>>,
    monitor_sample_rate: u32,
    monitor_channels: u16,

    // Pause/resume fields
    elapsed_before_pause: std::time::Duration,
    is_paused: bool,
}

// SAFETY: cpal::Stream is !Send because it contains platform-specific handles
// that are not safe to move between threads in general. However, in our usage,
// the Stream is created and dropped on the main thread (via Mutex<Recorder>),
// and only the audio callback runs on a separate thread (which accesses only
// the Arc-shared buffer and atomics, not the Stream itself). The Stream field
// is only accessed through the Mutex, ensuring single-threaded access.
unsafe impl Send for Recorder {}

#[derive(Debug)]
pub struct RecordingResult {
    pub audio_data: Vec<f32>,
    pub duration_ms: i64,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_active: Arc::new(AtomicBool::new(false)),
            rms_level: Arc::new(AtomicU32::new(0u32)),
            device_sample_rate: 0,
            device_channels: 0,
            start_time: None,
            monitor_stream: None,
            monitor_buffer: Arc::new(Mutex::new(Vec::new())),
            monitor_sample_rate: 0,
            monitor_channels: 0,
            elapsed_before_pause: std::time::Duration::ZERO,
            is_paused: false,
        }
    }

    pub fn start(&mut self, mode: CaptureMode) -> Result<(), String> {
        if self.stream.is_some() {
            return Err("Already recording".to_string());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No input device available".to_string())?;

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        self.device_sample_rate = config.sample_rate().0;
        self.device_channels = config.channels();

        eprintln!(
            "[LCARS] recording: device={:?}, rate={}, channels={}",
            device.name().unwrap_or_else(|_| "unknown".to_string()),
            self.device_sample_rate,
            self.device_channels,
        );

        // Clear the buffers for a new recording
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
        if let Ok(mut buf) = self.monitor_buffer.lock() {
            buf.clear();
        }

        self.is_active.store(true, Ordering::SeqCst);
        self.rms_level.store(0f32.to_bits(), Ordering::SeqCst);
        self.elapsed_before_pause = std::time::Duration::ZERO;
        self.is_paused = false;

        let buffer = Arc::clone(&self.buffer);
        let is_active = Arc::clone(&self.is_active);
        let rms_level = Arc::clone(&self.rms_level);
        let mut sample_counter: usize = 0;

        let stream_config: cpal::StreamConfig = config.config();

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !is_active.load(Ordering::SeqCst) {
                        return;
                    }

                    if let Ok(mut buf) = buffer.lock() {
                        buf.extend_from_slice(data);
                    }

                    // Compute RMS every ~1600 samples for UI updates
                    sample_counter += data.len();
                    if sample_counter >= 1600 {
                        sample_counter = 0;
                        let sum_sq: f32 =
                            data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32;
                        let rms = sum_sq.sqrt();
                        rms_level.store(rms.to_bits(), Ordering::SeqCst);
                    }
                },
                move |err| {
                    eprintln!("[LCARS] recording: stream error: {}", err);
                },
                None,
            )
            .map_err(|e| format!("Failed to build input stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start stream: {}", e))?;

        self.stream = Some(stream);

        // Start monitor stream if in dual-stream mode
        if let CaptureMode::MicAndMonitor { monitor_device } = mode {
            let monitor_config = monitor_device
                .default_input_config()
                .map_err(|e| format!("Failed to get monitor input config: {}", e))?;

            self.monitor_sample_rate = monitor_config.sample_rate().0;
            self.monitor_channels = monitor_config.channels();

            eprintln!(
                "[LCARS] recording: monitor device={:?}, rate={}, channels={}",
                monitor_device
                    .name()
                    .unwrap_or_else(|_| "unknown".to_string()),
                self.monitor_sample_rate,
                self.monitor_channels,
            );

            let monitor_buffer = Arc::clone(&self.monitor_buffer);
            let monitor_is_active = Arc::clone(&self.is_active);
            let monitor_stream_config: cpal::StreamConfig = monitor_config.config();

            let monitor_stream = monitor_device
                .build_input_stream(
                    &monitor_stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !monitor_is_active.load(Ordering::SeqCst) {
                            return;
                        }
                        if let Ok(mut buf) = monitor_buffer.lock() {
                            buf.extend_from_slice(data);
                        }
                    },
                    move |err| {
                        eprintln!("[LCARS] recording: monitor stream error: {}", err);
                    },
                    None,
                )
                .map_err(|e| format!("Failed to build monitor input stream: {}", e))?;

            monitor_stream
                .play()
                .map_err(|e| format!("Failed to start monitor stream: {}", e))?;

            self.monitor_stream = Some(monitor_stream);
        }

        self.start_time = Some(Instant::now());

        eprintln!("[LCARS] recording: stream started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<RecordingResult, String> {
        if self.stream.is_none() {
            return Err("Not recording".to_string());
        }

        // Signal the callback to stop capturing
        self.is_active.store(false, Ordering::SeqCst);

        // Calculate duration accounting for pauses
        let mut total_elapsed = self.elapsed_before_pause;
        if let Some(start) = self.start_time {
            if !self.is_paused {
                total_elapsed += start.elapsed();
            }
        }
        let duration_ms = total_elapsed.as_millis() as i64;

        // Drop the streams to stop the audio devices
        self.stream = None;
        self.monitor_stream = None;
        self.start_time = None;
        self.elapsed_before_pause = std::time::Duration::ZERO;
        self.is_paused = false;

        // Take the mic buffer contents
        let raw_samples = if let Ok(mut buf) = self.buffer.lock() {
            std::mem::take(&mut *buf)
        } else {
            return Err("Failed to lock buffer".to_string());
        };

        if raw_samples.is_empty() {
            return Err("No audio data captured".to_string());
        }

        eprintln!(
            "[LCARS] recording: captured {} raw samples ({} ms)",
            raw_samples.len(),
            duration_ms
        );

        // Downmix mic to mono and resample to 16KHz
        let mono = downmix_to_mono(&raw_samples, self.device_channels);
        let mic_audio = resample_to_16khz(&mono, self.device_sample_rate)?;

        // Check if we have monitor data
        let monitor_samples = if let Ok(mut buf) = self.monitor_buffer.lock() {
            std::mem::take(&mut *buf)
        } else {
            Vec::new()
        };

        let audio_data = if !monitor_samples.is_empty() {
            eprintln!(
                "[LCARS] recording: captured {} monitor raw samples",
                monitor_samples.len()
            );
            let monitor_mono = downmix_to_mono(&monitor_samples, self.monitor_channels);
            let monitor_audio = resample_to_16khz(&monitor_mono, self.monitor_sample_rate)?;
            mix_streams(&mic_audio, &monitor_audio)
        } else {
            mic_audio
        };

        eprintln!(
            "[LCARS] recording: processed to {} mono 16KHz samples",
            audio_data.len()
        );

        // Reset RMS
        self.rms_level.store(0f32.to_bits(), Ordering::SeqCst);

        Ok(RecordingResult {
            audio_data,
            duration_ms,
        })
    }

    pub fn current_rms_level(&self) -> f32 {
        f32::from_bits(self.rms_level.load(Ordering::SeqCst))
    }

    pub fn pause(&mut self) -> Result<(), String> {
        if self.stream.is_none() {
            return Err("Not recording".to_string());
        }
        if self.is_paused {
            return Err("Already paused".to_string());
        }
        self.is_active.store(false, Ordering::SeqCst);
        if let Some(start) = self.start_time {
            self.elapsed_before_pause += start.elapsed();
        }
        self.start_time = None;
        self.is_paused = true;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), String> {
        if !self.is_paused {
            return Err("Not paused".to_string());
        }
        self.is_active.store(true, Ordering::SeqCst);
        self.start_time = Some(Instant::now());
        self.is_paused = false;
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn elapsed_seconds(&self) -> f64 {
        let mut total = self.elapsed_before_pause;
        if let Some(start) = self.start_time {
            if !self.is_paused {
                total += start.elapsed();
            }
        }
        total.as_secs_f64()
    }
}

/// Downmix interleaved multi-channel samples to mono by averaging all channels
/// per frame. For mono input (channels == 1), returns the input unchanged.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Mix two audio streams by sample-wise averaging with clamping.
pub fn mix_streams(mic: &[f32], monitor: &[f32]) -> Vec<f32> {
    let len = mic.len().max(monitor.len());
    (0..len)
        .map(|i| match (mic.get(i).copied(), monitor.get(i).copied()) {
            (Some(a), Some(b)) => ((a + b) / 2.0).clamp(-1.0, 1.0),
            (Some(a), None) => a.clamp(-1.0, 1.0),
            (None, Some(b)) => b.clamp(-1.0, 1.0),
            (None, None) => 0.0,
        })
        .collect()
}

/// Resample mono audio from `source_rate` to 16000 Hz using rubato's
/// SincFixedIn resampler. If the source is already 16000 Hz, returns
/// the input unchanged (passthrough).
pub fn resample_to_16khz(mono: &[f32], source_rate: u32) -> Result<Vec<f32>, String> {
    const TARGET_RATE: u32 = 16000;

    if source_rate == TARGET_RATE {
        return Ok(mono.to_vec());
    }

    if mono.is_empty() {
        return Ok(Vec::new());
    }

    let ratio = TARGET_RATE as f64 / source_rate as f64;

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    // Process in chunks; use a reasonable chunk size
    let chunk_size = 1024;

    let mut resampler = SincFixedIn::<f64>::new(
        ratio, 2.0, params, chunk_size, 1, // mono
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let mut output: Vec<f32> = Vec::with_capacity((mono.len() as f64 * ratio * 1.1) as usize);

    // Convert f32 input to f64 for rubato
    let mono_f64: Vec<f64> = mono.iter().map(|&s| s as f64).collect();

    // Process in chunks of chunk_size
    let mut offset = 0;
    while offset + chunk_size <= mono_f64.len() {
        let chunk = &mono_f64[offset..offset + chunk_size];
        let result = resampler
            .process(&[chunk.to_vec()], None)
            .map_err(|e| format!("Resample error: {}", e))?;
        output.extend(result[0].iter().map(|&s| s as f32));
        offset += chunk_size;
    }

    // Handle the remaining samples by padding with zeros
    let remaining = mono_f64.len() - offset;
    if remaining > 0 {
        let mut last_chunk = vec![0.0f64; chunk_size];
        last_chunk[..remaining].copy_from_slice(&mono_f64[offset..]);
        let result = resampler
            .process(&[last_chunk], None)
            .map_err(|e| format!("Resample error (final chunk): {}", e))?;
        // Only take the proportional number of output samples for the remaining input
        let expected_out = (remaining as f64 * ratio).ceil() as usize;
        let take = expected_out.min(result[0].len());
        output.extend(result[0][..take].iter().map(|&s| s as f32));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downmix_mono_passthrough() {
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let result = downmix_to_mono(&input, 1);
        assert_eq!(result, input);
    }

    #[test]
    fn test_downmix_stereo() {
        // Stereo frames: (L, R) pairs
        let input = vec![0.2, 0.8, 0.4, 0.6, 1.0, 0.0];
        let result = downmix_to_mono(&input, 2);
        assert_eq!(result.len(), 3);
        assert!((result[0] - 0.5).abs() < 1e-6); // (0.2 + 0.8) / 2
        assert!((result[1] - 0.5).abs() < 1e-6); // (0.4 + 0.6) / 2
        assert!((result[2] - 0.5).abs() < 1e-6); // (1.0 + 0.0) / 2
    }

    #[test]
    fn test_downmix_quad_channel() {
        // 4-channel frame: 4 samples per frame
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.8, 0.6, 0.4, 0.2];
        let result = downmix_to_mono(&input, 4);
        assert_eq!(result.len(), 2);
        assert!((result[0] - 0.25).abs() < 1e-6); // (0.1+0.2+0.3+0.4)/4
        assert!((result[1] - 0.5).abs() < 1e-6); // (0.8+0.6+0.4+0.2)/4
    }

    #[test]
    fn test_resample_passthrough_at_16khz() {
        let input: Vec<f32> = (0..160).map(|i| (i as f32) / 160.0).collect();
        let result = resample_to_16khz(&input, 16000).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_resample_44100_to_16000() {
        // 1 second of 44100Hz mono audio
        let num_samples = 44100;
        let input: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let result = resample_to_16khz(&input, 44100).unwrap();
        // Should produce approximately 16000 samples (within 5% tolerance)
        let expected = 16000;
        let tolerance = (expected as f32 * 0.05) as usize;
        assert!(
            (result.len() as i64 - expected as i64).unsigned_abs() as usize <= tolerance,
            "Expected ~{} samples, got {}",
            expected,
            result.len()
        );
    }

    #[test]
    fn test_resample_48000_to_16000() {
        // 1 second of 48000Hz mono audio
        let num_samples = 48000;
        let input: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
            .collect();
        let result = resample_to_16khz(&input, 48000).unwrap();
        // Should produce approximately 16000 samples (within 5% tolerance)
        let expected = 16000;
        let tolerance = (expected as f32 * 0.05) as usize;
        assert!(
            (result.len() as i64 - expected as i64).unsigned_abs() as usize <= tolerance,
            "Expected ~{} samples, got {}",
            expected,
            result.len()
        );
    }

    #[test]
    fn test_recorder_initial_state() {
        let recorder = Recorder::new();
        assert!(recorder.stream.is_none());
        assert!((recorder.current_rms_level() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_stop_without_start() {
        let mut recorder = Recorder::new();
        let result = recorder.stop();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Not recording");
    }

    #[test]
    fn test_recording_result_fields() {
        let audio = vec![0.1, 0.2, 0.3];
        let result = RecordingResult {
            audio_data: audio.clone(),
            duration_ms: 1500,
        };
        assert_eq!(result.audio_data, audio);
        assert_eq!(result.duration_ms, 1500);
    }

    // --- mix_streams tests ---

    #[test]
    fn test_mix_streams_equal_length() {
        let mic = vec![0.4, 0.6, -0.2];
        let monitor = vec![0.2, 0.4, 0.8];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 0.3).abs() < 1e-6); // (0.4+0.2)/2
        assert!((mixed[1] - 0.5).abs() < 1e-6); // (0.6+0.4)/2
        assert!((mixed[2] - 0.3).abs() < 1e-6); // (-0.2+0.8)/2
    }

    #[test]
    fn test_mix_streams_mic_longer() {
        let mic = vec![0.5, 0.5, 0.5];
        let monitor = vec![0.5];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 0.5).abs() < 1e-6); // (0.5+0.5)/2
        assert!((mixed[1] - 0.5).abs() < 1e-6); // mic only, full volume
        assert!((mixed[2] - 0.5).abs() < 1e-6); // mic only, full volume
    }

    #[test]
    fn test_mix_streams_monitor_longer() {
        let mic = vec![0.5];
        let monitor = vec![0.5, 0.5, 0.5];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 0.5).abs() < 1e-6);
        assert!((mixed[1] - 0.5).abs() < 1e-6); // monitor only, full volume
    }

    #[test]
    fn test_mix_streams_clamp() {
        let mic = vec![0.9];
        let monitor = vec![0.9];
        let mixed = mix_streams(&mic, &monitor);
        // (0.9+0.9)/2 = 0.9, should not exceed 1.0
        assert!((mixed[0] - 0.9).abs() < 1e-6);

        // Test extreme values
        let mic2 = vec![1.5]; // already clipping
        let monitor2 = vec![1.5];
        let mixed2 = mix_streams(&mic2, &monitor2);
        assert!(mixed2[0] <= 1.0);
    }

    #[test]
    fn test_mix_streams_empty_both() {
        let mixed = mix_streams(&[], &[]);
        assert!(mixed.is_empty());
    }

    #[test]
    fn test_mix_streams_one_empty() {
        let mic = vec![0.6, 0.4];
        let mixed = mix_streams(&mic, &[]);
        assert_eq!(mixed.len(), 2);
        assert!((mixed[0] - 0.6).abs() < 1e-6); // mic only, full volume
        assert!((mixed[1] - 0.4).abs() < 1e-6); // mic only, full volume
    }

    #[test]
    fn test_mix_streams_single_source_full_volume() {
        let mic = vec![0.8, 0.6, 0.4];
        let monitor = vec![0.8];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 0.8).abs() < 1e-6); // both present, averaged: (0.8+0.8)/2
        assert!((mixed[1] - 0.6).abs() < 1e-6); // mic only, FULL volume
        assert!((mixed[2] - 0.4).abs() < 1e-6); // mic only, FULL volume
    }

    #[test]
    fn test_mix_streams_single_source_monitor_only_full_volume() {
        let mic = vec![];
        let monitor = vec![0.7, 0.5];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 2);
        assert!((mixed[0] - 0.7).abs() < 1e-6);
        assert!((mixed[1] - 0.5).abs() < 1e-6);
    }

    // --- pause/resume tests ---

    #[test]
    fn test_recorder_initial_pause_state() {
        let recorder = Recorder::new();
        assert!(!recorder.is_paused());
        assert!(recorder.elapsed_seconds() < 0.01);
    }

    #[test]
    fn test_pause_not_recording() {
        let mut recorder = Recorder::new();
        let result = recorder.pause();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Not recording");
    }

    #[test]
    fn test_resume_not_paused() {
        let mut recorder = Recorder::new();
        let result = recorder.resume();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Not paused");
    }
}
