use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimestampMark {
    pub time: String,
    pub seconds: u64,
    pub ticket: Option<String>,
    pub note: Option<String>,
}

pub struct TimestampManager {
    marks: Vec<TimestampMark>,
}

pub struct MeetingSession {
    pub output_dir: PathBuf,
    pub start_time: DateTime<Utc>,
    pub timestamps: TimestampManager,
}

fn format_elapsed(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

impl TimestampManager {
    pub fn new() -> Self {
        Self { marks: Vec::new() }
    }

    pub fn add_mark(
        &mut self,
        elapsed_seconds: u64,
        ticket: Option<String>,
        note: Option<String>,
    ) -> TimestampMark {
        let mark = TimestampMark {
            time: format_elapsed(elapsed_seconds),
            seconds: elapsed_seconds,
            ticket,
            note,
        };
        self.marks.push(mark.clone());
        mark
    }

    pub fn get_marks(&self) -> &[TimestampMark] {
        &self.marks
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let data = serde_json::json!({ "marks": self.marks });
        let content =
            serde_json::to_string_pretty(&data).map_err(|e| format!("JSON error: {}", e))?;
        std::fs::write(path, content).map_err(|e| format!("Write error: {}", e))
    }
}

impl MeetingSession {
    pub fn new(output_base: Option<&str>) -> Result<Self, String> {
        let base = match output_base {
            Some(p) => PathBuf::from(p),
            None => dirs::home_dir()
                .ok_or("Cannot determine home directory")?
                .join("zoom-recordings"),
        };

        let now = Utc::now();
        let dir_name = now.format("%Y-%m-%d-%H%M%S").to_string();
        let output_dir = base.join(dir_name);

        std::fs::create_dir_all(&output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;

        Ok(Self {
            output_dir,
            start_time: now,
            timestamps: TimestampManager::new(),
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

    pub fn save_timestamps(&self) -> Result<(), String> {
        let ts_path = self.output_dir.join("timestamps.json");
        self.timestamps.save(&ts_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    fn test_format_elapsed_zero() {
        assert_eq!(format_elapsed(0), "00:00:00");
    }

    #[test]
    fn test_format_elapsed_minutes() {
        assert_eq!(format_elapsed(323), "00:05:23");
    }

    #[test]
    fn test_format_elapsed_hours() {
        assert_eq!(format_elapsed(3661), "01:01:01");
    }

    #[test]
    fn test_timestamp_mark_with_ticket() {
        let mut mgr = TimestampManager::new();
        let mark = mgr.add_mark(323, Some("GT-1234".to_string()), None);
        assert_eq!(mark.time, "00:05:23");
        assert_eq!(mark.seconds, 323);
        assert_eq!(mark.ticket, Some("GT-1234".to_string()));
        assert_eq!(mark.note, None);
    }

    #[test]
    fn test_timestamp_mark_without_ticket() {
        let mut mgr = TimestampManager::new();
        let mark = mgr.add_mark(60, None, Some("important point".to_string()));
        assert_eq!(mark.ticket, None);
        assert_eq!(mark.note, Some("important point".to_string()));
    }

    #[test]
    fn test_timestamp_manager_add_multiple() {
        let mut mgr = TimestampManager::new();
        mgr.add_mark(10, None, None);
        mgr.add_mark(20, None, None);
        mgr.add_mark(30, None, None);
        assert_eq!(mgr.get_marks().len(), 3);
    }

    #[test]
    fn test_timestamp_manager_save_json() {
        let dir = test_dir();
        let path = dir.join("timestamps.json");

        let mut mgr = TimestampManager::new();
        mgr.add_mark(323, Some("GT-1234".to_string()), None);
        mgr.add_mark(600, None, Some("discussion".to_string()));

        mgr.save(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(parsed["marks"].is_array());
        assert_eq!(parsed["marks"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["marks"][0]["time"], "00:05:23");
        assert_eq!(parsed["marks"][0]["seconds"], 323);
        assert_eq!(parsed["marks"][0]["ticket"], "GT-1234");

        cleanup(&dir);
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

    #[test]
    fn test_meeting_session_save_timestamps() {
        let base = test_dir();
        let mut session = MeetingSession::new(Some(base.to_str().unwrap())).unwrap();
        session
            .timestamps
            .add_mark(100, Some("GT-100".to_string()), None);
        session.save_timestamps().unwrap();

        let ts_path = session.output_dir.join("timestamps.json");
        assert!(ts_path.exists());

        let content = fs::read_to_string(&ts_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["marks"][0]["ticket"], "GT-100");

        cleanup(&base);
    }

    #[test]
    fn test_timestamp_mark_serializable() {
        let mark = TimestampMark {
            time: "00:01:00".to_string(),
            seconds: 60,
            ticket: Some("GT-1".to_string()),
            note: None,
        };
        let json = serde_json::to_string(&mark).unwrap();
        let deserialized: TimestampMark = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.time, "00:01:00");
        assert_eq!(deserialized.seconds, 60);
        assert_eq!(deserialized.ticket, Some("GT-1".to_string()));
    }
}
