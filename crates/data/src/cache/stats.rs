//! Cache statistics — aggregate file count and size reporting.

/// Aggregate statistics for the on-disk cache.
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct CacheStats {
    /// Total number of cached files
    pub total_files: usize,
    /// Total size of all cached files in bytes
    pub total_size_bytes: u64,
    /// Modification time of the oldest cached file, if any
    pub oldest_file: Option<chrono::DateTime<chrono::Utc>>,
}

impl CacheStats {
    /// Returns the total cache size as a human-readable string (e.g. "1.23 GB")
    #[must_use]
    pub fn size_human_readable(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size_bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}
