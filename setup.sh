#!/bin/bash

# AlgoTrader Interactive Setup Script
# This script guides you through configuring the trading bot

set -e

echo "╔════════════════════════════════════════════════════════════════════╗"
echo "║              AlgoTrader - Interactive Setup                        ║"
echo "║         Dynamic Hedge Grid Strategy for Bybit                      ║"
echo "╚════════════════════════════════════════════════════════════════════╝"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
DEFAULT_SYMBOL="ETHUSDT"
DEFAULT_RISK_PCT="0.02"
DEFAULT_GRID_RANGE="2.0"
DEFAULT_LEVERAGE="5"
DEFAULT_ENTRY_MODE="ema_trend"
DEFAULT_EMA_CANDLES="20"
DEFAULT_EMA_TIMEFRAME="15"
DEFAULT_MAX_GRID_LEVELS="4"
DEFAULT_MAX_HOLD_HOURS="168"

# Function to prompt user
prompt() {
    local prompt_text="$1"
    local default_value="$2"
    local result
    
    if [ -n "$default_value" ]; then
        read -rp "$prompt_text [$default_value]: " result
        echo "${result:-$default_value}"
    else
        read -rp "$prompt_text: " result
        echo "$result"
    fi
}

# Function to prompt yes/no
prompt_yes_no() {
    local prompt_text="$1"
    local default_value="$2"
    local result
    
    while true; do
        read -rp "$prompt_text [Y/n]: " result
        result="${result:-$default_value}"
        case "$result" in
            [Yy]* ) echo "true"; return ;;
            [Nn]* ) echo "false"; return ;;
            * ) echo "Please answer yes or no." ;;
        esac
    done
}

# Function to select from options
select_option() {
    local prompt_text="$1"
    shift
    local options=("$@")
    local choice
    
    echo "$prompt_text"
    for i in "${!options[@]}"; do
        echo "  $((i+1)). ${options[$i]}"
    done
    
    while true; do
        read -rp "Enter choice [1-${#options[@]}]: " choice
        if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "${#options[@]}" ]; then
            echo "${options[$((choice-1))]}"
            return
        fi
        echo "Invalid choice. Please try again."
    done
}

echo -e "${BLUE}This script will help you configure AlgoTrader.${NC}"
echo "You'll need:"
echo "  - Bybit Production API credentials (for market data)"
echo "  - Bybit Demo API credentials (for trading)"
echo ""
echo -e "${YELLOW}Press Enter to continue...${NC}"
read

clear

# ============================================
# API CONFIGURATION
# ============================================
echo -e "${GREEN}=== Step 1: API Configuration ===${NC}"
echo ""
echo -e "${BLUE}Production API (Read-Only - for market data):${NC}"
echo "Get from: https://www.bybit.com/app/user/api-management"
echo ""
PROD_KEY=$(prompt "Enter Production API Key" "")
PROD_SECRET=$(prompt "Enter Production API Secret" "")

echo ""
echo -e "${BLUE}Demo API (Trading - for actual trades):${NC}"
echo "Get from: https://www.bybit.com/demo-trading"
echo ""
DEMO_KEY=$(prompt "Enter Demo API Key" "")
DEMO_SECRET=$(prompt "Enter Demo API Secret" "")

clear

# ============================================
# TRADING PAIR SELECTION
# ============================================
echo -e "${GREEN}=== Step 2: Trading Pair ===${NC}"
echo ""
echo "Select the cryptocurrency pair to trade:"
echo ""

SYMBOL=$(select_option "Available pairs:" \
    "ETHUSDT (Ethereum - recommended for beginners)" \
    "BTCUSDT (Bitcoin - higher volatility)" \
    "SOLUSDT (Solana - mid volatility)" \
    "Custom (enter manually)")

if [ "$SYMBOL" = "Custom (enter manually)" ]; then
    SYMBOL=$(prompt "Enter trading pair (e.g., XRPUSDT)" "")
else
    # Extract symbol from description
    SYMBOL=$(echo "$SYMBOL" | cut -d' ' -f1)
fi

clear

# ============================================
# RISK CONFIGURATION
# ============================================
echo -e "${GREEN}=== Step 3: Risk Configuration ===${NC}"
echo ""
echo "These settings control how much capital is risked per trade."
echo ""

echo -e "${BLUE}Position Risk Percentage:${NC}"
echo "This is the percentage of your free USDT that will be used"
echo "for each initial position entry."
echo ""
echo "  0.01 = 1% (conservative, smaller positions)"
echo "  0.02 = 2% (recommended, balanced)"
echo "  0.05 = 5% (aggressive, larger positions)"
echo ""

