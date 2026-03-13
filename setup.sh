#!/bin/bash

# AlgoTrader Interactive Setup - CLI Menu Edition
# A user-friendly configuration tool with jump-to-section capability

# Don't exit on error - we want to handle errors gracefully
# set -e

# ============================================
# COLORS
# ============================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# ============================================
# DEFAULT VALUES
# ============================================
DEFAULT_SYMBOL="ETHUSDT"
DEFAULT_RISK_PCT="0.02"
DEFAULT_LEVERAGE="5"
DEFAULT_GRID_RANGE="2.0"
DEFAULT_MAX_GRID="4"
DEFAULT_ENTRY_MODE="ema_trend"
DEFAULT_EMA_CANDLES="20"
DEFAULT_EMA_TIMEFRAME="15"
DEFAULT_EMA_FALLBACK="true"
DEFAULT_MAX_HOLD_HOURS="168"
DEFAULT_PARTIAL_EXITS="true"

# ============================================
# GLOBAL STATE
# ============================================
CONFIG_FILE="config/production.toml"
IS_CONFIGURED=false

# Config values (will be populated)
PROD_KEY=""
PROD_SECRET=""
DEMO_KEY=""
DEMO_SECRET=""
SYMBOL=""
RISK_PCT=""
LEVERAGE=""
GRID_RANGE=""
MAX_GRID_LEVELS=""
ENTRY_MODE=""
EMA_CANDLES=""
EMA_TIMEFRAME=""
EMA_FALLBACK=""
MAX_HOLD_HOURS=""
PARTIAL_EXITS=""
PARTIAL_LEVELS=""
EXIT_MULTIPLIERS=""
MAX_DAILY_LOSS=""
POSITION_FACTOR=""

# ============================================
# UTILITY FUNCTIONS
# ============================================

print_banner() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}              ${BOLD}AlgoTrader - Configuration Wizard${NC}                   ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}         Dynamic Hedge Grid Strategy for Bybit                      ${CYAN}║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

press_enter() {
    echo ""
    read -p "Press Enter to continue..."
}

# Input with default value
get_input() {
    local prompt="$1"
    local default="$2"
    
    if [ -n "$default" ]; then
        echo -ne "${prompt} [${default}]: "
    else
        echo -ne "${prompt}: "
    fi
    
    read -r result
    if [ -z "$result" ] && [ -n "$default" ]; then
        echo "$default"
    else
        echo "$result"
    fi
}

# Yes/No input
get_yes_no() {
    local prompt="$1"
    local default="${2:-Y}"
    local result
    
    while true; do
        if [ "$default" = "Y" ]; then
            echo -ne "${prompt} [Y/n]: "
        else
            echo -ne "${prompt} [y/N]: "
        fi
        
        read -r result
        result="${result:-$default}"
        
        case "$result" in
            [Yy]*) echo "true"; return ;;
            [Nn]*) echo "false"; return ;;
            *) echo "Please answer Y or N" ;;
        esac
    done
}

# Show menu and get selection
show_menu() {
    local title="$1"
    shift
    local options=("$@")
    local choice
    
    echo ""
    echo -e "${BOLD}${title}${NC}"
    echo "────────────────────────────────────────"
    
    local i=1
    for option in "${options[@]}"; do
        printf "  %2d. %s\n" "$i" "$option"
        ((i++))
    done
    
    echo "────────────────────────────────────────"
    
    while true; do
        echo ""
        echo -ne "Select [1-${#options[@]}]: "
        read -r choice
        
        if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "${#options[@]}" ]; then
            return $choice
        fi
        print_error "Invalid choice. Please enter 1-${#options[@]}"
    done
}

# ============================================
# CONFIGURATION SECTIONS
# ============================================

