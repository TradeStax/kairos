//! Conditional filtering for IVB session records.

use super::session_record::IvbSessionRecord;

/// OR range regime classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RangeRegime {
    Narrow,
    Normal,
    Wide,
}

/// Gap regime classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GapRegime {
    GapUp,
    GapDown,
    NoGap,
}

/// Volume/range regime classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VolRegime {
    Low,
    Normal,
    High,
}

/// Conditional filter for stratifying historical records.
#[derive(Debug, Clone)]
pub struct ConditionalFilter {
    pub range_regime: Option<RangeRegime>,
    pub day_of_week: Option<u8>,
    pub high_formed_first: Option<bool>,
    pub gap_regime: Option<GapRegime>,
    pub vol_regime: Option<VolRegime>,
    pub narrow_pct: f64,
    pub wide_pct: f64,
}

impl ConditionalFilter {
    /// Build filter from current session's context vs historical.
    pub fn from_current_session(
        current_or_range: i64,
        records: &[&IvbSessionRecord],
        narrow_pct: f64,
        wide_pct: f64,
        current_day_of_week: Option<u8>,
        current_high_first: Option<bool>,
        current_overnight_gap: Option<i64>,
        current_session_range: Option<i64>,
        gap_threshold_pct: f64,
    ) -> Self {
        let mut or_ranges: Vec<f64> = records.iter().map(|r| r.or_range_units as f64).collect();
        or_ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let range_regime = if or_ranges.is_empty() {
            None
        } else {
            let rank = crate::util::math::percentile_rank(&or_ranges, current_or_range as f64);
            Some(classify_range_from_rank(rank, narrow_pct, wide_pct))
        };

        // Gap regime
        let gap_regime = current_overnight_gap.and_then(|gap| {
            if current_or_range <= 0 {
                return None;
            }
            let ratio = (gap as f64).abs() / current_or_range as f64;
            Some(if ratio > gap_threshold_pct {
                if gap > 0 {
                    GapRegime::GapUp
                } else {
                    GapRegime::GapDown
                }
            } else {
                GapRegime::NoGap
            })
        });

        // Vol regime from session range percentile
        let vol_regime = current_session_range.map(|sr| {
            let mut session_ranges: Vec<f64> = records
                .iter()
                .map(|r| r.session_range_units as f64)
                .collect();
            session_ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let rank = crate::util::math::percentile_rank(&session_ranges, sr as f64);
            classify_vol_from_rank(rank, narrow_pct, wide_pct)
        });

        Self {
            range_regime,
            day_of_week: current_day_of_week,
            high_formed_first: current_high_first,
            gap_regime,
            vol_regime,
            narrow_pct,
            wide_pct,
        }
    }

    /// Apply filter with priority-based progressive relaxation.
    ///
    /// Features are sorted by importance (highest priority kept
    /// longest). Lowest-priority features are dropped first.
    /// `range_regime` is never dropped — if it alone doesn't meet
    /// `min_samples`, we fall back to unfiltered.
    pub fn apply<'a>(
        &self,
        records: &[&'a IvbSessionRecord],
        min_samples: usize,
    ) -> (Vec<&'a IvbSessionRecord>, Vec<String>) {
        // Build features with priority weights.
        // Higher priority = kept longer during relaxation.
        let mut active: Vec<(&str, Feature, u8)> = Vec::new();
        if let Some(regime) = self.range_regime {
            active.push((
                "range_regime",
                Feature::Range(regime, self.narrow_pct, self.wide_pct),
                5,
            ));
        }
        if let Some(dow) = self.day_of_week {
            active.push(("day_of_week", Feature::DayOfWeek(dow), 3));
        }
        if let Some(hff) = self.high_formed_first {
            active.push(("high_formed_first", Feature::HighFirst(hff), 2));
        }
        if let Some(regime) = self.gap_regime {
            active.push(("gap_regime", Feature::Gap(regime), 2));
        }
        if let Some(regime) = self.vol_regime {
            active.push((
                "vol_regime",
                Feature::Vol(regime, self.narrow_pct, self.wide_pct),
                1,
            ));
        }

        if active.is_empty() {
            return (records.to_vec(), Vec::new());
        }

        // Sort by priority descending so highest-priority features
        // are at the front and lowest at the back.
        active.sort_by(|a, b| b.2.cmp(&a.2));

        // Progressive relaxation: try all, then drop lowest-
        // priority (from back). Never drop range_regime.
        for drop_count in 0..active.len() {
            let count = active.len() - drop_count;
            let subset = &active[..count];

            let filtered: Vec<&'a IvbSessionRecord> = records
                .iter()
                .filter(|r| subset.iter().all(|(_, feat, _)| feat.matches(r, records)))
                .copied()
                .collect();

            if filtered.len() >= min_samples {
                let names = subset
                    .iter()
                    .map(|(name, _, _)| (*name).to_string())
                    .collect();
                return (filtered, names);
            }

            // Don't drop range_regime — it's always the first
            // element after sorting. If we'd drop it next, stop.
            if count <= 1 || subset.last().is_some_and(|(n, _, _)| *n == "range_regime") {
                break;
            }
        }

        // Fallback: return all records
        (records.to_vec(), Vec::new())
    }
}

