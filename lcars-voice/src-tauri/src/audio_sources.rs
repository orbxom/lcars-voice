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

/// Known webcam identifier patterns to deprioritize as mic sources.
const WEBCAM_PATTERNS: &[&str] = &["brio", "c920", "c922", "c930", "webcam", "046d_"];

/// Check if a device name matches known webcam patterns (case-insensitive).
pub fn is_webcam(name: &str) -> bool {
    let lower = name.to_lowercase();
    WEBCAM_PATTERNS.iter().any(|pat| lower.contains(pat))
}

/// Classify source names into (mics, monitors).
///
/// A source is a monitor if its name contains ".monitor".
pub fn classify_sources(device_names: &[String]) -> (Vec<String>, Vec<String>) {
    let mut mics = Vec::new();
    let mut monitors = Vec::new();
    for name in device_names {
        if name.contains(".monitor") {
            monitors.push(name.clone());
        } else {
            mics.push(name.clone());
        }
    }
    (mics, monitors)
}

/// Pick the best microphone from candidates.
///
/// Prefers non-webcam sources; falls back to webcam if that's all there is.
pub fn pick_best_mic(candidates: &[String]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    // Prefer non-webcam sources
    for c in candidates {
        if !is_webcam(c) {
            return Some(c.clone());
        }
    }
    // Fall back to first candidate (webcam)
    Some(candidates[0].clone())
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

/// Find the first input device with `.monitor` in its name.
pub fn find_monitor_device() -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {}", e))?;
    for dev in devices {
        if let Ok(name) = dev.name() {
            if name.contains(".monitor") {
                return Ok(dev);
            }
        }
    }
    Err("No monitor source found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_webcam_brio() {
        assert!(is_webcam("Logitech Brio"));
    }

    #[test]
    fn test_is_webcam_c920() {
        assert!(is_webcam("HD Pro Webcam C920"));
    }

    #[test]
    fn test_is_webcam_regular_mic() {
        assert!(!is_webcam("Blue Yeti"));
    }

    #[test]
    fn test_is_webcam_case_insensitive() {
        assert!(is_webcam("BRIO 4K"));
    }

    #[test]
    fn test_is_webcam_046d_pattern() {
        assert!(is_webcam("046d_0825 Analog Stereo"));
    }

    #[test]
    fn test_classify_sources_separates_monitors() {
        let names = vec![
            "alsa_input.pci-0000.analog-stereo".to_string(),
            "alsa_output.pci-0000.analog-stereo.monitor".to_string(),
        ];
        let (mics, monitors) = classify_sources(&names);
        assert_eq!(mics.len(), 1);
        assert_eq!(monitors.len(), 1);
        assert!(monitors[0].contains(".monitor"));
    }

    #[test]
    fn test_classify_sources_regular_mics() {
        let names = vec!["Blue_Yeti".to_string(), "Built-in_Mic".to_string()];
        let (mics, monitors) = classify_sources(&names);
        assert_eq!(mics.len(), 2);
        assert_eq!(monitors.len(), 0);
    }

    #[test]
    fn test_pick_best_mic_prefers_non_webcam() {
        let candidates = vec!["HD Pro Webcam C920".to_string(), "Blue Yeti".to_string()];
        let best = pick_best_mic(&candidates);
        assert_eq!(best, Some("Blue Yeti".to_string()));
    }

    #[test]
    fn test_pick_best_mic_falls_back_to_webcam() {
        let candidates = vec!["HD Pro Webcam C920".to_string()];
        let best = pick_best_mic(&candidates);
        assert_eq!(best, Some("HD Pro Webcam C920".to_string()));
    }

    #[test]
    fn test_pick_best_mic_empty() {
        let candidates: Vec<String> = vec![];
        let best = pick_best_mic(&candidates);
        assert_eq!(best, None);
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
