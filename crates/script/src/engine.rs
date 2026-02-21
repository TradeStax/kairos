//! ScriptEngine: manages rquickjs Runtime, compilation, and context creation.

use crate::compiler::ScriptCompiler;
use crate::error::ScriptError;
use crate::manifest::ScriptManifest;

use rquickjs::Runtime;
use std::path::Path;
use std::time::{Duration, Instant};

/// Memory limit per runtime (16MB).
const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
/// Max stack size (512KB).
const MAX_STACK_SIZE: usize = 512 * 1024;
/// GC threshold (2MB).
const GC_THRESHOLD: usize = 2 * 1024 * 1024;
/// Default execution timeout (100ms).
const DEFAULT_TIMEOUT_MS: u64 = 100;

/// The core script engine that manages the QuickJS runtime and script lifecycle.
pub struct ScriptEngine {
    runtime: Runtime,
    compiler: ScriptCompiler,
    timeout_ms: u64,
}

impl ScriptEngine {
    /// Create a new ScriptEngine with default resource limits.
    pub fn new() -> Result<Self, ScriptError> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        runtime.set_max_stack_size(MAX_STACK_SIZE);
        runtime.set_gc_threshold(GC_THRESHOLD);

        let compiler = ScriptCompiler::new()?;

        Ok(Self {
            runtime,
            compiler,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        })
    }

    /// Get a reference to the underlying runtime.
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Get a reference to the compiler.
    pub fn compiler(&self) -> &ScriptCompiler {
        &self.compiler
    }

    /// Get a mutable reference to the compiler.
    pub fn compiler_mut(&mut self) -> &mut ScriptCompiler {
        &mut self.compiler
    }

    /// Set execution timeout in milliseconds.
    pub fn set_timeout_ms(&mut self, ms: u64) {
        self.timeout_ms = ms;
    }

    /// Get execution timeout.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Set up the interrupt handler for execution timeout.
    pub fn arm_interrupt(&self) {
        let deadline = Instant::now() + Duration::from_millis(self.timeout_ms);
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    }

    /// Clear the interrupt handler after execution.
    pub fn disarm_interrupt(&self) {
        self.runtime.set_interrupt_handler(None);
    }

    /// Compile a script source to bytecode, using cache if available.
    pub fn compile(
        &mut self,
        source_path: &Path,
        source: &str,
    ) -> Result<Vec<u8>, ScriptError> {
        self.compiler.compile(&self.runtime, source_path, source)
    }

    /// Create a new JavaScript context with the indicator runtime environment.
    pub fn create_context(&self) -> Result<rquickjs::Context, ScriptError> {
        let ctx = rquickjs::Context::full(&self.runtime)?;
        Ok(ctx)
    }

    /// Run a declaration pass on a script to extract its manifest.
    ///
    /// Executes the script once with no bar data to collect the `indicator()`
    /// call metadata and `input.*()` parameter declarations.
    pub fn extract_manifest(
        &self,
        source_path: &Path,
        bytecode: &[u8],
    ) -> Result<ScriptManifest, ScriptError> {
        let ctx = self.create_context()?;
        let file_stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let modified = std::fs::metadata(source_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or_else(std::time::SystemTime::now);

        self.arm_interrupt();

        let result = ctx.with(|ctx| {
            // Install declaration-pass runtime (collects indicator() and input.*() calls)
            let manifest_state =
                crate::runtime::inputs::install_declaration_pass(&ctx)?;
            crate::runtime::globals::install_stub_globals(&ctx)?;
            crate::runtime::ta::install_ta_stubs(&ctx)?;
            crate::runtime::plot::install_plot_stubs(&ctx)?;
            crate::runtime::math::install_math(&ctx)?;
            crate::runtime::drawing::install_drawing_stubs(&ctx)?;

            // Execute the script bytecode
            let module = unsafe { rquickjs::Module::load(ctx.clone(), bytecode)? };
            let _ = module.eval()?;

            // Extract the manifest from collected declarations
            let state = manifest_state.borrow();
            let manifest = ScriptManifest {
                id: file_stem.clone(),
                name: state
                    .name
                    .clone()
                    .unwrap_or_else(|| file_stem.clone()),
                overlay: state.overlay,
                placement: state.placement,
                category: state.category,
                path: source_path.to_path_buf(),
                modified,
                inputs: state.inputs.clone(),
                marker_render_config: state.marker_render_config.clone(),
                candle_render_config: state.candle_render_config,
            };
            Ok(manifest)
        });

        self.disarm_interrupt();
        result
    }
}
