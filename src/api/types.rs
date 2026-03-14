use serde::{Deserialize, Serialize};

/// Generic API response wrapper from Bybit
#[derive(Debug, Clone, Deserialize)]
pub struct BybitResponse<T> {
    #[serde(rename = "retCode")]
    pub ret_code: i32,
    #[serde(rename = "retMsg")]
    pub ret_msg: String,
    pub result: T,
    pub time: u64,
}

impl<T> BybitResponse<T> {
    pub fn is_success(&self) -> bool {
        self.ret_code == 0
    }

    pub fn into_result(self) -> crate::error::BotResult<T> {
        if self.is_success() {
            Ok(self.result)
        } else {
            Err(crate::error::BotError::ApiError {
                message: self.ret_msg,
                retryable: matches!(self.ret_code, 10001 | 10002 | 10003 | 10004), // Rate limit errors
            })
        }
    }
}

/// Wallet balance response
#[derive(Debug, Clone, Deserialize)]
pub struct WalletBalanceResponse {
    #[serde(default)]
    pub list: Vec<WalletBalanceItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletBalanceItem {
    #[serde(default)]
    pub coin: Vec<CoinBalance>,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Deserialize)]
pub struct CoinBalance {
    pub coin: String,
    #[serde(rename = "walletBalance")]
    pub wallet_balance: String,
    #[serde(default, rename = "availableToWithdraw")]
    pub available_to_withdraw: Option<String>,
    #[serde(default, rename = "availableBalance")]
    pub available_balance: Option<String>,
    #[serde(default)]
    pub equity: Option<String>,
    #[serde(default, rename = "unrealisedPnl")]
    pub unrealised_pnl: Option<String>,
}

impl CoinBalance {
    pub fn wallet_balance_f64(&self) -> f64 {
        self.wallet_balance.parse().unwrap_or(0.0)
    }

    pub fn available_balance_f64(&self) -> f64 {
        self.available_balance.as_ref()
            .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
            .unwrap_or_else(|| self.wallet_balance_f64())
    }

    pub fn equity_f64(&self) -> f64 {
        self.equity.as_ref()
            .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
            .unwrap_or_else(|| self.wallet_balance_f64())
    }
}

/// Position response
#[derive(Debug, Clone, Deserialize)]
pub struct PositionResponse {
    pub list: Vec<PositionItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionItem {
    pub symbol: String,
    pub side: String,
    pub size: String,
    #[serde(rename = "avgPrice")]
    pub avg_price: String,
    pub leverage: String,
    #[serde(rename = "positionValue")]
    pub position_value: String,
    #[serde(rename = "unrealisedPnl")]
    pub unrealised_pnl: String,
    #[serde(rename = "cumRealisedPnl")]
    pub cum_realised_pnl: String,
}

impl PositionItem {
    pub fn size_f64(&self) -> f64 {
        self.size.parse().unwrap_or(0.0)
    }

    pub fn avg_price_f64(&self) -> f64 {
        self.avg_price.parse().unwrap_or(0.0)
    }

    pub fn unrealised_pnl_f64(&self) -> f64 {
        self.unrealised_pnl.parse().unwrap_or(0.0)
    }
}

/// Ticker response
#[derive(Debug, Clone, Deserialize)]
pub struct TickerResponse {
    pub list: Vec<TickerItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TickerItem {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "bid1Price")]
    pub bid1_price: String,
    #[serde(rename = "ask1Price")]
    pub ask1_price: String,
    #[serde(rename = "volume24h")]
    pub volume24h: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: Option<String>,
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: Option<String>,
}

impl TickerItem {
    pub fn last_price_f64(&self) -> f64 {
        self.last_price.parse().unwrap_or(0.0)
    }

    pub fn bid_price_f64(&self) -> f64 {
        self.bid1_price.parse().unwrap_or(0.0)
    }

    pub fn ask_price_f64(&self) -> f64 {
        self.ask1_price.parse().unwrap_or(0.0)
    }

    pub fn funding_rate_f64(&self) -> Option<f64> {
        self.funding_rate.as_ref()?.parse().ok()
    }
}

/// Orderbook response
#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookResponse {
    pub s: String,
    pub a: Option<Vec<OrderbookLevel>>, // Asks
    pub b: Option<Vec<OrderbookLevel>>, // Bids
    pub ts: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookLevel {
    #[serde(rename = "0")]
    pub price: String,
    #[serde(rename = "1")]
    pub qty: String,
}

/// Order placement request
#[derive(Debug, Clone, Serialize)]
pub struct PlaceOrderRequest {
    pub category: String,
    pub symbol: String,
    pub side: String,
    #[serde(rename = "orderType")]
    pub order_type: String,
    pub qty: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "price")]
    pub price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "timeInForce")]
    pub time_in_force: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "reduceOnly")]
    pub reduce_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "closeOnTrigger")]
    pub close_on_trigger: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "positionIdx")]
    pub position_idx: Option<i32>, // 0: one-way, 1: hedge buy, 2: hedge sell
    #[serde(skip_serializing_if = "Option::is_none", rename = "marketUnit")]
    pub market_unit: Option<String>, // "baseCoin" or "quoteCoin" for market orders
}

