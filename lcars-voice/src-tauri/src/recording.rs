//! Audio recording via Linux arecord command.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct Recorder {
    process: Option<Child>,
    output_path: PathBuf,
}

impl Recorder {
    pub fn new() -> Self {
        let output_path = std::env::temp_dir().join("lcars-voice-recording.wav");
        Self {
            process: None,
            output_path,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.process.is_some() {
            return Err("Already recording".to_string());
        }

        // Remove old recording if exists
        let _ = std::fs::remove_file(&self.output_path);

        let child = Command::new("arecord")
            .args([
                "-f",
                "S16_LE",
                "-r",
                "16000",
                "-c",
                "1",
                self.output_path
                    .to_str()
                    .ok_or_else(|| "Output path contains invalid UTF-8".to_string())?,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                eprintln!("[LCARS] recording: Failed to spawn arecord: {}", e);
                format!("Failed to start arecord: {}", e)
            })?;

        eprintln!("[LCARS] recording: arecord started, pid = {:?}", child.id());
        self.process = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<PathBuf, String> {
        if let Some(mut child) = self.process.take() {
            eprintln!(
                "[LCARS] recording: Stopping arecord, pid = {:?}",
                child.id()
            );
            // Send SIGTERM for graceful shutdown (lets arecord flush WAV headers)
            unsafe {
                libc::kill(child.id() as i32, libc::SIGTERM);
            }

            // Wait with timeout
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if start.elapsed() > std::time::Duration::from_secs(5) {
                            eprintln!("[LCARS] recording: SIGTERM timeout, force killing");
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(e) => {
                        eprintln!("[LCARS] recording: Error waiting for process: {}", e);
                        break;
                    }
                }
            }

            if self.output_path.exists() {
                Ok(self.output_path.clone())
            } else {
                Err("Recording file not found".to_string())
            }
        } else {
            Err("Not recording".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_initial_state() {
        let recorder = Recorder::new();
        assert!(recorder.process.is_none());
    }

    #[test]
    fn test_stop_without_start() {
        let mut recorder = Recorder::new();
        let result = recorder.stop();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Not recording");
    }
}
