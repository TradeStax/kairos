//! Chart interaction tools: add_horizontal_line, add_text_annotation,
//! add_price_level, add_rectangle, add_arrow, remove_drawing

use serde_json::{Value, json};

use crate::app::core::globals::AiStreamEventClone;

use super::{
    ToolContext, ToolExecResult, parse_color_name, parse_iso_to_millis,
    parse_line_style,
};

/// Drawing action pushed through the event buffer to be handled on
/// the main thread.
#[derive(Debug, Clone)]
pub enum AiDrawingAction {
    AddDrawing {
        drawing: data::SerializableDrawing,
        description: String,
    },
    RemoveDrawing {
        id: String,
        description: String,
    },
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "add_horizontal_line",
                "description": "Draw a horizontal price level line \
                    on the chart. Use to mark key levels like \
                    support, resistance, POC, VAH, VAL.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "price": {
                            "type": "number",
                            "description": "Price level (required)"
                        },
                        "label": {
                            "type": "string",
                            "description": "Label text for the line"
                        },
                        "color": {
                            "type": "string",
                            "description": "Color name: red, green, \
                                blue, yellow, orange, purple, white, \
                                gray (default: blue)"
                        },
                        "style": {
                            "type": "string",
                            "enum": ["solid", "dashed", "dotted"],
                            "description": "Line style (default: \
                                dashed)"
                        }
                    },
                    "required": ["price"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "add_text_annotation",
                "description": "Place a text label at a specific \
                    price and time on the chart.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "price": {
                            "type": "number",
                            "description": "Price level (required)"
                        },
                        "time": {
                            "type": "string",
                            "description": "ISO 8601 time (required)"
                        },
                        "text": {
                            "type": "string",
                            "description": "Text to display \
                                (max 50 chars, required)"
                        },
                        "color": {
                            "type": "string",
                            "description": "Color name (default: \
                                white)"
                        }
                    },
                    "required": ["price", "time", "text"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "add_price_level",
                "description": "Add a labeled price level marker \
                    on the chart. Similar to horizontal line but \
                    with a prominent label.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "price": {
                            "type": "number",
                            "description": "Price level (required)"
                        },
                        "label": {
                            "type": "string",
                            "description": "Label text (required)"
                        },
                        "color": {
                            "type": "string",
                            "description": "Color name (default: \
                                yellow)"
                        }
                    },
                    "required": ["price", "label"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "add_rectangle",
                "description": "Draw a highlighted rectangular zone \
                    on the chart. Use to mark areas like value area, \
                    opening range, consolidation zones.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "price_high": {
                            "type": "number",
                            "description": "Top price (required)"
                        },
                        "price_low": {
                            "type": "number",
                            "description": "Bottom price (required)"
                        },
                        "time_start": {
                            "type": "string",
                            "description": "ISO 8601 left time \
                                (required)"
                        },
                        "time_end": {
                            "type": "string",
                            "description": "ISO 8601 right time \
                                (required)"
                        },
                        "label": {
                            "type": "string",
                            "description": "Label text"
                        },
                        "color": {
                            "type": "string",
                            "description": "Color name (default: \
                                blue)"
                        },
                        "opacity": {
                            "type": "number",
                            "description": "Fill opacity 0.0-1.0 \
                                (default: 0.15)"
                        }
                    },
                    "required": [
                        "price_high", "price_low",
                        "time_start", "time_end"
                    ],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "add_arrow",
                "description": "Draw an arrow between two points \
                    on the chart. Use to indicate direction or \
                    highlight moves.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "from_price": {
                            "type": "number",
                            "description": "Start price (required)"
                        },
                        "from_time": {
                            "type": "string",
                            "description": "ISO 8601 start time \
                                (required)"
                        },
                        "to_price": {
                            "type": "number",
                            "description": "End price (required)"
                        },
                        "to_time": {
                            "type": "string",
                            "description": "ISO 8601 end time \
                                (required)"
                        },
                        "color": {
                            "type": "string",
                            "description": "Color name (default: \
                                yellow)"
                        }
                    },
                    "required": [
                        "from_price", "from_time",
                        "to_price", "to_time"
                    ],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "remove_drawing",
                "description": "Remove a drawing from the chart \
                    by its ID. Use get_drawings first to find IDs.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "drawing_id": {
                            "type": "string",
                            "description": "UUID of the drawing \
                                to remove (required)"
                        }
                    },
                    "required": ["drawing_id"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

/// Push a drawing action through the event sender.
fn push_drawing_action(
    ctx: &ToolContext<'_>,
    action: AiDrawingAction,
) {
    let _ = ctx.sender.send(AiStreamEventClone::DrawingAction {
        conversation_id: ctx.conversation_id,
        action,
    });
}

/// Convert f64 price to fixed-point i64 units (Price type).
fn price_to_units(price: f64) -> i64 {
    data::Price::from_f64(price).units()
}

/// Convert a time string to milliseconds (SerializablePoint uses ms).
/// Accepts ISO 8601, epoch seconds, or epoch milliseconds.
fn time_to_ms(s: &str) -> Option<u64> {
    // Try ISO 8601 first
    if let Some(ms) = parse_iso_to_millis(s) {
        return Some(ms);
    }
    // Try epoch seconds / milliseconds (AI might pass numeric strings)
    if let Ok(n) = s.trim().parse::<u64>() {
        return if n > 1_000_000_000_000 {
            // Already milliseconds (after year ~2001)
            Some(n)
        } else if n > 1_000_000_000 {
            // Seconds
            Some(n * 1000)
        } else {
            None // Too small to be a valid timestamp
        };
    }
    None
}

/// Dispatch drawing tool calls.
pub fn execute_drawing_tool(
    name: &str,
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    log::debug!("Drawing tool '{}' args: {}", name, args);
    match name {
        "add_horizontal_line" => exec_add_horizontal_line(args, ctx),
        "add_text_annotation" => {
            exec_add_text_annotation(args, ctx)
        }
        "add_price_level" => exec_add_price_level(args, ctx),
        "add_rectangle" => exec_add_rectangle(args, ctx),
        "add_arrow" => exec_add_arrow(args, ctx),
        "remove_drawing" => exec_remove_drawing(args, ctx),
        _ => ToolExecResult {
            content_json: json!({
                "error": format!("Unknown drawing tool: {}", name)
            })
            .to_string(),
            display_summary: format!("Unknown: {}", name),
            is_error: true,
        },
    }
}

fn exec_add_horizontal_line(
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    let Some(price) = args["price"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: price"
            })
            .to_string(),
            display_summary: "Missing price".to_string(),
            is_error: true,
        };
    };

    let color_name =
        args["color"].as_str().unwrap_or("blue");
    let style_name =
        args["style"].as_str().unwrap_or("dashed");
    let label = args["label"].as_str().map(String::from);

    let color = parse_color_name(color_name);
    let line_style = parse_line_style(style_name);

    let mut drawing =
        data::SerializableDrawing::new(data::DrawingTool::HorizontalLine);
    drawing.points = vec![data::SerializablePoint::new(
        price_to_units(price),
        0,
    )];
    drawing.style.stroke_color = color;
    drawing.style.line_style = line_style;
    drawing.style.stroke_width = 1.5;
    drawing.label = label;
    let id = drawing.id.0.to_string();

    push_drawing_action(
        ctx,
        AiDrawingAction::AddDrawing {
            drawing,
            description: format!(
                "H-Line at {:.2}",
                price
            ),
        },
    );

    ToolExecResult {
        content_json: json!({
            "success": true,
            "drawing_id": id,
            "price": price,
        })
        .to_string(),
        display_summary: format!("H-Line at {:.2}", price),
        is_error: false,
    }
}

