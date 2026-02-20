//! Script compilation and bytecode caching.

use crate::error::ScriptError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Compiles JavaScript source to QuickJS bytecode with disk caching.
pub struct ScriptCompiler {
    /// In-memory bytecode cache: source path -> (bytecode, source_modified)
    cache: HashMap<PathBuf, CacheEntry>,
    /// Disk cache directory
    cache_dir: PathBuf,
}

struct CacheEntry {
    bytecode: Vec<u8>,
    source_modified: SystemTime,
}

impl ScriptCompiler {
    /// Create a new compiler with the default cache directory.
    pub fn new() -> Result<Self, ScriptError> {
        let cache_dir = crate::path::data_path(Some("cache/bytecode"));
        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            cache: HashMap::new(),
            cache_dir,
        })
    }

    /// Create a compiler with a custom cache directory.
    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self, ScriptError> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache: HashMap::new(),
            cache_dir,
        })
    }

    /// Compile JS source to bytecode, using cache if available and fresh.
    pub fn compile(
        &mut self,
        runtime: &rquickjs::Runtime,
        source_path: &Path,
        source: &str,
    ) -> Result<Vec<u8>, ScriptError> {
        let source_modified = std::fs::metadata(source_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or_else(SystemTime::now);

        // Check in-memory cache
        if let Some(entry) = self.cache.get(source_path)
            && entry.source_modified >= source_modified
        {
            return Ok(entry.bytecode.clone());
        }

        // Check disk cache
        if let Some(bytecode) = self.load_disk_cache(source_path, source_modified) {
            self.cache.insert(
                source_path.to_path_buf(),
                CacheEntry {
                    bytecode: bytecode.clone(),
                    source_modified,
                },
            );
            return Ok(bytecode);
        }

        // Compile from source
        let bytecode = self.compile_source(runtime, source_path, source)?;

        // Save to disk cache
        let _ = self.save_disk_cache(source_path, &bytecode);

        // Save to in-memory cache
        self.cache.insert(
            source_path.to_path_buf(),
            CacheEntry {
                bytecode: bytecode.clone(),
                source_modified,
            },
        );

        Ok(bytecode)
    }

    /// Compile JS source to bytecode directly (no caching).
    fn compile_source(
        &self,
        runtime: &rquickjs::Runtime,
        source_path: &Path,
        source: &str,
    ) -> Result<Vec<u8>, ScriptError> {
        let filename = source_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("script.js");

        let ctx = rquickjs::Context::full(runtime)?;
        ctx.with(|ctx| {
            let module = rquickjs::Module::declare(ctx.clone(), filename, source)
                .map_err(|e| ScriptError::Parse {
                    file: filename.to_string(),
                    message: e.to_string(),
                })?;
            let bytecode = module.write(rquickjs::WriteOptions::default()).map_err(|e| ScriptError::Parse {
                file: filename.to_string(),
                message: format!("bytecode compilation failed: {e}"),
            })?;
            Ok(bytecode)
        })
    }

    /// Invalidate the cache for a specific script (used for hot-reload).
    pub fn invalidate(&mut self, source_path: &Path) {
        self.cache.remove(source_path);
        let _ = std::fs::remove_file(self.disk_cache_path(source_path));
    }

    fn disk_cache_path(&self, source_path: &Path) -> PathBuf {
        let stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        self.cache_dir.join(format!("{stem}.qjsc"))
    }

    fn load_disk_cache(
        &self,
        source_path: &Path,
        source_modified: SystemTime,
    ) -> Option<Vec<u8>> {
        let cache_path = self.disk_cache_path(source_path);
        let cache_meta = std::fs::metadata(&cache_path).ok()?;
        let cache_modified = cache_meta.modified().ok()?;

        if cache_modified >= source_modified {
            std::fs::read(&cache_path).ok()
        } else {
            None
        }
    }

    fn save_disk_cache(
        &self,
        source_path: &Path,
        bytecode: &[u8],
    ) -> Result<(), std::io::Error> {
        let cache_path = self.disk_cache_path(source_path);
        std::fs::write(&cache_path, bytecode)
    }
}
