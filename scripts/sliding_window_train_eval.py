#!/usr/bin/env python3
"""
Sliding Window LSTM Training and Evaluation Pipeline

This script trains LSTM models using a sliding window approach:
- Train on 3 months of NQ data (5m timeframe)
- Test on 1 month of data
- Slide the window 3 times for 3 months of testing

Usage:
    python3 sliding_window_train_eval.py
    
The script will:
1. Train models for each window
2. Run backtests on test periods
3. Create monthly performance reports with charts
4. Generate a final cumulative report with visualizations
"""

import os
import sys
import json
import subprocess
import argparse
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any
from pathlib import Path
from enum import Enum
import base64
from io import BytesIO

# Try to import matplotlib
try:
    import matplotlib
    matplotlib.use('Agg')  # Non-interactive backend
    import matplotlib.pyplot as plt
    import matplotlib.dates as mdates
    from matplotlib.gridspec import GridSpec
    import numpy as np
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False
    np = None
    print("Warning: matplotlib not installed. Charts will not be generated.")
    print("Install with: pip install matplotlib")

# Configuration
PROJECT_ROOT = Path("/data/jbutler/algo-data/kairos")
MODELS_DIR = PROJECT_ROOT / "models"
REPORTS_DIR = PROJECT_ROOT / "reports"
DATA_DIR = Path("/data/jbutler/algo-data/nq")
TIMEFRAME = "5min"
SYMBOL = "NQ"

# Style for charts
if HAS_MATPLOTLIB:
    plt.style.use('seaborn-v0_8-darkgrid')
    plt.rcParams['figure.figsize'] = (12, 8)
    plt.rcParams['font.size'] = 10
    plt.rcParams['axes.titlesize'] = 12
    plt.rcParams['axes.labelsize'] = 10

# Default training config
TRAINING_CONFIG = {
    "model_type": "lstm",
    "learning_rate": 0.001,
    "batch_size": 256,
    "epochs": 50,
    "optimizer": "Adam",
    "weight_decay": 0.0001,
    "validation_split": 0.2,
    "early_stopping_patience": 10,
    "label_config": {
        "horizon": 5,
        "long_threshold": 0.005,
        "short_threshold": 0.005,
        "warmup_bars": 20
    },
    "lstm_config": {
        "hidden_size": 64,
        "num_layers": 2,
        "dropout": 0.2,
        "bidirectional": False
    }
}


class Status(Enum):
    SUCCESS = "success"
    FAILED = "failed"
    SKIPPED = "skipped"


@dataclass
class WindowPeriod:
    """Represents a training/testing window period."""
    train_start: str  # YYYY-MM-DD
    train_end: str
    test_start: str
    test_end: str
    window_num: int
    model_name: str
    config_name: str


@dataclass
class TrainingResult:
    """Results from a training run."""
    window: WindowPeriod
    epochs_trained: int
    final_train_loss: float
    final_val_loss: Optional[float]
    early_stopped: bool
    num_samples: int
    status: Status
    output: str = ""
    error: Optional[str] = None


@dataclass
class BacktestMetrics:
    """Metrics extracted from Kairos backtest export."""
    # P&L
    net_pnl_usd: float
    gross_pnl_usd: float
    total_commission_usd: float
    net_pnl_ticks: int
    
    # Trade counts
    total_trades: int
    winning_trades: int
    losing_trades: int
    breakeven_trades: int
    
    # Win/loss statistics
    win_rate: float
    avg_win_usd: float
    avg_loss_usd: float
    profit_factor: float
    avg_rr: float
    best_trade_usd: float
    worst_trade_usd: float
    largest_win_streak: int
    largest_loss_streak: int
    
    # Drawdown
    max_drawdown_usd: float
    max_drawdown_pct: float
    
    # Risk-adjusted
    sharpe_ratio: float
    sortino_ratio: float
    calmar_ratio: float
    
    # MAE/MFE
    avg_mae_ticks: float
    avg_mfe_ticks: float
    
    # Equity
    initial_capital_usd: float
    final_equity_usd: float
    total_return_pct: float
    trading_days: int
    
    # Additional
    avg_trade_duration_ms: float
    expectancy_usd: float
    
    # Daily snapshots
    daily_pnl: Dict[str, float] = field(default_factory=dict)


@dataclass
class BacktestResult:
    """Results from a backtest run using Kairos export."""
    window: WindowPeriod
    metrics: BacktestMetrics
    status: Status
    export_path: Optional[Path] = None
    equity_curve: List[Dict] = field(default_factory=list)
    trades: List[Dict] = field(default_factory=list)
    output: str = ""
    error: Optional[str] = None


@dataclass 
class MonthlyReport:
    """Monthly performance report."""
    month: str
    window: int
    window_info: WindowPeriod  # Store the full window period
    train_period: str
    test_period: str
    model_name: str
    training: TrainingResult
    backtest: BacktestResult
    created_at: str = field(default_factory=lambda: datetime.now().isoformat())
    charts: Dict[str, str] = field(default_factory=dict)  # chart_name -> base64 encoded PNG


# =============================================================================
# CHART GENERATION FUNCTIONS
# =============================================================================

def equity_curve_to_df(equity_curve: List[Dict]) -> tuple:
    """Convert equity curve data to lists for plotting."""
    timestamps = []
    equities = []
    realized = []
    unrealized = []
    
    for point in equity_curve:
        try:
            ts_str = point.get('timestamp', '')
            # Parse ISO timestamp
            if 'T' in ts_str:
                dt = datetime.fromisoformat(ts_str.replace('Z', '+00:00'))
            else:
                dt = datetime.fromtimestamp(int(ts_str)/1000)
            timestamps.append(dt)
            equities.append(point.get('total_equity_usd', 0))
            realized.append(point.get('realized_equity_usd', 0))
            unrealized.append(point.get('unrealized_pnl_usd', 0))
        except:
            continue
    
    return timestamps, equities, realized, unrealized