/// Order placement response
#[derive(Debug, Clone, Deserialize)]
pub struct PlaceOrderResult {
    #[serde(default, rename = "orderId")]
    pub order_id: Option<String>,
    #[serde(default, rename = "orderLinkId")]
    pub order_link_id: Option<String>,
}

/// Order status response
#[derive(Debug, Clone, Deserialize)]
pub struct OrderStatusResponse {
    pub list: Vec<OrderStatusItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderStatusItem {
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: String,
    pub qty: String,
    #[serde(rename = "cumExecQty")]
    pub cum_exec_qty: String,
    #[serde(rename = "cumExecValue")]
    pub cum_exec_value: String,
    #[serde(rename = "cumExecFee")]
    pub cum_exec_fee: String,
    #[serde(rename = "orderStatus")]
    pub order_status: String,
    #[serde(rename = "avgPrice")]
    pub avg_price: String,
    #[serde(rename = "createdTime")]
    pub created_time: String,
    #[serde(rename = "updatedTime")]
    pub updated_time: String,
}

impl OrderStatusItem {
    pub fn is_filled(&self) -> bool {
        self.order_status == "Filled"
    }

    pub fn is_open(&self) -> bool {
        matches!(self.order_status.as_str(), "Created" | "New" | "PartiallyFilled")
    }

    pub fn qty_f64(&self) -> f64 {
        self.qty.parse().unwrap_or(0.0)
    }

    pub fn cum_exec_qty_f64(&self) -> f64 {
        self.cum_exec_qty.parse().unwrap_or(0.0)
    }

    pub fn avg_price_f64(&self) -> f64 {
        self.avg_price.parse().unwrap_or(0.0)
    }

    pub fn cum_exec_fee_f64(&self) -> f64 {
        self.cum_exec_fee.parse().unwrap_or(0.0)
    }
}

/// Cancel order response
#[derive(Debug, Clone, Deserialize)]
pub struct CancelOrderResult {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "orderLinkId")]
    pub order_link_id: String,
}

/// Funding rate response
#[derive(Debug, Clone, Deserialize)]
pub struct FundingRateResponse {
    pub list: Vec<FundingRateItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FundingRateItem {
    pub symbol: String,
    #[serde(rename = "fundingRate")]
    pub funding_rate: String,
    #[serde(rename = "fundingRateTimestamp")]
    pub funding_rate_timestamp: String,
}

impl FundingRateItem {
    pub fn funding_rate_f64(&self) -> f64 {
        self.funding_rate.parse().unwrap_or(0.0)
    }
}

/// Kline response
#[derive(Debug, Clone, Deserialize)]
pub struct KlineResponse {
    pub list: Vec<KlineItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KlineItem {
    #[serde(rename = "0")]
    pub start_time: String,
    #[serde(rename = "1")]
    pub open: String,
    #[serde(rename = "2")]
    pub high: String,
    #[serde(rename = "3")]
    pub low: String,
    #[serde(rename = "4")]
    pub close: String,
    #[serde(rename = "5")]
    pub volume: String,
}

impl KlineItem {
    pub fn start_time_ms(&self) -> i64 {
        self.start_time.parse().unwrap_or(0)
    }

    pub fn close_f64(&self) -> f64 {
        self.close.parse().unwrap_or(0.0)
    }

    pub fn high_f64(&self) -> f64 {
        self.high.parse().unwrap_or(0.0)
    }

    pub fn low_f64(&self) -> f64 {
        self.low.parse().unwrap_or(0.0)
    }
}

/// WebSocket message types
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "topic")]
pub enum WsMessage {
    #[serde(rename = "tickers.{symbol}")]
    Ticker { data: WsTickerData },
    #[serde(rename = "orderbook.50.{symbol}")]
    Orderbook { data: WsOrderbookData },
    #[serde(rename = "publicTrade.{symbol}")]
    Trade { data: Vec<WsTradeData> },
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WsTickerData {
    pub symbol: String,
    #[serde(default, rename = "lastPrice")]
    pub last_price: Option<String>,
    #[serde(default, rename = "bid1Price")]
    pub bid1_price: Option<String>,
    #[serde(default, rename = "ask1Price")]
    pub ask1_price: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WsOrderbookData {
    pub s: String,
    pub a: Option<Vec<Vec<String>>>, // Asks: [[price, qty], ...]
    pub b: Option<Vec<Vec<String>>>, // Bids: [[price, qty], ...]
    pub u: u64, // Update ID
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Deserialize)]
pub struct WsTradeData {
    pub T: i64, // Timestamp
    pub s: String, // Symbol
    pub S: String, // Side
    pub v: String, // Size
    pub p: String, // Price
}

/// Convert WebSocket ticker to internal ticker
impl From<WsTickerData> for crate::db::Ticker {
    fn from(data: WsTickerData) -> Self {
        Self {
            symbol: data.symbol.clone(),
            last_price: data.last_price.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            bid_price: data.bid1_price.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            ask_price: data.ask1_price.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            volume_24h: 0.0,
            funding_rate: None,
            next_funding_time: None,
        }
    }
}
