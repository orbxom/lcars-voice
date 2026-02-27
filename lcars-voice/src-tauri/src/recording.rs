//! Audio recording via cpal (cross-platform) with rubato resampling.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

pub enum CaptureMode {
    MicOnly,
    MicAndMonitor,
}

/// Number of raw mono samples kept for waveform visualization.
const WAVEFORM_BUFFER_SIZE: usize = 2048;
/// Number of downsampled points returned to the frontend.
const WAVEFORM_DISPLAY_POINTS: usize = 128;

pub struct Recorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_active: Arc<AtomicBool>,
    rms_level: Arc<AtomicU32>,
    device_sample_rate: u32,
    device_channels: u16,
    start_time: Option<Instant>,

    /// Ring buffer of recent mono samples for real-time waveform visualization.
    waveform_buffer: Arc<Mutex<VecDeque<f32>>>,

    // Monitor capture via parec subprocess
    monitor_process: Option<std::process::Child>,
    monitor_reader_thread: Option<std::thread::JoinHandle<()>>,
    monitor_buffer: Arc<Mutex<Vec<f32>>>,
    monitor_sample_rate: u32,
    monitor_channels: u16,
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
            waveform_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(WAVEFORM_BUFFER_SIZE))),
            monitor_process: None,
            monitor_reader_thread: None,
            monitor_buffer: Arc::new(Mutex::new(Vec::new())),
            monitor_sample_rate: 0,
            monitor_channels: 0,
        }
    }

    /// Start capturing monitor audio via parec subprocess.
    ///
    /// Detects the default sink's monitor source via `pactl` and spawns `parec`
    /// to capture from it. A reader thread converts the raw float32 LE PCM from
    /// stdout into f32 samples in `monitor_buffer`.
    ///
    /// Note: We resolve the monitor source name explicitly rather than using
    /// `@DEFAULT_MONITOR@` because the latter doesn't work on some PipeWire setups.
    fn start_parec_monitor(&mut self) -> Result<(), String> {
        use crate::audio_sources;

        const PAREC_RATE: u32 = 48000;
        const PAREC_CHANNELS: u16 = 1;

        let monitor_source = audio_sources::get_default_monitor_source()?;
        eprintln!(
            "[LCARS] recording: using monitor source: {}",
            monitor_source
        );

        self.monitor_sample_rate = PAREC_RATE;
        self.monitor_channels = PAREC_CHANNELS;

        let mut child = std::process::Command::new("parec")
            .args([
                "--format=float32le",
                "--channels=1",
                "--rate=48000",
                "-d",
                &monitor_source,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                format!(
                    "Failed to start parec: {}. Is pulseaudio-utils installed?",
                    e
                )
            })?;

        eprintln!(
            "[LCARS] recording: parec started (pid={}), capturing @DEFAULT_MONITOR@ at {}Hz mono",
            child.id(),
            PAREC_RATE,
        );

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture parec stdout".to_string())?;

        let monitor_buffer = Arc::clone(&self.monitor_buffer);
        let monitor_is_active = Arc::clone(&self.is_active);

        let reader_thread = std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = std::io::BufReader::with_capacity(16384, stdout);
            let mut byte_buf = vec![0u8; 4096 * 4]; // 4096 floats per read
            let mut total_bytes: usize = 0;
            let mut total_samples: usize = 0;
            let mut discarded_samples: usize = 0;

            loop {
                let bytes_read = match reader.read(&mut byte_buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("[LCARS] recording: parec read error: {}", e);
                        break;
                    }
                };

                total_bytes += bytes_read;
                let samples = bytes_to_f32_samples(&byte_buf[..bytes_read]);

                // When inactive, drain pipe to prevent parec blocking but discard samples
                if !monitor_is_active.load(Ordering::SeqCst) {
                    discarded_samples += samples.len();
                    continue;
                }

                if !samples.is_empty() {
                    total_samples += samples.len();
                    if let Ok(mut buf) = monitor_buffer.lock() {
                        buf.extend_from_slice(&samples);
                    }
                }
            }
            eprintln!(
                "[LCARS] recording: parec reader exiting: total_bytes={}, total_samples={}, discarded={}",
                total_bytes, total_samples, discarded_samples
            );
        });

        self.monitor_process = Some(child);
        self.monitor_reader_thread = Some(reader_thread);

        Ok(())
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

        let buffer = Arc::clone(&self.buffer);
        let is_active = Arc::clone(&self.is_active);
        let rms_level = Arc::clone(&self.rms_level);
        let waveform_buf = Arc::clone(&self.waveform_buffer);
        let channels = self.device_channels;
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

                    // Feed mono samples into the waveform visualization buffer
                    if let Ok(mut wf) = waveform_buf.try_lock() {
                        let ch = channels as usize;
                        for frame in data.chunks(ch) {
                            let mono = frame.iter().sum::<f32>() / ch as f32;
                            if wf.len() >= WAVEFORM_BUFFER_SIZE {
                                wf.pop_front();
                            }
                            wf.push_back(mono);
                        }
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

        // Start monitor capture via parec if in dual-stream mode
        if let CaptureMode::MicAndMonitor = mode {
            if let Err(e) = self.start_parec_monitor() {
                eprintln!(
                    "[LCARS] recording: monitor capture failed ({}), continuing mic-only",
                    e
                );
            }
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

        // Calculate duration
        let duration_ms = self
            .start_time
            .map(|start| start.elapsed().as_millis() as i64)
            .unwrap_or(0);

        // Drop the streams to stop the audio devices
        self.stream = None;

        // Kill the parec subprocess and join the reader thread
        if let Some(mut child) = self.monitor_process.take() {
            let _ = child.kill();
            let _ = child.wait();
            eprintln!("[LCARS] recording: parec process terminated");
        }
        if let Some(thread) = self.monitor_reader_thread.take() {
            let _ = thread.join();
            eprintln!("[LCARS] recording: parec reader thread joined");
        }
        self.start_time = None;

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
            // Diagnostic stats for monitor buffer
            let mon_min = monitor_samples
                .iter()
                .cloned()
                .fold(f32::INFINITY, f32::min);
            let mon_max = monitor_samples
                .iter()
                .cloned()
                .fold(f32::NEG_INFINITY, f32::max);
            let mon_rms = (monitor_samples.iter().map(|s| s * s).sum::<f32>()
                / monitor_samples.len() as f32)
                .sqrt();
            eprintln!(
                "[LCARS] recording: monitor raw: count={}, min={:.6}, max={:.6}, rms={:.6}",
                monitor_samples.len(),
                mon_min,
                mon_max,
                mon_rms
            );
            // Mic stats too
            let mic_rms =
                (mic_audio.iter().map(|s| s * s).sum::<f32>() / mic_audio.len() as f32).sqrt();
            eprintln!(
                "[LCARS] recording: mic resampled: count={}, rms={:.6}",
                mic_audio.len(),
                mic_rms
            );

            let monitor_mono = downmix_to_mono(&monitor_samples, self.monitor_channels);
            let monitor_audio = resample_to_16khz(&monitor_mono, self.monitor_sample_rate)?;

            let mon_res_rms = (monitor_audio.iter().map(|s| s * s).sum::<f32>()
                / monitor_audio.len() as f32)
                .sqrt();
            eprintln!(
                "[LCARS] recording: monitor resampled: count={}, rms={:.6}",
                monitor_audio.len(),
                mon_res_rms
            );

            mix_streams(&mic_audio, &monitor_audio)
        } else {
            eprintln!("[LCARS] recording: NO monitor samples captured!");
            mic_audio
        };

        eprintln!(
            "[LCARS] recording: processed to {} mono 16KHz samples",
            audio_data.len()
        );

        // Reset RMS and waveform buffer
        self.rms_level.store(0f32.to_bits(), Ordering::SeqCst);
        if let Ok(mut wf) = self.waveform_buffer.lock() {
            wf.clear();
        }

        Ok(RecordingResult {
            audio_data,
            duration_ms,
        })
    }

    pub fn current_rms_level(&self) -> f32 {
        f32::from_bits(self.rms_level.load(Ordering::SeqCst))
    }

    /// Return downsampled waveform data for visualization.
    ///
    /// Takes the last `WAVEFORM_BUFFER_SIZE` raw mono samples and downsamples
    /// them to `WAVEFORM_DISPLAY_POINTS` values (each in roughly -1.0..1.0).
    pub fn current_waveform_data(&self) -> Vec<f32> {
        let wf = self.waveform_buffer.lock().unwrap_or_else(|e| e.into_inner());
        let len = wf.len();
        if len == 0 {
            return vec![0.0; WAVEFORM_DISPLAY_POINTS];
        }
        if len <= WAVEFORM_DISPLAY_POINTS {
            return wf.iter().copied().collect();
        }
        // Downsample by averaging windows
        let window = len / WAVEFORM_DISPLAY_POINTS;
        (0..WAVEFORM_DISPLAY_POINTS)
            .map(|i| {
                let start = i * window;
                let end = (start + window).min(len);
                let sum: f32 = (start..end).map(|j| wf[j]).sum();
                sum / (end - start) as f32
            })
            .collect()
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.start_time
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }
}

