//! Plotting functions that collect draw commands during script execution.
//!
//! Functions: plot(), plotBar(), plotHistogram(), plotShape(), marker(),
//! hline(), fill(), bgcolor(), barcolor().

use crate::error::ScriptError;
use crate::runtime::inputs::parse_hex_color;
use data::SerializableColor;
use rquickjs::{Ctx, Function, Object};
use std::cell::RefCell;
use std::rc::Rc;
use study::config::LineStyleValue;
use study::output::{
    FootprintCandlePosition, FootprintDataType, FootprintRenderMode,
    FootprintScaling, ProfileSide,
};

/// A collected plot command from JS execution.
#[derive(Debug, Clone)]
pub enum PlotCommand {
    Line {
        id: usize,
        name: String,
        points: Vec<f64>,
        color: SerializableColor,
        width: f32,
        style: LineStyleValue,
    },
    Bar {
        name: String,
        /// (value, color_hex) per bar; color_hex may be per-bar or uniform
        points: Vec<(f64, SerializableColor)>,
    },
    Histogram {
        name: String,
        points: Vec<(f64, SerializableColor)>,
    },
    Marker {
        time: f64,
        price: f64,
        size: f64,
        color: SerializableColor,
        label: Option<String>,
        is_buy: bool,
    },
    HLine {
        price: f64,
        name: String,
        color: SerializableColor,
        style: LineStyleValue,
        opacity: f32,
    },
    Fill {
        plot_id_a: usize,
        plot_id_b: usize,
        color: SerializableColor,
        opacity: f32,
    },
    Profile {
        levels: Vec<ProfileLevelJS>,
        poc_index: Option<usize>,
        value_area: Option<(usize, usize)>,
        side: ProfileSide,
    },
    Footprint {
        candles: Vec<FootprintCandleJS>,
        mode: FootprintRenderMode,
        data_type: FootprintDataType,
        scaling: FootprintScaling,
        candle_position: FootprintCandlePosition,
    },
}

/// A single level within a profile plot command.
#[derive(Debug, Clone)]
pub struct ProfileLevelJS {
    pub price: f64,
    pub buy_volume: f64,
    pub sell_volume: f64,
}

/// Per-candle footprint data from JS.
#[derive(Debug, Clone)]
pub struct FootprintCandleJS {
    pub x: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub levels: Vec<FootprintLevelJS>,
    pub poc_index: Option<usize>,
}

/// Per-price-level data within a footprint candle from JS.
#[derive(Debug, Clone)]
pub struct FootprintLevelJS {
    pub price: f64,
    pub buy_volume: f64,
    pub sell_volume: f64,
}

/// Shared state for collecting plot commands during execution.
#[derive(Debug, Default)]
pub struct PlotCollector {
    pub commands: Vec<PlotCommand>,
    next_plot_id: usize,
}

impl PlotCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.next_plot_id = 0;
    }

    fn next_id(&mut self) -> usize {
        let id = self.next_plot_id;
        self.next_plot_id += 1;
        id
    }
}