fn exec_add_text_annotation(
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    let Some(price) = args["price"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: price"
            })
            .to_string(),
            display_summary: "Missing price".to_string(),
            is_error: true,
        };
    };
    let Some(time_str) = args["time"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: time"
            })
            .to_string(),
            display_summary: "Missing time".to_string(),
            is_error: true,
        };
    };
    let Some(text) = args["text"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: text"
            })
            .to_string(),
            display_summary: "Missing text".to_string(),
            is_error: true,
        };
    };

    let text = if text.len() > 50 {
        &text[..50]
    } else {
        text
    };

    let color_name = args["color"].as_str().unwrap_or("white");
    let color = parse_color_name(color_name);

    let Some(time_ms) = time_to_ms(time_str) else {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "Could not parse time '{}'. Use ISO 8601 \
                     (e.g. 2024-01-15T14:30:00Z) or epoch seconds.",
                    time_str
                )
            })
            .to_string(),
            display_summary: "Bad time format".to_string(),
            is_error: true,
        };
    };

    let mut drawing =
        data::SerializableDrawing::new(data::DrawingTool::TextLabel);
    drawing.points = vec![data::SerializablePoint::new(
        price_to_units(price),
        time_ms,
    )];
    drawing.style.stroke_color = color;
    drawing.style.text = Some(text.to_string());
    let id = drawing.id.0.to_string();

    push_drawing_action(
        ctx,
        AiDrawingAction::AddDrawing {
            drawing,
            description: format!("Text: {}", text),
        },
    );

    ToolExecResult {
        content_json: json!({
            "success": true,
            "drawing_id": id,
        })
        .to_string(),
        display_summary: format!("Text: {}", text),
        is_error: false,
    }
}

