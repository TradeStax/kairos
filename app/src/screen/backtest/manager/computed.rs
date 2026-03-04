//! Analytics computation from BacktestResult.
//!
//! All analytics are derived purely from the result's trades and
//! equity curve — no external data needed.

use crate::config::UserTimezone;
use std::sync::Arc;

// ── Account Specifications ──────────────────────────────────────────

/// Prop firm account specification with dollar-based limits.
#[derive(Debug, Clone)]
pub struct AccountSpec {
    pub name: &'static str,
    pub size: f64,
    pub profit_target: f64,
    pub max_trailing_dd: f64,
    pub daily_loss_limit: f64,
}

pub const PROP_FIRM_ACCOUNTS: &[AccountSpec] = &[
    AccountSpec {
        name: "50K Eval",
        size: 50_000.0,
        profit_target: 3_000.0,
        max_trailing_dd: 2_500.0,
        daily_loss_limit: 1_000.0,
    },
    AccountSpec {
        name: "100K Eval",
        size: 100_000.0,
        profit_target: 6_000.0,
        max_trailing_dd: 3_000.0,
        daily_loss_limit: 2_000.0,
    },
    AccountSpec {
        name: "150K Eval",
        size: 150_000.0,
        profit_target: 9_000.0,
        max_trailing_dd: 4_500.0,
        daily_loss_limit: 2_250.0,
    },
    AccountSpec {
        name: "250K Funded",
        size: 250_000.0,
        profit_target: 15_000.0,
        max_trailing_dd: 6_250.0,
        daily_loss_limit: 3_000.0,
    },
];

// ── Status ──────────────────────────────────────────────────────────

/// Three-state status for prop firm simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropFirmStatus {
    Passed,
    Failed,
    Active,
}

// ── Monte Carlo Types ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PropFirmMonteCarloResult {
    pub num_iterations: usize,
    #[allow(dead_code)]
    pub pass_count: usize,
    pub pass_rate: f64,
    pub avg_trades_to_pass: Option<f64>,
    pub avg_trades_to_fail: Option<f64>,
    pub median_final_pnl: f64,
    pub p5_final_pnl: f64,
    pub p95_final_pnl: f64,
    pub sample_paths: Vec<McPropFirmPath>,
}

#[derive(Debug, Clone)]
pub struct McPropFirmPath {
    pub equity_curve: Vec<f64>,
    pub passed: bool,
    pub completion_idx: Option<usize>,
}

// ── Prop Firm Result ────────────────────────────────────────────────

/// Prop firm simulation result for a single account configuration.
pub struct PropFirmResult {
    pub account: AccountSpec,
    pub status: PropFirmStatus,
    pub hit_drawdown_limit: bool,
    pub hit_daily_limit: bool,
    pub worst_drawdown: f64,
    pub worst_daily_loss: f64,
    pub final_pnl: f64,
    pub progress_pct: f64,
    pub equity_curve: Vec<f64>,
    pub breach_trade_idx: Option<usize>,
    pub monte_carlo: PropFirmMonteCarloResult,
}

/// Pre-computed analytics for the management modal.
pub struct ComputedAnalytics {
    /// Monthly returns: (year, month, return_pct)
    pub monthly_returns: Vec<(u16, u8, f64)>,
    /// P&L distribution histogram: (bin_center, count)
    pub pnl_histogram: Vec<(f64, usize)>,
    /// Monte Carlo equity paths (each path = cumulative equity)
    pub monte_carlo_paths: Vec<Vec<f64>>,
    /// 5th percentile at each trade step
    pub monte_carlo_p5: Vec<f64>,
    /// Median at each trade step
    pub monte_carlo_p50: Vec<f64>,
    /// 95th percentile at each trade step
    pub monte_carlo_p95: Vec<f64>,
    /// Expected value per trade
    pub expectancy_per_trade: f64,
    /// Kelly criterion fraction
    pub kelly_criterion: f64,
    /// Optimal f for position sizing
    pub optimal_f: f64,
    /// Payoff ratio (avg_win / |avg_loss|)
    pub payoff_ratio: f64,
    /// Value at Risk (95th percentile loss)
    pub var_95: f64,
    /// Conditional VaR (mean of worst 1%)
    pub cvar_99: f64,
    /// Probability of ruin (simplified)
    pub risk_of_ruin: f64,
    /// Max consecutive losses
    pub max_consecutive_losses: usize,
    /// P&L by hour of day: all 24 hours (hour 0-23, net_pnl)
    pub pnl_by_hour: Vec<(u8, f64)>,
    /// MAE vs MFE scatter: (mae_ticks, mfe_ticks, is_winner, trade_idx)
    pub mae_mfe_scatter: Vec<(i64, i64, bool, usize)>,
    /// Prop firm simulation results
    pub prop_firm_results: Vec<PropFirmResult>,
    /// Daily P&L: (date_label "MM/DD", net_pnl)
    #[allow(dead_code)]
    pub daily_pnl: Vec<(String, f64)>,
    /// Maximum daily loss (absolute value)
    #[allow(dead_code)]
    pub max_daily_loss: f64,
}