/// Install plot functions into the JS context.
///
/// Returns a shared PlotCollector that accumulates commands.
pub fn install_plot(
    ctx: &Ctx<'_>,
) -> Result<Rc<RefCell<PlotCollector>>, ScriptError> {
    let collector = Rc::new(RefCell::new(PlotCollector::new()));
    let globals = ctx.globals();

    // plot(series, name, { color, lineWidth, style }) -> plotRef (int)
    {
        let coll = collector.clone();
        let plot_fn = Function::new(
            ctx.clone(),
            move |series: Vec<f64>,
                  name: rquickjs::function::Opt<String>,
                  opts: rquickjs::function::Opt<Object<'_>>|
                  -> i32 {
                let name = name.0.unwrap_or_default();
                let color = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("color").ok())
                    .map(|h| parse_hex_color(&h))
                    .unwrap_or(SerializableColor {
                        r: 0.13,
                        g: 0.59,
                        b: 0.95,
                        a: 1.0,
                    });
                let width = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("lineWidth").ok())
                    .unwrap_or(1.5) as f32;
                let style = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("style").ok())
                    .map(|s| parse_line_style(&s))
                    .unwrap_or(LineStyleValue::Solid);

                let mut c = coll.borrow_mut();
                let id = c.next_id();
                c.commands.push(PlotCommand::Line {
                    id,
                    name,
                    points: series,
                    color,
                    width,
                    style,
                });
                id as i32
            },
        )?;
        globals.set("plot", plot_fn)?;
    }

    // plotBar(series, name, { color, opacity })
    {
        let coll = collector.clone();
        let plot_bar_fn = Function::new(
            ctx.clone(),
            move |series: Vec<f64>,
                  name: rquickjs::function::Opt<String>,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let _name = name.0.unwrap_or_default();
                let default_color = SerializableColor {
                    r: 0.3,
                    g: 0.69,
                    b: 0.31,
                    a: 0.8,
                };
                let color = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("color").ok())
                    .map(|h| parse_hex_color(&h))
                    .unwrap_or(default_color);

                let points: Vec<(f64, SerializableColor)> =
                    series.into_iter().map(|v| (v, color)).collect();

                coll.borrow_mut().commands.push(PlotCommand::Bar {
                    name: _name,
                    points,
                });
            },
        )?;
        globals.set("plotBar", plot_bar_fn)?;
    }

    // plotBarColored(values, colors, name) - bars with per-bar colors
    {
        let coll = collector.clone();
        let plot_bar_colored_fn = Function::new(
            ctx.clone(),
            move |values: Vec<f64>,
                  colors: Vec<String>,
                  name: rquickjs::function::Opt<String>| {
                let name = name.0.unwrap_or_default();
                let points: Vec<(f64, SerializableColor)> = values
                    .into_iter()
                    .zip(colors.iter())
                    .map(|(v, c)| (v, parse_hex_color(c)))
                    .collect();

                coll.borrow_mut()
                    .commands
                    .push(PlotCommand::Bar { name, points });
            },
        )?;
        globals.set("plotBarColored", plot_bar_colored_fn)?;
    }

    // plotHistogram(series, name, { color })
    {
        let coll = collector.clone();
        let plot_hist_fn = Function::new(
            ctx.clone(),
            move |series: Vec<f64>,
                  name: rquickjs::function::Opt<String>,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let name = name.0.unwrap_or_default();
                let color = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("color").ok())
                    .map(|h| parse_hex_color(&h))
                    .unwrap_or(SerializableColor {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 0.7,
                    });
                let points: Vec<(f64, SerializableColor)> =
                    series.into_iter().map(|v| (v, color)).collect();
                coll.borrow_mut()
                    .commands
                    .push(PlotCommand::Histogram { name, points });
            },
        )?;
        globals.set("plotHistogram", plot_hist_fn)?;
    }

    // plotHistogramColored(values, colors, name)
    {
        let coll = collector.clone();
        let plot_hist_colored_fn = Function::new(
            ctx.clone(),
            move |values: Vec<f64>,
                  colors: Vec<String>,
                  name: rquickjs::function::Opt<String>| {
                let name = name.0.unwrap_or_default();
                let points: Vec<(f64, SerializableColor)> = values
                    .into_iter()
                    .zip(colors.iter())
                    .map(|(v, c)| (v, parse_hex_color(c)))
                    .collect();
                coll.borrow_mut()
                    .commands
                    .push(PlotCommand::Histogram { name, points });
            },
        )?;
        globals.set("plotHistogramColored", plot_hist_colored_fn)?;
    }

    // marker(time, price, { size, color, label, isBuy })
    {
        let coll = collector.clone();
        let marker_fn = Function::new(
            ctx.clone(),
            move |time: f64,
                  price: f64,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let size = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("size").ok())
                    .unwrap_or(1.0);
                let color = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("color").ok())
                    .map(|h| parse_hex_color(&h))
                    .unwrap_or(SerializableColor {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 0.5,
                    });
                let label = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("label").ok());
                let is_buy = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, bool>("isBuy").ok())
                    .unwrap_or(true);

                coll.borrow_mut().commands.push(PlotCommand::Marker {
                    time,
                    price,
                    size,
                    color,
                    label,
                    is_buy,
                });
            },
        )?;
        globals.set("marker", marker_fn)?;
    }

    // hline(price, name, { color, style, opacity })
    {
        let coll = collector.clone();
        let hline_fn = Function::new(
            ctx.clone(),
            move |price: f64,
                  name: rquickjs::function::Opt<String>,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let name = name.0.unwrap_or_default();
                let color = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("color").ok())
                    .map(|h| parse_hex_color(&h))
                    .unwrap_or(SerializableColor {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 0.8,
                    });
                let style = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("style").ok())
                    .map(|s| parse_line_style(&s))
                    .unwrap_or(LineStyleValue::Dashed);
                let opacity = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("opacity").ok())
                    .unwrap_or(0.8) as f32;

                coll.borrow_mut().commands.push(PlotCommand::HLine {
                    price,
                    name,
                    color,
                    style,
                    opacity,
                });
            },
        )?;
        globals.set("hline", hline_fn)?;
    }

    // fill(plotRef1, plotRef2, { color, opacity })
    {
        let coll = collector.clone();
        let fill_fn = Function::new(
            ctx.clone(),
            move |ref_a: i32,
                  ref_b: i32,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let color = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("color").ok())
                    .map(|h| parse_hex_color(&h))
                    .unwrap_or(SerializableColor {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 0.1,
                    });
                let opacity = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("opacity").ok())
                    .unwrap_or(0.1) as f32;

                coll.borrow_mut().commands.push(PlotCommand::Fill {
                    plot_id_a: ref_a as usize,
                    plot_id_b: ref_b as usize,
                    color,
                    opacity,
                });
            },
        )?;
        globals.set("fill", fill_fn)?;
    }

    // plotProfile(profileData, { side })
    {
        let coll = collector.clone();
        let plot_profile_fn = Function::new(
            ctx.clone(),
            move |profile: Object<'_>,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let side = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("side").ok())
                    .map(|s| parse_profile_side(&s))
                    .unwrap_or(ProfileSide::Left);

                let prices: Vec<f64> = profile
                    .get::<_, Vec<f64>>("prices")
                    .unwrap_or_default();
                let buy_vols: Vec<f64> = profile
                    .get::<_, Vec<f64>>("buyVolumes")
                    .unwrap_or_default();
                let sell_vols: Vec<f64> = profile
                    .get::<_, Vec<f64>>("sellVolumes")
                    .unwrap_or_default();
                let poc_index: Option<usize> = profile
                    .get::<_, i32>("pocIndex")
                    .ok()
                    .and_then(|v| {
                        if v >= 0 { Some(v as usize) } else { None }
                    });

                let value_area = profile
                    .get::<_, Object<'_>>("valueArea")
                    .ok()
                    .and_then(|va| {
                        let vah: i32 = va.get("vahIndex").ok()?;
                        let val: i32 = va.get("valIndex").ok()?;
                        Some((vah as usize, val as usize))
                    });

                let levels: Vec<ProfileLevelJS> = prices
                    .into_iter()
                    .enumerate()
                    .map(|(i, p)| ProfileLevelJS {
                        price: p,
                        buy_volume: buy_vols
                            .get(i)
                            .copied()
                            .unwrap_or(0.0),
                        sell_volume: sell_vols
                            .get(i)
                            .copied()
                            .unwrap_or(0.0),
                    })
                    .collect();

                coll.borrow_mut()
                    .commands
                    .push(PlotCommand::Profile {
                        levels,
                        poc_index,
                        value_area,
                        side,
                    });
            },
        )?;
        globals.set("plotProfile", plot_profile_fn)?;
    }

    // plotFootprint(footprintData, { mode, dataType, scaling,
    //   candlePosition })
    {
        let coll = collector.clone();
        let plot_fp_fn = Function::new(
            ctx.clone(),
            move |fp_data: Object<'_>,
                  opts: rquickjs::function::Opt<Object<'_>>| {
                let mode = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, String>("mode").ok())
                    .map(|s| parse_fp_render_mode(&s))
                    .unwrap_or_default();
                let data_type = opts
                    .0
                    .as_ref()
                    .and_then(|o| {
                        o.get::<_, String>("dataType").ok()
                    })
                    .map(|s| parse_fp_data_type(&s))
                    .unwrap_or_default();
                let scaling = opts
                    .0
                    .as_ref()
                    .and_then(|o| {
                        o.get::<_, String>("scaling").ok()
                    })
                    .map(|s| parse_fp_scaling(&s))
                    .unwrap_or_default();
                let candle_position = opts
                    .0
                    .as_ref()
                    .and_then(|o| {
                        o.get::<_, String>("candlePosition").ok()
                    })
                    .map(|s| parse_fp_candle_pos(&s))
                    .unwrap_or_default();

                let mut candles = Vec::new();
                if let Ok(candles_arr) =
                    fp_data.get::<_, rquickjs::Array<'_>>("candles")
                {
                    for i in 0..candles_arr.len() {
                        let Ok(cobj) =
                            candles_arr.get::<Object<'_>>(i)
                        else {
                            continue;
                        };

                        let x: f64 =
                            cobj.get("x").unwrap_or(0.0);
                        let open: f64 =
                            cobj.get("open").unwrap_or(0.0);
                        let high: f64 =
                            cobj.get("high").unwrap_or(0.0);
                        let low: f64 =
                            cobj.get("low").unwrap_or(0.0);
                        let close: f64 =
                            cobj.get("close").unwrap_or(0.0);
                        let poc_idx: Option<usize> = cobj
                            .get::<_, i32>("pocIndex")
                            .ok()
                            .and_then(|v| {
                                if v >= 0 {
                                    Some(v as usize)
                                } else {
                                    None
                                }
                            });

                        let mut levels = Vec::new();
                        if let Ok(levels_arr) = cobj
                            .get::<_, rquickjs::Array<'_>>(
                                "levels",
                            )
                        {
                            for j in 0..levels_arr.len() {
                                let Ok(lobj) = levels_arr
                                    .get::<Object<'_>>(j)
                                else {
                                    continue;
                                };
                                levels.push(FootprintLevelJS {
                                    price: lobj
                                        .get("price")
                                        .unwrap_or(0.0),
                                    buy_volume: lobj
                                        .get("buy")
                                        .unwrap_or(0.0),
                                    sell_volume: lobj
                                        .get("sell")
                                        .unwrap_or(0.0),
                                });
                            }
                        }

                        candles.push(FootprintCandleJS {
                            x,
                            open,
                            high,
                            low,
                            close,
                            levels,
                            poc_index: poc_idx,
                        });
                    }
                }

                coll.borrow_mut()
                    .commands
                    .push(PlotCommand::Footprint {
                        candles,
                        mode,
                        data_type,
                        scaling,
                        candle_position,
                    });
            },
        )?;
        globals.set("plotFootprint", plot_fp_fn)?;
    }

    Ok(collector)
}

