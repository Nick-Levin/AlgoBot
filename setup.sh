#!/bin/bash

# AlgoTrader Interactive Setup - Menu Edition
# Simple numbered menu that's easy to navigate

# ============================================
# COLORS
# ============================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ============================================
# DEFAULTS
# ============================================
DEFAULT_SYMBOL="ETHUSDT"
DEFAULT_RISK_PCT="0.02"
DEFAULT_LEVERAGE="5"
DEFAULT_GRID_RANGE="2.0"
DEFAULT_MAX_GRID="4"
DEFAULT_ENTRY_MODE="ema_trend"
DEFAULT_EMA_CANDLES="20"
DEFAULT_EMA_TIMEFRAME="15"
DEFAULT_MAX_HOLD_HOURS="168"

# ============================================
# GLOBALS
# ============================================
CONFIG_FILE="config/production.toml"
IS_CONFIGURED=false

# Config values
PROD_KEY="" PROD_SECRET="" DEMO_KEY="" DEMO_SECRET=""
SYMBOL="" RISK_PCT="" LEVERAGE="" GRID_RANGE=""
MAX_GRID_LEVELS="" ENTRY_MODE="" EMA_CANDLES=""
EMA_TIMEFRAME="" EMA_FALLBACK="" MAX_HOLD_HOURS=""
PARTIAL_EXITS="" MAX_DAILY_LOSS="" POSITION_FACTOR=""

# ============================================
# UTILITIES
# ============================================

print_banner() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}           ${BOLD}AlgoTrader - Configuration Wizard${NC}                     ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}      Dynamic Hedge Grid Strategy for Bybit                         ${CYAN}║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_success() { echo -e "${GREEN}✓${NC} $1"; }
print_warning() { echo -e "${YELLOW}⚠${NC} $1"; }
print_error() { echo -e "${RED}✗${NC} $1"; }
print_info() { echo -e "${CYAN}ℹ${NC} $1"; }

press_enter() {
    echo ""
    read -p "Press Enter to continue..."
}

get_input() {
    local prompt="$1" default="$2"
    if [ -n "$default" ]; then
        echo -ne "$prompt [$default]: "
    else
        echo -ne "$prompt: "
    fi
    read -r result
    echo "${result:-$default}"
}

get_yes_no() {
    local prompt="$1" default="${2:-Y}"
    while true; do
        if [ "$default" = "Y" ]; then
            echo -ne "$prompt [Y/n]: "
        else
            echo -ne "$prompt [y/N]: "
        fi
        read -r result
        result="${result:-$default}"
        case "$result" in
            [Yy]*) return 0 ;;
            [Nn]*) return 1 ;;
            *) echo "Please answer Y or N" ;;
        esac
    done
}

# ============================================
# MENU SYSTEM - Using select for clarity
# ============================================

show_main_menu() {
    print_banner
    
    if [ -f "$CONFIG_FILE" ]; then
        IS_CONFIGURED=true
        echo -e "Status: ${GREEN}✓ Configured${NC}"
        echo ""
    else
        echo -e "Status: ${YELLOW}⚠ Not configured${NC}"
        echo ""
    fi
    
    echo -e "${BOLD}Select an option:${NC}"
    echo ""
    
    PS3=$'\nEnter choice (1-12): '
    
    local options=(
        "🚀 Quick Start - Configure everything with defaults"
        "⚙️  Configure API Keys (Required first step)"
        "💱 Configure Trading Pair"
        "⚖️  Configure Risk Settings"
        "📊 Configure Strategy Settings"
        "🚪 Configure Entry Mode (EMA/Immediate/Wait)"
        "🔧 Advanced Settings"
        "💾 Save Configuration to File"
        "📄 View Current Configuration"
        "🔨 Build Project"
        "▶️  Run Bot"
        "❌ Exit"
    )
    
    select opt in "${options[@]}"; do
        case $REPLY in
            1) quick_start; break ;;
            2) configure_api; break ;;
            3) configure_symbol; break ;;
            4) configure_risk; break ;;
            5) configure_strategy; break ;;
            6) configure_entry; break ;;
            7) configure_advanced; break ;;
            8) save_configuration; break ;;
            9) view_config; break ;;
            10) build_project; break ;;
            11) run_bot; break ;;
            12) exit 0 ;;
            *) print_error "Invalid option: $REPLY"; sleep 1; break ;;
        esac
    done
}

# ============================================
# CONFIG SECTIONS
# ============================================

