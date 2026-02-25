//! Declarative macros for reducing Chart trait boilerplate.
//!
//! The `chart_impl!()` macro generates the standard boilerplate `Chart` trait
//! method implementations for chart structs with the conventional field layout:
//! - `chart: ViewState`
//! - `drawings: DrawingManager`
//! - `studies: Vec<Box<dyn study::Study>>`
//! - `panel_cache: Cache`, `panel_labels_cache: Cache`, `panel_crosshair_cache: Cache`

/// Generate standard `Chart` trait boilerplate method bodies.
///
/// Expands to the seven standard boilerplate method implementations:
/// `state`, `mut_state`, `drawings`, `studies`, `panel_cache`,
/// `panel_labels_cache`, `panel_crosshair_cache`.
///
/// Place this invocation INSIDE an existing `impl Chart for YourChart { ... }` block.
/// Chart-specific required methods (`invalidate_all`, `invalidate_crosshair`,
/// `interval_keys`, `autoscaled_coords`, `is_empty`, `plot_limits`,
/// `supports_fit_autoscaling`) must still be written manually alongside it.
///
/// # Usage
/// ```rust,ignore
/// impl Chart for ProfileChart {
///     chart_impl!(ProfileChart);
///
///     fn invalidate_all(&mut self) { self.invalidate(); }
///     fn invalidate_crosshair(&mut self) { ... }
///     // ... other required methods
/// }
/// ```
#[macro_export]
macro_rules! chart_impl {
    ($ty:ty) => {
        fn state(&self) -> &$crate::chart::ViewState {
            &self.chart
        }
        fn mut_state(&mut self) -> &mut $crate::chart::ViewState {
            &mut self.chart
        }
        fn drawings(&self) -> Option<&$crate::chart::drawing::DrawingManager> {
            Some(&self.drawings)
        }
        fn studies(&self) -> &[Box<dyn study::Study>] {
            &self.studies
        }
        fn panel_cache(&self) -> Option<&iced::widget::canvas::Cache> {
            Some(&self.panel_cache)
        }
        fn panel_labels_cache(&self) -> Option<&iced::widget::canvas::Cache> {
            Some(&self.panel_labels_cache)
        }
        fn panel_crosshair_cache(&self) -> Option<&iced::widget::canvas::Cache> {
            Some(&self.panel_crosshair_cache)
        }
    };
}
