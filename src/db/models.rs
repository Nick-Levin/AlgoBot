use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::strategy::types::Zone;

/// Represents the current state of a strategy instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyState {
    pub symbol: String,
    pub is_active: bool,
    pub grid_upper_price: Option<f64>,
    pub grid_lower_price: Option<f64>,
    pub grid_level: u8,
    pub max_grid_levels: u8,
    pub long_size: f64,
    pub short_size: f64,
    pub long_avg_price: f64,
    pub short_avg_price: f64,
    pub initial_position_value_usdt: f64,
    pub initial_risk_percentage: f64,
    pub entry_time: Option<DateTime<Utc>>,
    pub last_action_time: Option<DateTime<Utc>>,
    pub max_hold_until: Option<DateTime<Utc>>,
    pub realized_pnl: f64,
    pub funding_fees_paid: f64,
}

impl StrategyState {
    /// Create a new inactive state
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            is_active: false,
            grid_upper_price: None,
            grid_lower_price: None,
            grid_level: 0,
            max_grid_levels: 4,
            long_size: 0.0,
            short_size: 0.0,
            long_avg_price: 0.0,
            short_avg_price: 0.0,
            initial_position_value_usdt: 0.0,
            initial_risk_percentage: 0.0,
            entry_time: None,
            last_action_time: None,
            max_hold_until: None,
            realized_pnl: 0.0,
            funding_fees_paid: 0.0,
        }
    }

    /// Check if the strategy has any open positions
    pub fn has_positions(&self) -> bool {
        self.long_size > 0.0 || self.short_size > 0.0
    }

    /// Get the net exposure (positive = long, negative = short)
    pub fn net_exposure(&self) -> f64 {
        self.long_size - self.short_size
    }

    /// Get the total gross exposure
    pub fn total_exposure(&self) -> f64 {
        self.long_size + self.short_size
    }

    /// Check if the strategy has timed out
    pub fn is_timed_out(&self) -> bool {
        match self.max_hold_until {
            Some(timeout) => Utc::now() > timeout,
            None => false,
        }
    }

    /// Calculate unrealized P&L at a given price
    pub fn unrealized_pnl(&self, current_price: f64) -> f64 {
        let long_pnl = if self.long_size > 0.0 {
            self.long_size * (current_price - self.long_avg_price)
        } else {
            0.0
        };

        let short_pnl = if self.short_size > 0.0 {
            self.short_size * (self.short_avg_price - current_price)
        } else {
            0.0
        };

        long_pnl + short_pnl
    }

    /// Calculate total P&L (realized + unrealized)
    pub fn total_pnl(&self, current_price: f64) -> f64 {
        self.realized_pnl + self.unrealized_pnl(current_price)
    }

    /// Get which zone we're currently in
    pub fn current_zone(&self, price: f64) -> Option<Zone> {
        match (self.grid_upper_price, self.grid_lower_price) {
            (Some(upper), _) if price >= upper => Some(Zone::Upper),
            (_, Some(lower)) if price <= lower => Some(Zone::Lower),
            _ => None,
        }
    }

    /// Check if price is in the range (not in any zone)
    pub fn is_in_range(&self, price: f64) -> bool {
        self.current_zone(price).is_none()
    }
}

/// Record of a trade execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub order_id: String,
    pub symbol: String,
    pub side: String, // "Buy" or "Sell"
    pub order_type: String,
    pub qty: f64,
    pub price: Option<f64>,
    pub avg_filled_price: Option<f64>,
    pub filled_qty: Option<f64>,
    pub order_status: String,
    pub closed_pnl: Option<f64>,
    pub exec_fee: f64,
}

impl TradeRecord {
    pub fn new(
        order_id: impl Into<String>,
        symbol: impl Into<String>,
        side: impl Into<String>,
        order_type: impl Into<String>,
        qty: f64,
    ) -> Self {
        Self {
            order_id: order_id.into(),
            symbol: symbol.into(),
            side: side.into(),
            order_type: order_type.into(),
            qty,
            price: None,
            avg_filled_price: None,
            filled_qty: None,
            order_status: "Created".to_string(),
            closed_pnl: None,
            exec_fee: 0.0,
        }
    }
}

/// Record of a funding fee payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRecord {
    pub symbol: String,
    pub funding_rate: f64,
    pub fee_paid: f64,
    pub position_size: f64,
    pub side: String,
    pub exec_time: DateTime<Utc>,
}

/// Record of a partial exit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialExitRecord {
    pub level: i32,
    pub symbol: String,
    pub long_closed_qty: Option<f64>,
    pub long_avg_close_price: Option<f64>,
    pub short_closed_qty: Option<f64>,
    pub short_avg_close_price: Option<f64>,
    pub realized_pnl: f64,
}

/// Event log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLog {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub data: Option<String>,
}

/// Position information from the exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: String, // "Buy" or "Sell"
    pub size: f64,
    pub avg_price: f64,
    pub leverage: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

impl Position {
    pub fn is_long(&self) -> bool {
        self.side == "Buy"
    }

    pub fn is_short(&self) -> bool {
        self.side == "Sell"
    }
}

/// Account balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub coin: String,
    pub wallet_balance: f64,
    pub available_balance: f64,
    pub unrealized_pnl: f64,
}

/// Order request to be sent to the exchange
#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub qty: f64,
    pub price: Option<f64>,
    pub time_in_force: Option<String>,
    pub reduce_only: bool,
    pub close_on_trigger: bool,
}

impl OrderRequest {
    pub fn market(symbol: impl Into<String>, side: impl Into<String>, qty: f64) -> Self {
        Self {
            symbol: symbol.into(),
            side: side.into(),
            order_type: "Market".to_string(),
            qty,
            price: None,
            time_in_force: None,
            reduce_only: false,
            close_on_trigger: false,
        }
    }

    pub fn limit(
        symbol: impl Into<String>,
        side: impl Into<String>,
        qty: f64,
        price: f64,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side: side.into(),
            order_type: "Limit".to_string(),
            qty,
            price: Some(price),
            time_in_force: Some("GTC".to_string()),
            reduce_only: false,
            close_on_trigger: false,
        }
    }

    pub fn reduce_only(mut self) -> Self {
        self.reduce_only = true;
        self
    }
}

/// Order response from the exchange
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub order_link_id: Option<String>,
}

/// Ticker information
#[derive(Debug, Clone, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub last_price: f64,
    pub bid_price: f64,
    pub ask_price: f64,
    pub volume_24h: f64,
    pub funding_rate: Option<f64>,
    pub next_funding_time: Option<DateTime<Utc>>,
}

/// Kline/candlestick data
#[derive(Debug, Clone, Deserialize)]
pub struct Kline {
    pub start_time: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Orderbook data
#[derive(Debug, Clone, Deserialize)]
pub struct Orderbook {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>, // (price, qty)
    pub asks: Vec<(f64, f64)>,
    pub timestamp: DateTime<Utc>,
}

/// Market data update from WebSocket
#[derive(Debug, Clone)]
pub enum MarketDataUpdate {
    Ticker(Ticker),
    Trade { symbol: String, price: f64, qty: f64, side: String },
    Orderbook(Orderbook),
}