configure_api() {
    print_banner
    echo -e "${BLUE}=== API Configuration ===${NC}"
    echo ""
    print_info "You need TWO API key pairs from Bybit:"
    echo ""
    echo "1. Production API (Read-Only)"
    echo "   https://www.bybit.com/app/user/api-management"
    echo "   Permissions: Read-Only"
    echo ""
    echo "2. Demo API (Trading)"
    echo "   https://www.bybit.com/demo-trading"
    echo "   Permissions: Orders, Positions, Wallet"
    echo ""
    
    echo "--- Production API ---"
    PROD_KEY=$(get_input "Production API Key" "$PROD_KEY")
    echo -n "Production API Secret: "
    read -s PROD_SECRET
    echo ""
    
    echo ""
    echo "--- Demo API ---"
    DEMO_KEY=$(get_input "Demo API Key" "$DEMO_KEY")
    echo -n "Demo API Secret: "
    read -s DEMO_SECRET
    echo ""
    
    if [ -z "$PROD_KEY" ] || [ -z "$PROD_SECRET" ] || [ -z "$DEMO_KEY" ] || [ -z "$DEMO_SECRET" ]; then
        print_error "All API keys are required!"
        press_enter
        return 1
    fi
    
    print_success "API keys configured"
    press_enter
}

configure_symbol() {
    print_banner
    echo -e "${BLUE}=== Trading Pair ===${NC}"
    echo ""
    
    PS3=$'\nSelect trading pair: '
    options=(
        "ETHUSDT - Ethereum (Recommended)"
        "BTCUSDT - Bitcoin (Higher volatility)"
        "SOLUSDT - Solana (Mid volatility)"
        "XRPUSDT - Ripple (Good for testing)"
        "Custom pair"
    )
    
    select opt in "${options[@]}"; do
        case $REPLY in
            1) SYMBOL="ETHUSDT"; break ;;
            2) SYMBOL="BTCUSDT"; break ;;
            3) SYMBOL="SOLUSDT"; break ;;
            4) SYMBOL="XRPUSDT"; break ;;
            5) 
                SYMBOL=$(get_input "Enter trading pair" "ETHUSDT")
                break
                ;;
            *) print_error "Invalid choice"; sleep 1; break ;;
        esac
    done
    
    [ -n "$SYMBOL" ] && print_success "Selected: $SYMBOL"
    press_enter
}

configure_risk() {
    print_banner
    echo -e "${BLUE}=== Risk Settings ===${NC}"
    echo ""
    
    echo "Position Risk Percentage:"
    echo "  0.01 = 1% (conservative)"
    echo "  0.02 = 2% (recommended)"
    echo "  0.05 = 5% (aggressive)"
    echo ""
    RISK_PCT=$(get_input "Risk percentage (decimal)" "${RISK_PCT:-$DEFAULT_RISK_PCT}")
    
    echo ""
    LEVERAGE=$(get_input "Leverage (1-100)" "${LEVERAGE:-$DEFAULT_LEVERAGE}")
    
    echo ""
    MAX_DAILY_LOSS=$(get_input "Daily loss limit %" "${MAX_DAILY_LOSS:-2.0}")
    
    print_success "Risk settings saved"
    press_enter
}

configure_strategy() {
    print_banner
    echo -e "${BLUE}=== Strategy Settings ===${NC}"
    echo ""
    
    echo "Grid Range (distance between zones):"
    echo "  1.0% = tight, more trades"
    echo "  2.0% = balanced (recommended)"
    echo "  3.0% = wide, fewer trades"
    echo ""
    GRID_RANGE=$(get_input "Grid range %" "${GRID_RANGE:-$DEFAULT_GRID_RANGE}")
    
    echo ""
    MAX_GRID_LEVELS=$(get_input "Max grid levels (2-8)" "${MAX_GRID_LEVELS:-$DEFAULT_MAX_GRID}")
    
    echo ""
    POSITION_FACTOR=$(get_input "Position sizing factor (1.1-2.0)" "${POSITION_FACTOR:-1.5}")
    
    print_success "Strategy settings saved"
    press_enter
}