impl ComputedAnalytics {
    pub fn from_result(result: &Arc<backtest::BacktestResult>, timezone: UserTimezone) -> Self {
        let trades = &result.trades;
        let metrics = &result.metrics;

        // ── Monthly returns ──────────────────────────────────
        let monthly_returns = Self::compute_monthly_returns(result, timezone);

        // ── P&L histogram ────────────────────────────────────
        let pnl_histogram = Self::compute_pnl_histogram(trades);

        // ── Monte Carlo ──────────────────────────────────────
        let (monte_carlo_paths, monte_carlo_p5, monte_carlo_p50, monte_carlo_p95) =
            Self::compute_monte_carlo(trades, result.config.initial_capital_usd);

        // ── Expectancy ───────────────────────────────────────
        let win_rate = metrics.win_rate;
        let avg_win = if metrics.winning_trades > 0 {
            trades
                .iter()
                .filter(|t| t.pnl_net_usd > 0.0)
                .map(|t| t.pnl_net_usd)
                .sum::<f64>()
                / metrics.winning_trades as f64
        } else {
            0.0
        };
        let avg_loss = if metrics.losing_trades > 0 {
            trades
                .iter()
                .filter(|t| t.pnl_net_usd < 0.0)
                .map(|t| t.pnl_net_usd.abs())
                .sum::<f64>()
                / metrics.losing_trades as f64
        } else {
            0.0
        };

        let payoff_ratio = if avg_loss > 0.0 {
            avg_win / avg_loss
        } else {
            0.0
        };

        let expectancy_per_trade = win_rate * avg_win - (1.0 - win_rate) * avg_loss;

        let kelly_criterion = if payoff_ratio > 0.0 {
            win_rate - (1.0 - win_rate) / payoff_ratio
        } else {
            0.0
        };

        let optimal_f = Self::compute_optimal_f(trades);

        // ── Risk metrics ─────────────────────────────────────
        let var_95 = Self::compute_var(trades, 0.05);
        let cvar_99 = Self::compute_cvar(trades, 0.01);
        let risk_of_ruin = Self::compute_risk_of_ruin(
            win_rate,
            avg_win,
            avg_loss,
            result.config.initial_capital_usd,
        );
        let max_consecutive_losses = Self::compute_max_consecutive_losses(trades);

        // ── P&L by hour ──────────────────────────────────────
        let pnl_by_hour = Self::compute_pnl_by_hour(trades, timezone);

        // ── MAE/MFE scatter ──────────────────────────────────
        let mae_mfe_scatter: Vec<(i64, i64, bool, usize)> = trades
            .iter()
            .enumerate()
            .map(|(i, t)| (t.mae_ticks, t.mfe_ticks, t.pnl_net_usd > 0.0, i))
            .collect();

        // ── Daily P&L ────────────────────────────────────────
        let daily_pnl = Self::compute_daily_pnl(trades, timezone);
        let max_daily_loss = daily_pnl
            .iter()
            .map(|(_, p)| *p)
            .fold(0.0_f64, f64::min)
            .abs();

        // ── Prop firm simulation ─────────────────────────────
        let prop_firm_results = Self::compute_prop_firm_results(
            trades,
            &result.equity_curve,
            result.config.initial_capital_usd,
            timezone,
        );

        Self {
            monthly_returns,
            pnl_histogram,
            monte_carlo_paths,
            monte_carlo_p5,
            monte_carlo_p50,
            monte_carlo_p95,
            expectancy_per_trade,
            kelly_criterion,
            optimal_f,
            payoff_ratio,
            var_95,
            cvar_99,
            risk_of_ruin,
            max_consecutive_losses,
            pnl_by_hour,
            mae_mfe_scatter,
            prop_firm_results,
            daily_pnl,
            max_daily_loss,
        }
    }

