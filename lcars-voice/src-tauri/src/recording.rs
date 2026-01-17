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
                "-f", "S16_LE",
                "-r", "16000",
                "-c", "1",
                self.output_path.to_str()
                    .ok_or_else(|| "Output path contains invalid UTF-8".to_string())?,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start arecord: {}", e))?;

        self.process = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<PathBuf, String> {
        if let Some(mut child) = self.process.take() {
            // Send SIGKILL to terminate arecord; wait() ensures process cleanup
            let _ = child.kill();
            let _ = child.wait();

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
