use crate::output::trade_record::TradeRecord;
use crate::portfolio::equity::EquityCurve;
use serde::{Deserialize, Serialize};

/// Aggregated performance statistics for a completed backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // ── P&L ────────────────────────────────────────────────────────────
    pub net_pnl_usd: f64,
    pub gross_pnl_usd: f64,
    pub total_commission_usd: f64,
    pub net_pnl_ticks: i64,

    // ── Trade counts ───────────────────────────────────────────────────
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub breakeven_trades: usize,

    // ── Win/loss statistics ────────────────────────────────────────────
    pub win_rate: f64,
    pub avg_win_usd: f64,
    pub avg_loss_usd: f64,
    pub profit_factor: f64,
    pub avg_rr: f64,
    pub best_trade_usd: f64,
    pub worst_trade_usd: f64,
    pub largest_win_streak: usize,
    pub largest_loss_streak: usize,

    // ── Drawdown ───────────────────────────────────────────────────────
    pub max_drawdown_usd: f64,
    pub max_drawdown_pct: f64,

    // ── Risk-adjusted ─────────────────────────────────────────────────
    /// Annualized Sharpe: mean(excess_daily) / std(daily) x sqrt(252)
    pub sharpe_ratio: f64,
    /// Annualized Sortino: mean(excess_daily) / downside_std x sqrt(252)
    pub sortino_ratio: f64,
    /// Calmar: annualized_return / abs(max_drawdown_pct)
    pub calmar_ratio: f64,

    // ── MAE / MFE ─────────────────────────────────────────────────────
    pub avg_mae_ticks: f64,
    pub avg_mfe_ticks: f64,

    // ── Equity ─────────────────────────────────────────────────────────
    pub initial_capital_usd: f64,
    pub final_equity_usd: f64,
    pub total_return_pct: f64,
    pub trading_days: usize,

    // ── Benchmark comparison ─────────────────────────────────────────
    /// Buy-and-hold return for the same period.
    #[serde(default)]
    pub benchmark_return_pct: f64,
    /// Strategy alpha = strategy return - benchmark return.
    #[serde(default)]
    pub alpha_pct: f64,

    // ── Additional statistics ────────────────────────────────────────
    /// Average trade duration in milliseconds.
    #[serde(default)]
    pub avg_trade_duration_ms: f64,
    /// Expectancy per trade in USD =
    /// win_rate * avg_win + (1 - win_rate) * avg_loss.
    #[serde(default)]
    pub expectancy_usd: f64,
}