    fn compute_monthly_returns(
        result: &backtest::BacktestResult,
        tz: UserTimezone,
    ) -> Vec<(u16, u8, f64)> {
        use chrono::Datelike;

        let points = &result.equity_curve.points;
        if points.len() < 2 {
            return vec![];
        }

        let mut monthly: std::collections::BTreeMap<(u16, u8), (f64, f64)> =
            std::collections::BTreeMap::new();

        for point in points {
            let secs = (point.timestamp.0 / 1000) as i64;
            let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) else {
                continue;
            };
            let key = match tz {
                UserTimezone::Local => {
                    let local = dt.with_timezone(&chrono::Local);
                    (local.year() as u16, local.month() as u8)
                }
                UserTimezone::Utc => (dt.year() as u16, dt.month() as u8),
            };
            let entry = monthly
                .entry(key)
                .or_insert((point.total_equity_usd, point.total_equity_usd));
            entry.1 = point.total_equity_usd;
        }

        monthly
            .into_iter()
            .map(|((year, month), (first, last))| {
                let ret = if first.abs() > 0.0 {
                    (last - first) / first * 100.0
                } else {
                    0.0
                };
                (year, month, ret)
            })
            .collect()
    }

    fn compute_pnl_histogram(trades: &[backtest::TradeRecord]) -> Vec<(f64, usize)> {
        if trades.is_empty() {
            return vec![];
        }

        let pnls: Vec<f64> = trades.iter().map(|t| t.pnl_net_usd).collect();
        let min = pnls.iter().copied().fold(f64::INFINITY, f64::min);
        let max = pnls.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        if (max - min).abs() < f64::EPSILON {
            return vec![(min, trades.len())];
        }

        // Sturges' rule for bin count
        let n_bins = ((trades.len() as f64).log2().ceil() as usize + 1).clamp(5, 25);
        let bin_width = (max - min) / n_bins as f64;

        let mut bins = vec![0usize; n_bins];
        for &pnl in &pnls {
            let idx = ((pnl - min) / bin_width).floor() as usize;
            let idx = idx.min(n_bins - 1);
            bins[idx] += 1;
        }

        bins.into_iter()
            .enumerate()
            .map(|(i, count)| {
                let center = min + (i as f64 + 0.5) * bin_width;
                (center, count)
            })
            .collect()
    }

    fn compute_monte_carlo(
        trades: &[backtest::TradeRecord],
        initial_capital: f64,
    ) -> (Vec<Vec<f64>>, Vec<f64>, Vec<f64>, Vec<f64>) {
        const NUM_PATHS: usize = 100;

        if trades.is_empty() {
            return (vec![], vec![], vec![], vec![]);
        }

        let pnls: Vec<f64> = trades.iter().map(|t| t.pnl_net_usd).collect();
        let n = pnls.len();

        // Simple LCG PRNG (deterministic, no external dependency)
        let mut seed: u64 = 42;
        let mut next_rand = move || -> usize {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };

        let mut paths = Vec::with_capacity(NUM_PATHS);
        for _ in 0..NUM_PATHS {
            let mut equity = vec![initial_capital; n + 1];
            // Fisher-Yates shuffle of indices
            let mut indices: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = next_rand() % (i + 1);
                indices.swap(i, j);
            }
            for (step, &idx) in indices.iter().enumerate() {
                equity[step + 1] = equity[step] + pnls[idx];
            }
            paths.push(equity);
        }

        // Extract percentiles at each step
        let mut p5 = vec![0.0; n + 1];
        let mut p50 = vec![0.0; n + 1];
        let mut p95 = vec![0.0; n + 1];

        for step in 0..=n {
            let mut vals: Vec<f64> = paths.iter().map(|p| p[step]).collect();
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let len = vals.len();
            p5[step] = vals[(len as f64 * 0.05) as usize];
            p50[step] = vals[len / 2];
            p95[step] = vals[((len as f64 * 0.95) as usize).min(len - 1)];
        }

        (paths, p5, p50, p95)
    }

    fn compute_optimal_f(trades: &[backtest::TradeRecord]) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }

        let pnls: Vec<f64> = trades.iter().map(|t| t.pnl_net_usd).collect();
        let max_loss = pnls
            .iter()
            .copied()
            .fold(0.0_f64, |acc, x| acc.min(x))
            .abs();

        if max_loss < f64::EPSILON {
            return 0.0;
        }

        let n = pnls.len() as f64;
        let mut best_f = 0.0;
        let mut best_growth = f64::NEG_INFINITY;

        // Binary search style: test f from 0.01 to 1.0
        let mut f = 0.01;
        while f <= 1.0 {
            let log_growth: f64 = pnls
                .iter()
                .map(|&pnl| {
                    let ratio = 1.0 + f * pnl / max_loss;
                    if ratio > 0.0 {
                        ratio.ln()
                    } else {
                        f64::NEG_INFINITY
                    }
                })
                .sum::<f64>()
                / n;

            if log_growth > best_growth {
                best_growth = log_growth;
                best_f = f;
            }
            f += 0.01;
        }

        best_f
    }

    fn compute_var(trades: &[backtest::TradeRecord], pct: f64) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }
        let mut pnls: Vec<f64> = trades.iter().map(|t| t.pnl_net_usd).collect();
        pnls.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (pnls.len() as f64 * pct).floor() as usize;
        pnls[idx.min(pnls.len() - 1)]
    }

    fn compute_cvar(trades: &[backtest::TradeRecord], pct: f64) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }
        let mut pnls: Vec<f64> = trades.iter().map(|t| t.pnl_net_usd).collect();
        pnls.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let cutoff = (pnls.len() as f64 * pct).ceil() as usize;
        let cutoff = cutoff.max(1).min(pnls.len());
        let sum: f64 = pnls[..cutoff].iter().sum();
        sum / cutoff as f64
    }

    fn compute_risk_of_ruin(win_rate: f64, _avg_win: f64, avg_loss: f64, capital: f64) -> f64 {
        if avg_loss <= 0.0 || capital <= 0.0 {
            return 0.0;
        }
        let edge = 2.0 * win_rate - 1.0;
        if edge <= 0.0 {
            return 100.0;
        }
        let units = capital / avg_loss;
        let base = (1.0 - edge) / (1.0 + edge);
        let ror = base.powf(units);
        (ror * 100.0).min(100.0)
    }

    fn compute_max_consecutive_losses(trades: &[backtest::TradeRecord]) -> usize {
        let mut max_streak = 0;
        let mut current = 0;
        for trade in trades {
            if trade.pnl_net_usd < 0.0 {
                current += 1;
                max_streak = max_streak.max(current);
            } else {
                current = 0;
            }
        }
        max_streak
    }

    fn compute_pnl_by_hour(trades: &[backtest::TradeRecord], tz: UserTimezone) -> Vec<(u8, f64)> {
        use chrono::Timelike;
        let mut hourly = [0.0f64; 24];
        for trade in trades {
            if let Some(dt) = chrono::DateTime::from_timestamp_millis(trade.entry_time.0 as i64) {
                let hour = match tz {
                    UserTimezone::Local => dt.with_timezone(&chrono::Local).hour(),
                    UserTimezone::Utc => dt.hour(),
                } as usize;
                hourly[hour] += trade.pnl_net_usd;
            }
        }
        hourly
            .iter()
            .enumerate()
            .map(|(h, &pnl)| (h as u8, pnl))
            .collect()
    }

    fn compute_daily_pnl(trades: &[backtest::TradeRecord], tz: UserTimezone) -> Vec<(String, f64)> {
        let mut daily: std::collections::BTreeMap<(i32, u32, u32), f64> =
            std::collections::BTreeMap::new();
        for trade in trades {
            let secs = (trade.exit_time.0 / 1000) as i64;
            if let Some(components) = tz.date_components(secs) {
                *daily.entry(components).or_insert(0.0) += trade.pnl_net_usd;
            }
        }
        daily
            .into_iter()
            .map(|((_, m, d), pnl)| (format!("{:02}/{:02}", m, d), pnl))
            .collect()
    }

    fn compute_prop_firm_results(
        trades: &[backtest::TradeRecord],
        _equity_curve: &backtest::EquityCurve,
        initial_capital: f64,
        tz: UserTimezone,
    ) -> Vec<PropFirmResult> {
        let empty_mc = PropFirmMonteCarloResult {
            num_iterations: 0,
            pass_count: 0,
            pass_rate: 0.0,
            avg_trades_to_pass: None,
            avg_trades_to_fail: None,
            median_final_pnl: 0.0,
            p5_final_pnl: 0.0,
            p95_final_pnl: 0.0,
            sample_paths: vec![],
        };

        if initial_capital <= 0.0 || trades.is_empty() {
            return PROP_FIRM_ACCOUNTS
                .iter()
                .map(|a| PropFirmResult {
                    account: a.clone(),
                    status: PropFirmStatus::Active,
                    hit_drawdown_limit: false,
                    hit_daily_limit: false,
                    worst_drawdown: 0.0,
                    worst_daily_loss: 0.0,
                    final_pnl: 0.0,
                    progress_pct: 0.0,
                    equity_curve: vec![a.size],
                    breach_trade_idx: None,
                    monte_carlo: empty_mc.clone(),
                })
                .collect();
        }

        let scale_factor = |acct_size: f64| acct_size / initial_capital;

        PROP_FIRM_ACCOUNTS
            .iter()
            .map(|acct| {
                let scale = scale_factor(acct.size);

                let mut equity = acct.size;
                let mut peak = acct.size;
                let mut worst_dd = 0.0_f64;
                let mut worst_daily_loss = 0.0_f64;
                let mut hit_profit_target = false;
                let mut hit_drawdown_limit = false;
                let mut hit_daily_limit = false;
                let mut breach_trade_idx: Option<usize> = None;
                let mut equity_curve = Vec::with_capacity(trades.len() + 1);
                equity_curve.push(acct.size);

                let mut current_day: Option<(i32, u32, u32)> = None;
                let mut daily_pnl = 0.0_f64;

                for (i, trade) in trades.iter().enumerate() {
                    let trade_day = tz.date_components((trade.exit_time.0 / 1000) as i64);
                    let scaled_pnl = trade.pnl_net_usd * scale;

                    if trade_day != current_day && current_day.is_some() {
                        if daily_pnl < 0.0 {
                            let loss = daily_pnl.abs();
                            worst_daily_loss = worst_daily_loss.max(loss);
                            if loss >= acct.daily_loss_limit {
                                if breach_trade_idx.is_none() {
                                    breach_trade_idx = Some(i);
                                }
                                hit_daily_limit = true;
                            }
                        }
                        daily_pnl = 0.0;
                    }
                    current_day = trade_day;

                    equity += scaled_pnl;
                    daily_pnl += scaled_pnl;
                    equity_curve.push(equity);

                    if equity > peak {
                        peak = equity;
                    }

                    let dd = peak - equity;
                    worst_dd = worst_dd.max(dd);

                    if dd >= acct.max_trailing_dd {
                        if breach_trade_idx.is_none() {
                            breach_trade_idx = Some(i);
                        }
                        hit_drawdown_limit = true;
                    }
                    if equity - acct.size >= acct.profit_target {
                        hit_profit_target = true;
                    }
                }

                // Final day check
                if daily_pnl < 0.0 {
                    let loss = daily_pnl.abs();
                    worst_daily_loss = worst_daily_loss.max(loss);
                    if loss >= acct.daily_loss_limit {
                        if breach_trade_idx.is_none() {
                            breach_trade_idx = Some(trades.len().saturating_sub(1));
                        }
                        hit_daily_limit = true;
                    }
                }

                let final_pnl = equity - acct.size;
                let breached = hit_drawdown_limit || hit_daily_limit;
                let status = if breached {
                    PropFirmStatus::Failed
                } else if hit_profit_target {
                    PropFirmStatus::Passed
                } else {
                    PropFirmStatus::Active
                };

                let progress_pct = if acct.profit_target > 0.0 {
                    (final_pnl / acct.profit_target * 100.0).clamp(0.0, 999.0)
                } else {
                    0.0
                };

                let monte_carlo =
                    Self::compute_prop_firm_monte_carlo(trades, acct, initial_capital);

                PropFirmResult {
                    account: acct.clone(),
                    status,
                    hit_drawdown_limit,
                    hit_daily_limit,
                    worst_drawdown: worst_dd,
                    worst_daily_loss,
                    final_pnl,
                    progress_pct,
                    equity_curve,
                    breach_trade_idx,
                    monte_carlo,
                }
            })
            .collect()
    }

    fn compute_prop_firm_monte_carlo(
        trades: &[backtest::TradeRecord],
        acct: &AccountSpec,
        initial_capital: f64,
    ) -> PropFirmMonteCarloResult {
        const NUM_ITERATIONS: usize = 1_000;
        const SAMPLE_PATHS: usize = 50;

        if trades.is_empty() || initial_capital <= 0.0 {
            return PropFirmMonteCarloResult {
                num_iterations: 0,
                pass_count: 0,
                pass_rate: 0.0,
                avg_trades_to_pass: None,
                avg_trades_to_fail: None,
                median_final_pnl: 0.0,
                p5_final_pnl: 0.0,
                p95_final_pnl: 0.0,
                sample_paths: vec![],
            };
        }

        let scale = acct.size / initial_capital;
        let pnls: Vec<f64> = trades.iter().map(|t| t.pnl_net_usd * scale).collect();
        let n = pnls.len();

        // LCG PRNG (same pattern as existing MC)
        let mut seed: u64 = 12345;
        let mut next_rand = move || -> usize {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };

        let mut pass_count = 0usize;
        let mut total_trades_to_pass = 0usize;
        let mut total_trades_to_fail = 0usize;
        let mut pass_trade_count = 0usize;
        let mut fail_trade_count = 0usize;
        let mut final_pnls = Vec::with_capacity(NUM_ITERATIONS);
        let mut sample_paths = Vec::with_capacity(SAMPLE_PATHS);

        for iter in 0..NUM_ITERATIONS {
            // Fisher-Yates shuffle
            let mut indices: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = next_rand() % (i + 1);
                indices.swap(i, j);
            }

            let mut equity = acct.size;
            let mut peak = acct.size;
            let mut passed = false;
            let mut failed = false;
            let mut completion_idx: Option<usize> = None;
            let store_path = iter < SAMPLE_PATHS;
            let mut path_equity = if store_path {
                Vec::with_capacity(n + 1)
            } else {
                Vec::new()
            };
            if store_path {
                path_equity.push(acct.size);
            }

            for (step, &idx) in indices.iter().enumerate() {
                equity += pnls[idx];
                if store_path {
                    path_equity.push(equity);
                }

                if equity > peak {
                    peak = equity;
                }

                let dd = peak - equity;

                if !passed && !failed {
                    if equity - acct.size >= acct.profit_target {
                        passed = true;
                        completion_idx = Some(step);
                    } else if dd >= acct.max_trailing_dd {
                        failed = true;
                        completion_idx = Some(step);
                    }
                }
            }

            if passed {
                pass_count += 1;
                if let Some(idx) = completion_idx {
                    total_trades_to_pass += idx + 1;
                    pass_trade_count += 1;
                }
            } else if failed && let Some(idx) = completion_idx {
                total_trades_to_fail += idx + 1;
                fail_trade_count += 1;
            }

            final_pnls.push(equity - acct.size);

            if store_path {
                // Truncate path at completion for visual clarity
                if let Some(ci) = completion_idx {
                    path_equity.truncate(ci + 2); // +1 for start, +1 inclusive
                }
                sample_paths.push(McPropFirmPath {
                    equity_curve: path_equity,
                    passed,
                    completion_idx,
                });
            }
        }

        final_pnls.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let len = final_pnls.len();
        let p5_idx = ((len as f64 * 0.05) as usize).min(len - 1);
        let p95_idx = ((len as f64 * 0.95) as usize).min(len - 1);

        PropFirmMonteCarloResult {
            num_iterations: NUM_ITERATIONS,
            pass_count,
            pass_rate: pass_count as f64 / NUM_ITERATIONS as f64,
            avg_trades_to_pass: if pass_trade_count > 0 {
                Some(total_trades_to_pass as f64 / pass_trade_count as f64)
            } else {
                None
            },
            avg_trades_to_fail: if fail_trade_count > 0 {
                Some(total_trades_to_fail as f64 / fail_trade_count as f64)
            } else {
                None
            },
            median_final_pnl: final_pnls[len / 2],
            p5_final_pnl: final_pnls[p5_idx],
            p95_final_pnl: final_pnls[p95_idx],
            sample_paths,
        }
    }
}
