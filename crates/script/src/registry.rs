//! ScriptRegistry: discovers scripts, compiles them, and creates ScriptStudy instances.

use crate::engine::ScriptEngine;
use crate::loader::ScriptLoader;
use crate::manifest::ScriptManifest;
use crate::study_adapter::ScriptStudy;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use study::registry::{StudyInfo, StudyRegistry};
use study::traits::Study;

/// Registry of compiled script indicators.
pub struct ScriptRegistry {
    manifests: HashMap<String, ScriptManifest>,
    bytecodes: HashMap<String, Arc<Vec<u8>>>,
}

impl ScriptRegistry {
    /// Create an empty registry (fallback when engine initialization fails).
    pub fn empty() -> Self {
        Self {
            manifests: HashMap::new(),
            bytecodes: HashMap::new(),
        }
    }

    /// Create a new registry by discovering and compiling all available scripts.
    pub fn new(engine: &mut ScriptEngine) -> Self {
        let loader = ScriptLoader::new();
        let script_paths = loader.discover();

        let mut manifests = HashMap::new();
        let mut bytecodes = HashMap::new();

        for path in script_paths {
            match Self::load_script(engine, &path) {
                Ok((manifest, bytecode)) => {
                    let id = manifest.id.clone();
                    log::info!("Loaded script indicator: {} ({})", manifest.name, id);
                    manifests.insert(id.clone(), manifest);
                    bytecodes.insert(id, Arc::new(bytecode));
                }
                Err(e) => {
                    log::warn!("Failed to load script {:?}: {}", path, e);
                }
            }
        }

        Self {
            manifests,
            bytecodes,
        }
    }

    /// Load and compile a single script file.
    fn load_script(
        engine: &mut ScriptEngine,
        path: &Path,
    ) -> Result<(ScriptManifest, Vec<u8>), crate::error::ScriptError> {
        let source = std::fs::read_to_string(path)?;
        let bytecode = engine.compile(path, &source)?;
        let manifest = engine.extract_manifest(path, &bytecode)?;
        Ok((manifest, bytecode))
    }

    /// Create a ScriptStudy instance by script ID.
    pub fn create(&self, id: &str) -> Option<Box<dyn Study>> {
        let manifest = self.manifests.get(id)?;
        let bytecode = self.bytecodes.get(id)?;
        Some(Box::new(ScriptStudy::new(manifest.clone(), bytecode.clone())))
    }

    /// List all loaded script indicators.
    pub fn list(&self) -> Vec<StudyInfo> {
        let mut studies: Vec<StudyInfo> = self
            .manifests
            .values()
            .map(|m| StudyInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                category: m.category,
                placement: m.resolved_placement(),
                description: format!("Script: {}", m.name),
            })
            .collect();
        studies.sort_by(|a, b| a.name.cmp(&b.name));
        studies
    }

    /// Register all script indicators into an existing StudyRegistry.
    ///
    /// This merges script-based indicators alongside native studies so
    /// the rest of the application can use a single unified registry.
    /// Script studies do NOT override existing native studies with the
    /// same ID — native implementations take priority until explicitly
    /// removed.
    pub fn register_into(&self, registry: &mut StudyRegistry) {
        for (id, manifest) in &self.manifests {
            // Skip script studies that would override existing native studies
            if registry.contains(id) {
                log::debug!(
                    "Skipping script '{}': native study already registered",
                    id
                );
                continue;
            }

            let bytecode = match self.bytecodes.get(id) {
                Some(b) => b.clone(),
                None => continue,
            };
            let manifest_clone = manifest.clone();
            let info = StudyInfo {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                category: manifest.category,
                placement: manifest.resolved_placement(),
                description: format!("Script: {}", manifest.name),
            };

            registry.register(id, info, move || {
                Box::new(ScriptStudy::new(manifest_clone.clone(), bytecode.clone()))
            });
        }
    }

    /// Reload a specific script (for hot-reload).
    pub fn reload(
        &mut self,
        engine: &mut ScriptEngine,
        id: &str,
    ) -> Result<(), crate::error::ScriptError> {
        let path = match self.manifests.get(id) {
            Some(m) => m.path.clone(),
            None => {
                return Err(crate::error::ScriptError::Runtime {
                    file: id.to_string(),
                    message: "script not found in registry".to_string(),
                });
            }
        };

        engine.compiler_mut().invalidate(&path);
        let (manifest, bytecode) = Self::load_script(engine, &path)?;
        self.manifests.insert(id.to_string(), manifest);
        self.bytecodes.insert(id.to_string(), Arc::new(bytecode));
        Ok(())
    }

    /// Check if a script ID is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.manifests.contains_key(id)
    }
}
