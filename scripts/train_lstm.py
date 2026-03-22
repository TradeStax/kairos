#!/usr/bin/env python3
"""
LSTM Training Script for Kairos Trading Strategy

This script trains an LSTM model on NQ futures data using technical indicators.
It prepares the data, calls the Rust training backend, and saves the model.

Usage:
    python3 scripts/train_lstm.py --data-dir ../nq --start 2021-01-01 --end 2021-12-31
"""

import os
import sys
import json
import argparse
import subprocess
from pathlib import Path
from datetime import datetime, timedelta
from typing import List, Dict, Tuple, Optional
import numpy as np

# Try to import optional dependencies
try:
    import torch
    HAS_TORCH = True
except ImportError:
    HAS_TORCH = False
    print("Warning: PyTorch not found. GPU training may not be available.")

try:
    import pandas as pd
    import polars as pl
    HAS_POLARS = True
except ImportError:
    HAS_POLARS = False
    print("Warning: Polars not found. Using pandas fallback.")

try:
    from databento import Historical
    HAS_DATABENTO = True
except ImportError:
    HAS_DATABENTO = False


class TechnicalIndicators:
    """Calculate technical indicators for trading signals."""
    
    @staticmethod
    def sma(prices: np.ndarray, period: int) -> np.ndarray:
        """Simple Moving Average."""
        result = np.full_like(prices, np.nan)
        for i in range(period - 1, len(prices)):
            result[i] = np.mean(prices[i - period + 1:i + 1])
        return result
    
    @staticmethod
    def ema(prices: np.ndarray, period: int) -> np.ndarray:
        """Exponential Moving Average."""
        result = np.full_like(prices, np.nan)
        if len(prices) < period:
            return result
        
        alpha = 2 / (period + 1)
        result[period - 1] = np.mean(prices[:period])
        
        for i in range(period, len(prices)):
            result[i] = alpha * prices[i] + (1 - alpha) * result[i - 1]
        return result
    
    @staticmethod
    def rsi(prices: np.ndarray, period: int = 14) -> np.ndarray:
        """Relative Strength Index."""
        result = np.full_like(prices, np.nan)
        if len(prices) < period + 1:
            return result
        
        deltas = np.diff(prices)
        gains = np.where(deltas > 0, deltas, 0)
        losses = np.where(deltas < 0, -deltas, 0)
        
        avg_gain = np.mean(gains[:period])
        avg_loss = np.mean(losses[:period])
        
        if avg_loss == 0:
            result[period] = 100
        else:
            rs = avg_gain / avg_loss
            result[period] = 100 - (100 / (1 + rs))
        
        for i in range(period + 1, len(prices)):
            avg_gain = (avg_gain * (period - 1) + gains[i - 1]) / period
            avg_loss = (avg_loss * (period - 1) + losses[i - 1]) / period
            
            if avg_loss == 0:
                result[i] = 100
            else:
                rs = avg_gain / avg_loss
                result[i] = 100 - (100 / (1 + rs))
        
        return result
    
    @staticmethod
    def atr(high: np.ndarray, low: np.ndarray, close: np.ndarray, period: int = 14) -> np.ndarray:
        """Average True Range."""
        result = np.full_like(close, np.nan)
        if len(close) < period + 1:
            return result
        
        tr = np.zeros(len(close) - 1)
        tr[0] = high[0] - low[0]
        
        for i in range(1, len(tr)):
            hl = high[i] - low[i]
            hc = abs(high[i] - close[i - 1])
            lc = abs(low[i] - close[i - 1])
            tr[i] = max(hl, hc, lc)
        
        result[period] = np.mean(tr[:period])
        
        for i in range(period + 1, len(close)):
            result[i] = (result[i - 1] * (period - 1) + tr[i - 1]) / period
        
        return result
    
    @staticmethod
    def macd(prices: np.ndarray, fast: int = 12, slow: int = 26, signal: int = 9) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        """MACD (Moving Average Convergence Divergence)."""
        ema_fast = TechnicalIndicators.ema(prices, fast)
        ema_slow = TechnicalIndicators.ema(prices, slow)
        macd_line = ema_fast - ema_slow
        signal_line = TechnicalIndicators.ema(macd_line, signal)
        histogram = macd_line - signal_line
        return macd_line, signal_line, histogram
    
    @staticmethod
    def bollinger_bands(prices: np.ndarray, period: int = 20, std_dev: float = 2.0) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Bollinger Bands."""
        middle = TechnicalIndicators.sma(prices, period)
        std = np.full_like(prices, np.nan)
        
        for i in range(period - 1, len(prices)):
            std[i] = np.std(prices[i - period + 1:i + 1])
        
        upper = middle + std_dev * std
        lower = middle - std_dev * std
        return upper, middle, lower
    
    @staticmethod
    def vwap(high: np.ndarray, low: np.ndarray, close: np.ndarray, volume: np.ndarray) -> np.ndarray:
        """Volume Weighted Average Price."""
        typical_price = (high + low + close) / 3
        cumulative_tp_vol = np.cumsum(typical_price * volume)
        cumulative_vol = np.cumsum(volume)
        
        result = np.full_like(close, np.nan)
        for i in range(len(close)):
            if cumulative_vol[i] > 0:
                result[i] = cumulative_tp_vol[i] / cumulative_vol[i]
        return result


class DatasetGenerator:
    """Generate training datasets from market data."""
    
    def __init__(self, lookback: int = 20, horizon: int = 5, 
                 long_threshold: float = 0.005, short_threshold: float = 0.005):
        self.lookback = lookback
        self.horizon = horizon
        self.long_threshold = long_threshold
        self.short_threshold = short_threshold
        self.indicators = TechnicalIndicators()
    
    def calculate_features(self, candles: np.ndarray, volumes: np.ndarray) -> Dict[str, np.ndarray]:
        """Calculate all features from candle data."""
        high = candles[:, 2]  # High
        low = candles[:, 3]  # Low
        close = candles[:, 4]  # Close
        
        features = {}
        
        # Price-based features
        features['sma_20'] = self.indicators.sma(close, 20)
        features['sma_50'] = self.indicators.sma(close, 50)
        features['ema_12'] = self.indicators.ema(close, 12)
        features['ema_26'] = self.indicators.ema(close, 26)
        
        # Momentum
        features['rsi_14'] = self.indicators.rsi(close, 14)
        
        # Volatility
        features['atr_14'] = self.indicators.atr(high, low, close, 14)
        
        # MACD
        macd_line, signal_line, histogram = self.indicators.macd(close)
        features['macd'] = macd_line
        features['macd_signal'] = signal_line
        features['macd_hist'] = histogram
        
        # Bollinger Bands
        bb_upper, bb_middle, bb_lower = self.indicators.bollinger_bands(close)
        features['bb_upper'] = bb_upper
        features['bb_middle'] = bb_middle
        features['bb_lower'] = bb_lower
        
        # VWAP
        features['vwap'] = self.indicators.vwap(high, low, close, volumes)
        
        # Normalize features relative to close
        for key in features:
            if features[key] is not None and len(features[key]) == len(close):
                features[key] = features[key] / close - 1
        
        return features
    
    def generate_labels(self, close: np.ndarray) -> np.ndarray:
        """Generate labels based on future returns."""
        labels = np.full(len(close), 1, dtype=np.int32)  # Default: Neutral
        
        for i in range(self.lookback, len(close) - self.horizon):
            future_return = (close[i + self.horizon] - close[i]) / close[i]
            
            if future_return > self.long_threshold:
                labels[i] = 0  # Long
            elif future_return < -self.short_threshold:
                labels[i] = 2  # Short
            else:
                labels[i] = 1  # Neutral
        
        return labels
    
    def prepare_dataset(self, candles: np.ndarray, volumes: np.ndarray) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Prepare features and labels for training."""
        features_dict = self.calculate_features(candles, volumes)
        labels = self.generate_labels(candles[:, 4])
        
        # Stack features
        feature_names = list(features_dict.keys())
        num_samples = len(candles) - self.lookback
        lookback = self.lookback
        num_features = len(feature_names)
        
        X = np.zeros((num_samples, lookback, num_features), dtype=np.float32)
        y = np.zeros(num_samples, dtype=np.int64)
        
        for i in range(self.lookback, len(candles)):
            sample_idx = i - self.lookback
            for j, name in enumerate(feature_names):
                feature = features_dict[name]
                X[sample_idx, :, j] = feature[i - lookback:i]
            y[sample_idx] = labels[i]
        
        # Remove samples with NaN
        valid_mask = ~np.isnan(X).any(axis=(1, 2)) & (y != 1)  # Exclude neutral for now
        X = X[valid_mask]
        y = y[valid_mask]
        
        return X, y, np.arange(len(y))  # X, labels, timestamps
    
    def to_json_format(self, X: np.ndarray, y: np.ndarray, timestamps: np.ndarray) -> Dict:
        """Convert to Kairos training format."""
        # Convert to list format for JSON
        features_list = []
        for sample in X:
            features_list.append(sample.tolist())
        
        return {
            'features': features_list,
            'labels': y.tolist(),
            'timestamps': timestamps.tolist(),
            'shape': {
                'num_samples': len(X),
                'lookback': self.lookback,
                'num_features': X.shape[2]
            }
        }


