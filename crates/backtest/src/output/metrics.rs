//! Performance metrics computed from backtest trade results.
//!
//! [`PerformanceMetrics`] aggregates P&L, win/loss statistics, drawdown,
//! risk-adjusted ratios (Sharpe, Sortino, Calmar), and trade excursion
//! data from a set of completed [`TradeRecord`]s and an [`EquityCurve`].

use crate::output::trade_record::TradeRecord;
use crate::portfolio::equity::EquityCurve;
use serde::{Deserialize, Serialize};

/// Number of trading days per year, used for annualizing returns
/// and risk-adjusted ratios (Sharpe, Sortino, Calmar).
const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Milliseconds in one UTC calendar day.
const MS_PER_DAY: u64 = 86_400_000;

/// Aggregated performance statistics for a completed backtest run.
///
/// All monetary values are denominated in USD. Percentage values are
/// expressed as percentages (e.g. `50.0` means 50%). Tick values use
/// the instrument's minimum tick size as the unit.
///
/// Construct via [`PerformanceMetrics::compute`] after a backtest
/// completes, or obtain from
/// [`BacktestResult::metrics`](super::result::BacktestResult::metrics).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // ── P&L ──────────────────────────────────────────────────────────
    /// Net profit/loss after commissions, in USD.
    pub net_pnl_usd: f64,
    /// Gross profit/loss before commissions, in USD.
    pub gross_pnl_usd: f64,
    /// Total commissions paid across all trades, in USD.
    pub total_commission_usd: f64,
    /// Net profit/loss measured in instrument ticks.
    pub net_pnl_ticks: i64,

    // ── Trade counts ─────────────────────────────────────────────────
    /// Total number of completed round-trip trades.
    pub total_trades: usize,
    /// Number of trades with positive net P&L.
    pub winning_trades: usize,
    /// Number of trades with negative net P&L.
    pub losing_trades: usize,
    /// Number of trades with exactly zero net P&L.
    pub breakeven_trades: usize,

    // ── Win/loss statistics ──────────────────────────────────────────
    /// Fraction of trades that were profitable (0.0..=1.0).
    ///
    /// Formula: `winning_trades / total_trades`.
    pub win_rate: f64,
    /// Average net P&L of winning trades, in USD.
    pub avg_win_usd: f64,
    /// Average net P&L of losing trades, in USD (negative value).
    pub avg_loss_usd: f64,
    /// Ratio of gross wins to gross losses.
    ///
    /// Formula: `sum(winning_pnl) / abs(sum(losing_pnl))`.
    /// Returns [`f64::MAX`] when there are no losing trades but
    /// wins exist, and `0.0` when there are no trades.
    pub profit_factor: f64,
    /// Average risk-reward ratio across all trades.
    ///
    /// Each trade's R:R is `pnl_ticks / stop_distance_ticks`.
    pub avg_rr: f64,
    /// Largest single-trade net profit, in USD.
    pub best_trade_usd: f64,
    /// Largest single-trade net loss, in USD.
    pub worst_trade_usd: f64,
    /// Longest consecutive run of winning trades.
    pub largest_win_streak: usize,
    /// Longest consecutive run of non-winning trades.
    pub largest_loss_streak: usize,

    // ── Drawdown ─────────────────────────────────────────────────────
    /// Maximum peak-to-trough drawdown, in USD.
    pub max_drawdown_usd: f64,
    /// Maximum peak-to-trough drawdown as a percentage of the peak
    /// equity value.
    pub max_drawdown_pct: f64,

    // ── Risk-adjusted ────────────────────────────────────────────────
    /// Annualized Sharpe ratio.
    ///
    /// Formula: `mean(excess_daily_returns) / std(daily_returns)
    /// * sqrt(252)`.
    ///
    /// Uses sample standard deviation (Bessel's correction, N-1
    /// divisor). A risk-free rate is subtracted from daily returns
    /// to compute excess returns.
    pub sharpe_ratio: f64,
    /// Annualized Sortino ratio.
    ///
    /// Formula: `mean(excess_daily_returns) / downside_deviation
    /// * sqrt(252)`.
    ///
    /// Downside deviation only penalizes returns below the
    /// risk-free rate, making this more appropriate than Sharpe
    /// for strategies with asymmetric return distributions.
    pub sortino_ratio: f64,
    /// Calmar ratio: annualized return divided by maximum drawdown.
    ///
    /// Formula: `annualized_return_pct / abs(max_drawdown_pct)`.
    /// Returns `0.0` when drawdown is effectively zero.
    pub calmar_ratio: f64,

    // ── MAE / MFE ────────────────────────────────────────────────────
    /// Average Maximum Adverse Excursion across all trades, in ticks.
    ///
    /// MAE measures how far a trade moved against the position before
    /// closing. Useful for evaluating stop-loss placement.
    pub avg_mae_ticks: f64,
    /// Average Maximum Favorable Excursion across all trades, in
    /// ticks.
    ///
    /// MFE measures how far a trade moved in the position's favor
    /// before closing. Useful for evaluating take-profit placement.
    pub avg_mfe_ticks: f64,

    // ── Equity ────────────────────────────────────────────────────────
    /// Starting account balance, in USD.
    pub initial_capital_usd: f64,
    /// Account balance after all trades, in USD.
    ///
    /// Equal to `initial_capital_usd + net_pnl_usd`.
    pub final_equity_usd: f64,
    /// Total return as a percentage of initial capital.
    ///
    /// Formula: `net_pnl_usd / initial_capital_usd * 100`.
    pub total_return_pct: f64,
    /// Number of distinct trading days in the backtest period.
    pub trading_days: usize,

    // ── Benchmark comparison ─────────────────────────────────────────
    /// Buy-and-hold return for the same period, as a percentage.
    #[serde(default)]
    pub benchmark_return_pct: f64,
    /// Strategy alpha: `total_return_pct - benchmark_return_pct`.
    #[serde(default)]
    pub alpha_pct: f64,

    // ── Additional statistics ────────────────────────────────────────
    /// Average trade duration in milliseconds.
    #[serde(default)]
    pub avg_trade_duration_ms: f64,
    /// Expectancy per trade in USD.
    ///
    /// Formula: `net_pnl_usd / total_trades`. Represents the average
    /// dollar amount you can expect to win (or lose) per trade.
    #[serde(default)]
    pub expectancy_usd: f64,
}

