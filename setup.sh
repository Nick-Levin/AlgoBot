#!/bin/bash

# AlgoTrader Interactive Setup - CLI Menu Edition
# A user-friendly configuration tool with jump-to-section capability

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Default values
DEFAULTS=(
    ["symbol"]="ETHUSDT"
    ["risk_pct"]="0.02"
    ["leverage"]="5"
    ["grid_range"]="2.0"
    ["max_grid"]="4"
    ["entry_mode"]="ema_trend"
    ["ema_candles"]="20"
    ["ema_timeframe"]="15"
    ["ema_fallback"]="true"
    ["max_hold_hours"]="168"
    ["partial_exits"]="true"
)

# Current configuration
CONFIG_FILE="config/production.toml"
IS_CONFIGURED=false

# ============================================
# UTILITY FUNCTIONS
# ============================================

clear_screen() {
    clear
}

print_header() {
    clear_screen
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}              ${BOLD}AlgoTrader - Configuration Wizard${NC}                   ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}         Dynamic Hedge Grid Strategy for Bybit                      ${CYAN}║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_section() {
    echo -e "\n${BLUE}▶ $1${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ $1${NC}"
}

# Press any key to continue
press_any_key() {
    echo ""
    read -n 1 -s -r -p "Press any key to continue..."
}

# Input with default value
input_with_default() {
    local prompt="$1"
    local default="$2"
    local result
    
    echo -ne "${prompt} ${CYAN}[${default}]${NC}: "
    read -r result
    echo "${result:-$default}"
}

# Yes/No prompt
confirm() {
    local prompt="$1"
    local default="${2:-Y}"
    local result
    
    if [[ "$default" == "Y" ]]; then
        echo -ne "${prompt} ${CYAN}[Y/n]${NC}: "
    else
        echo -ne "${prompt} ${CYAN}[y/N]${NC}: "
    fi
    
    read -r result
    result="${result:-$default}"
    
    [[ "$result" =~ ^[Yy]$ ]] && echo "true" || echo "false"
}

# Select from menu
select_option() {
    local title="$1"
    shift
    local options=("$@")
    local choice
    
    echo -e "\n${BOLD}${title}${NC}"
    echo "────────────────────────────────────────"
    
    for i in "${!options[@]}"; do
        printf "  ${CYAN}%2d.${NC} %s\n" $((i+1)) "${options[$i]}"
    done
    echo "────────────────────────────────────────"
    
    while true; do
        echo -ne "\nSelect option ${CYAN}[1-${#options[@]}]${NC}: "
        read -r choice
        
        if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "${#options[@]}" ]; then
            echo "$choice"
            return
        fi
        print_error "Invalid choice. Please enter a number between 1 and ${#options[@]}."
    done
}

# ============================================
# CONFIGURATION SECTIONS
# ============================================

# Section 1: API Keys
configure_api() {
    print_header
    print_section "API Configuration"
    
    print_info "You need TWO sets of API keys from Bybit:"
    echo ""
    echo -e "${BOLD}1. Production API${NC} (Read-Only - for market data)"
    echo "   Get from: https://www.bybit.com/app/user/api-management"
    echo "   Permissions needed: Read-Only (Position, Order, Wallet)"
    echo ""
    echo -e "${BOLD}2. Demo API${NC} (Trading - for actual trades)"
    echo "   Get from: https://www.bybit.com/demo-trading"
    echo "   Permissions needed: Orders, Positions, Wallet"
    echo ""
    print_warning "These keys are sensitive and will be stored in config/production.toml"
    echo ""
    
    # Production API
    echo -e "${BLUE}Production API (Read-Only):${NC}"
    PROD_KEY=$(input_with_default "Enter Production API Key" "${PROD_KEY:-}")
    echo -ne "Enter Production API Secret: "
    read -s PROD_SECRET
    echo ""
    
    # Demo API
    echo ""
    echo -e "${BLUE}Demo API (Trading):${NC}"
    DEMO_KEY=$(input_with_default "Enter Demo API Key" "${DEMO_KEY:-}")
    echo -ne "Enter Demo API Secret: "
    read -s DEMO_SECRET
    echo ""
    
    if [[ -z "$PROD_KEY" || -z "$PROD_SECRET" || -z "$DEMO_KEY" || -z "$DEMO_SECRET" ]]; then
        print_error "All API keys are required!"
        press_any_key
        return 1
    fi
    
    print_success "API keys configured"
    press_any_key
    return 0
}

