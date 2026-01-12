//! Path utilities for data directory management

use std::path::PathBuf;

/// Get the data directory for the application
///
/// Returns the platform-specific data directory:
/// - Linux: `~/.local/share/flowsurface/`
/// - macOS: `~/Library/Application Support/flowsurface/`
/// - Windows: `%APPDATA%\flowsurface\`
///
/// Can be overridden with `FLOWSURFACE_DATA_PATH` environment variable.
pub fn get_data_directory() -> Option<PathBuf> {
    // Check for environment variable override
    if let Ok(custom_path) = std::env::var("FLOWSURFACE_DATA_PATH") {
        return Some(PathBuf::from(custom_path));
    }

    // Get platform-specific data directory
    #[cfg(target_os = "linux")]
    {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".local/share/flowsurface"))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join("Library/Application Support/flowsurface"))
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join("flowsurface"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_data_directory() {
        let dir = get_data_directory();
        assert!(dir.is_some(), "Data directory should be available");

        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains("flowsurface"));
    }

    #[test]
    fn test_custom_data_path() {
        unsafe {
            std::env::set_var("FLOWSURFACE_DATA_PATH", "/tmp/test_flowsurface");
        }
        let dir = get_data_directory();
        assert_eq!(dir.unwrap(), PathBuf::from("/tmp/test_flowsurface"));
        unsafe {
            std::env::remove_var("FLOWSURFACE_DATA_PATH");
        }
    }
}