impl PerformanceMetrics {
    /// Compute all performance metrics from completed trades and
    /// the equity curve.
    ///
    /// # Arguments
    ///
    /// * `trades` — completed round-trip trades in chronological
    ///   order.
    /// * `initial_capital_usd` — starting account balance in USD.
    /// * `trading_days` — number of distinct trading sessions in
    ///   the backtest period.
    /// * `risk_free_annual` — annualized risk-free rate as a
    ///   decimal (e.g. `0.05` for 5%). Used for Sharpe and Sortino
    ///   calculations.
    /// * `equity_curve` — equity curve sampled throughout the run,
    ///   used for drawdown computation.
    ///
    /// Returns a zeroed-out [`PerformanceMetrics`] when `trades`
    /// is empty.
    #[must_use]
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

        let win_rate = winning_trades as f64 / total_trades as f64;

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

        let avg_rr = trades.iter().map(|t| t.rr_ratio).sum::<f64>() / total_trades as f64;

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

        let risk_free_daily = (1.0 + risk_free_annual).powf(1.0 / TRADING_DAYS_PER_YEAR) - 1.0;
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
            (1.0 + total_return_pct / 100.0).powf(TRADING_DAYS_PER_YEAR / trading_days as f64) - 1.0
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

        let avg_trade_duration_ms = {
            let durations: Vec<f64> = trades
                .iter()
                .filter_map(|t| t.duration_ms)
                .map(|d| d as f64)
                .collect();
            if durations.is_empty() {
                0.0
            } else {
                durations.iter().sum::<f64>() / durations.len() as f64
            }
        };
        let expectancy_usd = net_pnl_usd / total_trades as f64;

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