def create_equity_chart(equity_curve: List[Dict], title: str = "Equity Curve") -> Optional[str]:
    """Create an equity curve chart and return as base64 encoded PNG."""
    if not HAS_MATPLOTLIB or not equity_curve:
        return None
    
    timestamps, equities, realized, unrealized = equity_curve_to_df(equity_curve)
    
    if not timestamps:
        return None
    
    fig, ax = plt.subplots(figsize=(12, 6))
    
    ax.plot(timestamps, equities, label='Total Equity', color='#2196F3', linewidth=1.5)
    ax.fill_between(timestamps, equities, alpha=0.3, color='#2196F3')
    
    # Add realized equity line
    ax.plot(timestamps, realized, label='Realized Equity', color='#4CAF50', linewidth=1, alpha=0.7)
    
    # Mark start and end
    ax.scatter([timestamps[0]], [equities[0]], color='green', s=100, zorder=5, label=f'Start: ${equities[0]:,.0f}')
    ax.scatter([timestamps[-1]], [equities[-1]], color='red', s=100, zorder=5, label=f'End: ${equities[-1]:,.0f}')
    
    ax.set_title(title, fontweight='bold', pad=20)
    ax.set_xlabel('Date')
    ax.set_ylabel('Equity ($)')
    ax.legend(loc='upper left')
    ax.grid(True, alpha=0.3)
    
    # Set y-axis limits to focus on the equity range with padding
    min_eq = min(equities)
    max_eq = max(equities)
    eq_range = max_eq - min_eq
    # Add 20% padding above and below, but at least $1000 padding
    padding = max(eq_range * 0.2, 1000)
    ax.set_ylim(min_eq - padding, max_eq + padding)
    
    # Format y-axis as currency
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'${x:,.0f}'))
    
    # Format x-axis dates
    ax.xaxis.set_major_formatter(mdates.DateFormatter('%Y-%m-%d'))
    plt.xticks(rotation=45)
    
    # Calculate return
    if equities[0] > 0:
        ret = (equities[-1] - equities[0]) / equities[0] * 100
        peak = equities[0]
        max_dd = 0
        for eq in equities:
            if eq > peak:
                peak = eq
            dd = (peak - eq) / peak * 100
            max_dd = max(max_dd, dd)
        
        textstr = f'Return: {ret:+.2f}%\nMax DD: {max_dd:.2f}%\nPeak: ${max_eq:,.0f}\nTrough: ${min_eq:,.0f}'
        props = dict(boxstyle='round', facecolor='wheat', alpha=0.8)
        ax.text(0.98, 0.02, textstr, transform=ax.transAxes, fontsize=9,
                verticalalignment='bottom', horizontalalignment='right', bbox=props)
    
    plt.tight_layout()
    
    # Convert to base64
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_drawdown_chart(equity_curve: List[Dict], title: str = "Drawdown") -> Optional[str]:
    """Create a drawdown chart."""
    if not HAS_MATPLOTLIB or not equity_curve:
        return None
    
    timestamps, equities, _, _ = equity_curve_to_df(equity_curve)
    
    if not timestamps:
        return None
    
    # Calculate drawdown
    peak = equities[0]
    drawdowns = []
    for eq in equities:
        if eq > peak:
            peak = eq
        dd = (peak - eq) / peak * 100 if peak > 0 else 0
        drawdowns.append(dd)
    
    fig, ax = plt.subplots(figsize=(12, 4))
    
    ax.fill_between(timestamps, drawdowns, alpha=0.4, color='#F44336')
    ax.plot(timestamps, drawdowns, color='#F44336', linewidth=1)
    
    ax.set_title(title, fontweight='bold', pad=20)
    ax.set_xlabel('Date')
    ax.set_ylabel('Drawdown (%)')
    ax.grid(True, alpha=0.3)
    
    # Mark max drawdown
    max_dd_idx = drawdowns.index(max(drawdowns))
    ax.scatter([timestamps[max_dd_idx]], [drawdowns[max_dd_idx]], color='darkred', s=100, zorder=5)
    ax.annotate(f'Max DD: {drawdowns[max_dd_idx]:.2f}%', 
                xy=(timestamps[max_dd_idx], drawdowns[max_dd_idx]),
                xytext=(10, 10), textcoords='offset points',
                fontsize=9, color='darkred')
    
    ax.xaxis.set_major_formatter(mdates.DateFormatter('%Y-%m-%d'))
    plt.xticks(rotation=45)
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_monthly_returns_chart(reports: List['MonthlyReport'], title: str = "Monthly Returns") -> Optional[str]:
    """Create a bar chart of monthly returns."""
    if not HAS_MATPLOTLIB:
        return None
    
    successful = [r for r in reports if r.backtest.status == Status.SUCCESS]
    
    if not successful:
        return None
    
    months = [f"W{r.window}\n({r.month})" for r in successful]
    returns = [r.backtest.metrics.total_return_pct for r in successful]
    colors = ['#4CAF50' if r >= 0 else '#F44336' for r in returns]
    
    fig, ax = plt.subplots(figsize=(10, 6))
    
    bars = ax.bar(months, returns, color=colors, edgecolor='white', linewidth=1.5)
    
    # Add value labels
    for bar, val in zip(bars, returns):
        yval = bar.get_height()
        offset = 0.5 if yval >= 0 else -1.5
        ax.text(bar.get_x() + bar.get_width()/2, yval + offset,
                f'{val:+.2f}%', ha='center', va='bottom' if yval >= 0 else 'top',
                fontsize=10, fontweight='bold')
    
    ax.axhline(y=0, color='black', linestyle='-', linewidth=0.5)
    ax.set_title(title, fontweight='bold', pad=20)
    ax.set_xlabel('Window')
    ax.set_ylabel('Return (%)')
    ax.grid(True, alpha=0.3, axis='y')
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_daily_pnl_chart(reports: List['MonthlyReport'], title: str = "Daily P&L Distribution") -> Optional[str]:
    """Create a histogram of daily P&L."""
    if not HAS_MATPLOTLIB:
        return None
    
    all_daily_pnl = []
    for r in reports:
        if r.backtest.status == Status.SUCCESS:
            all_daily_pnl.extend(list(r.backtest.metrics.daily_pnl.values()))
    
    if not all_daily_pnl:
        return None
    
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 5))
    
    # Histogram
    ax1.hist(all_daily_pnl, bins=50, color='#2196F3', edgecolor='white', alpha=0.7)
    ax1.axvline(x=0, color='red', linestyle='--', linewidth=1)
    ax1.axvline(x=sum(all_daily_pnl)/len(all_daily_pnl), color='green', linestyle='--', linewidth=1, label=f'Mean: ${sum(all_daily_pnl)/len(all_daily_pnl):,.0f}')
    ax1.set_title('Daily P&L Distribution', fontweight='bold')
    ax1.set_xlabel('P&L ($)')
    ax1.set_ylabel('Frequency')
    ax1.legend()
    ax1.grid(True, alpha=0.3)
    
    # Cumulative P&L
    sorted_pnl = sorted(all_daily_pnl)
    cumulative = [sum(sorted_pnl[:i+1]) for i in range(len(sorted_pnl))]
    ax2.plot(range(len(cumulative)), cumulative, color='#4CAF50', linewidth=2)
    ax2.fill_between(range(len(cumulative)), cumulative, alpha=0.3, color='#4CAF50')
    ax2.axhline(y=0, color='red', linestyle='--', linewidth=1)
    ax2.set_title('Cumulative Daily P&L', fontweight='bold')
    ax2.set_xlabel('Trading Day')
    ax2.set_ylabel('Cumulative P&L ($)')
    ax2.grid(True, alpha=0.3)
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_metrics_radar_chart(report: 'MonthlyReport', title: str = "Performance Metrics") -> Optional[str]:
    """Create a radar chart of key performance metrics."""
    if not HAS_MATPLOTLIB:
        return None
    
    m = report.backtest.metrics
    
    # Normalize metrics to 0-1 scale for radar chart
    metrics_names = ['Win Rate', 'Profit Factor', 'Sharpe', 'Sortino', 'Expectancy', 'Avg R:R']
    values = [
        min(m.win_rate, 1.0),  # Win rate already 0-1
        min(m.profit_factor / 3.0, 1.0) if m.profit_factor > 0 else 0,  # Normalize PF
        min(max(m.sharpe_ratio / 3.0, -1), 1.0) if m.sharpe_ratio != 0 else 0.5,
        min(max(m.sortino_ratio / 3.0, -1), 1.0) if m.sortino_ratio != 0 else 0.5,
        min(max(m.expectancy_usd / 100, -1), 1.0) if m.expectancy_usd != 0 else 0.5,
        min(m.avg_rr / 2.0, 1.0) if m.avg_rr > 0 else 0.5,
    ]
    
    # Close the polygon
    values += values[:1]
    
    angles = [n / float(len(metrics_names)) * 2 * 3.14159 for n in range(len(metrics_names))]
    angles += angles[:1]
    
    fig, ax = plt.subplots(figsize=(8, 8), subplot_kw=dict(polar=True))
    
    ax.plot(angles, values, 'o-', linewidth=2, color='#2196F3')
    ax.fill(angles, values, alpha=0.25, color='#2196F3')
    
    ax.set_xticks(angles[:-1])
    ax.set_xticklabels(metrics_names, size=10)
    ax.set_ylim(0, 1)
    
    ax.set_title(title, fontweight='bold', pad=20)
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_trade_distribution_chart(trades: List[Dict], title: str = "Trade P&L Distribution") -> Optional[str]:
    """Create a histogram of individual trade P&Ls."""
    if not HAS_MATPLOTLIB or not trades:
        return None
    
    pnl_values = [t.get('pnl_net_usd', 0) for t in trades]
    
    fig, ax = plt.subplots(figsize=(10, 5))
    
    # Color by profit/loss
    colors = ['#4CAF50' if p >= 0 else '#F44336' for p in pnl_values]
    
    ax.hist(pnl_values, bins=30, color='#2196F3', edgecolor='white', alpha=0.7)
    ax.axvline(x=0, color='black', linestyle='-', linewidth=1)
    
    winning = [p for p in pnl_values if p > 0]
    losing = [p for p in pnl_values if p < 0]
    
    if winning:
        avg_win = sum(winning) / len(winning)
        ax.axvline(x=avg_win, color='green', linestyle='--', linewidth=2, label=f'Avg Win: ${avg_win:,.0f}')
    if losing:
        avg_loss = sum(losing) / len(losing)
        ax.axvline(x=avg_loss, color='red', linestyle='--', linewidth=2, label=f'Avg Loss: ${avg_loss:,.0f}')
    
    ax.set_title(title, fontweight='bold', pad=20)
    ax.set_xlabel('P&L ($)')
    ax.set_ylabel('Frequency')
    ax.legend()
    ax.grid(True, alpha=0.3)
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_monthly_performance_matrix(reports: List['MonthlyReport'], title: str = "Performance Matrix") -> Optional[str]:
    """Create a heatmap-style performance matrix."""
    if not HAS_MATPLOTLIB or np is None:
        return None
    
    successful = [r for r in reports if r.backtest.status == Status.SUCCESS]
    
    if not successful:
        return None
    
    # Create metrics matrix
    metrics_to_show = ['total_return_pct', 'win_rate', 'profit_factor', 'sharpe_ratio', 'max_drawdown_pct']
    metric_labels = ['Return (%)', 'Win Rate', 'Profit Factor', 'Sharpe', 'Max DD (%)']
    
    data = []
    for r in successful:
        m = r.backtest.metrics
        row = [
            m.total_return_pct,
            m.win_rate * 100,
            m.profit_factor,
            m.sharpe_ratio,
            m.max_drawdown_pct
        ]
        data.append(row)
    
    if not data:
        return None
    
    fig, ax = plt.subplots(figsize=(10, 4))
    
    # Simple bar chart for each metric across windows
    x = np.arange(len(successful))
    width = 0.15
    
    for i, (metric, label) in enumerate(zip(metrics_to_show, metric_labels)):
        vals = [d[i] for d in data]
        offset = (i - len(metrics_to_show)/2) * width
        bars = ax.bar([xi + offset for xi in x], vals, width, label=label)
    
    ax.set_xlabel('Window')
    ax.set_ylabel('Value')
    ax.set_title(title, fontweight='bold', pad=20)
    ax.set_xticks(x)
    ax.set_xticklabels([f"W{r.window}" for r in successful])
    ax.legend(loc='upper right', ncol=5)
    ax.grid(True, alpha=0.3, axis='y')
    ax.axhline(y=0, color='black', linestyle='-', linewidth=0.5)
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def create_equity_vs_benchmark_chart(reports: List['MonthlyReport'], title: str = "Cumulative Equity vs Benchmark") -> Optional[str]:
    """Create a chart showing cumulative equity vs buy-and-hold benchmark."""
    if not HAS_MATPLOTLIB:
        return None
    
    successful = [r for r in reports if r.backtest.status == Status.SUCCESS]
    
    if not successful:
        return None
    
    # Aggregate equity curves
    all_points = []
    for r in successful:
        for point in r.backtest.equity_curve:
            try:
                ts_str = point.get('timestamp', '')
                if 'T' in ts_str:
                    dt = datetime.fromisoformat(ts_str.replace('Z', '+00:00'))
                else:
                    dt = datetime.fromtimestamp(int(ts_str)/1000)
                all_points.append((dt, point.get('total_equity_usd', 0), r.window))
            except:
                continue
    
    if not all_points:
        return None
    
    # Sort by timestamp
    all_points.sort(key=lambda x: x[0])
    
    timestamps = [p[0] for p in all_points]
    equities = [p[1] for p in all_points]
    
    fig, ax = plt.subplots(figsize=(14, 6))
    
    # Plot equity
    ax.plot(timestamps, equities, label='Strategy Equity', color='#2196F3', linewidth=2)
    ax.fill_between(timestamps, equities, alpha=0.3, color='#2196F3')
    
    # Calculate simple benchmark (just buy and hold based on starting equity)
    if successful and all_points:
        initial = successful[0].backtest.metrics.initial_capital_usd
        # Simple benchmark line at initial capital
        ax.axhline(y=initial, color='gray', linestyle='--', linewidth=1, label='Initial Capital')
    
    # Set y-axis limits to focus on the equity range with padding
    min_eq = min(equities)
    max_eq = max(equities)
    eq_range = max_eq - min_eq
    padding = max(eq_range * 0.2, 1000)
    ax.set_ylim(min_eq - padding, max_eq + padding)
    
    ax.set_title(title, fontweight='bold', pad=20)
    ax.set_xlabel('Date')
    ax.set_ylabel('Equity ($)')
    ax.legend(loc='upper left')
    ax.grid(True, alpha=0.3)
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'${x:,.0f}'))
    
    ax.xaxis.set_major_formatter(mdates.DateFormatter('%Y-%m-%d'))
    plt.xticks(rotation=45)
    
    plt.tight_layout()
    
    buf = BytesIO()
    plt.savefig(buf, format='png', dpi=100, bbox_inches='tight', facecolor='white')
    buf.seek(0)
    img_base64 = base64.b64encode(buf.read()).decode('utf-8')
    plt.close(fig)
    
    return img_base64