configure_api() {
    print_banner
    echo -e "${BLUE}=== API Configuration ===${NC}"
    echo ""
    
    print_info "You need TWO sets of API keys from Bybit:"
    echo ""
    echo "1. Production API (Read-Only - for market data)"
    echo "   URL: https://www.bybit.com/app/user/api-management"
    echo "   Permissions: Read-Only (Position, Order, Wallet)"
    echo ""
    echo "2. Demo API (Trading - for actual trades)"
    echo "   URL: https://www.bybit.com/demo-trading"
    echo "   Permissions: Orders, Positions, Wallet"
    echo ""
    print_warning "These keys will be stored in config/production.toml"
    echo ""
    
    # Production API
    echo "--- Production API (Read-Only) ---"
    PROD_KEY=$(get_input "Production API Key" "$PROD_KEY")
    echo -n "Production API Secret: "
    read -s PROD_SECRET
    echo ""
    
    # Demo API
    echo ""
    echo "--- Demo API (Trading) ---"
    DEMO_KEY=$(get_input "Demo API Key" "$DEMO_KEY")
    echo -n "Demo API Secret: "
    read -s DEMO_SECRET
    echo ""
    
    # Validate
    if [[ -z "$PROD_KEY" || -z "$PROD_SECRET" || -z "$DEMO_KEY" || -z "$DEMO_SECRET" ]]; then
        print_error "All API keys are required!"
        press_enter
        return 1
    fi
    
    print_success "API keys configured"
    press_enter
}

configure_symbol() {
    print_banner
    echo -e "${BLUE}=== Trading Pair Selection ===${NC}"
    echo ""
    
    echo "Select the cryptocurrency pair to trade:"
    echo ""
    
    show_menu "Available Pairs" \
        "ETHUSDT - Ethereum (Recommended, stable)" \
        "BTCUSDT - Bitcoin (Higher volatility)" \
        "SOLUSDT - Solana (Mid volatility)" \
        "XRPUSDT - Ripple (Good for testing)" \
        "Custom - Enter your own"
    
    local choice=$?
    
    case $choice in
        1) SYMBOL="ETHUSDT" ;;
        2) SYMBOL="BTCUSDT" ;;
        3) SYMBOL="SOLUSDT" ;;
        4) SYMBOL="XRPUSDT" ;;
        5) 
            echo ""
            SYMBOL=$(get_input "Enter trading pair" "ETHUSDT")
            ;;
    esac
    
    print_success "Selected: $SYMBOL"
    press_enter
}

configure_risk() {
    print_banner
    echo -e "${BLUE}=== Risk Settings ===${NC}"
    echo ""
    
    echo "Position Risk Percentage:"
    echo "  How much of your free USDT to use per trade"
    echo "  0.01 = 1% (conservative)"
    echo "  0.02 = 2% (recommended)"
    echo "  0.05 = 5% (aggressive)"
    echo ""
    RISK_PCT=$(get_input "Risk percentage (as decimal)" "${RISK_PCT:-$DEFAULT_RISK_PCT}")
    
    echo ""
    echo "Leverage:"
    echo "  Higher = larger positions but more risk"
    LEVERAGE=$(get_input "Leverage (1-100)" "${LEVERAGE:-$DEFAULT_LEVERAGE}")
    
    echo ""
    echo "Daily Loss Limit:"
    echo "  Bot stops trading after this daily loss"
    MAX_DAILY_LOSS=$(get_input "Max daily loss %" "${MAX_DAILY_LOSS:-2.0}")
    
    print_success "Risk settings saved"
    press_enter
}

configure_strategy() {
    print_banner
    echo -e "${BLUE}=== Strategy Configuration ===${NC}"
    echo ""
    
    echo "Grid Range:"
    echo "  Distance between upper and lower zones"
    echo "  1.0% = tight, more trades"
    echo "  2.0% = balanced (recommended)"
    echo "  3.0% = wide, fewer trades"
    echo ""
    GRID_RANGE=$(get_input "Grid range %" "${GRID_RANGE:-$DEFAULT_GRID_RANGE}")
    
    echo ""
    echo "Max Grid Levels:"
    echo "  Max oscillations before forced exit"
    MAX_GRID_LEVELS=$(get_input "Max grid levels (2-8)" "${MAX_GRID_LEVELS:-$DEFAULT_MAX_GRID}")
    
    echo ""
    echo "Position Sizing Factor:"
    echo "  How much larger the winning position is"
    echo "  1.5 = 50% larger (recommended)"
    POSITION_FACTOR=$(get_input "Sizing factor (1.1-2.0)" "${POSITION_FACTOR:-1.5}")
    
    print_success "Strategy settings saved"
    press_enter
}

