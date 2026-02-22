//! ScriptStudy: implements the Study trait, bridging JS scripts to the chart renderer.

use crate::error::ScriptError;
use crate::limits;
use crate::manifest::ScriptManifest;
use crate::runtime::{drawing, globals, inputs, math, plot, ta};
use rquickjs::{Context, Runtime};
use std::sync::Arc;
use std::time::{Duration, Instant};
use study::config::{ParameterDef, ParameterValue, StudyConfig};
use study::error::StudyError;
use study::output::StudyOutput;
use study::traits::{Study, StudyCategory, StudyInput, StudyPlacement};

/// A script-backed indicator that implements the Study trait.
///
/// The JS runtime and context are created once in `new()` and reused across
/// `compute()` calls, avoiding the overhead of runtime creation per call.
pub struct ScriptStudy {
    manifest: ScriptManifest,
    bytecode: Arc<Vec<u8>>,
    config: StudyConfig,
    parameters: Vec<ParameterDef>,
    primary_output: StudyOutput,
    secondary_outputs: Vec<StudyOutput>,
    output: StudyOutput,
    runtime: Runtime,
    context: Context,
}

impl ScriptStudy {
    /// Create a new ScriptStudy from a manifest and compiled bytecode.
    pub fn new(manifest: ScriptManifest, bytecode: Arc<Vec<u8>>) -> Result<Self, ScriptError> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(limits::MEMORY_LIMIT);
        runtime.set_max_stack_size(limits::MAX_STACK_SIZE);
        runtime.set_gc_threshold(limits::GC_THRESHOLD);
        let context = Context::full(&runtime)?;

        let parameters: Vec<ParameterDef> =
            manifest.inputs.iter().map(|i| i.to_parameter_def()).collect();

        let mut config = StudyConfig::new(&manifest.id);
        for input in &manifest.inputs {
            config.set(input.key.clone(), input.default.clone());
        }

        Ok(Self {
            manifest,
            bytecode,
            config,
            parameters,
            primary_output: StudyOutput::Empty,
            secondary_outputs: Vec::new(),
            output: StudyOutput::Empty,
            runtime,
            context,
        })
    }

    /// Create a fresh runtime + context pair (used by clone_study).
    fn make_runtime_context() -> Result<(Runtime, Context), ScriptError> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(limits::MEMORY_LIMIT);
        runtime.set_max_stack_size(limits::MAX_STACK_SIZE);
        runtime.set_gc_threshold(limits::GC_THRESHOLD);
        let context = Context::full(&runtime)?;
        Ok((runtime, context))
    }

    /// Execute the script in the persistent JS context and collect outputs.
    fn execute(&self, input: &StudyInput) -> Result<Vec<StudyOutput>, ScriptError> {
        let deadline = Instant::now() + Duration::from_millis(limits::TIMEOUT_MS);
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));

        let result = self.context.with(|ctx| -> Result<Vec<StudyOutput>, ScriptError> {
            // Install compute-pass runtime
            inputs::install_compute_inputs(&ctx, &self.config, &self.manifest.inputs)?;
            globals::inject_candle_globals(&ctx, input.candles, input.tick_size)?;

            if let Some(trades) = input.trades {
                globals::inject_trades_global(&ctx, trades)?;
            } else {
                globals::inject_trades_global(&ctx, &[])?;
            }

            ta::install_ta(&ctx)?;
            let collector = plot::install_plot(&ctx)?;
            math::install_math(&ctx)?;
            drawing::install_drawing_stubs(&ctx)?;

            // Execute the compiled script
            let module = unsafe { rquickjs::Module::load(ctx.clone(), &self.bytecode)? };
            let _ = module.eval()?;

            // Convert plot commands to StudyOutput
            let commands = collector.borrow();
            Ok(crate::bridge::convert_plot_commands(
                &commands.commands,
                input.candles,
            ))
        });

        self.runtime.set_interrupt_handler(None);
        result
    }
}

impl Study for ScriptStudy {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn category(&self) -> StudyCategory {
        self.manifest.category
    }

    fn placement(&self) -> StudyPlacement {
        self.manifest.resolved_placement()
    }

    fn candle_render_config(&self) -> Option<study::output::CandleRenderConfig> {
        self.manifest.candle_render_config
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.parameters
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn set_parameter(
        &mut self,
        key: &str,
        value: ParameterValue,
    ) -> Result<(), StudyError> {
        if !self.parameters.iter().any(|p| p.key == key) {
            return Err(StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            });
        }
        self.config.set(key, value);
        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), study::StudyError> {
        match self.execute(input) {
            Ok(outputs) => {
                self.primary_output = outputs
                    .first()
                    .cloned()
                    .unwrap_or(StudyOutput::Empty);
                self.secondary_outputs = if outputs.len() > 1 {
                    outputs[1..].to_vec()
                } else {
                    Vec::new()
                };
                self.output = if self.secondary_outputs.is_empty() {
                    self.primary_output.clone()
                } else {
                    let mut all = vec![self.primary_output.clone()];
                    all.extend(self.secondary_outputs.iter().cloned());
                    StudyOutput::Composite(all)
                };
                Ok(())
            }
            Err(e) => {
                log::warn!(
                    "Script '{}' compute error: {}",
                    self.manifest.id,
                    e
                );
                self.primary_output = StudyOutput::Empty;
                self.secondary_outputs.clear();
                self.output = StudyOutput::Empty;
                Err(study::StudyError::Compute(e.to_string()))
            }
        }
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.primary_output = StudyOutput::Empty;
        self.secondary_outputs.clear();
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        // Runtime and Context are not Clone in a meaningful way for isolation,
        // so create fresh instances for the cloned study.
        let (runtime, context) = Self::make_runtime_context()
            .expect("failed to create JS runtime for cloned ScriptStudy");
        Box::new(ScriptStudy {
            manifest: self.manifest.clone(),
            bytecode: self.bytecode.clone(),
            config: self.config.clone(),
            parameters: self.parameters.clone(),
            primary_output: StudyOutput::Empty,
            secondary_outputs: Vec::new(),
            output: StudyOutput::Empty,
            runtime,
            context,
        })
    }
}