    /// Returns a zeroed-out metrics struct for a backtest with no
    /// trades.
    #[must_use]
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

/// Group trade P&L by UTC calendar day and compute sequential
/// daily returns relative to a running equity balance.
///
/// Days are derived by flooring exit timestamps to 86,400,000 ms
/// boundaries. Returns are computed as `daily_pnl / equity` where
/// equity rolls forward each day.
fn compute_daily_returns(trades: &[TradeRecord], initial_capital_usd: f64) -> Vec<f64> {
    use std::collections::BTreeMap;

    let mut daily_pnl: BTreeMap<u64, f64> = BTreeMap::new();
    for trade in trades {
        let day = trade.exit_time.0 / MS_PER_DAY;
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

/// Arithmetic mean of a slice of values.
///
/// Returns `0.0` for an empty slice.
fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Sample standard deviation using Bessel's correction (N-1
/// divisor).
///
/// The sample variance formula (`sum((x - mean)^2) / (N - 1)`)
/// is the standard convention for performance statistics like the
/// Sharpe ratio, where daily returns represent a sample rather
/// than a complete population.
///
/// Returns `0.0` when fewer than two data points are available.
fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

/// Compute the annualized Sharpe ratio from daily returns.
///
/// Formula: `mean(excess) / std(excess) * sqrt(252)`
///
/// where `excess = daily_return - risk_free_daily`.
///
/// Returns `0.0` when fewer than two return observations exist or
/// when standard deviation is zero.
fn compute_sharpe(daily_returns: &[f64], risk_free_daily: f64) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let excess: Vec<f64> = daily_returns.iter().map(|r| r - risk_free_daily).collect();
    let sd = std_dev(&excess);
    if sd == 0.0 {
        return 0.0;
    }
    mean(&excess) / sd * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Compute the annualized Sortino ratio from daily returns.
///
/// Formula: `mean(excess) / downside_dev * sqrt(252)`
///
/// Downside deviation is computed as
/// `sqrt(mean(min(r - rf, 0)^2))`, using all observations in the
/// denominator (zero contribution for non-negative excess returns).
/// This "continuous" downside deviation variant avoids inflating
/// the ratio when only a few days have negative returns.
///
/// Returns `0.0` when fewer than two return observations exist or
/// when downside deviation is zero (no negative excess returns).
fn compute_sortino(daily_returns: &[f64], risk_free_daily: f64) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let excess: Vec<f64> = daily_returns.iter().map(|r| r - risk_free_daily).collect();
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
    mean(&excess) / downside_dev * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Walk the equity curve to find the maximum peak-to-trough
/// drawdown.
///
/// Returns `(max_drawdown_usd, max_drawdown_pct)` where the
/// percentage is relative to the peak equity at the time of the
/// drawdown.
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

/// Compute the longest consecutive winning and losing streaks.
///
/// A trade with `pnl_net_usd > 0` is counted as a win; anything
/// else (including breakeven) increments the loss streak. Returns
/// `(max_win_streak, max_loss_streak)`.
fn compute_streaks(trades: &[TradeRecord]) -> (usize, usize) {
    let mut max_win = 0_usize;
    let mut max_loss = 0_usize;
    let mut cur_win = 0_usize;
    let mut cur_loss = 0_usize;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::trade_record::{ExitReason, TradeRecord};
    use crate::portfolio::equity::EquityCurve;
    use kairos_data::{Price, Side, Timestamp};

    fn make_trade(pnl_net: f64, pnl_gross: f64, commission: f64, exit_day_ms: u64) -> TradeRecord {
        TradeRecord {
            index: 1,
            entry_time: Timestamp(exit_day_ms.saturating_sub(1000)),
            exit_time: Timestamp(exit_day_ms),
            side: Side::Buy,
            quantity: 1.0,
            entry_price: Price::from_f64(5000.0),
            exit_price: Price::from_f64(5010.0),
            initial_stop_loss: Price::from_f64(4990.0),
            initial_take_profit: None,
            pnl_ticks: (pnl_net * 4.0) as i64, // approximate
            pnl_gross_usd: pnl_gross,
            commission_usd: commission,
            pnl_net_usd: pnl_net,
            rr_ratio: 0.0,
            mae_ticks: 2,
            mfe_ticks: 5,
            exit_reason: ExitReason::Manual,
            label: None,
            instrument: None,
            duration_ms: Some(1000),
            snapshot: None,
        }
    }

    fn make_equity_curve(initial: f64, equity_values: &[f64]) -> EquityCurve {
        let mut curve = EquityCurve::new(initial);
        for (i, &eq) in equity_values.iter().enumerate() {
            curve.record(Timestamp((i as u64 + 1) * 86_400_000), eq, 0.0);
        }
        curve
    }

    // ── Empty trades ──────────────────────────────────────────────

    #[test]
    fn test_empty_trades_returns_zeroed_metrics() {
        let curve = EquityCurve::new(100_000.0);
        let m = PerformanceMetrics::compute(&[], 100_000.0, 10, 0.05, &curve);

        assert_eq!(m.total_trades, 0);
        assert_eq!(m.net_pnl_usd, 0.0);
        assert_eq!(m.win_rate, 0.0);
        assert_eq!(m.profit_factor, 0.0);
        assert_eq!(m.sharpe_ratio, 0.0);
        assert_eq!(m.sortino_ratio, 0.0);
        assert_eq!(m.calmar_ratio, 0.0);
        assert_eq!(m.max_drawdown_usd, 0.0);
        assert_eq!(m.final_equity_usd, 100_000.0);
        assert_eq!(m.total_return_pct, 0.0);
        assert_eq!(m.expectancy_usd, 0.0);
    }

    // ── Single winning trade ─────────────────────────────────────

    #[test]
    fn test_single_winning_trade() {
        let t = make_trade(500.0, 510.0, 10.0, 86_400_000);
        let curve = make_equity_curve(100_000.0, &[100_500.0]);
        let m = PerformanceMetrics::compute(&[t], 100_000.0, 1, 0.05, &curve);

        assert_eq!(m.total_trades, 1);
        assert_eq!(m.winning_trades, 1);
        assert_eq!(m.losing_trades, 0);
        assert_eq!(m.breakeven_trades, 0);
        assert!((m.win_rate - 1.0).abs() < 1e-10);
        assert!((m.net_pnl_usd - 500.0).abs() < 1e-10);
        assert!((m.gross_pnl_usd - 510.0).abs() < 1e-10);
        assert!((m.total_commission_usd - 10.0).abs() < 1e-10);
        assert_eq!(m.profit_factor, f64::MAX);
        assert!((m.avg_win_usd - 500.0).abs() < 1e-10);
        assert!((m.avg_loss_usd - 0.0).abs() < 1e-10);
        assert!((m.best_trade_usd - 500.0).abs() < 1e-10);
        assert!((m.worst_trade_usd - 500.0).abs() < 1e-10);
    }

    // ── Single losing trade ──────────────────────────────────────

    #[test]
    fn test_single_losing_trade() {
        let t = make_trade(-300.0, -290.0, 10.0, 86_400_000);
        let curve = make_equity_curve(100_000.0, &[99_700.0]);
        let m = PerformanceMetrics::compute(&[t], 100_000.0, 1, 0.05, &curve);

        assert_eq!(m.winning_trades, 0);
        assert_eq!(m.losing_trades, 1);
        assert!((m.win_rate - 0.0).abs() < 1e-10);
        assert!((m.profit_factor - 0.0).abs() < 1e-10);
        assert!((m.avg_loss_usd - (-300.0)).abs() < 1e-10);
    }

    // ── Win rate computation ─────────────────────────────────────

    #[test]
    fn test_win_rate_mixed_trades() {
        // 3 wins, 2 losses => 0.6
        let day = 86_400_000;
        let trades = vec![
            make_trade(100.0, 110.0, 10.0, day),
            make_trade(-50.0, -40.0, 10.0, day * 2),
            make_trade(200.0, 210.0, 10.0, day * 3),
            make_trade(-80.0, -70.0, 10.0, day * 4),
            make_trade(150.0, 160.0, 10.0, day * 5),
        ];
        let curve = make_equity_curve(
            100_000.0,
            &[100_100.0, 100_050.0, 100_250.0, 100_170.0, 100_320.0],
        );
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 5, 0.05, &curve);

        assert_eq!(m.total_trades, 5);
        assert_eq!(m.winning_trades, 3);
        assert_eq!(m.losing_trades, 2);
        assert!((m.win_rate - 0.6).abs() < 1e-10);
    }

    // ── Profit factor ────────────────────────────────────────────

    #[test]
    fn test_profit_factor_mixed() {
        // Wins total: 100 + 200 = 300 net
        // Losses total: -50 + -80 = -130 net
        // PF = 300 / 130 = 2.307...
        let day = 86_400_000;
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day),
            make_trade(-50.0, -50.0, 0.0, day * 2),
            make_trade(200.0, 200.0, 0.0, day * 3),
            make_trade(-80.0, -80.0, 0.0, day * 4),
        ];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_050.0, 100_250.0, 100_170.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 4, 0.05, &curve);