# Section 2: Trading Pair
configure_symbol() {
    print_header
    print_section "Trading Pair Selection"
    
    print_info "Select the cryptocurrency pair you want to trade:"
    echo ""
    
    local options=(
        "ETHUSDT - Ethereum (Recommended for beginners, stable)"
        "BTCUSDT - Bitcoin (Higher volatility, larger moves)"
        "SOLUSDT - Solana (Mid volatility, faster price action)"
        "XRPUSDT - Ripple (Lower price, good for testing)"
        "Custom - Enter your own pair"
    )
    
    local choice=$(select_option "Available Trading Pairs" "${options[@]}")
    
    case "$choice" in
        1) SYMBOL="ETHUSDT" ;;
        2) SYMBOL="BTCUSDT" ;;
        3) SYMBOL="SOLUSDT" ;;
        4) SYMBOL="XRPUSDT" ;;
        5) 
            echo ""
            SYMBOL=$(input_with_default "Enter trading pair (e.g., ADAUSDT)" "ETHUSDT")
            ;;
    esac
    
    print_success "Selected trading pair: $SYMBOL"
    press_any_key
}

# Section 3: Risk Settings
configure_risk() {
    print_header
    print_section "Risk Management Settings"
    
    echo -e "${BOLD}Position Risk Percentage${NC}"
    echo "This determines how much of your free USDT to use per initial trade."
    echo ""
    echo -e "  ${CYAN}0.01${NC} (1%)  - Conservative, smaller positions"
    echo -e "  ${CYAN}0.02${NC} (2%)  - Recommended, balanced approach"
    echo -e "  ${CYAN}0.05${NC} (5%)  - Aggressive, larger positions"
    echo ""
    
    RISK_PCT=$(input_with_default "Position risk percentage (as decimal)" "${RISK_PCT:-${DEFAULTS[risk_pct]}}")
    
    echo ""
    echo -e "${BOLD}Leverage${NC}"
    echo "Higher leverage = larger positions but higher liquidation risk"
    echo ""
    LEVERAGE=$(input_with_default "Leverage (1-100)" "${LEVERAGE:-${DEFAULTS[leverage]}}")
    
    echo ""
    echo -e "${BOLD}Daily Loss Limit${NC}"
    echo "Bot will stop trading if daily loss exceeds this percentage"
    MAX_DAILY_LOSS=$(input_with_default "Max daily loss %" "2.0")
    
    print_success "Risk settings configured"
    press_any_key
}

# Section 4: Strategy Settings
configure_strategy() {
    print_header
    print_section "Strategy Configuration"
    
    # Grid Range
    echo -e "${BOLD}Grid Range${NC}"
    echo "The distance between upper and lower trading zones."
    echo "Example: 2% range = zones at ±1% from entry price"
    echo ""
    echo -e "  ${CYAN}1.0%${NC} - Tight grid, more frequent trades, lower profit per trade"
    echo -e "  ${CYAN}2.0%${NC} - Balanced grid (recommended)"
    echo -e "  ${CYAN}3.0%${NC} - Wide grid, fewer trades, higher profit per trade"
    echo ""
    GRID_RANGE=$(input_with_default "Grid range percentage" "${GRID_RANGE:-${DEFAULTS[grid_range]}}")
    
    # Max Grid Levels
    echo ""
    echo -e "${BOLD}Maximum Grid Levels${NC}"
    echo "Maximum number of oscillations before forced exit"
    echo "Higher = more capital required but more chances to profit"
    MAX_GRID_LEVELS=$(input_with_default "Max grid levels (2-8)" "${MAX_GRID_LEVELS:-${DEFAULTS[max_grid]}}")
    
    # Position Sizing Factor
    echo ""
    echo -e "${BOLD}Position Sizing Factor${NC}"
    echo "How much larger the winning-side position should be"
    echo "1.5x = winning position is 50% larger than losing position"
    POSITION_FACTOR=$(input_with_default "Sizing factor (1.1-2.0)" "1.5")
    
    print_success "Strategy settings configured"
    press_any_key
}

# Section 5: Entry Mode
configure_entry() {
    print_header
    print_section "Entry Configuration"
    
    echo -e "${BOLD}Entry Mode${NC}"
    echo "How the bot decides when and in which direction to enter:"
    echo ""
    
    local options=(
        "EMA Trend - Uses EMA slope to determine LONG/SHORT (Recommended)"
        "Immediate - Always enters LONG immediately (Simple)"
        "Wait for Zone - Waits for price to hit a zone (Conservative)"
    )
    
    local choice=$(select_option "Select Entry Mode" "${options[@]}")
    
    case "$choice" in
        1) ENTRY_MODE="ema_trend" ;;
        2) ENTRY_MODE="immediate" ;;
        3) ENTRY_MODE="wait_for_zone" ;;
    esac
    
    # EMA-specific settings
    if [[ "$ENTRY_MODE" == "ema_trend" ]]; then
        echo ""
        echo -e "${BOLD}EMA Timeframe${NC}"
        echo "Candle timeframe for EMA calculation:"
        echo ""
        echo -e "  ${CYAN}5m${NC}  - Fast signals, more entries (scalping)"
        echo -e "  ${CYAN}15m${NC} - Balanced (recommended)"
        echo -e "  ${CYAN}60m${NC} - Slower signals, fewer false entries"
        echo -e "  ${CYAN}240m${NC}- Very slow, trend following"
        echo ""
        
        local tf_options=("5" "15 (recommended)" "60" "240")
        local tf_choice=$(select_option "Select EMA Timeframe" "${tf_options[@]}")
        EMA_TIMEFRAME=$(echo "${tf_options[$tf_choice-1]}" | cut -d' ' -f1)
        
        echo ""
        EMA_CANDLES=$(input_with_default "Number of EMA candles (5-100)" "${EMA_CANDLES:-${DEFAULTS[ema_candles]}}")
        
        echo ""
        EMA_FALLBACK=$(confirm "Wait for zone touch if no clear trend?" "Y")
    fi
    
    print_success "Entry mode configured: $ENTRY_MODE"
    press_any_key
}

