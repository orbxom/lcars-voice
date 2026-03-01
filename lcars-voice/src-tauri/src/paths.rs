//! Shared application directory paths.

use std::path::PathBuf;

/// Returns the application data root: `~/.local/share/lcars-voice/`
pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lcars-voice")
}