/// Convert raw little-endian bytes to f32 samples.
/// Incomplete trailing bytes (less than 4) are discarded.
pub fn bytes_to_f32_samples(bytes: &[u8]) -> Vec<f32> {
    let complete_samples = bytes.len() / 4;
    (0..complete_samples)
        .map(|i| {
            let offset = i * 4;
            f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ])
        })
        .collect()
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

/// Mix two audio streams by sample-wise summing with clamping.
pub fn mix_streams(mic: &[f32], monitor: &[f32]) -> Vec<f32> {
    let len = mic.len().max(monitor.len());
    (0..len)
        .map(|i| match (mic.get(i).copied(), monitor.get(i).copied()) {
            (Some(a), Some(b)) => (a + b).clamp(-1.0, 1.0),
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
        assert!((mixed[0] - 0.6).abs() < 1e-6); // 0.4+0.2
        assert!((mixed[1] - 1.0).abs() < 1e-6); // 0.6+0.4
        assert!((mixed[2] - 0.6).abs() < 1e-6); // -0.2+0.8
    }

    #[test]
    fn test_mix_streams_mic_longer() {
        let mic = vec![0.5, 0.5, 0.5];
        let monitor = vec![0.5];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 1.0).abs() < 1e-6); // 0.5+0.5, clamped
        assert!((mixed[1] - 0.5).abs() < 1e-6); // mic only
        assert!((mixed[2] - 0.5).abs() < 1e-6); // mic only
    }

    #[test]
    fn test_mix_streams_monitor_longer() {
        let mic = vec![0.5];
        let monitor = vec![0.5, 0.5, 0.5];
        let mixed = mix_streams(&mic, &monitor);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 1.0).abs() < 1e-6); // 0.5+0.5, clamped
        assert!((mixed[1] - 0.5).abs() < 1e-6); // monitor only
    }

    #[test]
    fn test_mix_streams_clamp() {
        let mic = vec![0.9];
        let monitor = vec![0.9];
        let mixed = mix_streams(&mic, &monitor);
        // 0.9+0.9 = 1.8, clamped to 1.0
        assert!((mixed[0] - 1.0).abs() < 1e-6);

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
        assert!((mixed[0] - 1.0).abs() < 1e-6); // both present, summed: 0.8+0.8 = 1.6, clamped
        assert!((mixed[1] - 0.6).abs() < 1e-6); // mic only
        assert!((mixed[2] - 0.4).abs() < 1e-6); // mic only
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

    #[test]
    fn test_elapsed_seconds_zero_when_not_recording() {
        let recorder = Recorder::new();
        assert!(recorder.elapsed_seconds() < 0.01);
    }

    // --- bytes_to_f32_samples tests ---

    #[test]
    fn test_bytes_to_f32_samples_single() {
        let bytes = 1.0f32.to_le_bytes();
        let samples = bytes_to_f32_samples(&bytes);
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bytes_to_f32_samples_multiple() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0.5f32.to_le_bytes());
        bytes.extend_from_slice(&(-0.25f32).to_le_bytes());
        bytes.extend_from_slice(&0.0f32.to_le_bytes());
        let samples = bytes_to_f32_samples(&bytes);
        assert_eq!(samples.len(), 3);
        assert!((samples[0] - 0.5).abs() < 1e-6);
        assert!((samples[1] - (-0.25)).abs() < 1e-6);
        assert!((samples[2] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_bytes_to_f32_samples_partial() {
        let mut bytes = vec![0u8; 5];
        bytes[..4].copy_from_slice(&0.75f32.to_le_bytes());
        let samples = bytes_to_f32_samples(&bytes);
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_bytes_to_f32_samples_empty() {
        let samples = bytes_to_f32_samples(&[]);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_bytes_to_f32_samples_too_short() {
        let samples = bytes_to_f32_samples(&[0, 1, 2]);
        assert!(samples.is_empty());
    }
}