configure_entry() {
    print_banner
    echo -e "${BLUE}=== Entry Configuration ===${NC}"
    echo ""
    
    echo "How should the bot decide entry direction?"
    echo ""
    
    show_menu "Entry Mode" \
        "EMA Trend - Follow EMA slope (Recommended)" \
        "Immediate - Always enter LONG" \
        "Wait for Zone - Wait for price to hit zone"
    
    local choice=$?
    
    case $choice in
        1) ENTRY_MODE="ema_trend" ;;
        2) ENTRY_MODE="immediate" ;;
        3) ENTRY_MODE="wait_for_zone" ;;
    esac
    
    if [[ "$ENTRY_MODE" == "ema_trend" ]]; then
        echo ""
        echo "EMA Timeframe:"
        echo "  5m  = fast, more signals"
        echo "  15m = balanced (recommended)"
        echo "  60m = slow, fewer false signals"
        echo "  240m = very slow"
        echo ""
        
        show_menu "Select Timeframe" "5" "15" "60" "240"
        local tf_choice=$?
        
        case $tf_choice in
            1) EMA_TIMEFRAME="5" ;;
            2) EMA_TIMEFRAME="15" ;;
            3) EMA_TIMEFRAME="60" ;;
            4) EMA_TIMEFRAME="240" ;;
        esac
        
        echo ""
        EMA_CANDLES=$(get_input "Number of EMA candles (5-100)" "${EMA_CANDLES:-$DEFAULT_EMA_CANDLES}")
        
        echo ""
        EMA_FALLBACK=$(get_yes_no "Wait for zone if no clear trend?" "Y")
    fi
    
    print_success "Entry mode: $ENTRY_MODE"
    press_enter
}

configure_advanced() {
    print_banner
    echo -e "${BLUE}=== Advanced Settings ===${NC}"
    echo ""
    
    echo "Partial Exits:"
    echo "  Close positions gradually at different profit levels"
    PARTIAL_EXITS=$(get_yes_no "Enable partial exits?" "Y")
    
    if [[ "$PARTIAL_EXITS" == "true" ]]; then
        echo ""
        PARTIAL_LEVELS=$(get_input "Number of exit levels (2-5)" "3")
    fi
    
    echo ""
    echo "Max Hold Time:"
    echo "  Force exit after this many hours"
    MAX_HOLD_HOURS=$(get_input "Max hold time (hours)" "${MAX_HOLD_HOURS:-$DEFAULT_MAX_HOLD_HOURS}")
    
    print_success "Advanced settings saved"
    press_enter
}

save_configuration() {
    print_banner
    echo -e "${BLUE}=== Saving Configuration ===${NC}"
    echo ""
    
    # Create directories
    mkdir -p config data
    
    # Set defaults for unset values
    SYMBOL="${SYMBOL:-$DEFAULT_SYMBOL}"
    RISK_PCT="${RISK_PCT:-$DEFAULT_RISK_PCT}"
    LEVERAGE="${LEVERAGE:-$DEFAULT_LEVERAGE}"
    GRID_RANGE="${GRID_RANGE:-$DEFAULT_GRID_RANGE}"
    MAX_GRID_LEVELS="${MAX_GRID_LEVELS:-$DEFAULT_MAX_GRID}"
    ENTRY_MODE="${ENTRY_MODE:-$DEFAULT_ENTRY_MODE}"
    EMA_CANDLES="${EMA_CANDLES:-$DEFAULT_EMA_CANDLES}"
    EMA_TIMEFRAME="${EMA_TIMEFRAME:-$DEFAULT_EMA_TIMEFRAME}"
    EMA_FALLBACK="${EMA_FALLBACK:-$DEFAULT_EMA_FALLBACK}"
    MAX_HOLD_HOURS="${MAX_HOLD_HOURS:-$DEFAULT_MAX_HOLD_HOURS}"
    PARTIAL_EXITS="${PARTIAL_EXITS:-$DEFAULT_PARTIAL_EXITS}"
    MAX_DAILY_LOSS="${MAX_DAILY_LOSS:-2.0}"
    POSITION_FACTOR="${POSITION_FACTOR:-1.5}"
    
    # Build exit config
    local exit_config
    if [[ "$PARTIAL_EXITS" == "true" ]]; then
        exit_config="partial_exit_enabled = true
partial_exit_levels = ${PARTIAL_LEVELS:-3}
partial_exit_percentages = [30, 30, 40]
partial_exit_multipliers = [1.0, 2.0, 3.5]"
    else
        exit_config="partial_exit_enabled = false
partial_exit_levels = 1
partial_exit_percentages = [100]
partial_exit_multipliers = [1.0]"
    fi
    
    # Write config
    cat > "$CONFIG_FILE" << EOF
# AlgoTrader Configuration
# Generated on $(date)

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
$exit_config
EOF
    
    IS_CONFIGURED=true
    
    print_success "Configuration saved to: $CONFIG_FILE"
    echo ""
    echo "Summary:"
    echo "  Symbol: $SYMBOL"
    echo "  Risk: ${RISK_PCT}% of free USDT"
    echo "  Leverage: ${LEVERAGE}x"
    echo "  Grid Range: ${GRID_RANGE}%"
    echo "  Entry Mode: $ENTRY_MODE"
    
    press_enter
}