def generate_all_charts_for_report(report: MonthlyReport) -> Dict[str, str]:
    """Generate all charts for a monthly report."""
    charts = {}
    
    # Equity curve
    if report.backtest.equity_curve:
        charts['equity_curve'] = create_equity_chart(
            report.backtest.equity_curve,
            f"Window {report.window} - Equity Curve ({report.month})"
        )
        charts['drawdown'] = create_drawdown_chart(
            report.backtest.equity_curve,
            f"Window {report.window} - Drawdown ({report.month})"
        )
    
    # Trade distribution
    if report.backtest.trades:
        charts['trade_distribution'] = create_trade_distribution_chart(
            report.backtest.trades,
            f"Window {report.window} - Trade P&L Distribution ({report.month})"
        )
    
    # Metrics radar
    charts['metrics_radar'] = create_metrics_radar_chart(
        report,
        f"Window {report.window} - Performance Metrics ({report.month})"
    )
    
    return {k: v for k, v in charts.items() if v is not None}


# =============================================================================
# REPORT GENERATION FUNCTIONS
# =============================================================================

# Dark theme CSS styles
DARK_THEME_STYLES = """
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body { 
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #1a1a2e;
        padding: 20px;
        color: #e0e0e0;
    }
    .container { max-width: 1200px; margin: 0 auto; }
    .header {
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
        padding: 30px;
        border-radius: 10px;
        margin-bottom: 20px;
    }
    .header h1 { font-size: 28px; margin-bottom: 10px; }
    .header p { opacity: 0.9; font-size: 14px; }
    .card {
        background: #16213e;
        border-radius: 10px;
        padding: 20px;
        margin-bottom: 20px;
        border: 1px solid #2d3a5c;
    }
    .card h2 {
        color: #00d9ff;
        border-bottom: 2px solid #00d9ff;
        padding-bottom: 10px;
        margin-bottom: 15px;
        font-size: 18px;
    }
    .metrics-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: 15px;
    }
    .metric {
        background: #1a1a2e;
        padding: 15px;
        border-radius: 8px;
        text-align: center;
        border: 1px solid #2d3a5c;
    }
    .metric-value {
        font-size: 24px;
        font-weight: bold;
        color: #00d9ff;
    }
    .metric-label {
        font-size: 12px;
        color: #888;
        margin-top: 5px;
        text-transform: uppercase;
    }
    .metric.positive .metric-value { color: #00ff88; }
    .metric.negative .metric-value { color: #ff4757; }
    .chart-container {
        margin: 20px 0;
        text-align: center;
    }
    .chart-container img {
        max-width: 100%;
        border-radius: 8px;
        border: 1px solid #2d3a5c;
    }
    .summary-table {
        width: 100%;
        border-collapse: collapse;
    }
    .summary-table th, .summary-table td {
        padding: 12px;
        text-align: left;
        border-bottom: 1px solid #2d3a5c;
    }
    .summary-table th { color: #00d9ff; font-weight: 600; }
    .summary-table tr:hover { background: #1e2a4a; }
    .summary-table td { color: #e0e0e0; }
    .footer {
        text-align: center;
        color: #666;
        font-size: 12px;
        margin-top: 20px;
        padding: 20px;
        border-top: 1px solid #2d3a5c;
    }
    .badge {
        display: inline-block;
        padding: 4px 12px;
        border-radius: 20px;
        font-size: 12px;
        font-weight: bold;
    }
    .badge-success { background: #00ff88; color: #1a1a2e; }
    .badge-danger { background: #ff4757; color: white; }
    .two-col {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 20px;
    }
    @media (max-width: 768px) {
        .two-col { grid-template-columns: 1fr; }
    }
"""


