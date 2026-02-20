//! ScriptStudy: implements the Study trait, bridging JS scripts to the chart renderer.

use crate::error::ScriptError;
use crate::manifest::ScriptManifest;
use crate::runtime::{drawing, globals, inputs, math, plot, ta};
use rquickjs::Runtime;
use std::sync::Arc;
use std::time::{Duration, Instant};
use study::config::{ParameterDef, ParameterValue, StudyConfig};
use study::error::StudyError;
use study::output::StudyOutput;
use study::traits::{Study, StudyCategory, StudyInput, StudyPlacement};

const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
const MAX_STACK_SIZE: usize = 512 * 1024;
const GC_THRESHOLD: usize = 2 * 1024 * 1024;
const TIMEOUT_MS: u64 = 100;

/// A script-backed indicator that implements the Study trait.
///
/// Each compute() call creates a fresh JS runtime+context, injects
/// candle/trade data and current parameter values, executes the script,
/// and converts plot commands into StudyOutput.
pub struct ScriptStudy {
    manifest: ScriptManifest,
    bytecode: Arc<Vec<u8>>,
    config: StudyConfig,
    parameters: Vec<ParameterDef>,
    primary_output: StudyOutput,
    secondary_outputs: Vec<StudyOutput>,
}

impl ScriptStudy {
    /// Create a new ScriptStudy from a manifest and compiled bytecode.
    pub fn new(manifest: ScriptManifest, bytecode: Arc<Vec<u8>>) -> Self {
        let parameters: Vec<ParameterDef> =
            manifest.inputs.iter().map(|i| i.to_parameter_def()).collect();

        let mut config = StudyConfig::new(&manifest.id);
        for input in &manifest.inputs {
            config.set(input.key.clone(), input.default.clone());
        }

        Self {
            manifest,
            bytecode,
            config,
            parameters,
            primary_output: StudyOutput::Empty,
            secondary_outputs: Vec::new(),
        }
    }

    /// Execute the script in a fresh JS context and collect outputs.
    fn execute(&self, input: &StudyInput) -> Result<Vec<StudyOutput>, ScriptError> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        runtime.set_max_stack_size(MAX_STACK_SIZE);
        runtime.set_gc_threshold(GC_THRESHOLD);

        let deadline = Instant::now() + Duration::from_millis(TIMEOUT_MS);
        runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));

        let ctx = rquickjs::Context::full(&runtime)?;

        let result = ctx.with(|ctx| -> Result<Vec<StudyOutput>, ScriptError> {
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

        runtime.set_interrupt_handler(None);
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
        if self.manifest.overlay {
            StudyPlacement::Overlay
        } else {
            StudyPlacement::Panel
        }
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.parameters
    }

    fn config(&self) -> &StudyConfig {
        &self.config
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
                Err(study::StudyError::Compute(e.to_string()))
            }
        }
    }

    fn output(&self) -> &StudyOutput {
        &self.primary_output
    }

    fn reset(&mut self) {
        self.primary_output = StudyOutput::Empty;
        self.secondary_outputs.clear();
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(ScriptStudy {
            manifest: self.manifest.clone(),
            bytecode: self.bytecode.clone(),
            config: self.config.clone(),
            parameters: self.parameters.clone(),
            primary_output: StudyOutput::Empty,
            secondary_outputs: Vec::new(),
        })
    }
}