/// Install stub plot functions for the declaration pass (no-ops).
pub fn install_plot_stubs(ctx: &Ctx<'_>) -> Result<(), ScriptError> {
    let globals = ctx.globals();

    globals.set(
        "plot",
        Function::new(ctx.clone(), |_s: Vec<f64>| -> i32 { 0 }),
    )?;
    globals.set(
        "plotBar",
        Function::new(ctx.clone(), |_s: Vec<f64>| {}),
    )?;
    globals.set(
        "plotBarColored",
        Function::new(ctx.clone(), |_v: Vec<f64>, _c: Vec<String>| {}),
    )?;
    globals.set(
        "plotHistogram",
        Function::new(ctx.clone(), |_s: Vec<f64>| {}),
    )?;
    globals.set(
        "plotHistogramColored",
        Function::new(ctx.clone(), |_v: Vec<f64>, _c: Vec<String>| {}),
    )?;
    globals.set(
        "marker",
        Function::new(ctx.clone(), |_t: f64, _p: f64| {}),
    )?;
    globals.set(
        "hline",
        Function::new(ctx.clone(), |_p: f64| {}),
    )?;
    globals.set(
        "fill",
        Function::new(ctx.clone(), |_a: i32, _b: i32| {}),
    )?;
    globals.set(
        "plotProfile",
        Function::new(
            ctx.clone(),
            |_data: rquickjs::Value<'_>| {},
        ),
    )?;
    globals.set(
        "plotFootprint",
        Function::new(
            ctx.clone(),
            |_data: rquickjs::Value<'_>| {},
        ),
    )?;

    Ok(())
}

