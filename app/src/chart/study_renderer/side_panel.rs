//! Side Panel Canvas
//!
//! Renders side-panel-placement studies (VBP cumulative profile, etc.)
//! in a separate vertical canvas to the right of the main chart.
//! The Y axis is shared with the main chart (same price scale), so
//! volume bars are drawn as horizontal bars whose heights align with
//! the price levels shown on the shared Y-axis labels.

use super::chart_views::{SidePanelChartView, theme_from_palette};
use super::iced_canvas::IcedCanvas;
use crate::chart::core::SidePanelStudyInfo;
use crate::chart::{Message, ViewState};
use iced::widget::canvas::{self, Cache, Event, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};

/// Canvas program that renders side-panel studies (horizontal VBP bars)
/// sharing the main chart's price Y-axis.
pub struct SidePanelCanvas<'a> {
    pub studies: Vec<SidePanelStudyInfo<'a>>,
    pub state: &'a ViewState,
    pub cache: &'a Cache,
    pub crosshair_cache: &'a Cache,
}

impl<'a> canvas::Program<Message> for SidePanelCanvas<'a> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let y = cursor.position_in(bounds).map(|p| p.y);
                Some(canvas::Action::publish(Message::SidePanelCrosshairMoved(y)))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let theme_colors = theme_from_palette(palette);

        let content = self.cache.draw(renderer, bounds.size(), |frame| {
            draw_side_panel_content(
                frame,
                &self.studies,
                self.state,
                bounds.size(),
                theme_colors,
            );
        });

        let crosshair = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
            if let Some(y) = self.state.crosshair.y.get() {
                draw_crosshair(frame, y, bounds.size());
            }
        });

        vec![content, crosshair]
    }
}

// ── Content rendering ─────────────────────────────────────────────────

fn draw_side_panel_content(
    frame: &mut Frame,
    studies: &[SidePanelStudyInfo<'_>],
    state: &ViewState,
    bounds: Size,
    theme_colors: study::output::render::ThemeColors,
) {
    use study::StudyOutput;

    for info in studies {
        if let StudyOutput::Profile(profiles, config) = info.output {
            let view = SidePanelChartView::new(state, bounds, theme_colors);
            let mut canvas = IcedCanvas::new(frame);
            for profile in profiles {
                study::output::render::vbp::side_panel::render_side_panel_bars(
                    &mut canvas,
                    profile,
                    config,
                    &view,
                );
            }
        }
    }
}

// ── Crosshair ─────────────────────────────────────────────────────────

fn draw_crosshair(frame: &mut Frame, y: f32, bounds: Size) {
    if y < 0.0 || y > bounds.height {
        return;
    }
    let line = Path::line(Point::new(0.0, y), Point::new(bounds.width, y));
    frame.stroke(
        &line,
        Stroke::default()
            .with_color(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.35,
            })
            .with_width(1.0),
    );
}