configure_entry() {
    print_banner
    echo -e "${BLUE}=== Entry Mode ===${NC}"
    echo ""
    
    PS3=$'\nSelect entry mode: '
    options=(
        "EMA Trend - Follow EMA slope (Recommended)"
        "Immediate - Always enter LONG"
        "Wait for Zone - Wait for price to hit zone"
    )
    
    select opt in "${options[@]}"; do
        case $REPLY in
            1) ENTRY_MODE="ema_trend"; break ;;
            2) ENTRY_MODE="immediate"; break ;;
            3) ENTRY_MODE="wait_for_zone"; break ;;
            *) print_error "Invalid choice"; sleep 1; break ;;
        esac
    done
    
    if [ "$ENTRY_MODE" = "ema_trend" ]; then
        echo ""
        echo "EMA Timeframe:"
        PS3=$'\nSelect timeframe: '
        tf_opts=("5 minutes" "15 minutes (recommended)" "60 minutes" "240 minutes")
        select tf in "${tf_opts[@]}"; do
            case $REPLY in
                1) EMA_TIMEFRAME="5"; break ;;
                2) EMA_TIMEFRAME="15"; break ;;
                3) EMA_TIMEFRAME="60"; break ;;
                4) EMA_TIMEFRAME="240"; break ;;
                *) print_error "Invalid"; sleep 1; break ;;
            esac
        done
        
        echo ""
        EMA_CANDLES=$(get_input "EMA candles (5-100)" "${EMA_CANDLES:-$DEFAULT_EMA_CANDLES}")
        
        echo ""
        if get_yes_no "Wait for zone if no trend?" "Y"; then
            EMA_FALLBACK="true"
        else
            EMA_FALLBACK="false"
        fi
    fi
    
    print_success "Entry mode: $ENTRY_MODE"
    press_enter
}

configure_advanced() {
    print_banner
    echo -e "${BLUE}=== Advanced Settings ===${NC}"
    echo ""
    
    if get_yes_no "Enable partial exits?" "Y"; then
        PARTIAL_EXITS="true"
        echo ""
        PARTIAL_LEVELS=$(get_input "Number of exit levels" "3")
    else
        PARTIAL_EXITS="false"
    fi
    
    echo ""
    MAX_HOLD_HOURS=$(get_input "Max hold time (hours)" "${MAX_HOLD_HOURS:-$DEFAULT_MAX_HOLD_HOURS}")
    
    print_success "Advanced settings saved"
    press_enter
}

save_configuration() {
    print_banner
    echo -e "${BLUE}=== Saving Configuration ===${NC}"
    echo ""
    
    mkdir -p config data
    
    # Set defaults
    : "${SYMBOL:=$DEFAULT_SYMBOL}"
    : "${RISK_PCT:=$DEFAULT_RISK_PCT}"
    : "${LEVERAGE:=$DEFAULT_LEVERAGE}"
    : "${GRID_RANGE:=$DEFAULT_GRID_RANGE}"
    : "${MAX_GRID_LEVELS:=$DEFAULT_MAX_GRID}"
    : "${ENTRY_MODE:=$DEFAULT_ENTRY_MODE}"
    : "${EMA_CANDLES:=$DEFAULT_EMA_CANDLES}"
    : "${EMA_TIMEFRAME:=$DEFAULT_EMA_TIMEFRAME}"
    : "${EMA_FALLBACK:=$DEFAULT_EMA_FALLBACK}"
    : "${MAX_HOLD_HOURS:=$DEFAULT_MAX_HOLD_HOURS}"
    : "${PARTIAL_EXITS:=$DEFAULT_PARTIAL_EXITS}"
    : "${MAX_DAILY_LOSS:=2.0}"
    : "${POSITION_FACTOR:=1.5}"
    
    # Exit config
    if [ "$PARTIAL_EXITS" = "true" ]; then
        exit_cfg="partial_exit_enabled = true
partial_exit_levels = ${PARTIAL_LEVELS:-3}
partial_exit_percentages = [30, 30, 40]
partial_exit_multipliers = [1.0, 2.0, 3.5]"
    else
        exit_cfg="partial_exit_enabled = false
partial_exit_levels = 1
partial_exit_percentages = [100]
partial_exit_multipliers = [1.0]"
    fi
    
    cat > "$CONFIG_FILE" << EOF
# AlgoTrader Configuration
# Generated: $(date)

[bot]
name = "AlgoTrader-DynaGrid"
version = "0.1.0"
log_level = "info"
data_dir = "./data"

[api.production]
key = "$PROD_KEY"
secret = "$PROD_SECRET"
base_url = "https://api.bybit.com"
ws_url = "wss://stream.bybit.com/v5/public/linear"

[api.demo]
key = "$DEMO_KEY"
secret = "$DEMO_SECRET"
base_url = "https://api-demo.bybit.com"

[database]
path = "./data/algotrader.db"
backup_enabled = true

[risk]
max_daily_loss_pct = $MAX_DAILY_LOSS
max_total_exposure_pct = 50.0
emergency_stop_loss_pct = 5.0

[strategy.dynagrid]
enabled = true
symbol = "$SYMBOL"
position_risk_percentage = $RISK_PCT
grid_range_pct = $GRID_RANGE
position_sizing_factor = $POSITION_FACTOR
max_grid_levels = $MAX_GRID_LEVELS
max_hold_time_hours = $MAX_HOLD_HOURS
leverage = $LEVERAGE

[strategy.dynagrid.entry]
mode = "$ENTRY_MODE"
ema_candles = $EMA_CANDLES
ema_timeframe = "$EMA_TIMEFRAME"
ema_fallback = $EMA_FALLBACK

[strategy.dynagrid.exit]
$exit_cfg
EOF
    
    IS_CONFIGURED=true
    
    print_success "Saved to: $CONFIG_FILE"
    echo ""
    echo "Summary:"
    echo "  Symbol: $SYMBOL"
    echo "  Risk: ${RISK_PCT}%"
    echo "  Leverage: ${LEVERAGE}x"
    echo "  Grid: ${GRID_RANGE}%"
    echo "  Mode: $ENTRY_MODE"
    
    press_enter
}

