use data::Candle;
use data::state::pane::CandleStyle;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Size};

use super::domain_to_exchange_price;

pub fn draw_footprint_candle(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    x_position: f32,
    candle_width: f32,
    candle: &Candle,
    palette: &Extended,
) {
    let y_open = price_to_y(domain_to_exchange_price(candle.open));
    let y_high = price_to_y(domain_to_exchange_price(candle.high));
    let y_low = price_to_y(domain_to_exchange_price(candle.low));
    let y_close = price_to_y(domain_to_exchange_price(candle.close));

    let body_color = if candle.close >= candle.open {
        palette.success.weak.color
    } else {
        palette.danger.weak.color
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 8.0), y_open.min(y_close)),
        Size::new(candle_width / 4.0, (y_open - y_close).abs()),
        body_color,
    );

    let wick_color = if candle.close >= candle.open {
        palette.success.weak.color
    } else {
        palette.danger.weak.color
    };
    let marker_line = Stroke::with_color(
        Stroke {
            width: 1.0,
            ..Default::default()
        },
        wick_color.scale_alpha(0.6),
    );
    frame.stroke(
        &Path::line(
            Point::new(x_position, y_high),
            Point::new(x_position, y_low),
        ),
        marker_line,
    );
}

/// Resolve the actual candle colors from the user's `CandleStyle` config,
/// falling back to the theme palette when a field is `None`.
struct ResolvedColors {
    bull_body: Color,
    bear_body: Color,
    bull_wick: Color,
    bear_wick: Color,
    bull_border: Option<Color>,
    bear_border: Option<Color>,
}

impl ResolvedColors {
    fn resolve(style: &CandleStyle, palette: &Extended) -> Self {
        Self {
            bull_body: style.bull_body_color.unwrap_or(palette.success.base.color),
            bear_body: style.bear_body_color.unwrap_or(palette.danger.base.color),
            bull_wick: style.bull_wick_color.unwrap_or(palette.success.base.color),
            bear_wick: style.bear_wick_color.unwrap_or(palette.danger.base.color),
            bull_border: style.bull_border_color,
            bear_border: style.bear_border_color,
        }
    }
}

pub fn draw_candle(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    candle_width: f32,
    palette: &Extended,
    style: &CandleStyle,
    x_position: f32,
    candle: &Candle,
) {
    let y_open = price_to_y(domain_to_exchange_price(candle.open));
    let y_high = price_to_y(domain_to_exchange_price(candle.high));
    let y_low = price_to_y(domain_to_exchange_price(candle.low));
    let y_close = price_to_y(domain_to_exchange_price(candle.close));

    let colors = ResolvedColors::resolve(style, palette);
    let is_bull = candle.close >= candle.open;

    let body_color = if is_bull {
        colors.bull_body
    } else {
        colors.bear_body
    };

    let body_x = x_position - (candle_width / 2.0);
    let body_y = y_open.min(y_close);
    let body_h = (y_open - y_close).abs().max(1.0);

    // Fill body
    frame.fill_rectangle(
        Point::new(body_x, body_y),
        Size::new(candle_width, body_h),
        body_color,
    );

    // Border (optional)
    let border_color = if is_bull {
        colors.bull_border
    } else {
        colors.bear_border
    };
    if let Some(border) = border_color {
        let border_stroke = Stroke::with_color(
            Stroke {
                width: 1.0,
                ..Default::default()
            },
            border,
        );
        frame.stroke(
            &Path::rectangle(Point::new(body_x, body_y), Size::new(candle_width, body_h)),
            border_stroke,
        );
    }

    // Wick
    let wick_color = if is_bull {
        colors.bull_wick
    } else {
        colors.bear_wick
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 8.0), y_high),
        Size::new(candle_width / 4.0, (y_high - y_low).abs()),
        wick_color,
    );
}
