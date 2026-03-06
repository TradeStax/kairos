//! Footprint Study
//!
//! A CandleReplace study that renders per-price-level trade data
//! (buy/sell volume) for each candle. Supports multiple render modes
//! (Box, Profile) and data types (Volume, BidAskSplit, Delta, etc.).

mod compute;

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{CandleRenderConfig, StudyOutput};
use data::{ChartBasis, SerializableColor, Side, Trade};
use std::collections::BTreeMap;

/// Footprint study that renders per-price-level trade data for each
/// candle.
///
/// Displays buy/sell volume at each price level within a candle,
/// supporting two render modes (Box grid and Profile bars) and
/// multiple data types (Volume, Bid/Ask Split, Delta, Delta +
/// Volume). Replaces the normal candlestick rendering with a
/// detailed order flow view.
pub struct FootprintStudy {
    pub(super) config: StudyConfig,
    pub(super) output: StudyOutput,
    pub(super) params: Vec<ParameterDef>,
    /// Per-candle levels: price_units -> (buy_vol, sell_vol).
    pub(super) candle_levels: Vec<BTreeMap<i64, (f32, f32)>>,
    /// Per-candle grouping quantum (price units per row).
    pub(super) candle_quantums: Vec<i64>,
    /// Consolidated metadata: name, category, placement, capabilities.
    metadata: StudyMetadata,
}

