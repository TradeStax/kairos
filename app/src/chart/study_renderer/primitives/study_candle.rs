//! Study candle renderer — OHLC mini-candlesticks for study output.
//!
//! Panel-placement rendering is handled by `panel::render_panel_study_candles`.
//! This module provides the overlay dispatch entry point (currently a no-op
//! since all StudyCandle studies use Panel placement).

use crate::chart::ViewState;
use iced::Size;
use iced::widget::canvas::Frame;
use study::StudyPlacement;
use study::output::StudyCandleSeries;

/// Render study candle series onto a chart canvas frame.
///
/// Panel studies are rendered via `panel.rs`; this function handles the
/// overlay dispatch case. Currently a no-op as no overlay-placement
/// studies use StudyCandles output.
pub fn render_study_candles(
    _frame: &mut Frame,
    _candle_series: &[StudyCandleSeries],
    _state: &ViewState,
    _bounds: Size,
    _placement: StudyPlacement,
) {
    // Panel rendering is handled by render_panel_study_candles() in panel.rs.
    // Overlay StudyCandles rendering can be added here if needed in the future.
}