# Section 6: Advanced Settings
configure_advanced() {
    print_header
    print_section "Advanced Configuration"
    
    # Partial Exits
    echo -e "${BOLD}Partial Exits${NC}"
    echo "Close positions gradually at different profit levels"
    echo "Example: Close 30% at +1%, 30% at +2%, 40% at +3.5%"
    echo ""
    PARTIAL_EXITS=$(confirm "Enable partial exits?" "Y")
    
    if [[ "$PARTIAL_EXITS" == "true" ]]; then
        echo ""
        PARTIAL_LEVELS=$(input_with_default "Number of partial exit levels (2-5)" "3")
        
        echo ""
        echo -e "${BOLD}Exit Distances${NC}"
        echo "Multiplier of grid_range for each exit level"
        echo "Example: 1.0 = 1x grid_range, 2.0 = 2x grid_range"
        
        EXIT_MULTIPLIERS=""
        for ((i=1; i<=PARTIAL_LEVELS; i++)); do
            local default_mult="1.0"
            [[ $i == 2 ]] && default_mult="2.0"
            [[ $i == 3 ]] && default_mult="3.5"
            local mult=$(input_with_default "Exit $i distance multiplier" "$default_mult")
            EXIT_MULTIPLIERS="$EXIT_MULTIPLIERS, $mult"
        done
        EXIT_MULTIPLIERS="[${EXIT_MULTIPLIERS#, }]"
    fi
    
    # Hold Time
    echo ""
    echo -e "${BOLD}Maximum Hold Time${NC}"
    echo "Force exit after this many hours to avoid excessive funding fees"
    MAX_HOLD_HOURS=$(input_with_default "Max hold time (hours)" "${MAX_HOLD_HOURS:-${DEFAULTS[max_hold_hours]}}")
    
    print_success "Advanced settings configured"
    press_any_key
}

# ============================================
# SAVE & BUILD
# ============================================

save_config() {
    print_header
    print_section "Saving Configuration"
    
    # Create directories
    mkdir -p config data
    
    # Determine exit config
    if [[ "$PARTIAL_EXITS" == "true" ]]; then
        # Generate percentages based on levels
        local percentages=""
        if [[ "$PARTIAL_LEVELS" == "3" ]]; then
            percentages="[30, 30, 40]"
        elif [[ "$PARTIAL_LEVELS" == "2" ]]; then
            percentages="[50, 50]"
        elif [[ "$PARTIAL_LEVELS" == "4" ]]; then
            percentages="[25, 25, 25, 25]"
        else
            percentages="[20, 20, 20, 20, 20]"
        fi
        
        EXIT_CONFIG="partial_exit_enabled = true
partial_exit_levels = $PARTIAL_LEVELS
partial_exit_percentages = $percentages
partial_exit_multipliers = ${EXIT_MULTIPLIERS:-[1.0, 2.0, 3.5]}"
    else
        EXIT_CONFIG="partial_exit_enabled = false
partial_exit_levels = 1
partial_exit_percentages = [100]
partial_exit_multipliers = [1.0]"
    fi
    
    # Write config file
    cat > "$CONFIG_FILE" << EOF
# AlgoTrader Configuration
# Generated on $(date)
# Edit with ./setup.sh or manually

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
max_daily_loss_pct = ${MAX_DAILY_LOSS:-2.0}
max_total_exposure_pct = 50.0
emergency_stop_loss_pct = 5.0

[strategy.dynagrid]
enabled = true
symbol = "${SYMBOL:-ETHUSDT}"
position_risk_percentage = ${RISK_PCT:-0.02}
grid_range_pct = ${GRID_RANGE:-2.0}
position_sizing_factor = ${POSITION_FACTOR:-1.5}
max_grid_levels = ${MAX_GRID_LEVELS:-4}
min_entry_interval_minutes = 60
max_hold_time_hours = ${MAX_HOLD_HOURS:-168}
leverage = ${LEVERAGE:-5}

[strategy.dynagrid.entry]
mode = "${ENTRY_MODE:-ema_trend}"
ema_candles = ${EMA_CANDLES:-20}
ema_timeframe = "${EMA_TIMEFRAME:-15}"
ema_fallback = ${EMA_FALLBACK:-true}

[strategy.dynagrid.exit]
${EXIT_CONFIG}
EOF
    
    print_success "Configuration saved to: $CONFIG_FILE"
    IS_CONFIGURED=true
    
    echo ""
    print_info "Configuration Summary:"
    echo "  Trading Pair: ${SYMBOL:-ETHUSDT}"
    echo "  Risk: ${RISK_PCT:-0.02} (${RISK_PCT:-0.02}% of free USDT)"
    echo "  Leverage: ${LEVERAGE:-5}x"
    echo "  Grid Range: ${GRID_RANGE:-2.0}%"
    echo "  Entry Mode: ${ENTRY_MODE:-ema_trend}"
    echo "  Max Grid Levels: ${MAX_GRID_LEVELS:-4}"
    
    press_any_key
}