impl FootprintStudy {
    /// Create a new Footprint study with default parameters.
    pub fn new() -> Self {
        let params = vec![
            // ── General > Typology ──
            ParameterDef {
                key: "data_type".into(),
                label: "Data Type".into(),
                description: "What data to display at each price level".into(),
                kind: ParameterKind::Choice {
                    options: &["Volume", "Bid/Ask Split", "Delta", "Delta + Volume"],
                },
                default: ParameterValue::Choice("Volume".to_string()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Render Mode",
                    order: 0,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "mode".into(),
                label: "Mode".into(),
                description: "Rendering mode: Box (colored grid) or Profile (bars)".into(),
                kind: ParameterKind::Choice {
                    options: &["Profile", "Box"],
                },
                default: ParameterValue::Choice("Profile".to_string()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Render Mode",
                    order: 0,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── General > Tick Grouping ──
            ParameterDef {
                key: "auto_grouping".into(),
                label: "Grouping".into(),
                description: "Automatic or Manual tick grouping".into(),
                kind: ParameterKind::Choice {
                    options: &["Automatic", "Manual"],
                },
                default: ParameterValue::Choice("Automatic".to_string()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Tick Grouping",
                    order: 1,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "auto_group_factor".into(),
                label: "Auto Group Factor".into(),
                description: "Tick size multiplier for automatic grouping".into(),
                kind: ParameterKind::Integer { min: 1, max: 100 },
                default: ParameterValue::Integer(1),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Tick Grouping",
                    order: 1,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenChoice {
                    key: "auto_grouping",
                    equals: "Automatic",
                },
            },
            ParameterDef {
                key: "manual_ticks".into(),
                label: "Manual Ticks".into(),
                description: "Number of ticks to group together".into(),
                kind: ParameterKind::Integer { min: 1, max: 100 },
                default: ParameterValue::Integer(1),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Tick Grouping",
                    order: 1,
                }),
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenChoice {
                    key: "auto_grouping",
                    equals: "Manual",
                },
            },
            ParameterDef {
                key: "group_mode".into(),
                label: "Group Mode".into(),
                description: "Bar-based (per candle) or Fixed (uniform) grouping".into(),
                kind: ParameterKind::Choice {
                    options: &["Bar-based", "Fixed"],
                },
                default: ParameterValue::Choice("Bar-based".to_string()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Tick Grouping",
                    order: 1,
                }),
                order: 3,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenChoice {
                    key: "auto_grouping",
                    equals: "Manual",
                },
            },
            // ── Style > Bar Marker ──
            ParameterDef {
                key: "bar_marker_width".into(),
                label: "Bar Marker Width".into(),
                description: "Width ratio for the candle body marker".into(),
                kind: ParameterKind::Float {
                    min: 0.05,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(0.25),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Bar Marker",
                    order: 0,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "outside_bar_style".into(),
                label: "Outside Bar Style".into(),
                description: "Style for the candle marker outside bars".into(),
                kind: ParameterKind::Choice {
                    options: &["Body", "Candle", "None"],
                },
                default: ParameterValue::Choice("Body".to_string()),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Bar Marker",
                    order: 0,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "marker_alignment".into(),
                label: "Marker Alignment".into(),
                description: "Where the candle body appears relative to bars".into(),
                kind: ParameterKind::Choice {
                    options: &["Left", "None", "Center", "Right"],
                },
                default: ParameterValue::Choice("Left".to_string()),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Bar Marker",
                    order: 0,
                }),
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "show_outside_border".into(),
                label: "Show Outside Border".into(),
                description: "Draw a border around the candle body marker".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(false),
                tab: ParameterTab::Display,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "max_bars_to_show".into(),
                label: "Max Bars".into(),
                description: "Maximum candles to render with footprint levels".into(),
                kind: ParameterKind::Integer { min: 10, max: 1000 },
                default: ParameterValue::Integer(200),
                tab: ParameterTab::Parameters,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "scaling".into(),
                label: "Scaling".into(),
                description: "How bar widths are scaled relative to each other".into(),
                kind: ParameterKind::Choice {
                    options: &[
                        "Square Root",
                        "Linear",
                        "Logarithmic",
                        "Visible Range",
                        "Datapoint",
                        "Hybrid",
                    ],
                },
                default: ParameterValue::Choice("Square Root".to_string()),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Bar Marker",
                    order: 0,
                }),
                order: 3,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Style > Background ──
            ParameterDef {
                key: "bg_color_mode".into(),
                label: "Background Color".into(),
                description: "Background coloring mode for cells".into(),
                kind: ParameterKind::Choice {
                    options: &["Volume Intensity", "Delta Intensity", "None"],
                },
                default: ParameterValue::Choice("Volume Intensity".to_string()),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Background",
                    order: 1,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "bg_max_alpha".into(),
                label: "Background Max Alpha".into(),
                description: "Maximum opacity for background fills".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(0.6),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Background",
                    order: 1,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "bg_buy_color".into(),
                label: "Buy Color".into(),
                description: "Buy/bullish color (defaults to theme)".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(SerializableColor::new(0.0, 0.0, 0.0, 0.0)),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Colors",
                    order: 2,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "bg_sell_color".into(),
                label: "Sell Color".into(),
                description: "Sell/bearish color (defaults to theme)".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(SerializableColor::new(0.0, 0.0, 0.0, 0.0)),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Colors",
                    order: 2,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "show_grid_lines".into(),
                label: "Grid Lines".into(),
                description: "Draw cell borders in Box mode".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
                tab: ParameterTab::Display,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Style > Text ──
            ParameterDef {
                key: "font_size".into(),
                label: "Font Size".into(),
                description: "Text size for level values".into(),
                kind: ParameterKind::Float {
                    min: 6.0,
                    max: 20.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(11.0),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Text",
                    order: 3,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "text_format".into(),
                label: "Text Format".into(),
                description: "How numeric values are displayed".into(),
                kind: ParameterKind::Choice {
                    options: &["Automatic", "Normal", "K"],
                },
                default: ParameterValue::Choice("Automatic".to_string()),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Text",
                    order: 3,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "dynamic_text_size".into(),
                label: "Dynamic Text Size".into(),
                description: "Automatically adjust text size based on cell size".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
                tab: ParameterTab::Display,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "show_zero_values".into(),
                label: "Show Zero Values".into(),
                description: "Display text for levels with zero volume".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(false),
                tab: ParameterTab::Display,
                section: None,
                order: 3,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "text_color".into(),
                label: "Text Color".into(),
                description: "Color for level value text (defaults to theme)".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(SerializableColor::new(0.0, 0.0, 0.0, 0.0)),
                tab: ParameterTab::Style,
                section: Some(ParameterSection {
                    label: "Text",
                    order: 3,
                }),
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("footprint");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            params,
            candle_levels: Vec::new(),
            candle_quantums: Vec::new(),
            metadata: StudyMetadata {
                name: "Footprint".into(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::CandleReplace,
                description: "Per-candle trade volume at each price level".into(),
                config_version: 1,
                capabilities: StudyCapabilities {
                    incremental: true,
                    needs_trades: true,
                    candle_replace: true,
                    ..StudyCapabilities::default()
                },
            },
        }
    }
}

impl Default for FootprintStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Study trait implementation for Footprint.
///
/// Placement is always `CandleReplace` -- the footprint view
/// replaces normal candlestick rendering entirely.
impl Study for FootprintStudy {
    fn id(&self) -> &str {
        "footprint"
    }

    fn metadata(&self) -> &StudyMetadata {
        &self.metadata
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn tab_labels(&self) -> Option<&[(&'static str, ParameterTab)]> {
        static LABELS: &[(&str, ParameterTab)] = &[
            ("General", ParameterTab::Parameters),
            ("Style", ParameterTab::Style),
            ("Colors", ParameterTab::Display),
        ];
        Some(LABELS)
    }

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        if input.candles.is_empty() {
            self.candle_levels.clear();
            self.candle_quantums.clear();
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let trades = match input.trades {
            Some(t) => t,
            None => {
                self.output = StudyOutput::Empty;
                return Ok(StudyResult::ok());
            }
        };

        let tick_units = input.tick_size.units();
        let candle_count = input.candles.len();
        let interval_ms = match input.basis {
            ChartBasis::Time(tf) => tf.to_milliseconds(),
            ChartBasis::Tick(_) => 1000,
        };

        self.candle_levels.clear();
        self.candle_levels.resize_with(candle_count, BTreeMap::new);
        self.candle_quantums.clear();
        self.candle_quantums.resize(candle_count, 1);

        for (idx, candle) in input.candles.iter().enumerate() {
            let candle_start = candle.time.0;
            let candle_end = if idx + 1 < candle_count {
                input.candles[idx + 1].time.0
            } else {
                candle.time.0 + interval_ms
            };

            let start_trade = trades
                .binary_search_by_key(&candle_start, |t| t.time.0)
                .unwrap_or_else(|i| i);
            let end_trade = trades[start_trade..]
                .binary_search_by_key(&candle_end, |t| t.time.0)
                .map(|i| start_trade + i)
                .unwrap_or_else(|i| start_trade + i);

            let quantum =
                self.quantum_for_candle(tick_units, candle.high.units(), candle.low.units());

            self.candle_quantums[idx] = quantum;

            self.aggregate_trades_for_candle(idx, &trades[start_trade..end_trade], quantum);
        }

        self.output = self.build_output(input.candles);
        Ok(StudyResult::ok())
    }

    fn append_trades(
        &mut self,
        new_trades: &[Trade],
        input: &StudyInput,
    ) -> Result<StudyResult, StudyError> {
        if input.candles.is_empty() || new_trades.is_empty() {
            return Ok(StudyResult::ok());
        }

        let tick_units = input.tick_size.units();
        let last_idx = input.candles.len() - 1;

        // Ensure candle_levels is sized correctly
        if self.candle_levels.len() <= last_idx {
            self.candle_levels.resize_with(last_idx + 1, BTreeMap::new);
        }
        if self.candle_quantums.len() <= last_idx {
            self.candle_quantums.resize(last_idx + 1, 1);
        }

        let last_candle = &input.candles[last_idx];
        let quantum = self.quantum_for_candle(
            tick_units,
            last_candle.high.units(),
            last_candle.low.units(),
        );
        self.candle_quantums[last_idx] = quantum;

        // Append trades to the last candle
        let map = &mut self.candle_levels[last_idx];
        for trade in new_trades {
            let price_units = trade.price.units();
            let rounded = if quantum > 0 {
                (price_units / quantum) * quantum
            } else {
                price_units
            };
            let entry = map.entry(rounded).or_insert((0.0, 0.0));
            match trade.side {
                Side::Buy | Side::Bid => {
                    entry.0 += trade.quantity.0 as f32;
                }
                Side::Sell | Side::Ask => {
                    entry.1 += trade.quantity.0 as f32;
                }
            }
        }

        self.output = self.build_output(input.candles);
        Ok(StudyResult::ok())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.candle_levels.clear();
        self.candle_quantums.clear();
        self.output = StudyOutput::Empty;
    }

    fn candle_render_config(&self) -> Option<CandleRenderConfig> {
        Some(CandleRenderConfig {
            default_cell_width: 80.0,
            max_cell_width: 500.0,
            min_cell_width: 10.0,
            cell_height_ratio: 4.0,
            initial_candle_window: 12,
            autoscale_x_cells: 1.0,
        })
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            candle_levels: self.candle_levels.clone(),
            candle_quantums: self.candle_quantums.clone(),
            metadata: self.metadata.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