/// A feature used for filtering.
#[derive(Debug, Clone, Copy)]
enum Feature {
    Range(RangeRegime, f64, f64), // regime, narrow, wide
    DayOfWeek(u8),
    HighFirst(bool),
    Gap(GapRegime),
    Vol(VolRegime, f64, f64), // regime, narrow, wide
}

impl Feature {
    fn matches(&self, record: &IvbSessionRecord, all: &[&IvbSessionRecord]) -> bool {
        match *self {
            Feature::Range(regime, narrow, wide) => {
                classify_record_range(record, all, narrow, wide) == regime
            }
            Feature::DayOfWeek(dow) => record.day_of_week == dow,
            Feature::HighFirst(hff) => record.or_high_formed_first == hff,
            Feature::Gap(regime) => classify_record_gap(record) == regime,
            Feature::Vol(regime, narrow, wide) => {
                classify_record_vol(record, all, narrow, wide) == regime
            }
        }
    }
}

fn classify_range_from_rank(rank: f64, narrow_pct: f64, wide_pct: f64) -> RangeRegime {
    if rank <= narrow_pct {
        RangeRegime::Narrow
    } else if rank >= wide_pct {
        RangeRegime::Wide
    } else {
        RangeRegime::Normal
    }
}

fn classify_vol_from_rank(rank: f64, narrow_pct: f64, wide_pct: f64) -> VolRegime {
    if rank <= narrow_pct {
        VolRegime::Low
    } else if rank >= wide_pct {
        VolRegime::High
    } else {
        VolRegime::Normal
    }
}

fn classify_record_range(
    record: &IvbSessionRecord,
    all: &[&IvbSessionRecord],
    narrow_pct: f64,
    wide_pct: f64,
) -> RangeRegime {
    let mut ranges: Vec<f64> = all.iter().map(|r| r.or_range_units as f64).collect();
    ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let rank = crate::util::math::percentile_rank(&ranges, record.or_range_units as f64);
    classify_range_from_rank(rank, narrow_pct, wide_pct)
}

fn classify_record_gap(record: &IvbSessionRecord) -> GapRegime {
    if record.or_range_units <= 0 {
        return GapRegime::NoGap;
    }
    let ratio = (record.overnight_gap_units as f64).abs() / record.or_range_units as f64;
    if ratio > 0.5 {
        if record.overnight_gap_units > 0 {
            GapRegime::GapUp
        } else {
            GapRegime::GapDown
        }
    } else {
        GapRegime::NoGap
    }
}

fn classify_record_vol(
    record: &IvbSessionRecord,
    all: &[&IvbSessionRecord],
    narrow_pct: f64,
    wide_pct: f64,
) -> VolRegime {
    let mut ranges: Vec<f64> = all.iter().map(|r| r.session_range_units as f64).collect();
    ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let rank = crate::util::math::percentile_rank(&ranges, record.session_range_units as f64);
    classify_vol_from_rank(rank, narrow_pct, wide_pct)
}