        let expected_pf = 300.0 / 130.0;
        assert!((m.profit_factor - expected_pf).abs() < 1e-10);
    }

    #[test]
    fn test_profit_factor_all_winning() {
        let day = 86_400_000;
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day),
            make_trade(200.0, 200.0, 0.0, day * 2),
        ];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_300.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert_eq!(m.profit_factor, f64::MAX);
    }

    #[test]
    fn test_profit_factor_all_losing() {
        let day = 86_400_000;
        let trades = vec![
            make_trade(-100.0, -100.0, 0.0, day),
            make_trade(-200.0, -200.0, 0.0, day * 2),
        ];
        let curve = make_equity_curve(100_000.0, &[99_900.0, 99_700.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert!((m.profit_factor - 0.0).abs() < 1e-10);
    }

    // ── Streaks ──────────────────────────────────────────────────

    #[test]
    fn test_win_streak() {
        let day = 86_400_000;
        // W W W L W W
        let trades = vec![
            make_trade(10.0, 10.0, 0.0, day),
            make_trade(20.0, 20.0, 0.0, day * 2),
            make_trade(30.0, 30.0, 0.0, day * 3),
            make_trade(-10.0, -10.0, 0.0, day * 4),
            make_trade(40.0, 40.0, 0.0, day * 5),
            make_trade(50.0, 50.0, 0.0, day * 6),
        ];
        let curve = make_equity_curve(
            100_000.0,
            &[
                100_010.0, 100_030.0, 100_060.0, 100_050.0, 100_090.0, 100_140.0,
            ],
        );
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 6, 0.05, &curve);

        assert_eq!(m.largest_win_streak, 3);
        assert_eq!(m.largest_loss_streak, 1);
    }

    #[test]
    fn test_loss_streak() {
        let day = 86_400_000;
        // L L L W L
        let trades = vec![
            make_trade(-10.0, -10.0, 0.0, day),
            make_trade(-20.0, -20.0, 0.0, day * 2),
            make_trade(-5.0, -5.0, 0.0, day * 3),
            make_trade(50.0, 50.0, 0.0, day * 4),
            make_trade(-15.0, -15.0, 0.0, day * 5),
        ];
        let curve = make_equity_curve(
            100_000.0,
            &[99_990.0, 99_970.0, 99_965.0, 100_015.0, 100_000.0],
        );
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 5, 0.05, &curve);

        assert_eq!(m.largest_win_streak, 1);
        assert_eq!(m.largest_loss_streak, 3);
    }

    // ── Breakeven trades ─────────────────────────────────────────

    #[test]
    fn test_breakeven_trade_counts_as_loss_streak() {
        let day = 86_400_000;
        // Breakeven (0.0 pnl) counts as non-winning for streak purposes
        let trades = vec![
            make_trade(0.0, 0.0, 0.0, day),
            make_trade(0.0, 0.0, 0.0, day * 2),
        ];
        let curve = make_equity_curve(100_000.0, &[100_000.0, 100_000.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert_eq!(m.breakeven_trades, 2);
        assert_eq!(m.largest_loss_streak, 2);
        assert_eq!(m.largest_win_streak, 0);
    }

    // ── Max drawdown ─────────────────────────────────────────────

    #[test]
    fn test_max_drawdown_simple() {
        // Equity: 100k -> 110k -> 95k -> 105k
        // Peak at 110k, trough at 95k => dd = 15k, dd_pct = 15/110*100 = 13.636...%
        let curve = make_equity_curve(100_000.0, &[110_000.0, 95_000.0, 105_000.0]);
        let (dd_usd, dd_pct) = compute_max_drawdown(&curve, 100_000.0);

        assert!((dd_usd - 15_000.0).abs() < 1e-10);
        assert!((dd_pct - (15_000.0 / 110_000.0 * 100.0)).abs() < 1e-10);
    }

    #[test]
    fn test_max_drawdown_no_drawdown() {
        // Monotonically increasing equity
        let curve = make_equity_curve(100_000.0, &[101_000.0, 102_000.0, 103_000.0]);
        let (dd_usd, dd_pct) = compute_max_drawdown(&curve, 100_000.0);

        assert!((dd_usd - 0.0).abs() < 1e-10);
        assert!((dd_pct - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_max_drawdown_immediate_decline() {
        // Equity drops from 100k to 80k immediately
        let curve = make_equity_curve(100_000.0, &[80_000.0, 90_000.0]);
        let (dd_usd, dd_pct) = compute_max_drawdown(&curve, 100_000.0);

        assert!((dd_usd - 20_000.0).abs() < 1e-10);
        assert!((dd_pct - 20.0).abs() < 1e-10);
    }

    // ── Sharpe / Sortino with zero variance ──────────────────────

    #[test]
    fn test_sharpe_zero_daily_volatility() {
        // Single day of data => < 2 returns => Sharpe = 0
        let day = 86_400_000;
        let trades = vec![make_trade(100.0, 100.0, 0.0, day)];
        let curve = make_equity_curve(100_000.0, &[100_100.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 1, 0.05, &curve);

        assert!((m.sharpe_ratio - 0.0).abs() < 1e-10);
        assert!((m.sortino_ratio - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_sharpe_with_multiple_days() {
        let day = 86_400_000;
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day),
            make_trade(-50.0, -50.0, 0.0, day * 2),
            make_trade(200.0, 200.0, 0.0, day * 3),
        ];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_050.0, 100_250.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 3, 0.05, &curve);

        // Should produce a non-zero sharpe since we have 3 daily returns
        // with non-zero variance
        assert!(m.sharpe_ratio.is_finite());
    }

    // ── Calmar ratio ─────────────────────────────────────────────

    #[test]
    fn test_calmar_zero_drawdown_returns_zero() {
        // Monotonically rising => no drawdown => calmar = 0
        let day = 86_400_000;
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day),
            make_trade(200.0, 200.0, 0.0, day * 2),
        ];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_300.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert!((m.calmar_ratio - 0.0).abs() < 1e-10);
    }

    // ── Total return / final equity ──────────────────────────────

    #[test]
    fn test_total_return_and_final_equity() {
        let day = 86_400_000;
        let trades = vec![
            make_trade(5000.0, 5000.0, 0.0, day),
            make_trade(-2000.0, -2000.0, 0.0, day * 2),
        ];
        let curve = make_equity_curve(100_000.0, &[105_000.0, 103_000.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert!((m.net_pnl_usd - 3000.0).abs() < 1e-10);
        assert!((m.final_equity_usd - 103_000.0).abs() < 1e-10);
        assert!((m.total_return_pct - 3.0).abs() < 1e-10);
    }

    // ── Best / worst trade ───────────────────────────────────────

    #[test]
    fn test_best_and_worst_trade() {
        let day = 86_400_000;
        let trades = vec![
            make_trade(500.0, 500.0, 0.0, day),
            make_trade(-300.0, -300.0, 0.0, day * 2),
            make_trade(1000.0, 1000.0, 0.0, day * 3),
            make_trade(-100.0, -100.0, 0.0, day * 4),
        ];
        let curve = make_equity_curve(100_000.0, &[100_500.0, 100_200.0, 101_200.0, 101_100.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 4, 0.05, &curve);

        assert!((m.best_trade_usd - 1000.0).abs() < 1e-10);
        assert!((m.worst_trade_usd - (-300.0)).abs() < 1e-10);
    }

    // ── Average win/loss ─────────────────────────────────────────

    #[test]
    fn test_avg_win_and_avg_loss() {
        let day = 86_400_000;
        // Wins: 100, 200 => avg = 150
        // Losses: -50, -80 => avg = -65
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day),
            make_trade(-50.0, -50.0, 0.0, day * 2),
            make_trade(200.0, 200.0, 0.0, day * 3),
            make_trade(-80.0, -80.0, 0.0, day * 4),
        ];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_050.0, 100_250.0, 100_170.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 4, 0.05, &curve);

        assert!((m.avg_win_usd - 150.0).abs() < 1e-10);
        assert!((m.avg_loss_usd - (-65.0)).abs() < 1e-10);
    }

    // ── Average MAE/MFE ──────────────────────────────────────────

    #[test]
    fn test_avg_mae_mfe() {
        let day = 86_400_000;
        let mut t1 = make_trade(100.0, 100.0, 0.0, day);
        t1.mae_ticks = 4;
        t1.mfe_ticks = 10;
        let mut t2 = make_trade(-50.0, -50.0, 0.0, day * 2);
        t2.mae_ticks = 8;
        t2.mfe_ticks = 2;
        let trades = vec![t1, t2];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_050.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert!((m.avg_mae_ticks - 6.0).abs() < 1e-10);
        assert!((m.avg_mfe_ticks - 6.0).abs() < 1e-10);
    }

    // ── Expectancy ───────────────────────────────────────────────

    #[test]
    fn test_expectancy() {
        let day = 86_400_000;
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day),
            make_trade(-50.0, -50.0, 0.0, day * 2),
            make_trade(200.0, 200.0, 0.0, day * 3),
        ];
        // net_pnl = 250, trades = 3, expectancy = 83.333...
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_050.0, 100_250.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 3, 0.05, &curve);

        assert!((m.expectancy_usd - 250.0 / 3.0).abs() < 1e-10);
    }

    // ── Average trade duration ───────────────────────────────────

    #[test]
    fn test_avg_trade_duration() {
        let day = 86_400_000;
        let mut t1 = make_trade(100.0, 100.0, 0.0, day);
        t1.duration_ms = Some(5000);
        let mut t2 = make_trade(-50.0, -50.0, 0.0, day * 2);
        t2.duration_ms = Some(3000);
        let trades = vec![t1, t2];
        let curve = make_equity_curve(100_000.0, &[100_100.0, 100_050.0]);
        let m = PerformanceMetrics::compute(&trades, 100_000.0, 2, 0.05, &curve);

        assert!((m.avg_trade_duration_ms - 4000.0).abs() < 1e-10);
    }

    // ── Compute_daily_returns ────────────────────────────────────

    #[test]
    fn test_daily_returns_grouping() {
        let day = 86_400_000_u64;
        // Two trades on day 1, one on day 2
        let trades = vec![
            make_trade(100.0, 100.0, 0.0, day + 1000),     // day 1
            make_trade(50.0, 50.0, 0.0, day + 2000),       // day 1
            make_trade(-30.0, -30.0, 0.0, day * 2 + 1000), // day 2
        ];
        let returns = compute_daily_returns(&trades, 100_000.0);

        assert_eq!(returns.len(), 2);
        // Day 1: pnl=150, equity=100000 => return = 150/100000 = 0.0015
        assert!((returns[0] - 0.0015).abs() < 1e-10);
        // Day 2: pnl=-30, equity=100150 => return = -30/100150
        let expected_r2 = -30.0 / 100_150.0;
        assert!((returns[1] - expected_r2).abs() < 1e-10);
    }

    // ── Helper function tests ────────────────────────────────────

    #[test]
    fn test_mean_empty() {
        assert!((mean(&[]) - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_mean_values() {
        assert!((mean(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-15);
    }

    #[test]
    fn test_std_dev_single_value() {
        assert!((std_dev(&[5.0]) - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_std_dev_known() {
        // data: [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
        // mean = 5.0, sample variance = 32/7, sample std_dev = sqrt(32/7) ~ 2.138
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = std_dev(&data);
        let expected = (32.0_f64 / 7.0).sqrt();
        assert!((sd - expected).abs() < 1e-10);
    }
}
