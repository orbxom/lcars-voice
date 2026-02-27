use chrono::{DateTime, Utc};
use std::io::Cursor;

pub struct MeetingSession {
    pub start_time: DateTime<Utc>,
}

impl MeetingSession {
    pub fn new() -> Self {
        let now = Utc::now();
        eprintln!("[LCARS] Meeting session created at {}", now);
        Self { start_time: now }
    }

    /// Generate a filename like "meeting-2025-02-27-143045.wav"
    pub fn filename(&self) -> String {
        format!("meeting-{}.wav", self.start_time.format("%Y-%m-%d-%H%M%S"))
    }

    /// Encode f32 audio samples to WAV bytes in memory.
    pub fn encode_wav(&self, samples: &[f32]) -> Result<Vec<u8>, String> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .map_err(|e| format!("WAV writer error: {}", e))?;
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
        }
        Ok(cursor.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meeting_session_filename_format() {
        let session = MeetingSession::new();
        let filename = session.filename();
        assert!(filename.starts_with("meeting-"));
        assert!(filename.ends_with(".wav"));
        // meeting-YYYY-MM-DD-HHMMSS.wav = 29 chars
        assert_eq!(filename.len(), 29);
    }

    #[test]
    fn test_encode_wav_silence() {
        let session = MeetingSession::new();
        let samples = vec![0.0f32; 16000]; // 1 second of silence
        let wav_bytes = session.encode_wav(&samples).unwrap();

        // Verify RIFF header
        assert_eq!(&wav_bytes[0..4], b"RIFF");

        // Parse back with hound
        let cursor = Cursor::new(wav_bytes);
        let reader = hound::WavReader::new(cursor).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(reader.len(), 16000);
    }

    #[test]
    fn test_encode_wav_sine_wave() {
        let session = MeetingSession::new();
        let samples: Vec<f32> = (0..16000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin() * 0.5)
            .collect();
        let wav_bytes = session.encode_wav(&samples).unwrap();

        let cursor = Cursor::new(wav_bytes);
        let reader = hound::WavReader::new(cursor).unwrap();
        assert_eq!(reader.len(), 16000);
    }

    #[test]
    fn test_encode_wav_empty() {
        let session = MeetingSession::new();
        let samples: Vec<f32> = vec![];
        let wav_bytes = session.encode_wav(&samples).unwrap();

        // Should still be valid WAV
        let cursor = Cursor::new(wav_bytes);
        let reader = hound::WavReader::new(cursor).unwrap();
        assert_eq!(reader.len(), 0);
    }
}