fn exec_add_price_level(
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    let Some(price) = args["price"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: price"
            })
            .to_string(),
            display_summary: "Missing price".to_string(),
            is_error: true,
        };
    };
    let Some(label) = args["label"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: label"
            })
            .to_string(),
            display_summary: "Missing label".to_string(),
            is_error: true,
        };
    };

    let color_name = args["color"].as_str().unwrap_or("yellow");
    let color = parse_color_name(color_name);

    // Use a Ray anchored at the latest candle time, extending right.
    let anchor_ms = ctx
        .snapshot
        .as_ref()
        .and_then(|s| s.candles.last())
        .map(|c| c.time.0)
        .unwrap_or(0);
    let units = price_to_units(price);

    let mut drawing = data::SerializableDrawing::new(
        data::DrawingTool::Ray,
    );
    // Two points at the same price = horizontal ray from anchor
    drawing.points = vec![
        data::SerializablePoint::new(units, anchor_ms),
        data::SerializablePoint::new(units, anchor_ms + 1),
    ];
    drawing.style.stroke_color = color;
    drawing.style.line_style = data::LineStyle::Dashed;
    drawing.style.stroke_width = 1.5;
    drawing.label = Some(label.to_string());
    let id = drawing.id.0.to_string();

    push_drawing_action(
        ctx,
        AiDrawingAction::AddDrawing {
            drawing,
            description: format!("Level: {} at {:.2}", label, price),
        },
    );

    ToolExecResult {
        content_json: json!({
            "success": true,
            "drawing_id": id,
            "price": price,
        })
        .to_string(),
        display_summary: format!("{} at {:.2}", label, price),
        is_error: false,
    }
}

fn exec_add_rectangle(
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    let Some(price_high) = args["price_high"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: price_high"
            })
            .to_string(),
            display_summary: "Missing price_high".to_string(),
            is_error: true,
        };
    };
    let Some(price_low) = args["price_low"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: price_low"
            })
            .to_string(),
            display_summary: "Missing price_low".to_string(),
            is_error: true,
        };
    };
    let Some(time_start) = args["time_start"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: time_start"
            })
            .to_string(),
            display_summary: "Missing time_start".to_string(),
            is_error: true,
        };
    };
    let Some(time_end) = args["time_end"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: time_end"
            })
            .to_string(),
            display_summary: "Missing time_end".to_string(),
            is_error: true,
        };
    };

    let color_name = args["color"].as_str().unwrap_or("blue");
    let opacity = args["opacity"]
        .as_f64()
        .unwrap_or(0.15)
        .clamp(0.0, 1.0) as f32;
    let label = args["label"].as_str().map(String::from);
    let color = parse_color_name(color_name);

    let Some(start_ms) = time_to_ms(time_start) else {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "Could not parse time_start '{}'. Use ISO 8601 \
                     (e.g. 2024-01-15T14:30:00Z) or epoch seconds.",
                    time_start
                )
            })
            .to_string(),
            display_summary: "Bad time_start".to_string(),
            is_error: true,
        };
    };
    let Some(end_ms) = time_to_ms(time_end) else {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "Could not parse time_end '{}'. Use ISO 8601 \
                     (e.g. 2024-01-15T14:30:00Z) or epoch seconds.",
                    time_end
                )
            })
            .to_string(),
            display_summary: "Bad time_end".to_string(),
            is_error: true,
        };
    };

    let mut drawing =
        data::SerializableDrawing::new(data::DrawingTool::Rectangle);
    drawing.points = vec![
        data::SerializablePoint::new(
            price_to_units(price_high),
            start_ms,
        ),
        data::SerializablePoint::new(
            price_to_units(price_low),
            end_ms,
        ),
    ];
    drawing.style.stroke_color = color;
    drawing.style.fill_color = Some(color);
    drawing.style.fill_opacity = opacity;
    drawing.style.stroke_width = 1.0;
    drawing.label = label.clone();
    let id = drawing.id.0.to_string();

    let desc = label
        .as_deref()
        .map(|l| format!("Rect: {}", l))
        .unwrap_or_else(|| {
            format!("Rect {:.2}-{:.2}", price_low, price_high)
        });

    push_drawing_action(
        ctx,
        AiDrawingAction::AddDrawing {
            drawing,
            description: desc.clone(),
        },
    );

    ToolExecResult {
        content_json: json!({
            "success": true,
            "drawing_id": id,
        })
        .to_string(),
        display_summary: desc,
        is_error: false,
    }
}