def generate_html_report(report: MonthlyReport) -> str:
    """Generate a standalone HTML report with embedded charts."""
    m = report.backtest.metrics
    
    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Backtest Report - Window {report.window}</title>
    <style>{DARK_THEME_STYLES}</style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📊 Backtest Report - Window {report.window}</h1>
            <p>Test Period: {report.test_period} | Model: {report.model_name}</p>
            <p>Generated: {report.created_at}</p>
        </div>
        
        <div class="card">
            <h2>Performance Summary</h2>
            <div class="metrics-grid">
                <div class="metric {'positive' if m.total_return_pct >= 0 else 'negative'}">
                    <div class="metric-value">{m.total_return_pct:+.2f}%</div>
                    <div class="metric-label">Total Return</div>
                </div>
                <div class="metric {'positive' if m.net_pnl_usd >= 0 else 'negative'}">
                    <div class="metric-value">${m.net_pnl_usd:+,.2f}</div>
                    <div class="metric-label">Net P&L</div>
                </div>
                <div class="metric">
                    <div class="metric-value">${m.final_equity_usd:,.2f}</div>
                    <div class="metric-label">Final Equity</div>
                </div>
                <div class="metric negative">
                    <div class="metric-value">{m.max_drawdown_pct:.2f}%</div>
                    <div class="metric-label">Max Drawdown</div>
                </div>
            </div>
        </div>
        
        <div class="card">
            <h2>Trade Statistics</h2>
            <table class="summary-table">
                <tr><th>Metric</th><th>Value</th></tr>
                <tr><td>Total Trades</td><td>{m.total_trades}</td></tr>
                <tr><td>Winning Trades</td><td>{m.winning_trades} ({m.win_rate*100:.1f}%)</td></tr>
                <tr><td>Losing Trades</td><td>{m.losing_trades}</td></tr>
                <tr><td>Profit Factor</td><td>{m.profit_factor:.2f}</td></tr>
                <tr><td>Avg Win</td><td>${m.avg_win_usd:,.2f}</td></tr>
                <tr><td>Avg Loss</td><td>${m.avg_loss_usd:,.2f}</td></tr>
                <tr><td>Best Trade</td><td>${m.best_trade_usd:,.2f}</td></tr>
                <tr><td>Worst Trade</td><td>${m.worst_trade_usd:,.2f}</td></tr>
                <tr><td>Avg R:R</td><td>{m.avg_rr:.2f}</td></tr>
            </table>
        </div>
        
        <div class="card">
            <h2>Risk Metrics</h2>
            <div class="metrics-grid">
                <div class="metric">
                    <div class="metric-value">{m.sharpe_ratio:.2f}</div>
                    <div class="metric-label">Sharpe Ratio</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{m.sortino_ratio:.2f}</div>
                    <div class="metric-label">Sortino Ratio</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{m.calmar_ratio:.2f}</div>
                    <div class="metric-label">Calmar Ratio</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{m.expectancy_usd:.2f}</div>
                    <div class="metric-label">Expectancy/Trade</div>
                </div>
            </div>
        </div>
        </div>
        
        <div class="card">
            <h2>Performance Summary</h2>
            <div class="metrics-grid">
                <div class="metric {'positive' if m.total_return_pct >= 0 else 'negative'}">
                    <div class="metric-value">{m.total_return_pct:+.2f}%</div>
                    <div class="metric-label">Total Return</div>
                </div>
                <div class="metric">
                    <div class="metric-value">${m.net_pnl_usd:+,.2f}</div>
                    <div class="metric-label">Net P&L</div>
                </div>
                <div class="metric">
                    <div class="metric-value">${m.final_equity_usd:,.2f}</div>
                    <div class="metric-label">Final Equity</div>
                </div>
                <div class="metric negative">
                    <div class="metric-value">{m.max_drawdown_pct:.2f}%</div>
                    <div class="metric-label">Max Drawdown</div>
                </div>
            </div>
        </div>
        
        <div class="card">
            <h2>Trade Statistics</h2>
            <table class="summary-table">
                <tr><th>Metric</th><th>Value</th></tr>
                <tr><td>Total Trades</td><td>{m.total_trades}</td></tr>
                <tr><td>Winning Trades</td><td>{m.winning_trades} ({m.win_rate*100:.1f}%)</td></tr>
                <tr><td>Losing Trades</td><td>{m.losing_trades}</td></tr>
                <tr><td>Profit Factor</td><td>{m.profit_factor:.2f}</td></tr>
                <tr><td>Avg Win</td><td>${m.avg_win_usd:,.2f}</td></tr>
                <tr><td>Avg Loss</td><td>${m.avg_loss_usd:,.2f}</td></tr>
                <tr><td>Best Trade</td><td>${m.best_trade_usd:,.2f}</td></tr>
                <tr><td>Worst Trade</td><td>${m.worst_trade_usd:,.2f}</td></tr>
                <tr><td>Avg R:R</td><td>{m.avg_rr:.2f}</td></tr>
            </table>
        </div>
        
        <div class="card">
            <h2>Risk Metrics</h2>
            <div class="metrics-grid">
                <div class="metric">
                    <div class="metric-value">{m.sharpe_ratio:.2f}</div>
                    <div class="metric-label">Sharpe Ratio</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{m.sortino_ratio:.2f}</div>
                    <div class="metric-label">Sortino Ratio</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{m.calmar_ratio:.2f}</div>
                    <div class="metric-label">Calmar Ratio</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{m.expectancy_usd:.2f}</div>
                    <div class="metric-label">Expectancy/Trade</div>
                </div>
            </div>
        </div>
"""
    
    # Add charts
    if 'equity_curve' in report.charts:
        html += f"""
        <div class="card">
            <h2>Equity Curve</h2>
            <div class="chart-container">
                <img src="data:image/png;base64,{report.charts['equity_curve']}" alt="Equity Curve">
            </div>
        </div>
"""
    
    if 'drawdown' in report.charts:
        html += f"""
        <div class="card">
            <h2>Drawdown</h2>
            <div class="chart-container">
                <img src="data:image/png;base64,{report.charts['drawdown']}" alt="Drawdown">
            </div>
        </div>
"""
    
    if 'trade_distribution' in report.charts:
        html += f"""
        <div class="card">
            <h2>Trade P&L Distribution</h2>
            <div class="chart-container">
                <img src="data:image/png;base64,{report.charts['trade_distribution']}" alt="Trade Distribution">
            </div>
        </div>
"""
    
    if 'metrics_radar' in report.charts:
        html += f"""
        <div class="card">
            <h2>Performance Metrics Radar</h2>
            <div class="chart-container">
                <img src="data:image/png;base64,{report.charts['metrics_radar']}" alt="Metrics Radar">
            </div>
        </div>
"""
    
    html += f"""
        <div class="card">
            <h2>Training Info</h2>
            <table class="summary-table">
                <tr><th>Metric</th><th>Value</th></tr>
                <tr><td>Epochs Trained</td><td>{report.training.epochs_trained}</td></tr>
                <tr><td>Final Train Loss</td><td>{report.training.final_train_loss:.4f}</td></tr>
                <tr><td>Final Val Loss</td><td>{report.training.final_val_loss if report.training.final_val_loss else 'N/A'}</td></tr>
                <tr><td>Training Status</td><td><span class="badge {'badge-success' if report.training.status == Status.SUCCESS else 'badge-danger'}">{report.training.status.value}</span></td></tr>
            </table>
        </div>
        
        <div class="footer">
            Generated by Kairos Sliding Window Pipeline | {report.created_at}
        </div>
    </div>
</body>
</html>
"""
    return html


def generate_cumulative_html_report(reports: List[MonthlyReport], capital: float) -> str:
    """Generate a cumulative HTML report with all charts."""
    successful = [r for r in reports if r.backtest.status == Status.SUCCESS]
    n = len(successful)
    
    if n == 0:
        return "<html><body><h1>No successful backtests</h1></body></html>"
    
    # Calculate cumulative metrics
    returns = [r.backtest.metrics.total_return_pct for r in successful]
    cumulative_return = 1.0
    for pct in returns:
        cumulative_return *= (1 + pct / 100)
    cumulative_return = (cumulative_return - 1) * 100
    
    total_pnl = sum(r.backtest.metrics.net_pnl_usd for r in successful)
    total_trades = sum(r.backtest.metrics.total_trades for r in successful)
    total_winning = sum(r.backtest.metrics.winning_trades for r in successful)
    avg_win_rate = sum(r.backtest.metrics.win_rate for r in successful) / n
    avg_pf = sum(r.backtest.metrics.profit_factor for r in successful) / n
    avg_sharpe = sum(r.backtest.metrics.sharpe_ratio for r in successful) / n
    avg_max_dd = sum(r.backtest.metrics.max_drawdown_pct for r in successful) / n
    
    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Cumulative Performance Report - Sliding Window LSTM</title>
    <style>{DARK_THEME_STYLES}</style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📈 Sliding Window LSTM - Cumulative Performance Report</h1>
            <p>Analysis Period: {reports[0].window_info.train_start} to {reports[-1].window_info.test_end}</p>
            <p>Windows: {n}/{len(reports)} successful | Timeframe: {TIMEFRAME} | Symbol: {SYMBOL}</p>
        </div>
        
        <div class="card">
            <h2>Cumulative Performance</h2>
            <div class="metrics-grid">
                <div class="metric {'positive' if cumulative_return >= 0 else 'negative'}">
                    <div class="metric-value">{cumulative_return:+.2f}%</div>
                    <div class="metric-label">Cumulative Return</div>
                </div>
                <div class="metric {'positive' if total_pnl >= 0 else 'negative'}">
                    <div class="metric-value">${total_pnl:+,.2f}</div>
                    <div class="metric-label">Total P&L</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{total_trades}</div>
                    <div class="metric-label">Total Trades</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{avg_win_rate*100:.1f}%</div>
                    <div class="metric-label">Avg Win Rate</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{avg_pf:.2f}</div>
                    <div class="metric-label">Avg Profit Factor</div>
                </div>
                <div class="metric">
                    <div class="metric-value">{avg_sharpe:.2f}</div>
                    <div class="metric-label">Avg Sharpe Ratio</div>
                </div>
                <div class="metric negative">
                    <div class="metric-value">{avg_max_dd:.2f}%</div>
                    <div class="metric-label">Avg Max Drawdown</div>
                </div>
            </div>
        </div>
        
        <div class="two-col">
            <div class="card">
                <h2>Per-Window Summary</h2>
                <table>
                    <thead>
                        <tr>
                            <th>Window</th>
                            <th>Test Period</th>
                            <th>Return</th>
                            <th>Trades</th>
                            <th>Win%</th>
                            <th>Sharpe</th>
                        </tr>
                    </thead>
                    <tbody>
"""
    
    for r in reports:
        status_icon = "✓" if r.backtest.status == Status.SUCCESS else "✗"
        html += f"""
                        <tr>
                            <td>{status_icon} W{r.window}</td>
                            <td>{r.month}</td>
                            <td style="color: {'green' if r.backtest.metrics.total_return_pct >= 0 else 'red'}">{r.backtest.metrics.total_return_pct:+.2f}%</td>
                            <td>{r.backtest.metrics.total_trades}</td>
                            <td>{r.backtest.metrics.win_rate*100:.1f}%</td>
                            <td>{r.backtest.metrics.sharpe_ratio:.2f}</td>
                        </tr>
"""
    
    html += """
                    </tbody>
                </table>
            </div>
            
            <div class="card">
                <h2>Training Summary</h2>
                <table>
                    <thead>
                        <tr>
                            <th>Window</th>
                            <th>Epochs</th>
                            <th>Train Loss</th>
                            <th>Val Loss</th>
                            <th>Samples</th>
                        </tr>
                    </thead>
                    <tbody>
"""
    
    for r in reports:
        status_icon = "✓" if r.training.status == Status.SUCCESS else "✗"
        val_str = f"{r.training.final_val_loss:.4f}" if r.training.final_val_loss else "N/A"
        html += f"""
                        <tr>
                            <td>{status_icon} W{r.window}</td>
                            <td>{r.training.epochs_trained}</td>
                            <td>{r.training.final_train_loss:.4f}</td>
                            <td>{val_str}</td>
                            <td>{r.training.num_samples}</td>
                        </tr>
"""
    
    html += """
                    </tbody>
                </table>
            </div>
        </div>
"""
    
    # Add cumulative charts
    html += """
        <div class="card">
            <h2>Monthly Returns</h2>
            <div class="chart-container">
"""
    if hasattr(sys.modules[__name__], 'monthly_returns_chart_base64') and monthly_returns_chart_base64:
        html += f'<img src="data:image/png;base64,{monthly_returns_chart_base64}" alt="Monthly Returns">'
    html += """
            </div>
        </div>
        
        <div class="card">
            <h2>Daily P&L Analysis</h2>
            <div class="chart-container">
"""
    if hasattr(sys.modules[__name__], 'daily_pnl_chart_base64') and daily_pnl_chart_base64:
        html += f'<img src="data:image/png;base64,{daily_pnl_chart_base64}" alt="Daily P&L">'
    html += """
            </div>
        </div>
        
        <div class="card">
            <h2>Cumulative Equity</h2>
            <div class="chart-container">
"""
    if hasattr(sys.modules[__name__], 'cumulative_equity_chart_base64') and cumulative_equity_chart_base64:
        html += f'<img src="data:image/png;base64,{cumulative_equity_chart_base64}" alt="Cumulative Equity">'
    html += """
            </div>
        </div>
        
        <div class="footer">
            Generated by Kairos Sliding Window Pipeline | """ + datetime.now().isoformat() + """
        </div>
    </div>
</body>
</html>
"""
    return html


