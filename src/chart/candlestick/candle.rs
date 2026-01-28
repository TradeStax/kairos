use data::Candle;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Path, Stroke};
use iced::{Point, Size};

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

pub fn draw_candle(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    candle_width: f32,
    palette: &Extended,
    x_position: f32,
    candle: &Candle,
) {
    let y_open = price_to_y(domain_to_exchange_price(candle.open));
    let y_high = price_to_y(domain_to_exchange_price(candle.high));
    let y_low = price_to_y(domain_to_exchange_price(candle.low));
    let y_close = price_to_y(domain_to_exchange_price(candle.close));

    let body_color = if candle.close >= candle.open {
        palette.success.base.color
    } else {
        palette.danger.base.color
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 2.0), y_open.min(y_close)),
        Size::new(candle_width, (y_open - y_close).abs()),
        body_color,
    );

    let wick_color = if candle.close >= candle.open {
        palette.success.base.color
    } else {
        palette.danger.base.color
    };
    frame.fill_rectangle(
        Point::new(x_position - (candle_width / 8.0), y_high),
        Size::new(candle_width / 4.0, (y_high - y_low).abs()),
        wick_color,
    );
}
