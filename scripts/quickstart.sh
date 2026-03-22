#!/bin/bash
# Kairos LSTM - Quick Start Script
# Usage: ./scripts/quickstart.sh [train|backtest|all]

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Kairos LSTM Quick Start ===${NC}"

# Check environment
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

echo "Project directory: $PROJECT_DIR"

# Set environment
export LIBTORCH_USE_PYTORCH=1
export LD_LIBRARY_PATH=/home/administrator/.local/lib/python3.12/site-packages/torch/lib:$LD_LIBRARY_PATH

# Check if binary exists
if [ ! -f "target/debug/kairos" ]; then
    echo -e "${YELLOW}Binary not found. Building...${NC}"
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        echo "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    echo "Building kairos-cli..."
    cargo build --package kairos-cli --features kairos-cli/tch
fi

echo -e "${GREEN}Binary ready!${NC}"

# Parse arguments
COMMAND="${1:-all}"

case "$COMMAND" in
    train)
        echo -e "${GREEN}=== Training LSTM Model ===${NC}"
        mkdir -p models
        ./target/debug/kairos ml train \
            --config training_config.json \
            --data-dir ../nq \
            --output models/nq_lstm_model.pt \
            --epochs 50 \
            --verbose
        ;;
    
    backtest)
        echo -e "${GREEN}=== Running Backtest ===${NC}"
        ./target/debug/kairos backtest \
            --symbol NQ \
            --start 2021-03-10 \
            --end 2021-03-31 \
            --strategy orb \
            --data-dir ../nq \
            --capital 100000 \
            --timeframe 1min
        ;;
    
    all|*)
        echo -e "${GREEN}=== Training LSTM Model ===${NC}"
        mkdir -p models
        ./target/debug/kairos ml train \
            --config training_config.json \
            --data-dir ../nq \
            --output models/nq_lstm_model.pt \
            --epochs 10 \
            --verbose
        
        echo ""
        echo -e "${GREEN}=== Running Backtest ===${NC}"
        ./target/debug/kairos backtest \
            --symbol NQ \
            --start 2021-03-10 \
            --end 2021-03-31 \
            --strategy orb \
            --data-dir ../nq \
            --capital 100000 \
            --timeframe 1min
        ;;
esac

echo ""
echo -e "${GREEN}=== Done! ===${NC}"