impl PerformanceMetrics {
    /// Compute all metrics from completed trades and the equity curve.
    pub fn compute(
        trades: &[TradeRecord],
        initial_capital_usd: f64,
        trading_days: usize,
        risk_free_annual: f64,
        equity_curve: &EquityCurve,
    ) -> Self {
        if trades.is_empty() {
            return Self::empty(initial_capital_usd, trading_days);
        }

        let total_trades = trades.len();
        let net_pnl_usd: f64 = trades.iter().map(|t| t.pnl_net_usd).sum();
        let gross_pnl_usd: f64 = trades.iter().map(|t| t.pnl_gross_usd).sum();
        let total_commission_usd: f64 = trades.iter().map(|t| t.commission_usd).sum();
        let net_pnl_ticks: i64 = trades.iter().map(|t| t.pnl_ticks).sum();

        let winning_trades = trades.iter().filter(|t| t.pnl_net_usd > 0.0).count();
        let losing_trades = trades.iter().filter(|t| t.pnl_net_usd < 0.0).count();
        let breakeven_trades = total_trades - winning_trades - losing_trades;

        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        let wins: Vec<f64> = trades
            .iter()
            .filter(|t| t.pnl_net_usd > 0.0)
            .map(|t| t.pnl_net_usd)
            .collect();
        let losses: Vec<f64> = trades
            .iter()
            .filter(|t| t.pnl_net_usd < 0.0)
            .map(|t| t.pnl_net_usd)
            .collect();

        let avg_win_usd = if wins.is_empty() {
            0.0
        } else {
            wins.iter().sum::<f64>() / wins.len() as f64
        };
        let avg_loss_usd = if losses.is_empty() {
            0.0
        } else {
            losses.iter().sum::<f64>() / losses.len() as f64
        };

        let gross_wins: f64 = wins.iter().sum();
        let gross_losses: f64 = losses.iter().sum::<f64>().abs();
        let profit_factor = if gross_losses == 0.0 {
            if gross_wins > 0.0 { f64::MAX } else { 0.0 }
        } else {
            gross_wins / gross_losses
        };

        let avg_rr = if total_trades > 0 {
            trades.iter().map(|t| t.rr_ratio).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };

        let best_trade_usd = trades
            .iter()
            .map(|t| t.pnl_net_usd)
            .fold(f64::NEG_INFINITY, f64::max);
        let worst_trade_usd = trades
            .iter()
            .map(|t| t.pnl_net_usd)
            .fold(f64::INFINITY, f64::min);

        let (largest_win_streak, largest_loss_streak) = compute_streaks(trades);
        let (max_drawdown_usd, max_drawdown_pct) =
            compute_max_drawdown(equity_curve, initial_capital_usd);

        let risk_free_daily = (1.0 + risk_free_annual).powf(1.0 / 252.0) - 1.0;
        let daily_returns = compute_daily_returns(trades, initial_capital_usd);
        let sharpe_ratio = compute_sharpe(&daily_returns, risk_free_daily);
        let sortino_ratio = compute_sortino(&daily_returns, risk_free_daily);

        let final_equity_usd = initial_capital_usd + net_pnl_usd;
        let total_return_pct = if initial_capital_usd > 0.0 {
            net_pnl_usd / initial_capital_usd * 100.0
        } else {
            0.0
        };
        let annualized_return = if trading_days > 0 {
            (1.0 + total_return_pct / 100.0).powf(252.0 / trading_days as f64) - 1.0
        } else {
            0.0
        };
        let calmar_ratio = if max_drawdown_pct.abs() < 1e-10 {
            0.0
        } else {
            (annualized_return * 100.0) / max_drawdown_pct.abs()
        };

        let avg_mae_ticks =
            trades.iter().map(|t| t.mae_ticks as f64).sum::<f64>() / total_trades as f64;
        let avg_mfe_ticks =
            trades.iter().map(|t| t.mfe_ticks as f64).sum::<f64>() / total_trades as f64;

        let avg_trade_duration_ms = if total_trades > 0 {
            trades
                .iter()
                .filter_map(|t| t.duration_ms)
                .map(|d| d as f64)
                .sum::<f64>()
                / total_trades as f64
        } else {
            0.0
        };
        let expectancy_usd = if total_trades > 0 {
            net_pnl_usd / total_trades as f64
        } else {
            0.0
        };

        Self {
            net_pnl_usd,
            gross_pnl_usd,
            total_commission_usd,
            net_pnl_ticks,
            total_trades,
            winning_trades,
            losing_trades,
            breakeven_trades,
            win_rate,
            avg_win_usd,
            avg_loss_usd,
            profit_factor,
            avg_rr,
            best_trade_usd,
            worst_trade_usd,
            largest_win_streak,
            largest_loss_streak,
            max_drawdown_usd,
            max_drawdown_pct,
            sharpe_ratio,
            sortino_ratio,
            calmar_ratio,
            avg_mae_ticks,
            avg_mfe_ticks,
            initial_capital_usd,
            final_equity_usd,
            total_return_pct,
            trading_days,
            benchmark_return_pct: 0.0,
            alpha_pct: 0.0,
            avg_trade_duration_ms,
            expectancy_usd,
        }
    }