# Global variables for cumulative charts
monthly_returns_chart_base64 = None
daily_pnl_chart_base64 = None
cumulative_equity_chart_base64 = None


# =============================================================================
# REMAINING FUNCTIONS (parse_args, run_training, etc.)
# =============================================================================

def parse_args():
    parser = argparse.ArgumentParser(description="Sliding Window LSTM Training and Evaluation")
    parser.add_argument("--train-months", type=int, default=3,
                        help="Number of months for training (default: 3)")
    parser.add_argument("--test-months", type=int, default=1,
                        help="Number of months for testing (default: 1)")
    parser.add_argument("--num-windows", type=int, default=3,
                        help="Number of sliding windows (default: 3)")
    parser.add_argument("--start-date", type=str, default="2021-03-01",
                        help="Start date for first training window (YYYY-MM-DD)")
    parser.add_argument("--timeframe", type=str, default="5min",
                        help="Candle timeframe (default: 5min)")
    parser.add_argument("--symbol", type=str, default="NQ",
                        help="Futures symbol (default: NQ)")
    parser.add_argument("--epochs", type=int, default=50,
                        help="Number of training epochs (default: 50)")
    parser.add_argument("--skip-training", action="store_true",
                        help="Skip training, use existing models")
    parser.add_argument("--skip-backtest", action="store_true",
                        help="Skip backtesting")
    parser.add_argument("--capital", type=float, default=100000.0,
                        help="Initial capital for backtest (default: 100000)")
    parser.add_argument("--verbose", action="store_true",
                        help="Verbose output")
    parser.add_argument("--data-dir", type=str, default=None,
                        help=f"Data directory (default: {DATA_DIR})")
    parser.add_argument("--skip-charts", action="store_true",
                        help="Skip chart generation")
    return parser.parse_args()


def ensure_dirs():
    """Ensure required directories exist."""
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    REPORTS_DIR.mkdir(parents=True, exist_ok=True)
    (REPORTS_DIR / "monthly").mkdir(parents=True, exist_ok=True)
    print(f"Directories ensured: {MODELS_DIR}, {REPORTS_DIR}")


def save_training_config(config_path: Path):
    """Save training configuration to JSON file."""
    with open(config_path, 'w') as f:
        json.dump(TRAINING_CONFIG, f, indent=2)


def get_month_date(date_str: str, months: int) -> str:
    """Add or subtract months from a date string."""
    dt = datetime.strptime(date_str, "%Y-%m-%d")
    if months > 0:
        year = dt.year + (dt.month + months - 1) // 12
        month = (dt.month + months - 1) % 12 + 1
        first_of_month = datetime(year, month, 1)
        last_of_prev_month = first_of_month - timedelta(days=1)
        return last_of_prev_month.strftime("%Y-%m-%d")
    else:
        year = dt.year + (dt.month + months - 1) // 12
        month = (dt.month + months - 1) % 12 + 1
        first_of_month = datetime(year, month, 1)
        last_of_month = first_of_month.replace(day=28) + timedelta(days=4)
        last_of_month = last_of_month.replace(day=1) - timedelta(days=1)
        return last_of_month.strftime("%Y-%m-%d")


def generate_windows(args) -> List[WindowPeriod]:
    """Generate training/testing windows."""
    windows = []
    current_start = datetime.strptime(args.start_date, "%Y-%m-%d")
    data_dir = Path(args.data_dir) if args.data_dir else DATA_DIR
    
    for i in range(args.num_windows):
        train_start = current_start.strftime("%Y-%m-%d")
        train_end = get_month_date(train_start, args.train_months)
        
        test_start_dt = datetime.strptime(train_end, "%Y-%m-%d") + timedelta(days=1)
        test_start = test_start_dt.strftime("%Y-%m-%d")
        test_end = get_month_date(test_start, args.test_months)
        
        window_num = i + 1
        model_name = f"nq_lstm_window{window_num:02d}"
        config_name = f"config_window{window_num:02d}.json"
        
        window = WindowPeriod(
            train_start=train_start,
            train_end=train_end,
            test_start=test_start,
            test_end=test_end,
            window_num=window_num,
            model_name=model_name,
            config_name=config_name
        )
        windows.append(window)
        
        next_test_start = datetime.strptime(test_end, "%Y-%m-%d") + timedelta(days=1)
        current_start = next_test_start
        
        print(f"\nWindow {window_num}:")
        print(f"  Train: {train_start} to {train_end}")
        print(f"  Test:  {test_start} to {test_end}")
        print(f"  Model: {model_name}")
    
    return windows


def run_training(window: WindowPeriod, epochs: int, verbose: bool) -> TrainingResult:
    """Run training for a single window."""
    print(f"\n{'='*60}")
    print(f"TRAINING Window {window.window_num}")
    print(f"{'='*60}")
    print(f"Period: {window.train_start} to {window.train_end}")
    
    config_path = MODELS_DIR / window.config_name
    save_training_config(config_path)
    
    model_path = MODELS_DIR / window.model_name
    
    cmd = [
        "./target/debug/kairos",
        "ml", "train",
        "--config", str(config_path),
        "--data-dir", str(DATA_DIR),
        "--output", str(model_path),
        "--symbol", SYMBOL,
        "--start", window.train_start,
        "--end", window.train_end,
        "--timeframe", TIMEFRAME,
        "--epochs", str(epochs),
    ]
    
    if verbose:
        cmd.append("--verbose")
    
    print(f"Command: {' '.join(cmd)}")
    
    try:
        result = subprocess.run(
            cmd,
            cwd=PROJECT_ROOT,
            capture_output=True,
            text=True,
            timeout=3600
        )
        
        output = result.stdout + result.stderr
        
        epochs_trained = epochs
        final_train_loss = 0.0
        final_val_loss = None
        early_stopped = False
        num_samples = 0
        
        for line in output.split('\n'):
            if 'Epochs trained:' in line:
                try: epochs_trained = int(line.split(':')[-1].strip())
                except: pass
            elif 'Final train loss:' in line:
                try: final_train_loss = float(line.split(':')[-1].strip())
                except: pass
            elif 'Final val loss:' in line:
                try: final_val_loss = float(line.split(':')[-1].strip())
                except: pass
            elif 'Total samples:' in line:
                try: num_samples = int(line.split(':')[-1].strip())
                except: pass
            elif 'Early stopped:' in line:
                early_stopped = 'true' in line.lower()
        
        if result.returncode == 0:
            status = Status.SUCCESS
            print(f"✓ Training completed successfully")
            print(f"  Epochs trained: {epochs_trained}")
            print(f"  Final train loss: {final_train_loss:.4f}")
            if final_val_loss: print(f"  Final val loss: {final_val_loss:.4f}")
            print(f"  Model saved: {model_path}")
        else:
            status = Status.FAILED
            print(f"✗ Training failed with exit code {result.returncode}")
        
        return TrainingResult(
            window=window, epochs_trained=epochs_trained,
            final_train_loss=final_train_loss, final_val_loss=final_val_loss,
            early_stopped=early_stopped, num_samples=num_samples,
            status=status, output=output[:2000]
        )
        
    except subprocess.TimeoutExpired:
        return TrainingResult(window=window, epochs_trained=0, final_train_loss=0.0,
            final_val_loss=None, early_stopped=False, num_samples=0,
            status=Status.FAILED, error="Training timed out after 1 hour")
    except Exception as e:
        return TrainingResult(window=window, epochs_trained=0, final_train_loss=0.0,
            final_val_loss=None, early_stopped=False, num_samples=0,
            status=Status.FAILED, error=str(e))


