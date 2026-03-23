#!/bin/bash
# Sliding Window LSTM Training and Evaluation Pipeline
# This script builds Kairos (if needed) and runs the sliding window pipeline

set -e

PROJECT_ROOT="/data/jbutler/algo-data/kairos"
SCRIPTS_DIR="$PROJECT_ROOT/scripts"
KAIROS_BINARY="$PROJECT_ROOT/target/debug/kairos"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}==============================================${NC}"
echo -e "${GREEN}  Sliding Window LSTM Training Pipeline${NC}"
echo -e "${GREEN}==============================================${NC}"

# Function to setup GPU environment
setup_gpu_env() {
    echo -e "${YELLOW}Setting up GPU environment...${NC}"
    
    # Check for PyTorch libtorch
    if [ -d "/home/administrator/.local/lib/python3.12/site-packages/torch/lib" ]; then
        export LIBTORCH_USE_PYTORCH=1
        export LD_LIBRARY_PATH="/home/administrator/.local/lib/python3.12/site-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
        echo -e "${GREEN}✓ GPU environment configured${NC}"
    elif [ -d "/usr/local/lib/python3/dist-packages/torch/lib" ]; then
        export LIBTORCH_USE_PYTORCH=1
        export LD_LIBRARY_PATH="/usr/local/lib/python3/dist-packages/torch/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
        echo -e "${GREEN}✓ GPU environment configured (system Python)${NC}"
    else
        # Try to find PyTorch installation
        TORCH_PATH=$(python3 -c "import torch; print(torch.__path__[0])" 2>/dev/null || echo "")
        if [ -n "$TORCH_PATH" ] && [ -d "$TORCH_PATH/lib" ]; then
            export LIBTORCH_USE_PYTORCH=1
            export LD_LIBRARY_PATH="$TORCH_PATH/lib:/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
            echo -e "${GREEN}✓ GPU environment configured (found torch at $TORCH_PATH)${NC}"
        else
            echo -e "${YELLOW}⚠ Warning: Could not find PyTorch/libtorch${NC}"
        fi
    fi
}

# Check if binary exists and is recent
check_binary() {
    if [ -f "$KAIROS_BINARY" ]; then
        BINARY_TIME=$(stat -c %Y "$KAIROS_BINARY" 2>/dev/null || stat -f %m "$KAIROS_BINARY" 2>/dev/null)
        NOW_TIME=$(date +%s)
        AGE_HOURS=$(( (NOW_TIME - BINARY_TIME) / 3600 ))
        
        if [ $AGE_HOURS -lt 24 ]; then
            echo -e "${GREEN}✓ Found recent kairos binary ($KAIROS_BINARY)${NC}"
            return 0
        else
            echo -e "${YELLOW}⚠ Binary exists but is ${AGE_HOURS} hours old${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}⚠ No kairos binary found${NC}"
        return 2
    fi
}

# Try to find cargo
find_cargo() {
    # Check common locations
    for path in "$HOME/.cargo/bin/cargo" "/usr/local/bin/cargo" "/usr/bin/cargo" "$PROJECT_ROOT/.cargo/bin/cargo" "$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo"; do
        if [ -f "$path" ]; then
            export PATH="$(dirname $path):$PATH"
            echo -e "${GREEN}✓ Found cargo at: $path${NC}"
            return 0
        fi
    done
    
    # Try which
    if command -v cargo &> /dev/null; then
        echo -e "${GREEN}✓ Found cargo in PATH${NC}"
        return 0
    fi
    
    return 1
}

# Build Kairos if needed
build_kairos() {
    echo ""
    echo -e "${YELLOW}Building Kairos...${NC}"
    echo ""
    
    # Setup GPU environment
    setup_gpu_env
    
    # Find cargo
    if ! find_cargo; then
        echo -e "${RED}✗ Error: cargo not found!${NC}"
        echo ""
        echo "Rust/Cargo is not installed. Please install Rust:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo ""
        echo "Or use an existing binary if available."
        return 1
    fi
    
    # Build Kairos with ML support
    echo "Running: cargo build --package kairos-cli --features kairos-cli/tch"
    cargo build --package kairos-cli --features kairos-cli/tch 2>&1
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Build successful!${NC}"
        return 0
    else
        echo -e "${RED}✗ Build failed!${NC}"
        return 1
    fi
}

# Step 1: Check/setup binary
echo ""
echo -e "${YELLOW}Step 1: Checking Kairos binary...${NC}"
echo ""

if check_binary; then
    echo "Using existing binary - skipping build"
    BINARY_OK=true
else
    echo "Need to build Kairos"
    BINARY_OK=false
fi

# Step 2: Build if needed
if [ "$BINARY_OK" != "true" ]; then
    echo ""
    read -p "Build Kairos now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if ! build_kairos; then
            exit 1
        fi
    else
        echo "Cannot proceed without Kairos binary"
        exit 1
    fi
fi

# Step 3: Run the sliding window pipeline
echo ""
echo -e "${YELLOW}Step 2: Running sliding window pipeline...${NC}"
echo ""

# Setup GPU env for the Python script (training)
setup_gpu_env

# Run the pipeline
python3 "$SCRIPTS_DIR/sliding_window_train_eval.py" "$@"

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}==============================================${NC}"
    echo -e "${GREEN}  Pipeline completed successfully!${NC}"
    echo -e "${GREEN}==============================================${NC}"
    echo ""
    echo "Reports saved to: $PROJECT_ROOT/reports/"
    echo "  - cumulative_report.html (interactive with charts)"
    echo "  - cumulative_report.json (full data)"
    echo "  - cumulative_report.txt (human readable)"
    echo "  - monthly/ (individual window reports)"
else
    echo ""
    echo -e "${RED}==============================================${NC}"
    echo -e "${RED}  Pipeline failed!${NC}"
    echo -e "${RED}==============================================${NC}"
    exit 1
fi
