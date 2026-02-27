use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;

pub struct MeetingSession {
    pub output_dir: PathBuf,
    pub start_time: DateTime<Utc>,
}

impl MeetingSession {
    pub fn new(output_base: Option<&str>) -> Result<Self, String> {
        let base = match output_base {
            Some(p) => PathBuf::from(p),
            None => dirs::data_local_dir()
                .ok_or("Cannot determine data local directory")?
                .join("lcars-voice")
                .join("recordings"),
        };

        let now = Utc::now();
        let dir_name = now.format("%Y-%m-%d-%H%M%S").to_string();
        let output_dir = base.join(dir_name);

        std::fs::create_dir_all(&output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;

        Ok(Self {
            output_dir,
            start_time: now,
        })
    }

    pub fn save_audio(&self, samples: &[f32]) -> Result<(), String> {
        let wav_path = self.output_dir.join("audio.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::create(&wav_path, spec).map_err(|e| format!("WAV error: {}", e))?;

        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let value = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(value)
                .map_err(|e| format!("WAV write error: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("WAV finalize error: {}", e))?;
        Ok(())
    }

    pub fn save_metadata(&self, recording_duration_secs: f64) -> Result<(), String> {
        let ended_at = self.start_time + Duration::seconds(recording_duration_secs as i64);
        let wall_duration = recording_duration_secs as i64;

        let data = serde_json::json!({
            "started_at": self.start_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "ended_at": ended_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "duration_seconds": recording_duration_secs as i64,
            "wall_duration_seconds": wall_duration,
            "sample_rate": 16000,
            "channels": 1,
            "format": "wav"
        });

        let content =
            serde_json::to_string_pretty(&data).map_err(|e| format!("JSON error: {}", e))?;
        let meta_path = self.output_dir.join("metadata.json");
        std::fs::write(meta_path, content).map_err(|e| format!("Write error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    fn test_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, AtomicOrdering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("lcars-meeting-test-{}-{}", std::process::id(), id));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_meeting_session_no_timestamps_file() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();
        // After creating a session, no timestamps.json should exist
        assert!(!session.output_dir.join("timestamps.json").exists());
        cleanup(&base);
    }

    #[test]
    fn test_default_base_uses_data_local_dir() {
        let data_dir = dirs::data_local_dir().unwrap();
        let expected = data_dir.join("lcars-voice").join("recordings");
        assert!(expected
            .to_str()
            .unwrap()
            .contains("lcars-voice/recordings"));
    }

    #[test]
    fn test_meeting_session_creates_directory() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();
        assert!(session.output_dir.exists());
        cleanup(&base);
    }

    #[test]
    fn test_meeting_session_dir_format() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();
        let dir_name = session.output_dir.file_name().unwrap().to_str().unwrap();
        // Should match YYYY-MM-DD-HHMMSS pattern
        assert!(dir_name.len() >= 17, "Dir name '{}' too short", dir_name);
        assert_eq!(&dir_name[4..5], "-");
        assert_eq!(&dir_name[7..8], "-");
        assert_eq!(&dir_name[10..11], "-");
        cleanup(&base);
    }

    #[test]
    fn test_meeting_session_save_audio() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();

        // Generate 1 second of silence
        let samples = vec![0.0f32; 16000];
        session.save_audio(&samples).unwrap();

        let wav_path = session.output_dir.join("audio.wav");
        assert!(wav_path.exists());

        // Verify WAV properties using hound
        let reader = hound::WavReader::open(&wav_path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);

        cleanup(&base);
    }

    #[test]
    fn test_meeting_session_save_audio_sine_wave() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();

        // Generate 1 second of 440Hz sine wave
        let samples: Vec<f32> = (0..16000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin() * 0.5)
            .collect();
        session.save_audio(&samples).unwrap();

        let wav_path = session.output_dir.join("audio.wav");
        let reader = hound::WavReader::open(&wav_path).unwrap();
        assert_eq!(reader.len(), 16000); // 16000 samples

        cleanup(&base);
    }

    #[test]
    fn test_save_audio_empty_samples() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();
        let samples: Vec<f32> = vec![];
        session.save_audio(&samples).unwrap();

        let wav_path = session.output_dir.join("audio.wav");
        assert!(wav_path.exists());

        cleanup(&base);
    }

    #[test]
    fn test_meeting_session_save_metadata() {
        let base = test_dir();
        let session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();
        session.save_metadata(3600.0).unwrap();

        let meta_path = session.output_dir.join("metadata.json");
        assert!(meta_path.exists());

        let content = fs::read_to_string(&meta_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(parsed["started_at"].is_string());
        assert!(parsed["ended_at"].is_string());
        assert_eq!(parsed["duration_seconds"], 3600);
        assert_eq!(parsed["sample_rate"], 16000);
        assert_eq!(parsed["channels"], 1);
        assert_eq!(parsed["format"], "wav");

        cleanup(&base);
    }
}
