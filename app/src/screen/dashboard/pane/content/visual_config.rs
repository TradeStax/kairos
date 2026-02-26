use super::Content;
use crate::screen::dashboard::pane::config::VisualConfig;

impl Content {
    pub fn change_visual_config(&mut self, config: VisualConfig) {
        match (self, config) {
            #[cfg(feature = "heatmap")]
            (Content::Heatmap { chart: Some(c), .. }, VisualConfig::Heatmap(cfg)) => {
                // Convert data::HeatmapConfig to chart::heatmap::VisualConfig
                let visual = crate::chart::heatmap::VisualConfig {
                    order_size_filter: cfg.order_size_filter,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_size_scale: cfg.trade_size_scale,
                    trade_rendering_mode: crate::chart::heatmap::TradeRenderingMode::Auto,
                    max_trade_markers: 10_000,
                };
                c.set_visual_config(visual);
            }
            (Content::Candlestick { chart, .. }, VisualConfig::Kline(cfg)) => {
                if let Some(c) = (**chart).as_mut() {
                    c.set_candle_style(cfg.candle_style);
                    c.set_show_debug_info(cfg.show_debug_info);
                }
            }
            #[cfg(feature = "heatmap")]
            (Content::Ladder(Some(panel)), VisualConfig::Ladder(cfg)) => {
                // Convert state config to panel config
                panel.config = crate::screen::dashboard::ladder::Config {
                    levels: cfg.levels,
                    group_by_ticks: panel.config.group_by_ticks, // Preserve existing value
                    show_chase: panel.config.show_chase,         // Preserve existing value
                    show_chase_tracker: cfg.show_chase_tracker,
                    show_spread: cfg.show_spread,
                    trade_retention: std::time::Duration::from_secs(cfg.trade_retention_secs),
                };
            }
            (Content::Comparison(_), VisualConfig::Comparison(_cfg)) => {
                // ComparisonChart doesn't expose set_config for runtime changes
                // Config is set during construction
            }
            (Content::Profile { chart, .. }, VisualConfig::Profile(cfg)) => {
                if let Some(c) = (**chart).as_mut() {
                    c.set_display_config(*cfg);
                }
            }
            _ => {}
        }
    }
}
