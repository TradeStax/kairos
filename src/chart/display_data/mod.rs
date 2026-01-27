//! Display Data Layer
//!
//! Separates domain data (trades, candles, depth) from rendering data structures.
//! Provides viewport-aware caching and progressive rendering support.
//!
//! ## Architecture
//!
//! ```text
//! Domain Layer (ChartData)
//!     ↓
//! Display Data Layer ← Viewport + LOD + Basis
//!     ↓
//! Rendering Layer (Canvas/Frame)
//! ```
//!
//! ## Benefits
//! - **Performance**: Pre-computed structures, no runtime aggregation
//! - **Cache Efficiency**: Invalidated only when viewport/data changes
//! - **Progressive Rendering**: Can render in phases (coarse → fine)
//! - **Clean Separation**: Domain logic separate from rendering

pub mod footprint;
pub mod heatmap;

use super::lod::LodLevel;
use super::viewport::ViewportBounds;
use data::ChartBasis;

/// Cache key for display data invalidation
///
/// Display data is only rebuilt when this key changes
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisplayCacheKey {
    /// Chart basis (time or tick)
    pub basis: ChartBasis,
    /// Viewport time range
    pub time_range: (u64, u64),
    /// Viewport price range (in units)
    pub price_range: (i64, i64),
    /// LOD level
    pub lod_level: LodLevel,
    /// Scaling factor (rounded to avoid float comparison issues)
    pub scaling_key: u32, // scaling * 1000 as u32
    /// Cell dimensions key
    pub cell_key: (u32, u32), // (cell_width * 100, cell_height * 100)
}

impl DisplayCacheKey {
    /// Create cache key from viewport state
    pub fn from_viewport(
        basis: ChartBasis,
        bounds: &ViewportBounds,
        lod_level: LodLevel,
        scaling: f32,
        cell_width: f32,
        cell_height: f32,
    ) -> Self {
        Self {
            basis,
            time_range: (bounds.time_start, bounds.time_end),
            price_range: (bounds.price_low, bounds.price_high),
            lod_level,
            scaling_key: (scaling * 1000.0) as u32,
            cell_key: ((cell_width * 100.0) as u32, (cell_height * 100.0) as u32),
        }
    }

    /// Check if key is similar enough to reuse cache (fuzzy matching)
    ///
    /// Allows small viewport movements without cache invalidation
    pub fn is_similar_to(&self, other: &Self, tolerance_percent: f32) -> bool {
        if self.basis != other.basis || self.lod_level != other.lod_level {
            return false;
        }

        // Calculate time range overlap
        let time_overlap = {
            let start = self.time_range.0.max(other.time_range.0);
            let end = self.time_range.1.min(other.time_range.1);
            if end > start {
                (end - start) as f32
            } else {
                0.0
            }
        };

        let self_time_span = (self.time_range.1 - self.time_range.0) as f32;
        let time_overlap_ratio = if self_time_span > 0.0 {
            time_overlap / self_time_span
        } else {
            0.0
        };

        // If >80% overlap, consider similar
        time_overlap_ratio >= (1.0 - tolerance_percent)
    }
}

/// Display data trait
///
/// Common interface for all display data structures
pub trait DisplayData: Sized {
    /// Type of source data (ChartData, etc.)
    type SourceData;

    /// Build display data from source data and viewport
    fn build(
        source: &Self::SourceData,
        bounds: &ViewportBounds,
        lod_level: LodLevel,
        extra_params: &Self::ExtraParams,
    ) -> Self;

    /// Extra parameters needed for building (tick size, etc.)
    type ExtraParams;

    /// Estimate memory usage in bytes
    fn memory_usage(&self) -> usize;

    /// Check if display data is empty
    fn is_empty(&self) -> bool;
}

/// Cached display data wrapper
///
/// Automatically invalidates when viewport changes significantly
pub struct DisplayDataCache<T: DisplayData> {
    /// Cached data
    data: Option<T>,
    /// Cache key for current data
    key: Option<DisplayCacheKey>,
    /// Number of cache hits
    hits: u64,
    /// Number of cache misses (rebuilds)
    misses: u64,
}

impl<T: DisplayData> Default for DisplayDataCache<T> {
    fn default() -> Self {
        Self {
            data: None,
            key: None,
            hits: 0,
            misses: 0,
        }
    }
}

impl<T: DisplayData> DisplayDataCache<T> {
    /// Create new empty cache
    pub fn new() -> Self {
        Self::default()
    }

    /// Get cached data or rebuild if invalidated
    ///
    /// Returns cached data if key matches (cache hit), otherwise rebuilds (cache miss)
    pub fn get_or_build(
        &mut self,
        new_key: DisplayCacheKey,
        source: &T::SourceData,
        bounds: &ViewportBounds,
        lod_level: LodLevel,
        params: &T::ExtraParams,
    ) -> &T {
        // Check if cache is valid (exact match or similar enough)
        let cache_valid = self
            .key
            .as_ref()
            .map(|k| k == &new_key || k.is_similar_to(&new_key, 0.2))
            .unwrap_or(false);

        if cache_valid {
            // Cache hit
            self.hits += 1;
            self.data.as_ref().unwrap()
        } else {
            // Cache miss - rebuild
            self.misses += 1;
            let new_data = T::build(source, bounds, lod_level, params);
            self.data = Some(new_data);
            self.key = Some(new_key);
            self.data.as_ref().unwrap()
        }
    }

    /// Force invalidate cache
    pub fn invalidate(&mut self) {
        self.data = None;
        self.key = None;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f32 / (self.hits + self.misses) as f32
            } else {
                0.0
            },
            memory_bytes: self.data.as_ref().map(|d| d.memory_usage()).unwrap_or(0),
        }
    }

    /// Check if cache contains data
    pub fn is_cached(&self) -> bool {
        self.data.is_some()
    }
}

/// Cache performance statistics
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f32,
    pub memory_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_similarity() {
        let key1 = DisplayCacheKey {
            basis: ChartBasis::Time(data::Timeframe::M5),
            time_range: (1000, 2000),
            price_range: (100, 200),
            lod_level: LodLevel::High,
            scaling_key: 1000,
            cell_key: (400, 400),
        };

        let key2 = DisplayCacheKey {
            basis: ChartBasis::Time(data::Timeframe::M5),
            time_range: (1100, 2100), // Shifted by 100ms
            price_range: (100, 200),
            lod_level: LodLevel::High,
            scaling_key: 1000,
            cell_key: (400, 400),
        };

        // Should be similar (90% overlap)
        assert!(key1.is_similar_to(&key2, 0.2));
    }

    #[test]
    fn test_cache_key_not_similar() {
        let key1 = DisplayCacheKey {
            basis: ChartBasis::Time(data::Timeframe::M5),
            time_range: (1000, 2000),
            price_range: (100, 200),
            lod_level: LodLevel::High,
            scaling_key: 1000,
            cell_key: (400, 400),
        };

        let key2 = DisplayCacheKey {
            basis: ChartBasis::Time(data::Timeframe::M5),
            time_range: (1000, 2000),
            price_range: (100, 200),
            lod_level: LodLevel::Low, // Different LOD
            scaling_key: 1000,
            cell_key: (400, 400),
        };

        // Should NOT be similar (different LOD level)
        assert!(!key1.is_similar_to(&key2, 0.2));
    }
}
