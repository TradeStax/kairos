//! Built-in study registration.
//!
//! Registers all 18 built-in studies into the [`StudyRegistry`] at
//! construction time. Each entry derives its [`StudyInfo`] automatically
//! from the study's own [`StudyMetadata`].

use super::StudyRegistry;

/// Register all built-in studies into the given registry.
///
/// Called once by [`StudyRegistry::new()`] during construction.
pub(super) fn register_built_ins(registry: &mut StudyRegistry) {
    // ── Volume ───────────────────────────────────────────
    registry.register_study(|| Box::new(super::volume::VolumeStudy::new()));
    registry.register_study(|| Box::new(super::volume::DeltaStudy::new()));
    registry.register_study(|| Box::new(super::volume::CvdStudy::new()));
    registry.register_study(|| Box::new(super::volume::ObvStudy::new()));

    // ── Trend ────────────────────────────────────────────
    registry.register_study(|| Box::new(super::trend::SmaStudy::new()));
    registry.register_study(|| Box::new(super::trend::EmaStudy::new()));
    registry.register_study(|| Box::new(super::trend::VwapStudy::new()));

    // ── Momentum ─────────────────────────────────────────
    registry.register_study(|| Box::new(super::momentum::RsiStudy::new()));
    registry.register_study(|| Box::new(super::momentum::MacdStudy::new()));
    registry.register_study(|| Box::new(super::momentum::StochasticStudy::new()));

    // ── Volatility ───────────────────────────────────────
    registry.register_study(|| Box::new(super::volatility::AtrStudy::new()));
    registry.register_study(|| Box::new(super::volatility::BollingerStudy::new()));

    // ── Statistical ─────────────────────────────────────
    registry.register_study(|| Box::new(super::statistical::IvbStudy::new()));

    // ── Order Flow ───────────────────────────────────────
    registry.register_study(|| Box::new(super::orderflow::ImbalanceStudy::new()));
    registry.register_study(|| Box::new(super::orderflow::BigTradesStudy::new()));
    registry.register_study(|| Box::new(super::orderflow::FootprintStudy::new()));
    registry.register_study(|| Box::new(super::orderflow::VbpStudy::new()));
    registry.register_study(|| Box::new(super::orderflow::SpeedOfTapeStudy::new()));
    registry.register_study(|| Box::new(super::orderflow::LevelAnalyzerStudy::new()));
}
