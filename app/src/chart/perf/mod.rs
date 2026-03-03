pub mod lod;

#[cfg(feature = "heatmap")]
pub use lod::LodLevel;
pub use lod::{LodCalculator, LodIteratorExt};
