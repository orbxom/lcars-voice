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
        println!("[LCARS] recording: start() called");
        if self.process.is_some() {
            println!("[LCARS] recording: Already recording, returning error");
            return Err("Already recording".to_string());
        }

        // Remove old recording if exists
        let _ = std::fs::remove_file(&self.output_path);
        println!("[LCARS] recording: Output path = {:?}", self.output_path);

        println!("[LCARS] recording: Spawning arecord command");
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
            .map_err(|e| {
                println!("[LCARS] recording: Failed to spawn arecord: {}", e);
                format!("Failed to start arecord: {}", e)
            })?;

        println!("[LCARS] recording: arecord process spawned successfully, pid = {:?}", child.id());
        self.process = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<PathBuf, String> {
        println!("[LCARS] recording: stop() called");
        if let Some(mut child) = self.process.take() {
            println!("[LCARS] recording: Killing arecord process, pid = {:?}", child.id());
            // Send SIGKILL to terminate arecord; wait() ensures process cleanup
            let kill_result = child.kill();
            println!("[LCARS] recording: kill() result = {:?}", kill_result);
            let wait_result = child.wait();
            println!("[LCARS] recording: wait() result = {:?}", wait_result);

            if self.output_path.exists() {
                println!("[LCARS] recording: Output file exists at {:?}", self.output_path);
                Ok(self.output_path.clone())
            } else {
                println!("[LCARS] recording: Output file NOT found at {:?}", self.output_path);
                Err("Recording file not found".to_string())
            }
        } else {
            println!("[LCARS] recording: Not recording, no process to stop");
            Err("Not recording".to_string())
        }
    }
}
