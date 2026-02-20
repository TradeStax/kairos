//! ScriptLoader: discovers .js files from script directories.

use std::path::{Path, PathBuf};

/// Discovers indicator script files from bundled and user directories.
pub struct ScriptLoader {
    /// Bundled indicators directory (e.g., assets/indicators/)
    bundled_dir: Option<PathBuf>,
    /// User scripts directory (e.g., ~/.kairos/scripts/)
    user_dir: PathBuf,
}

impl Default for ScriptLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptLoader {
    /// Create a new loader with the default user scripts directory.
    pub fn new() -> Self {
        Self {
            bundled_dir: Self::find_bundled_dir(),
            user_dir: Self::default_user_dir(),
        }
    }

    /// Create a loader with custom directories.
    pub fn with_dirs(bundled_dir: Option<PathBuf>, user_dir: PathBuf) -> Self {
        Self {
            bundled_dir,
            user_dir,
        }
    }

    /// Default user scripts directory.
    pub fn default_user_dir() -> PathBuf {
        crate::path::data_path(Some("indicators"))
    }

    /// Find the bundled scripts directory relative to the executable.
    fn find_bundled_dir() -> Option<PathBuf> {
        // Try relative to executable
        if let Ok(exe) = std::env::current_exe() {
            let exe_dir = exe.parent()?;
            let bundled = exe_dir.join("assets").join("indicators");
            if bundled.is_dir() {
                return Some(bundled);
            }
        }

        // Try relative to current directory (dev mode)
        let cwd_bundled = PathBuf::from("assets/scripts");
        if cwd_bundled.is_dir() {
            return Some(cwd_bundled);
        }

        None
    }

    /// Discover all .js script files from both bundled and user directories.
    pub fn discover(&self) -> Vec<PathBuf> {
        let mut scripts = Vec::new();

        // Load bundled scripts first
        if let Some(ref bundled_dir) = self.bundled_dir {
            scripts.extend(Self::scan_directory(bundled_dir));
        }

        // Load user scripts (can override bundled by same filename)
        if self.user_dir.is_dir() {
            scripts.extend(Self::scan_directory(&self.user_dir));
        }

        // Deduplicate by filename stem (user scripts override bundled)
        let mut seen = std::collections::HashMap::new();
        let mut deduped = Vec::new();

        for path in scripts {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if let Some(existing_idx) = seen.get(&stem) {
                // User script overrides bundled
                deduped[*existing_idx] = path;
            } else {
                seen.insert(stem, deduped.len());
                deduped.push(path);
            }
        }

        deduped
    }

    /// Scan a single directory for .js files (non-recursive).
    fn scan_directory(dir: &Path) -> Vec<PathBuf> {
        let mut scripts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("js") {
                    scripts.push(path);
                }
            }
        }
        scripts.sort();
        scripts
    }

    /// Get the user scripts directory path.
    pub fn user_dir(&self) -> &Path {
        &self.user_dir
    }

    /// Get the bundled scripts directory path, if found.
    pub fn bundled_dir(&self) -> Option<&Path> {
        self.bundled_dir.as_deref()
    }

    /// Ensure the user scripts directory exists.
    pub fn ensure_user_dir(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.user_dir)
    }
}