fn exec_add_arrow(
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    let Some(from_price) = args["from_price"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: from_price"
            })
            .to_string(),
            display_summary: "Missing from_price".to_string(),
            is_error: true,
        };
    };
    let Some(from_time) = args["from_time"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: from_time"
            })
            .to_string(),
            display_summary: "Missing from_time".to_string(),
            is_error: true,
        };
    };
    let Some(to_price) = args["to_price"].as_f64() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: to_price"
            })
            .to_string(),
            display_summary: "Missing to_price".to_string(),
            is_error: true,
        };
    };
    let Some(to_time) = args["to_time"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: to_time"
            })
            .to_string(),
            display_summary: "Missing to_time".to_string(),
            is_error: true,
        };
    };

    let color_name = args["color"].as_str().unwrap_or("yellow");
    let color = parse_color_name(color_name);

    let Some(from_ms) = time_to_ms(from_time) else {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "Could not parse from_time '{}'. Use ISO 8601 \
                     (e.g. 2024-01-15T14:30:00Z) or epoch seconds.",
                    from_time
                )
            })
            .to_string(),
            display_summary: "Bad from_time".to_string(),
            is_error: true,
        };
    };
    let Some(to_ms) = time_to_ms(to_time) else {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "Could not parse to_time '{}'. Use ISO 8601 \
                     (e.g. 2024-01-15T14:30:00Z) or epoch seconds.",
                    to_time
                )
            })
            .to_string(),
            display_summary: "Bad to_time".to_string(),
            is_error: true,
        };
    };

    let mut drawing =
        data::SerializableDrawing::new(data::DrawingTool::Arrow);
    drawing.points = vec![
        data::SerializablePoint::new(
            price_to_units(from_price),
            from_ms,
        ),
        data::SerializablePoint::new(
            price_to_units(to_price),
            to_ms,
        ),
    ];
    drawing.style.stroke_color = color;
    drawing.style.arrow_head_end = true;
    drawing.style.stroke_width = 2.0;
    let id = drawing.id.0.to_string();

    push_drawing_action(
        ctx,
        AiDrawingAction::AddDrawing {
            drawing,
            description: format!(
                "Arrow {:.2} → {:.2}",
                from_price, to_price
            ),
        },
    );

    ToolExecResult {
        content_json: json!({
            "success": true,
            "drawing_id": id,
        })
        .to_string(),
        display_summary: format!(
            "Arrow {:.2} → {:.2}",
            from_price, to_price
        ),
        is_error: false,
    }
}

fn exec_remove_drawing(
    args: &Value,
    ctx: &ToolContext<'_>,
) -> ToolExecResult {
    let Some(drawing_id) = args["drawing_id"].as_str() else {
        return ToolExecResult {
            content_json: json!({
                "error": "Missing required parameter: drawing_id"
            })
            .to_string(),
            display_summary: "Missing drawing_id".to_string(),
            is_error: true,
        };
    };

    push_drawing_action(
        ctx,
        AiDrawingAction::RemoveDrawing {
            id: drawing_id.to_string(),
            description: format!(
                "Remove drawing {}",
                &drawing_id[..8.min(drawing_id.len())]
            ),
        },
    );

    ToolExecResult {
        content_json: json!({
            "success": true,
            "drawing_id": drawing_id,
        })
        .to_string(),
        display_summary: format!(
            "Removed {}",
            &drawing_id[..8.min(drawing_id.len())]
        ),
        is_error: false,
    }
}