view_config() {
    print_banner
    echo -e "${BLUE}=== Current Configuration ===${NC}"
    echo ""
    
    if [ -f "$CONFIG_FILE" ]; then
        cat "$CONFIG_FILE"
    else
        print_warning "No configuration found"
    fi
    
    press_enter
}

build_project() {
    print_banner
    echo -e "${BLUE}=== Build Project ===${NC}"
    echo ""
    
    if [ "$IS_CONFIGURED" != "true" ] && [ ! -f "$CONFIG_FILE" ]; then
        print_error "Please configure first!"
        press_enter
        return 1
    fi
    
    print_info "Building..."
    echo ""
    
    if cargo build --release; then
        echo ""
        print_success "Build successful!"
        echo "Binary: ./target/release/algotrader"
    else
        echo ""
        print_error "Build failed!"
    fi
    
    press_enter
}

run_bot() {
    print_banner
    echo -e "${BLUE}=== Run Bot ===${NC}"
    echo ""
    
    if [ ! -f "target/release/algotrader" ]; then
        print_error "Binary not found. Build first."
        press_enter
        return 1
    fi
    
    print_info "Starting AlgoTrader..."
    echo ""
    
    ./target/release/algotrader
}

quick_start() {
    print_banner
    echo -e "${BLUE}=== Quick Start ===${NC}"
    echo ""
    
    print_info "Quick configuration with recommended defaults"
    echo ""
    
    # API required
    configure_api || return 1
    
    # Use defaults
    SYMBOL="$DEFAULT_SYMBOL"
    RISK_PCT="$DEFAULT_RISK_PCT"
    LEVERAGE="$DEFAULT_LEVERAGE"
    GRID_RANGE="$DEFAULT_GRID_RANGE"
    MAX_GRID_LEVELS="$DEFAULT_MAX_GRID"
    ENTRY_MODE="$DEFAULT_ENTRY_MODE"
    EMA_CANDLES="$DEFAULT_EMA_CANDLES"
    EMA_TIMEFRAME="$DEFAULT_EMA_TIMEFRAME"
    EMA_FALLBACK="$DEFAULT_EMA_FALLBACK"
    MAX_HOLD_HOURS="$DEFAULT_MAX_HOLD_HOURS"
    PARTIAL_EXITS="$DEFAULT_PARTIAL_EXITS"
    MAX_DAILY_LOSS="2.0"
    POSITION_FACTOR="1.5"
    
    save_configuration
    
    echo ""
    if get_yes_no "Build now?" "Y"; then
        build_project
    fi
}

# ============================================
# LOAD EXISTING
# ============================================

load_existing() {
    if [ -f "$CONFIG_FILE" ]; then
        IS_CONFIGURED=true
        PROD_KEY=$(grep "^key = " "$CONFIG_FILE" | head -1 | cut -d'"' -f2)
        DEMO_KEY=$(grep "^key = " "$CONFIG_FILE" | tail -1 | cut -d'"' -f2)
        SYMBOL=$(grep "^symbol = " "$CONFIG_FILE" | cut -d'"' -f2)
    fi
}

# ============================================
# MAIN
# ============================================

load_existing

while true; do
    show_main_menu
    echo ""
done