fn parse_line_style(s: &str) -> LineStyleValue {
    match s.to_lowercase().as_str() {
        "dashed" => LineStyleValue::Dashed,
        "dotted" => LineStyleValue::Dotted,
        _ => LineStyleValue::Solid,
    }
}

fn parse_profile_side(s: &str) -> ProfileSide {
    match s.to_lowercase().as_str() {
        "right" => ProfileSide::Right,
        "both" => ProfileSide::Both,
        _ => ProfileSide::Left,
    }
}

fn parse_fp_render_mode(s: &str) -> FootprintRenderMode {
    match s.to_lowercase().as_str() {
        "box" => FootprintRenderMode::Box,
        _ => FootprintRenderMode::Profile,
    }
}

fn parse_fp_data_type(s: &str) -> FootprintDataType {
    match s.to_lowercase().as_str() {
        "bidask" | "bid_ask_split" | "bidasksplit" => {
            FootprintDataType::BidAskSplit
        }
        "delta" => FootprintDataType::Delta,
        "deltaandvolume" | "delta_and_volume" => {
            FootprintDataType::DeltaAndVolume
        }
        _ => FootprintDataType::Volume,
    }
}

fn parse_fp_scaling(s: &str) -> FootprintScaling {
    match s.to_lowercase().as_str() {
        "linear" => FootprintScaling::Linear,
        "log" => FootprintScaling::Log,
        "visiblerange" | "visible_range" => {
            FootprintScaling::VisibleRange
        }
        "datapoint" => FootprintScaling::Datapoint,
        _ => FootprintScaling::Sqrt,
    }
}

fn parse_fp_candle_pos(s: &str) -> FootprintCandlePosition {
    match s.to_lowercase().as_str() {
        "none" => FootprintCandlePosition::None,
        "center" => FootprintCandlePosition::Center,
        "right" => FootprintCandlePosition::Right,
        _ => FootprintCandlePosition::Left,
    }
}