view_config() {
    print_banner
    echo -e "${BLUE}=== Current Configuration ===${NC}"
    echo ""
    
    if [[ -f "$CONFIG_FILE" ]]; then
        cat "$CONFIG_FILE"
    else
        print_warning "No configuration file found"
    fi
    
    press_enter
}

build_project() {
    print_banner
    echo -e "${BLUE}=== Build Project ===${NC}"
    echo ""
    
    if [[ "$IS_CONFIGURED" != "true" ]] && [[ ! -f "$CONFIG_FILE" ]]; then
        print_error "Please configure first!"
        press_enter
        return 1
    fi
    
    print_info "Building release binary..."
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
    
    if [[ ! -f "target/release/algotrader" ]]; then
        print_error "Binary not found. Please build first."
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
    
    print_info "This will configure everything with recommended defaults."
    echo ""
    
    # API Keys (required)
    configure_api || return 1
    
    # Use defaults for everything else
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
    local build_now=$(get_yes_no "Build the bot now?" "Y")
    if [[ "$build_now" == "true" ]]; then
        build_project
    fi
}

# ============================================
# MAIN MENU
# ============================================

main_menu() {
    while true; do
        print_banner
        
        # Check if configured
        if [[ -f "$CONFIG_FILE" ]]; then
            IS_CONFIGURED=true
            echo -e "Status: ${GREEN}Configured${NC}"
            echo ""
        fi
        
        echo -e "${BOLD}Main Menu${NC}"
        echo ""
        echo "  1. Quick Start (configure with defaults)"
        echo "  2. Configure API Keys"
        echo "  3. Configure Trading Pair"
        echo "  4. Configure Risk Settings"
        echo "  5. Configure Strategy"
        echo "  6. Configure Entry Mode"
        echo "  7. Advanced Settings"
        echo "  8. Save Configuration"
        echo "  9. View Current Config"
        echo "  10. Build Project"
        echo "  11. Run Bot"
        echo "  12. Exit"
        echo ""
        
        local choice=$(get_input "Select option" "")
        
        case "$choice" in
            1) quick_start ;;
            2) configure_api ;;
            3) configure_symbol ;;
            4) configure_risk ;;
            5) configure_strategy ;;
            6) configure_entry ;;
            7) configure_advanced ;;
            8) save_configuration ;;
            9) view_config ;;
            10) build_project ;;
            11) run_bot ;;
            12) exit 0 ;;
            *) print_error "Invalid option: $choice" && press_enter ;;
        esac
    done
}

# ============================================
# LOAD EXISTING CONFIG
# ============================================

load_existing_config() {
    if [[ ! -f "$CONFIG_FILE" ]]; then
        return
    fi
    
    IS_CONFIGURED=true
    
    # Extract values from existing config
    PROD_KEY=$(grep "^key = " "$CONFIG_FILE" | head -1 | cut -d'"' -f2)
    DEMO_KEY=$(grep "^key = " "$CONFIG_FILE" | tail -1 | cut -d'"' -f2)
    SYMBOL=$(grep "^symbol = " "$CONFIG_FILE" | cut -d'"' -f2)
    
    # Don't load secrets (they're hidden)
}

# ============================================
# START
# ============================================

# Load existing config if present
load_existing_config

# Show main menu
main_menu
