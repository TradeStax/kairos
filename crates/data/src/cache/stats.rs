//! Cache statistics

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub oldest_file: Option<chrono::DateTime<chrono::Utc>>,
}

impl CacheStats {
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