def parse_backtest_export(export_data: Dict) -> BacktestMetrics:
    """Parse the Kairos backtest export into BacktestMetrics."""
    metrics = export_data.get('metrics', {})
    
    daily_pnl = {}
    for snap in export_data.get('daily_snapshots', []):
        date = snap.get('date', '')[:10]
        daily_pnl[date] = snap.get('realized_pnl', 0.0)
    
    return BacktestMetrics(
        net_pnl_usd=metrics.get('net_pnl_usd', 0.0),
        gross_pnl_usd=metrics.get('gross_pnl_usd', 0.0),
        total_commission_usd=metrics.get('total_commission_usd', 0.0),
        net_pnl_ticks=metrics.get('net_pnl_ticks', 0),
        total_trades=metrics.get('total_trades', 0),
        winning_trades=metrics.get('winning_trades', 0),
        losing_trades=metrics.get('losing_trades', 0),
        breakeven_trades=metrics.get('breakeven_trades', 0),
        win_rate=metrics.get('win_rate', 0.0),
        avg_win_usd=metrics.get('avg_win_usd', 0.0),
        avg_loss_usd=metrics.get('avg_loss_usd', 0.0),
        profit_factor=metrics.get('profit_factor', 0.0),
        avg_rr=metrics.get('avg_rr', 0.0),
        best_trade_usd=metrics.get('best_trade_usd', 0.0),
        worst_trade_usd=metrics.get('worst_trade_usd', 0.0),
        largest_win_streak=metrics.get('largest_win_streak', 0),
        largest_loss_streak=metrics.get('largest_loss_streak', 0),
        max_drawdown_usd=metrics.get('max_drawdown_usd', 0.0),
        max_drawdown_pct=metrics.get('max_drawdown_pct', 0.0),
        sharpe_ratio=metrics.get('sharpe_ratio', 0.0),
        sortino_ratio=metrics.get('sortino_ratio', 0.0),
        calmar_ratio=metrics.get('calmar_ratio', 0.0),
        avg_mae_ticks=metrics.get('avg_mae_ticks', 0.0),
        avg_mfe_ticks=metrics.get('avg_mfe_ticks', 0.0),
        initial_capital_usd=metrics.get('initial_capital_usd', 0.0),
        final_equity_usd=metrics.get('final_equity_usd', 0.0),
        total_return_pct=metrics.get('total_return_pct', 0.0),
        trading_days=metrics.get('trading_days', 0),
        avg_trade_duration_ms=metrics.get('avg_trade_duration_ms', 0.0),
        expectancy_usd=metrics.get('expectancy_usd', 0.0),
        daily_pnl=daily_pnl
    )


def run_backtest(window: WindowPeriod, capital: float, verbose: bool, skip_charts: bool = False) -> BacktestResult:
    """Run backtest for a single window using Kairos export."""
    print(f"\n{'='*60}")
    print(f"BACKTEST Window {window.window_num}")
    print(f"{'='*60}")
    print(f"Period: {window.test_start} to {window.test_end}")
    
    model_path = MODELS_DIR / f"{window.model_name}.safetensors"
    strategy_config_path = MODELS_DIR / f"{window.model_name}_strategy.json"
    export_path = REPORTS_DIR / "monthly" / f"window_{window.window_num:02d}_export.json"
    
    if not strategy_config_path.exists():
        strategy_config = {
            "id": window.model_name,
            "name": f"NQ LSTM Window {window.window_num}",
            "feature_config": {
                "features": [
                    {"study_key": "sma_20", "output_field": "line"},
                    {"study_key": "sma_50", "output_field": "line"},
                    {"study_key": "ema_12", "output_field": "line"},
                    {"study_key": "ema_26", "output_field": "line"},
                    {"study_key": "rsi", "output_field": "value"},
                    {"study_key": "atr", "output_field": "value"},
                    {"study_key": "macd", "output_field": "lines.0"},
                    {"study_key": "macd_signal", "output_field": "lines.1"},
                    {"study_key": "macd_hist", "output_field": "histogram"},
                    {"study_key": "bollinger_upper", "output_field": "band.upper"},
                    {"study_key": "bollinger_lower", "output_field": "band.lower"},
                    {"study_key": "vwap", "output_field": "value"}
                ],
                "lookback_periods": 20,
                "normalization": "none"
            },
            # Higher thresholds for more selective trading
            "signal_threshold_long": 0.55,
            "signal_threshold_short": 0.55,
            "min_confidence": 0.50,
            # Wider stops for more realistic position trading
            "sl_tp": {"stop_loss_ticks": 50, "take_profit_ticks": 75, "use_atr_based": False}
        }
        with open(strategy_config_path, 'w') as f:
            json.dump(strategy_config, f, indent=2)
    
    cmd = [
        "./target/debug/kairos", "backtest",
        "--symbol", SYMBOL,
        "--start", window.test_start,
        "--end", window.test_end,
        "--strategy", "ml",
        "--model-path", str(model_path),
        "--strategy-config", str(strategy_config_path),
        "--timeframe", TIMEFRAME,
        "--data-dir", str(DATA_DIR),
        "--capital", str(capital),
        "--format", "json",
        "--export", str(export_path),
    ]
    
    if verbose: cmd.append("--verbose")
    
    print(f"Command: {' '.join(cmd)}")
    
    default_metrics = BacktestMetrics(
        net_pnl_usd=0, gross_pnl_usd=0, total_commission_usd=0, net_pnl_ticks=0,
        total_trades=0, winning_trades=0, losing_trades=0, breakeven_trades=0,
        win_rate=0, avg_win_usd=0, avg_loss_usd=0, profit_factor=0, avg_rr=0,
        best_trade_usd=0, worst_trade_usd=0, largest_win_streak=0,
        largest_loss_streak=0, max_drawdown_usd=0, max_drawdown_pct=0,
        sharpe_ratio=0, sortino_ratio=0, calmar_ratio=0, avg_mae_ticks=0,
        avg_mfe_ticks=0, initial_capital_usd=capital, final_equity_usd=capital,
        total_return_pct=0, trading_days=0, avg_trade_duration_ms=0, expectancy_usd=0
    )
    
    try:
        result = subprocess.run(cmd, cwd=PROJECT_ROOT, capture_output=True, text=True, timeout=1800)
        output = result.stdout + result.stderr
        
        if result.returncode == 0 and export_path.exists():
            with open(export_path, 'r') as f:
                export_data = json.load(f)
            
            metrics = parse_backtest_export(export_data)
            equity_curve = export_data.get('equity_curve', [])
            trades = export_data.get('trades', [])
            
            print(f"✓ Backtest completed successfully")
            print(f"  Final Equity: ${metrics.final_equity_usd:,.2f}")
            print(f"  Return: {metrics.total_return_pct:.2f}%")
            print(f"  Max Drawdown: {metrics.max_drawdown_pct:.2f}%")
            print(f"  Trades: {metrics.total_trades}")
            print(f"  Win Rate: {metrics.win_rate * 100:.1f}%")
            print(f"  Profit Factor: {metrics.profit_factor:.2f}")
            print(f"  Sharpe: {metrics.sharpe_ratio:.2f}")
            print(f"  Export saved: {export_path}")
            
            return BacktestResult(
                window=window, metrics=metrics, status=Status.SUCCESS,
                export_path=export_path, equity_curve=equity_curve,
                trades=trades, output=output[:1000]
            )
        else:
            print(f"✗ Backtest failed with exit code {result.returncode}")
            return BacktestResult(window=window, metrics=default_metrics,
                status=Status.FAILED, error=f"Exit code: {result.returncode}",
                output=output[:1000])
        
    except subprocess.TimeoutExpired:
        return BacktestResult(window=window, metrics=default_metrics,
            status=Status.FAILED, error="Backtest timed out after 30 minutes")
    except Exception as e:
        return BacktestResult(window=window, metrics=default_metrics,
            status=Status.FAILED, error=str(e))