RISK_PCT=$(prompt "Position risk percentage" "$DEFAULT_RISK_PCT")

echo ""
echo -e "${BLUE}Leverage:${NC}"
echo "Higher leverage = larger positions but higher risk"
echo ""
LEVERAGE=$(prompt "Leverage (1-100)" "$DEFAULT_LEVERAGE")

clear

# ============================================
# STRATEGY CONFIGURATION
# ============================================
echo -e "${GREEN}=== Step 4: Strategy Configuration ===${NC}"
echo ""

echo -e "${BLUE}Entry Mode:${NC}"
echo ""
echo "ema_trend     - Uses EMA slope to decide LONG/SHORT (recommended)"
echo "immediate     - Always enters LONG immediately"
echo "wait_for_zone - Waits for price to hit a zone first"
echo ""

ENTRY_MODE=$(select_option "Select entry mode:" \
    "ema_trend" \
    "immediate" \
    "wait_for_zone")

if [ "$ENTRY_MODE" = "ema_trend" ]; then
    echo ""
    echo -e "${BLUE}EMA Configuration:${NC}"
    echo ""
    echo "Timeframes: 5m (fast), 15m (balanced), 60m (slow), 240m (very slow)"
    echo ""
    
    EMA_TIMEFRAME=$(select_option "Select EMA timeframe:" \
        "5" \
        "15 (recommended)" \
        "60" \
        "240")
    
    # Extract just the number
    EMA_TIMEFRAME=$(echo "$EMA_TIMEFRAME" | cut -d' ' -f1)
    
    echo ""
    EMA_CANDLES=$(prompt "Number of candles for EMA (5-100)" "$DEFAULT_EMA_CANDLES")
    
    echo ""
    EMA_FALLBACK=$(prompt_yes_no "Wait for zone if no clear trend?" "Y")
fi

echo ""
echo -e "${BLUE}Grid Configuration:${NC}"
echo ""
echo "Grid range defines the distance between buy/sell zones."
echo "Example: 2% range = zones at ±1% from entry price"
echo ""
echo "  1.0% = Tight grid, more trades, lower profit per trade"
echo "  2.0% = Balanced grid (recommended)"
echo "  3.0% = Wide grid, fewer trades, higher profit per trade"
echo ""

GRID_RANGE=$(prompt "Grid range percentage" "$DEFAULT_GRID_RANGE")

echo ""
MAX_GRID_LEVELS=$(prompt "Maximum grid levels (2-8)" "$DEFAULT_MAX_GRID_LEVELS")

clear

# ============================================
# ADVANCED OPTIONS
# ============================================
echo -e "${GREEN}=== Step 5: Advanced Options (Optional) ===${NC}"
echo ""

CONFIGURE_ADVANCED=$(prompt_yes_no "Configure advanced options?" "N")

if [ "$CONFIGURE_ADVANCED" = "true" ]; then
    echo ""
    echo -e "${BLUE}Partial Exit Configuration:${NC}"
    echo ""
    echo "The bot can close positions in parts (laddered exits)."
    echo "Example: Close 30% at +1%, 30% at +2%, 40% at +3.5%"
    echo ""
    
    PARTIAL_EXITS=$(prompt_yes_no "Enable partial exits?" "Y")
    
    echo ""
    echo -e "${BLUE}Risk Limits:${NC}"
    echo ""
    MAX_HOLD_HOURS=$(prompt "Maximum hold time in hours" "$DEFAULT_MAX_HOLD_HOURS")
else
    PARTIAL_EXITS="true"
    MAX_HOLD_HOURS="$DEFAULT_MAX_HOLD_HOURS"
fi

clear

# ============================================
# CONFIGURATION SUMMARY
# ============================================
echo -e "${GREEN}=== Configuration Summary ===${NC}"
echo ""
echo -e "${BLUE}API Configuration:${NC}"
echo "  Production Key: ${PROD_KEY:0:8}..."
echo "  Demo Key: ${DEMO_KEY:0:8}..."
echo ""
echo -e "${BLUE}Trading Settings:${NC}"
echo "  Symbol: $SYMBOL"
echo "  Risk %: $RISK_PCT"
echo "  Leverage: ${LEVERAGE}x"
echo ""
echo -e "${BLUE}Strategy Settings:${NC}"
echo "  Entry Mode: $ENTRY_MODE"
if [ "$ENTRY_MODE" = "ema_trend" ]; then
    echo "  EMA Timeframe: ${EMA_TIMEFRAME}m"
    echo "  EMA Candles: $EMA_CANDLES"
    echo "  EMA Fallback: $EMA_FALLBACK"
