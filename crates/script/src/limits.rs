//! Resource limits for the QuickJS script runtime.

/// Memory limit per runtime instance (16MB).
pub const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
/// Maximum stack size (512KB).
pub const MAX_STACK_SIZE: usize = 512 * 1024;
/// Garbage collection threshold (2MB).
pub const GC_THRESHOLD: usize = 2 * 1024 * 1024;
/// Default script execution timeout in milliseconds.
pub const TIMEOUT_MS: u64 = 100;