def create_monthly_report(report: MonthlyReport, skip_charts: bool = False) -> Dict[str, Any]:
    """Create and save a monthly performance report with charts."""
    print(f"\nCreating monthly report for Window {report.window}...")
    
    m = report.backtest.metrics
    window = report.window_info  # Get the WindowPeriod object
    
    # Generate charts if not skipped
    if not skip_charts:
        print("  Generating charts...")
        report.charts = generate_all_charts_for_report(report)
    
    # Save HTML report with charts
    html_path = REPORTS_DIR / "monthly" / f"window_{report.window:02d}_{report.month}.html"
    html_content = generate_html_report(report)
    with open(html_path, 'w') as f:
        f.write(html_content)
    print(f"  HTML report saved: {html_path}")
    
    # Save JSON report
    report_data = {
        "report_type": "monthly_window_performance",
        "month": report.month,
        "window": report.window,
        "train_period": {"start": window.train_start, "end": window.train_end},
        "test_period": {"start": window.test_start, "end": window.test_end},
        "model": {"name": report.model_name, "file": f"{report.model_name}.safetensors"},
        "training": {
            "epochs_trained": report.training.epochs_trained,
            "final_train_loss": report.training.final_train_loss,
            "final_val_loss": report.training.final_val_loss,
            "early_stopped": report.training.early_stopped,
            "num_samples": report.training.num_samples,
            "status": report.training.status.value,
            "success": report.training.status == Status.SUCCESS
        },
        "backtest": {
            "net_pnl_usd": m.net_pnl_usd, "gross_pnl_usd": m.gross_pnl_usd,
            "total_commission_usd": m.total_commission_usd, "net_pnl_ticks": m.net_pnl_ticks,
            "total_trades": m.total_trades, "winning_trades": m.winning_trades,
            "losing_trades": m.losing_trades, "breakeven_trades": m.breakeven_trades,
            "win_rate": m.win_rate, "win_rate_pct": m.win_rate * 100,
            "avg_win_usd": m.avg_win_usd, "avg_loss_usd": m.avg_loss_usd,
            "profit_factor": m.profit_factor, "avg_rr": m.avg_rr,
            "best_trade_usd": m.best_trade_usd, "worst_trade_usd": m.worst_trade_usd,
            "largest_win_streak": m.largest_win_streak, "largest_loss_streak": m.largest_loss_streak,
            "max_drawdown_usd": m.max_drawdown_usd, "max_drawdown_pct": m.max_drawdown_pct,
            "sharpe_ratio": m.sharpe_ratio, "sortino_ratio": m.sortino_ratio,
            "calmar_ratio": m.calmar_ratio, "avg_mae_ticks": m.avg_mae_ticks,
            "avg_mfe_ticks": m.avg_mfe_ticks, "initial_capital_usd": m.initial_capital_usd,
            "final_equity_usd": m.final_equity_usd, "total_return_pct": m.total_return_pct,
            "trading_days": m.trading_days, "avg_trade_duration_ms": m.avg_trade_duration_ms,
            "expectancy_usd": m.expectancy_usd,
            "status": report.backtest.status.value,
            "success": report.backtest.status == Status.SUCCESS,
            "export_file": str(report.backtest.export_path) if report.backtest.export_path else None,
            "html_report": str(html_path)
        },
        "created_at": report.created_at
    }
    
    json_path = REPORTS_DIR / "monthly" / f"window_{report.window:02d}_{report.month}.json"
    with open(json_path, 'w') as f:
        json.dump(report_data, f, indent=2)
    print(f"  JSON report saved: {json_path}")
    
    return report_data


def create_cumulative_report(reports: List[MonthlyReport], capital: float, skip_charts: bool = False) -> Dict[str, Any]:
    """Create a final cumulative report with charts."""
    global monthly_returns_chart_base64, daily_pnl_chart_base64, cumulative_equity_chart_base64
    
    print(f"\n{'='*60}")
    print("CREATING CUMULATIVE REPORT")
    print(f"{'='*60}")
    
    successful = [r for r in reports if r.backtest.status == Status.SUCCESS]
    n = len(successful)
    
    if n == 0:
        print("No successful backtests to summarize!")
        return {}
    
    # Calculate aggregate statistics
    returns = [r.backtest.metrics.total_return_pct for r in successful]
    max_drawdowns = [r.backtest.metrics.max_drawdown_pct for r in successful]
    total_trades = sum(r.backtest.metrics.total_trades for r in successful)
    total_winning = sum(r.backtest.metrics.winning_trades for r in successful)
    win_rates = [r.backtest.metrics.win_rate for r in successful]
    profit_factors = [r.backtest.metrics.profit_factor for r in successful]
    sharpe_ratios = [r.backtest.metrics.sharpe_ratio for r in successful]
    sortino_ratios = [r.backtest.metrics.sortino_ratio for r in successful]
    
    cumulative_return = 1.0
    for pct in returns: cumulative_return *= (1 + pct / 100)
    cumulative_return = (cumulative_return - 1) * 100
    
    avg_return = sum(returns) / n
    avg_max_dd = sum(max_drawdowns) / n
    avg_win_rate = sum(win_rates) / n
    avg_pf = sum(profit_factors) / n
    avg_sharpe = sum(sharpe_ratios) / n
    avg_sortino = sum(sortino_ratios) / n
    total_pnl = sum(r.backtest.metrics.net_pnl_usd for r in successful)
    total_commission = sum(r.backtest.metrics.total_commission_usd for r in successful)
    total_losing = sum(r.backtest.metrics.losing_trades for r in successful)
    
    # Generate cumulative charts
    if not skip_charts and HAS_MATPLOTLIB:
        print("  Generating cumulative charts...")
        monthly_returns_chart_base64 = create_monthly_returns_chart(reports, "Monthly Returns Across Windows")
        daily_pnl_chart_base64 = create_daily_pnl_chart(reports, "Daily P&L Distribution")
        cumulative_equity_chart_base64 = create_equity_vs_benchmark_chart(reports, "Cumulative Equity Curve")
    
    # Save HTML report
    html_path = REPORTS_DIR / "cumulative_report.html"
    html_content = generate_cumulative_html_report(reports, capital)
    with open(html_path, 'w') as f:
        f.write(html_content)
    print(f"  HTML report saved: {html_path}")
    
    cumulative_report = {
        "report_type": "cumulative_performance",
        "analysis_period": {
            "start": reports[0].window_info.train_start if reports else "",
            "end": reports[-1].window_info.test_end if reports else ""
        },
        "windows": {
            "train_months": 3, "test_months": 1,
            "num_windows": len(reports), "successful": n, "failed": len(reports) - n
        },
        "cumulative_metrics": {
            "cumulative_return_pct": cumulative_return,
            "avg_return_per_window": avg_return,
            "avg_max_drawdown_pct": avg_max_dd,
            "total_pnl_usd": total_pnl,
            "total_commission_usd": total_commission,
            "total_trades": total_trades,
            "total_winning_trades": total_winning,
            "total_losing_trades": total_losing,
            "avg_win_rate": avg_win_rate,
            "avg_win_rate_pct": avg_win_rate * 100,
            "avg_profit_factor": avg_pf,
            "avg_sharpe_ratio": avg_sharpe,
            "avg_sortino_ratio": avg_sortino
        },
        "per_window": [
            {
                "window": r.window,
                "test_period": f"{r.window_info.test_start} to {r.window_info.test_end}",
                "model_name": r.model_name,
                "return_pct": r.backtest.metrics.total_return_pct,
                "max_dd_pct": r.backtest.metrics.max_drawdown_pct,
                "net_pnl_usd": r.backtest.metrics.net_pnl_usd,
                "trades": r.backtest.metrics.total_trades,
                "win_rate": r.backtest.metrics.win_rate * 100,
                "profit_factor": r.backtest.metrics.profit_factor,
                "sharpe": r.backtest.metrics.sharpe_ratio,
                "sortino": r.backtest.metrics.sortino_ratio,
                "status": r.backtest.status.value,
                "success": r.backtest.status == Status.SUCCESS,
                "export_file": str(r.backtest.export_path) if r.backtest.export_path else None,
                "html_report": str(REPORTS_DIR / "monthly" / f"window_{r.window:02d}_{r.month}.html")
            }
            for r in reports
        ],
        "training_summary": [
            {
                "window": r.window,
                "model_name": r.model_name,
                "epochs_trained": r.training.epochs_trained,
                "train_loss": r.training.final_train_loss,
                "val_loss": r.training.final_val_loss,
                "num_samples": r.training.num_samples,
                "status": r.training.status.value,
                "success": r.training.status == Status.SUCCESS
            }
            for r in reports
        ],
        "daily_aggregated_pnl": aggregate_daily_pnl(reports),
        "html_report": str(html_path),
        "created_at": datetime.now().isoformat(),
        "generated_by": "sliding_window_train_eval.py"
    }
    
    # Save JSON
    json_path = REPORTS_DIR / "cumulative_report.json"
    with open(json_path, 'w') as f:
        json.dump(cumulative_report, f, indent=2)
    
    # Save text summary
    text_path = REPORTS_DIR / "cumulative_report.txt"
    with open(text_path, 'w') as f:
        f.write("=" * 70 + "\n")
        f.write("SLIDING WINDOW LSTM BACKTEST - CUMULATIVE PERFORMANCE REPORT\n")
        f.write("=" * 70 + "\n\n")
        f.write(f"Analysis Period:    {reports[0].window_info.train_start} to {reports[-1].window_info.test_end}\n")
        f.write(f"Training Period:   3 months per window\n")
        f.write(f"Testing Period:    1 month per window\n")
        f.write(f"Number of Windows: {len(reports)}\n")
        f.write(f"Successful:        {n}\n")
        f.write(f"Timeframe:         {TIMEFRAME}\n")
        f.write(f"Symbol:           {SYMBOL}\n\n")
        f.write("-" * 70 + "\n")
        f.write("CUMULATIVE METRICS\n")
        f.write("-" * 70 + "\n")
        f.write(f"Cumulative Return:        {cumulative_return:+10.2f}%\n")
        f.write(f"Total P&L:               ${total_pnl:+10,.2f}\n")
        f.write(f"Total Commission:       ${total_commission:10,.2f}\n")
        f.write(f"Average Return/Window:  {avg_return:+10.2f}%\n")
        f.write(f"Average Max Drawdown:    {avg_max_dd:10.2f}%\n")
        f.write(f"Total Trades:            {total_trades:10}\n")
        f.write(f"Winning Trades:          {total_winning:10}\n")
        f.write(f"Losing Trades:          {total_losing:10}\n")
        f.write(f"Average Win Rate:        {avg_win_rate*100:9.1f}%\n")
        f.write(f"Average Profit Factor:   {avg_pf:10.2f}\n")
        f.write(f"Average Sharpe Ratio:    {avg_sharpe:10.2f}\n")
        f.write(f"Average Sortino Ratio:  {avg_sortino:10.2f}\n\n")
        f.write("-" * 70 + "\n")
        f.write("PER-WINDOW PERFORMANCE\n")
        f.write("-" * 70 + "\n")
        f.write(f"{'Win':^8} {'Test Period':^25} {'Return':^10} {'Max DD':^10} {'Trades':^8} {'Win%':^8} {'PF':^8} {'Sharpe':^8} {'Status':^8}\n")
        f.write("-" * 70 + "\n")
        for r in reports:
            status = "OK" if r.backtest.status == Status.SUCCESS else "FAIL"
            test_period = f"{r.window_info.test_start} - {r.window_info.test_end}"
            f.write(f"{r.window:^8} {test_period:^25} {r.backtest.metrics.total_return_pct:+9.2f}% {r.backtest.metrics.max_drawdown_pct:9.2f}% {r.backtest.metrics.total_trades:^8} {r.backtest.metrics.win_rate*100:7.1f}% {r.backtest.metrics.profit_factor:8.2f} {r.backtest.metrics.sharpe_ratio:8.2f} {status:^8}\n")
        f.write("-" * 70 + "\n")
        f.write(f"\nHTML Report: {html_path}\n")
        f.write(f"JSON Report: {json_path}\n")
        f.write(f"\nReport Generated: {datetime.now().isoformat()}\n")
    
    print(f"\n{'='*60}")
    print("CUMULATIVE PERFORMANCE SUMMARY")
    print(f"{'='*60}")
    print(f"Windows Analyzed:   {n}/{len(reports)}")
    print(f"Cumulative Return:  {cumulative_return:.2f}%")
    print(f"Total P&L:         ${total_pnl:+,.2f}")
    print(f"Avg Return/Window:  {avg_return:.2f}%")
    print(f"Avg Max Drawdown:   {avg_max_dd:.2f}%")
    print(f"Total Trades:       {total_trades}")
    print(f"Avg Win Rate:       {avg_win_rate*100:.1f}%")
    print(f"Avg Profit Factor:  {avg_pf:.2f}")
    print(f"Avg Sharpe Ratio:   {avg_sharpe:.2f}")
    print(f"Avg Sortino Ratio:  {avg_sortino:.2f}")
    print(f"\nPer-Window Returns:")
    for r in reports:
        status = "✓" if r.backtest.status == Status.SUCCESS else "✗"
        print(f"  Window {r.window}: {r.backtest.metrics.total_return_pct:+.2f}% {status}")
    
    print(f"\nReports saved:")
    print(f"  HTML: {html_path}")
    print(f"  JSON: {json_path}")
    print(f"  Text: {text_path}")
    
    return cumulative_report