    fn empty(initial_capital_usd: f64, trading_days: usize) -> Self {
        Self {
            net_pnl_usd: 0.0,
            gross_pnl_usd: 0.0,
            total_commission_usd: 0.0,
            net_pnl_ticks: 0,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            breakeven_trades: 0,
            win_rate: 0.0,
            avg_win_usd: 0.0,
            avg_loss_usd: 0.0,
            profit_factor: 0.0,
            avg_rr: 0.0,
            best_trade_usd: 0.0,
            worst_trade_usd: 0.0,
            largest_win_streak: 0,
            largest_loss_streak: 0,
            max_drawdown_usd: 0.0,
            max_drawdown_pct: 0.0,
            sharpe_ratio: 0.0,
            sortino_ratio: 0.0,
            calmar_ratio: 0.0,
            avg_mae_ticks: 0.0,
            avg_mfe_ticks: 0.0,
            initial_capital_usd,
            final_equity_usd: initial_capital_usd,
            total_return_pct: 0.0,
            trading_days,
            benchmark_return_pct: 0.0,
            alpha_pct: 0.0,
            avg_trade_duration_ms: 0.0,
            expectancy_usd: 0.0,
        }
    }
}

fn compute_daily_returns(trades: &[TradeRecord], initial_capital_usd: f64) -> Vec<f64> {
    use std::collections::BTreeMap;
    // Group net PnL by UTC day (floored to 86_400_000 ms)
    let mut daily_pnl: BTreeMap<u64, f64> = BTreeMap::new();
    for trade in trades {
        let day = trade.exit_time.0 / 86_400_000;
        *daily_pnl.entry(day).or_insert(0.0) += trade.pnl_net_usd;
    }
    let mut equity = initial_capital_usd;
    let mut returns = Vec::with_capacity(daily_pnl.len());
    for pnl in daily_pnl.values() {
        if equity > 0.0 {
            returns.push(pnl / equity);
        }
        equity += pnl;
    }
    returns
}

fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Sample standard deviation (N-1 divisor, Bessel's correction).
///
/// Uses sample variance (divides by N-1) which is the standard convention
/// for performance statistics like Sharpe ratio where the data represents
/// a sample of returns, not the full population.
fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

fn compute_sharpe(daily_returns: &[f64], risk_free_daily: f64) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let excess: Vec<f64> = daily_returns.iter().map(|r| r - risk_free_daily).collect();
    let sd = std_dev(&excess);
    if sd == 0.0 {
        return 0.0;
    }
    mean(&excess) / sd * 252_f64.sqrt()
}

fn compute_sortino(daily_returns: &[f64], risk_free_daily: f64) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let excess: Vec<f64> = daily_returns.iter().map(|r| r - risk_free_daily).collect();
    // Downside deviation: sqrt(mean of squared negative excess returns)
    // Uses all observations in denominator (zero for non-negative returns)
    let sum_sq_downside: f64 = daily_returns
        .iter()
        .map(|r| {
            let diff = r - risk_free_daily;
            if diff < 0.0 { diff * diff } else { 0.0 }
        })
        .sum();
    let downside_dev = (sum_sq_downside / daily_returns.len() as f64).sqrt();
    if downside_dev == 0.0 {
        return 0.0;
    }
    mean(&excess) / downside_dev * 252_f64.sqrt()
}

fn compute_max_drawdown(curve: &EquityCurve, initial_capital: f64) -> (f64, f64) {
    let mut peak = initial_capital;
    let mut max_dd_usd: f64 = 0.0;
    let mut max_dd_peak: f64 = initial_capital;

    for point in &curve.points {
        let equity = point.total_equity_usd;
        if equity > peak {
            peak = equity;
        }
        let dd = peak - equity;
        if dd > max_dd_usd {
            max_dd_usd = dd;
            max_dd_peak = peak;
        }
    }

    let max_dd_pct = if max_dd_peak > 0.0 {
        max_dd_usd / max_dd_peak * 100.0
    } else {
        0.0
    };
    (max_dd_usd, max_dd_pct)
}

fn compute_streaks(trades: &[TradeRecord]) -> (usize, usize) {
    let mut max_win = 0usize;
    let mut max_loss = 0usize;
    let mut cur_win = 0usize;
    let mut cur_loss = 0usize;

    for trade in trades {
        if trade.pnl_net_usd > 0.0 {
            cur_win += 1;
            cur_loss = 0;
        } else {
            cur_loss += 1;
            cur_win = 0;
        }
        max_win = max_win.max(cur_win);
        max_loss = max_loss.max(cur_loss);
    }
    (max_win, max_loss)
}