class KairosTrainer:
    """Wrapper for Kairos Rust training backend."""
    
    def __init__(self, config_path: str, output_path: str):
        self.config_path = config_path
        self.output_path = output_path
        self.binary = Path(__file__).parent.parent / 'target' / 'debug' / 'kairos'
    
    def train(self, data_dir: str, verbose: bool = True) -> bool:
        """Run Kairos training."""
        if not self.binary.exists():
            print(f"Error: Kairos binary not found at {self.binary}")
            print("Run: cargo build --package kairos-cli")
            return False
        
        cmd = [
            str(self.binary),
            'ml', 'train',
            '--config', self.config_path,
            '--data-dir', data_dir,
            '--output', self.output_path,
        ]
        
        if verbose:
            cmd.append('--verbose')
        
        env = os.environ.copy()
        env['LIBTORCH_USE_PYTORCH'] = '1'
        if 'LD_LIBRARY_PATH' not in env:
            if HAS_TORCH:
                torch_lib = Path(torch.__path__[0]) / 'lib'
                env['LD_LIBRARY_PATH'] = f"{torch_lib}:{env.get('LD_LIBRARY_PATH', '')}"
        
        print(f"Running: {' '.join(cmd)}")
        result = subprocess.run(cmd, env=env)
        
        return result.returncode == 0