build_project() {
    print_header
    print_section "Building Project"
    
    if [[ "$IS_CONFIGURED" != "true" ]]; then
        print_warning "No configuration found. Please configure first."
        press_any_key
        return 1
    fi
    
    print_info "Building release binary..."
    echo ""
    
    if cargo build --release; then
        print_success "Build successful!"
        print_info "Binary location: ./target/release/algotrader"
    else
        print_error "Build failed!"
        return 1
    fi
    
    press_any_key
}

run_bot() {
    print_header
    print_section "Running Bot"
    
    if [[ ! -f "target/release/algotrader" ]]; then
        print_warning "Binary not found. Please build first."
        press_any_key
        return 1
    fi
    
    echo -e "${GREEN}Starting AlgoTrader...${NC}"
    echo ""
    
    ./target/release/algotrader
}

show_config() {
    print_header
    print_section "Current Configuration"
    
    if [[ -f "$CONFIG_FILE" ]]; then
        cat "$CONFIG_FILE"
    else
        print_warning "No configuration file found at $CONFIG_FILE"
    fi
    
    press_any_key
}

# ============================================
# MAIN MENU
# ============================================

main_menu() {
    while true; do
        print_header
        
        echo -e "${BOLD}Main Menu${NC}\n"
        
        if [[ -f "$CONFIG_FILE" ]]; then
            echo -e "  Status: ${GREEN}Configured${NC}"
            echo ""
        fi
        
        local options=(
            "🚀 Quick Start - Configure everything with defaults"
            "⚙️  Configure API Keys (Required)"
            "💱 Configure Trading Pair"
            "⚖️  Configure Risk Settings"
            "📊 Configure Strategy"
            "🚪 Configure Entry Mode"
            "🔧 Advanced Settings"
            "💾 Save Configuration"
            "📄 View Current Config"
            "🔨 Build Project"
            "▶️  Run Bot"
            "❌ Exit"
        )
        
        local choice=$(select_option "Select an option" "${options[@]}")
        
        case "$choice" in
            1)  # Quick Start
                configure_api && \
                configure_symbol && \
                configure_risk && \
                configure_strategy && \
                configure_entry && \
                save_config && \
                build_project
                ;;
            2) configure_api ;;
            3) configure_symbol ;;
            4) configure_risk ;;
            5) configure_strategy ;;
            6) configure_entry ;;
            7) configure_advanced ;;
            8) save_config ;;
            9) show_config ;;
            10) build_project ;;
            11) run_bot ;;
            12) exit 0 ;;
        esac
    done
}

# ============================================
# INITIALIZATION
# ============================================

# Check if already configured
if [[ -f "$CONFIG_FILE" ]]; then
    IS_CONFIGURED=true
fi

# Load existing config if present
if [[ -f "$CONFIG_FILE" ]]; then
    # Extract values from existing config
    SYMBOL=$(grep "^symbol = " "$CONFIG_FILE" | cut -d'"' -f2)
    RISK_PCT=$(grep "^position_risk_percentage = " "$CONFIG_FILE" | cut -d' ' -f3)
    LEVERAGE=$(grep "^leverage = " "$CONFIG_FILE" | cut -d' ' -f3)
    GRID_RANGE=$(grep "^grid_range_pct = " "$CONFIG_FILE" | cut -d' ' -f3)
    MAX_GRID_LEVELS=$(grep "^max_grid_levels = " "$CONFIG_FILE" | cut -d' ' -f3)
    ENTRY_MODE=$(grep "^mode = " "$CONFIG_FILE" | cut -d'"' -f2)
fi

# Start main menu
main_menu