fi
echo "  Grid Range: ${GRID_RANGE}%"
echo "  Max Grid Levels: $MAX_GRID_LEVELS"
echo "  Partial Exits: $PARTIAL_EXITS"
echo "  Max Hold Time: ${MAX_HOLD_HOURS}h"
echo ""

CONFIRM=$(prompt_yes_no "Save this configuration?" "Y")

if [ "$CONFIRM" != "true" ]; then
    echo -e "${RED}Configuration cancelled.${NC}"
    exit 1
fi

# ============================================
# CREATE CONFIGURATION FILE
# ============================================
mkdir -p config

CONFIG_FILE="config/production.toml"

# Handle fallback boolean
if [ "$EMA_FALLBACK" = "true" ]; then
    FALLBACK_STR="true"
else
    FALLBACK_STR="false"
fi

cat > "$CONFIG_FILE" << EOF
# AlgoTrader Configuration
# Generated on $(date)

[bot]
name = "AlgoTrader-DynaGrid"
version = "0.1.0"
log_level = "info"
data_dir = "./data"

[api.production]
key = "${PROD_KEY}"
secret = "${PROD_SECRET}"
base_url = "https://api.bybit.com"
ws_url = "wss://stream.bybit.com/v5/public/linear"
rate_limit_requests = 50
rate_limit_window_ms = 1000

[api.demo]
key = "${DEMO_KEY}"
secret = "${DEMO_SECRET}"
base_url = "https://api-demo.bybit.com"
rate_limit_requests = 50
rate_limit_window_ms = 1000

[database]
path = "./data/algotrader.db"
backup_enabled = true
backup_interval_hours = 24
backup_retention_days = 30

[risk]
max_daily_loss_pct = 2.0
max_total_exposure_pct = 50.0
emergency_stop_loss_pct = 5.0

[strategy.dynagrid]
enabled = true
symbol = "${SYMBOL}"
position_risk_percentage = ${RISK_PCT}
grid_range_pct = ${GRID_RANGE}
position_sizing_factor = 1.5
max_grid_levels = ${MAX_GRID_LEVELS}
min_entry_interval_minutes = 60
max_hold_time_hours = ${MAX_HOLD_HOURS}
leverage = ${LEVERAGE}

[strategy.dynagrid.entry]
mode = "${ENTRY_MODE}"
ema_candles = ${EMA_CANDLES:-20}
ema_timeframe = "${EMA_TIMEFRAME:-15}"
ema_fallback = ${FALLBACK_STR}

[strategy.dynagrid.exit]
partial_exit_enabled = ${PARTIAL_EXITS}
partial_exit_levels = 3
partial_exit_percentages = [30, 30, 40]
partial_exit_multipliers = [1.0, 2.0, 3.5]
EOF

echo ""
echo -e "${GREEN}✓ Configuration saved to: $CONFIG_FILE${NC}"
echo ""

# Create data directory
mkdir -p data

# ============================================
# NEXT STEPS
# ============================================
echo -e "${GREEN}=== Next Steps ===${NC}"
echo ""
echo "1. Review your configuration:"
echo "   cat $CONFIG_FILE"
echo ""
echo "2. Build the bot:"
echo "   cargo build --release"
echo ""
echo "3. Run the bot:"
echo "   ./target/release/algotrader"
echo ""
echo -e "${YELLOW}⚠️  IMPORTANT:${NC}"
echo "   - Ensure your Demo account has USDT for trading"
echo "   - Start with small position risk (0.01-0.02) for testing"
echo "   - Monitor logs during first run"
echo ""
echo -e "${BLUE}Happy Trading! 🚀${NC}"
echo ""

# Ask if user wants to build now
BUILD_NOW=$(prompt_yes_no "Build the bot now?" "Y")

if [ "$BUILD_NOW" = "true" ]; then
    echo ""
    echo "Building..."
    cargo build --release
    echo ""
    echo -e "${GREEN}✓ Build complete!${NC}"
    echo ""
    
    RUN_NOW=$(prompt_yes_no "Run the bot now?" "N")
    if [ "$RUN_NOW" = "true" ]; then
        echo ""
        echo "Starting AlgoTrader..."
        ./target/release/algotrader
    fi
fi
