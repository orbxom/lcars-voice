//! Audio source enumeration and auto-detection for PipeWire/PulseAudio.
//!
//! Replaces zoom-recorder's audio.py with native cpal-based device enumeration.
//! On PipeWire/PulseAudio, monitor sources appear with `.monitor` in the device name.

use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

/// Information about an audio input source.
#[derive(Clone, Debug, Serialize)]
pub struct AudioSourceInfo {
    pub name: String,
    pub is_monitor: bool,
}

/// List all input devices via cpal, returning info about each source.
pub fn enumerate_sources() -> Vec<AudioSourceInfo> {
    let host = cpal::default_host();
    let devices = match host.input_devices() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    devices
        .filter_map(|dev| {
            let name = dev.name().ok()?;
            Some(AudioSourceInfo {
                is_monitor: name.contains(".monitor"),
                name,
            })
        })
        .collect()
}

/// Check if PulseAudio/PipeWire monitor capture is available.
///
/// Returns true if `parec` is available on the system PATH.
pub fn is_monitor_capture_available() -> bool {
    std::process::Command::new("parec")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the monitor source name for the default PulseAudio/PipeWire sink.
///
/// Runs `pactl get-default-sink` and appends `.monitor` to construct the
/// monitor source name. This is more reliable than `@DEFAULT_MONITOR@` which
/// doesn't resolve correctly on some PipeWire setups.
pub fn get_default_monitor_source() -> Result<String, String> {
    let output = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .map_err(|e| format!("Failed to run pactl: {}", e))?;
    if !output.status.success() {
        return Err("pactl get-default-sink failed".to_string());
    }
    let sink_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sink_name.is_empty() {
        return Err("No default sink found".to_string());
    }
    Ok(format!("{}.monitor", sink_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_monitor_capture_available_returns_bool() {
        // System-dependent: just verifies the function doesn't panic
        let result = is_monitor_capture_available();
        assert!(result == true || result == false);
    }

    #[test]
    fn test_get_default_monitor_source_format() {
        // System-dependent: if pactl is available, result should end with .monitor
        if let Ok(source) = get_default_monitor_source() {
            assert!(
                source.ends_with(".monitor"),
                "Expected source name ending with .monitor, got: {}",
                source
            );
        }
    }

    #[test]
    fn test_audio_source_info_serializable() {
        let source = AudioSourceInfo {
            name: "test".to_string(),
            is_monitor: false,
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"is_monitor\":false"));
    }
}