def load_dbn_files(data_dir: str, start_date: str, end_date: str) -> Tuple[np.ndarray, np.ndarray]:
    """Load and parse DBN files."""
    # This would use the databento Python library
    # For now, return synthetic data for testing
    print(f"Loading DBN files from {data_dir}")
    print(f"Date range: {start_date} to {end_date}")
    
    # Generate synthetic data for testing
    np.random.seed(42)
    num_candles = 1000
    
    dates = [datetime(2021, 1, 1) + timedelta(hours=i) for i in range(num_candles)]
    opens = 14000 + np.cumsum(np.random.randn(num_candles) * 10)
    highs = opens + np.abs(np.random.randn(num_candles) * 5)
    lows = opens - np.abs(np.random.randn(num_candles) * 5)
    closes = opens + np.random.randn(num_candles) * 5
    volumes = np.random.randint(1000, 10000, num_candles)
    
    # Stack as OHLCV
    candles = np.column_stack([dates, opens, highs, lows, closes, volumes])
    
    return candles, volumes


def main():
    parser = argparse.ArgumentParser(description='Train LSTM model for Kairos')
    parser.add_argument('--data-dir', type=str, default='../nq',
                        help='Directory containing DBN files')
    parser.add_argument('--start', type=str, default='2021-01-01',
                        help='Start date (YYYY-MM-DD)')
    parser.add_argument('--end', type=str, default='2021-12-31',
                        help='End date (YYYY-MM-DD)')
    parser.add_argument('--config', type=str, default='training_config.json',
                        help='Training configuration file')
    parser.add_argument('--output', type=str, default='models/nq_lstm.pt',
                        help='Output model path')
    parser.add_argument('--prepare-only', action='store_true',
                        help='Only prepare data, skip training')
    parser.add_argument('--epochs', type=int, default=None,
                        help='Override epochs from config')
    parser.add_argument('--gpu-device', type=int, default=None,
                        help='GPU device to use')
    
    args = parser.parse_args()
    
    # Create output directory
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    
    # Load configuration
    with open(args.config, 'r') as f:
        config = json.load(f)
    
    # Override config with CLI args
    if args.epochs:
        config['epochs'] = args.epochs
    if args.gpu_device is not None:
        config['gpu_device'] = args.gpu_device
    
    # Write updated config
    config_path = 'training_config_updated.json'
    with open(config_path, 'w') as f:
        json.dump(config, f, indent=2)
    
    # Load data
    candles, volumes = load_dbn_files(args.data_dir, args.start, args.end)
    
    # Create generator
    generator = DatasetGenerator(
        lookback=config['label_config']['warmup_bars'],
        horizon=config['label_config']['horizon'],
        long_threshold=config['label_config']['long_threshold'],
        short_threshold=config['label_config']['short_threshold']
    )
    
    # Prepare dataset
    print("Calculating features and generating labels...")
    X, y, timestamps = generator.prepare_dataset(candles, volumes)
    
    print(f"Dataset prepared:")
    print(f"  Samples: {len(X)}")
    print(f"  Lookback: {X.shape[1]}")
    print(f"  Features: {X.shape[2]}")
    print(f"  Labels: Long={np.sum(y==0)}, Neutral={np.sum(y==1)}, Short={np.sum(y==2)}")
    
    # Save dataset
    dataset_path = 'training_dataset.json'
    dataset_json = generator.to_json_format(X, y, timestamps)
    with open(dataset_path, 'w') as f:
        json.dump(dataset_json, f)
    print(f"Dataset saved to {dataset_path}")
    
    if args.prepare_only:
        print("Dataset preparation complete (--prepare-only specified)")
        return
    
    # Train model
    print("\nStarting training...")
    trainer = KairosTrainer(config_path, args.output)
    success = trainer.train(args.data_dir, verbose=True)
    
    if success:
        print(f"\nTraining complete! Model saved to {args.output}")
    else:
        print("\nTraining failed!")
        sys.exit(1)


if __name__ == '__main__':
    main()
