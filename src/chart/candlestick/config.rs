use data::KlineChartKind;

impl super::ConfigConstants for KlineChartKind {
    fn min_scaling(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 0.05,
            KlineChartKind::Candles => 0.1,
        }
    }

    fn max_scaling(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 2.0,
            KlineChartKind::Candles => 5.0,
        }
    }

    fn max_cell_width(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 500.0,
            KlineChartKind::Candles => 100.0,
        }
    }

    fn min_cell_width(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 10.0,
            KlineChartKind::Candles => 1.0,
        }
    }

    fn max_cell_height(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 100.0,
            KlineChartKind::Candles => 200.0,
        }
    }

    fn min_cell_height(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 1.0,
            KlineChartKind::Candles => 1.0,
        }
    }

    fn default_cell_width(&self) -> f32 {
        match self {
            KlineChartKind::Footprint { .. } => 80.0,
            KlineChartKind::Candles => 4.0,
        }
    }
}