def aggregate_daily_pnl(reports: List[MonthlyReport]) -> Dict[str, Dict]:
    """Aggregate daily P&L across all windows."""
    daily_agg = {}
    for r in reports:
        if r.backtest.status != Status.SUCCESS:
            continue
        for date, pnl in r.backtest.metrics.daily_pnl.items():
            if date not in daily_agg:
                daily_agg[date] = {"pnl": 0, "windows": 0}
            daily_agg[date]["pnl"] += pnl
            daily_agg[date]["windows"] += 1
    return dict(sorted(daily_agg.items()))


def print_banner():
    """Print the program banner."""
    print("""
╔══════════════════════════════════════════════════════════════════════════════╗
║                    SLIDING WINDOW LSTM TRAINING & EVALUATION                  ║
║                                                                              ║
║  Training:  3 months of NQ data (5m timeframe) per window                    ║
║  Testing:   1 month of data per window                                      ║
║  Windows:   3 sliding windows                                               ║
╚══════════════════════════════════════════════════════════════════════════════╝
""")


def main():
    """Main entry point."""
    print_banner()
    
    args = parse_args()
    
    global DATA_DIR
    if args.data_dir:
        DATA_DIR = Path(args.data_dir)
    
    skip_charts = args.skip_charts or not HAS_MATPLOTLIB
    
    print(f"Configuration:")
    print(f"  Training months:   {args.train_months}")
    print(f"  Testing months:    {args.test_months}")
    print(f"  Number of windows:   {args.num_windows}")
    print(f"  Start date:        {args.start_date}")
    print(f"  Timeframe:         {args.timeframe}")
    print(f"  Symbol:            {args.symbol}")
    print(f"  Epochs:            {args.epochs}")
    print(f"  Initial capital:   ${args.capital:,.2f}")
    print(f"  Data directory:    {DATA_DIR}")
    print(f"  Generate charts:   {'Yes' if not skip_charts else 'No (matplotlib not available)'}")
    print()
    
    ensure_dirs()
    windows = generate_windows(args)
    
    monthly_reports: List[MonthlyReport] = []
    
    for window in windows:
        print(f"\n{'#'*60}")
        print(f"# WINDOW {window.window_num}/{args.num_windows}")
        print(f"{'#'*60}")
        
        month = window.test_start[:7]
        
        if not args.skip_training:
            training_result = run_training(window, args.epochs, args.verbose)
        else:
            print("Skipping training (--skip-training flag)")
            training_result = TrainingResult(window=window, epochs_trained=args.epochs,
                final_train_loss=0.0, final_val_loss=None, early_stopped=False,
                num_samples=0, status=Status.SKIPPED)
        
        if not args.skip_backtest:
            backtest_result = run_backtest(window, args.capital, args.verbose, skip_charts)
        else:
            print("Skipping backtest (--skip-backtest flag)")
            backtest_result = BacktestResult(window=window,
                metrics=BacktestMetrics(
                    net_pnl_usd=0, gross_pnl_usd=0, total_commission_usd=0, net_pnl_ticks=0,
                    total_trades=0, winning_trades=0, losing_trades=0, breakeven_trades=0,
                    win_rate=0, avg_win_usd=0, avg_loss_usd=0, profit_factor=0, avg_rr=0,
                    best_trade_usd=0, worst_trade_usd=0, largest_win_streak=0,
                    largest_loss_streak=0, max_drawdown_usd=0, max_drawdown_pct=0,
                    sharpe_ratio=0, sortino_ratio=0, calmar_ratio=0, avg_mae_ticks=0,
                    avg_mfe_ticks=0, initial_capital_usd=args.capital,
                    final_equity_usd=args.capital, total_return_pct=0,
                    trading_days=0, avg_trade_duration_ms=0, expectancy_usd=0
                ), status=Status.SKIPPED)
        
        report = MonthlyReport(
            month=month, window=window.window_num,
            window_info=window,  # Store the full window period
            train_period=f"{window.train_start} to {window.train_end}",
            test_period=f"{window.test_start} to {window.test_end}",
            model_name=window.model_name,
            training=training_result, backtest=backtest_result
        )
        monthly_reports.append(report)
        
        create_monthly_report(report, skip_charts)
    
    if monthly_reports:
        create_cumulative_report(monthly_reports, args.capital, skip_charts)
    
    print(f"\n{'='*60}")
    print("SLIDING WINDOW PIPELINE COMPLETE")
    print(f"{'='*60}")
    print(f"Models saved in:      {MODELS_DIR}")
    print(f"Reports saved in:     {REPORTS_DIR}")
    print(f"  Monthly HTML/JSON:  {REPORTS_DIR}/monthly/")
    print(f"  Cumulative HTML:    {REPORTS_DIR}/cumulative_report.html")
    print(f"  Cumulative JSON:    {REPORTS_DIR}/cumulative_report.json")
    print(f"  Cumulative Text:    {REPORTS_DIR}/cumulative_report.txt")


if __name__ == "__main__":
    main()
