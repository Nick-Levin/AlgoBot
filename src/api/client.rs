use crate::api::rest::BybitRestClient;
use crate::api::websocket::{MarketDataCache, MarketDataManager};
use crate::config::ApiConfig;
use crate::db::{AccountBalance, OrderRequest, OrderResponse, Position, Ticker};
use crate::error::BotResult;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Dual API client that manages both Production (read) and Demo (trade) APIs
#[derive(Clone)]
pub struct ApiManager {
    /// Production API client - for market data only
    production: BybitRestClient,
    
    /// Demo API client - for trading actions and account state
    demo: BybitRestClient,
    
    /// Market data cache for accessing ticker data
    market_cache: Option<MarketDataCache>,
    
    /// Shutdown sender for market data manager
    _market_data_shutdown: Option<mpsc::Sender<()>>,
    
    /// Symbol being traded
    symbol: String,
}

impl ApiManager {
    pub fn new(config: &ApiConfig, symbol: impl Into<String>) -> BotResult<Self> {
        let symbol = symbol.into();
        
        let production = BybitRestClient::new(config.production.clone())?;
        let demo = BybitRestClient::new(config.demo.clone())?;

        info!("ApiManager initialized for symbol: {}", symbol);
        
        Ok(Self {
            production,
            demo,
            market_cache: None,
            _market_data_shutdown: None,
            symbol,
        })
    }

    /// Start market data streaming (WebSocket + REST backup)
    pub async fn start_market_data(&mut self, ws_url: &str) -> BotResult<()> {
        let manager = MarketDataManager::new(
            ws_url,
            self.symbol.clone(),
            self.production.clone(),
        );
        
        // Get cache before spawning
        let cache = manager.get_cache();
        
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        // Spawn the market data manager
        tokio::spawn(async move {
            manager.run(shutdown_rx).await;
        });

        // Give it time to connect
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        self.market_cache = Some(cache);
        self._market_data_shutdown = Some(shutdown_tx);

        info!("Market data streaming started for {}", self.symbol);
        Ok(())
    }

    /// Get the market data cache
    pub fn get_market_cache(&self) -> Option<MarketDataCache> {
        self.market_cache.clone()
    }

    // ============== PRODUCTION API METHODS (Read-Only) ==============

    /// Get current ticker from Production API
    /// Falls back to REST if WebSocket not available
    pub async fn get_ticker(&self) -> BotResult<Ticker> {
        // Try WebSocket cache first
        if let Some(cache) = self.get_market_cache() {
            if let Some(ticker) = cache.get_ticker().await {
                return Ok(ticker);
            }
        }

        // Fall back to REST API
        debug!("Fetching ticker via REST API");
        self.production.get_ticker(&self.symbol).await
    }

    /// Get funding rate from Production API
    pub async fn get_funding_rate(&self) -> BotResult<f64> {
        self.production.get_funding_rate(&self.symbol).await
    }

    /// Get klines/candles from Production API
    pub async fn get_klines(&self, interval: &str, limit: u32) -> BotResult<Vec<crate::db::Kline>> {
        self.production.get_klines(&self.symbol, interval, limit).await
    }

    // ============== DEMO API METHODS (Account & Trading) ==============

    /// Get wallet balance from Demo account
    pub async fn get_wallet_balance(&self, coin: &str) -> BotResult<AccountBalance> {
        debug!("Fetching wallet balance from Demo for {}", coin);
        self.demo.get_wallet_balance(coin).await
    }

    /// Get positions from Demo account
    pub async fn get_positions(&self, symbol: Option<&str>) -> BotResult<Vec<Position>> {
        let sym = symbol.unwrap_or(&self.symbol);
        debug!("Fetching positions from Demo for {}", sym);
        self.demo.get_positions(Some(sym)).await
    }

    /// Place an order on Demo account
    pub async fn place_order(&self, order: &OrderRequest) -> BotResult<OrderResponse> {
        info!(
            "Placing order on Demo: {} {} {} @ {:?}",
            order.side, order.qty, order.symbol, order.price
        );
        self.demo.place_order(order).await
    }

    /// Cancel an order on Demo account
    pub async fn cancel_order(&self, order_id: &str) -> BotResult<()> {
        info!("Cancelling order {} on Demo", order_id);
        self.demo.cancel_order(&self.symbol, order_id).await
    }

    /// Get order status from Demo account
    pub async fn get_order_status(&self, order_id: &str) -> BotResult<crate::api::types::OrderStatusItem> {
        debug!("Fetching order status {} from Demo", order_id);
        self.demo.get_order_status(&self.symbol, order_id).await
    }

    /// Close position on Demo account
    pub async fn close_position(&self, side: &str) -> BotResult<()> {
        info!("Closing {} position on Demo for {}", side, self.symbol);
        self.demo.close_position(&self.symbol, side).await
    }

    // ============== HELPER METHODS ==============

    /// Calculate initial position size based on account balance
    /// position_value = free_usdt * risk_percentage
    /// position_size = position_value / current_price
    pub async fn calculate_position_size(&self, risk_percentage: f64) -> BotResult<f64> {
        // Get free USDT balance from Demo
        let balance = self.get_wallet_balance("USDT").await?;
        let free_usdt = balance.available_balance;

        if free_usdt <= 0.0 {
            return Err(crate::error::BotError::InsufficientMargin {
                required: 1.0,
                available: free_usdt,
            });
        }

        // Get current price
        let ticker = self.get_ticker().await?;
        let current_price = ticker.last_price;

        if current_price <= 0.0 {
            return Err(crate::error::BotError::ApiError {
                message: "Invalid price from ticker".to_string(),
                retryable: true,
            });
        }

        // Calculate position value in USDT
        let position_value_usdt = free_usdt * risk_percentage;

        // Convert to coin amount
        let position_size = position_value_usdt / current_price;

        info!(
            "Position sizing: free_usdt={:.2}, risk_pct={:.4}, price={:.2}, size={:.6}",
            free_usdt, risk_percentage, current_price, position_size
        );

        Ok(position_size)
    }

    /// Check if position size meets minimum requirements
    pub fn validate_position_size(&self, size: f64, min_value_usdt: f64) -> BotResult<()> {
        // Bybit minimum order value is typically around $10 for linear contracts
        // But let's make it configurable
        if size <= 0.0 {
            return Err(crate::error::BotError::PositionSizeTooSmall {
                size,
                minimum: min_value_usdt,
            });
        }
        
        Ok(())
    }

    /// Reconcile local state with Demo API
    /// Returns the positions from Demo (API is source of truth)
    pub async fn reconcile_positions(&self) -> BotResult<Vec<Position>> {
        let api_positions = self.get_positions(None).await?;
        
        info!(
            "Reconciled positions from Demo: {} positions found",
            api_positions.len()
        );
        
        for pos in &api_positions {
            debug!(
                "  {}: {} {} @ avg {:.2}",
                pos.symbol, pos.side, pos.size, pos.avg_price
            );
        }

        Ok(api_positions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApiCredentials;

    fn create_test_credentials() -> ApiCredentials {
        ApiCredentials {
            key: "test_key".to_string(),
            secret: "test_secret".to_string(),
            base_url: "https://api-test.bybit.com".to_string(),
            ws_url: None,
            rate_limit_requests: 50,
            rate_limit_window_ms: 1000,
        }
    }

    #[test]
    fn test_api_manager_creation() {
        let config = ApiConfig {
            production: create_test_credentials(),
            demo: create_test_credentials(),
        };

        let manager = ApiManager::new(&config, "ETHUSDT");
        assert!(manager.is_ok());
    }
}
