use crate::api::types::*;
use crate::db::Ticker;
use crate::error::BotResult;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct MarketDataCache {
    pub ticker: Arc<RwLock<Option<Ticker>>>,
    pub last_update: Arc<RwLock<Instant>>,
}

impl MarketDataCache {
    pub fn new() -> Self {
        Self {
            ticker: Arc::new(RwLock::new(None)),
            last_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub async fn get_ticker(&self) -> Option<Ticker> {
        self.ticker.read().await.clone()
    }

    pub async fn is_stale(&self, max_age: Duration) -> bool {
        let last = *self.last_update.read().await;
        last.elapsed() > max_age
    }
}

pub struct BybitWebSocketClient {
    url: String,
    symbol: String,
    cache: MarketDataCache,
    reconnect_interval: Duration,
    stale_threshold: Duration,
}

impl BybitWebSocketClient {
    pub fn new(url: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            symbol: symbol.into(),
            cache: MarketDataCache::new(),
            reconnect_interval: Duration::from_secs(5),
            stale_threshold: Duration::from_secs(10),
        }
    }

    pub fn get_cache(&self) -> MarketDataCache {
        self.cache.clone()
    }

    pub async fn run(self, mut shutdown: mpsc::Receiver<()>) {
        let symbol = self.symbol.clone();
        let url = self.url.clone();
        let cache = self.cache.clone();
        let reconnect_interval = self.reconnect_interval;

        info!("Starting WebSocket client for {}", symbol);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("WebSocket shutdown signal received");
                    break;
                }
                result = Self::connection_loop(&url, &symbol, &cache) => {
                    match result {
                        Ok(_) => {
                            warn!("WebSocket connection closed, reconnecting in {:?}...", reconnect_interval);
                        }
                        Err(e) => {
                            error!("WebSocket error: {}, reconnecting in {:?}...", e, reconnect_interval);
                        }
                    }
                    sleep(reconnect_interval).await;
                }
            }
        }

        info!("WebSocket client stopped");
    }

    async fn connection_loop(
        url: &str,
        symbol: &str,
        cache: &MarketDataCache,
    ) -> BotResult<()> {
        let (ws_stream, _) = connect_async(url).await.map_err(|e| {
            crate::error::BotError::WebSocketError(format!("Connection failed: {}", e))
        })?;

        info!("WebSocket connected to {}", url);

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to ticker topic
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": [
                {
                    "channel": "tickers",
                    "symbol": symbol
                }
            ]
        });

        write
            .send(Message::Text(subscribe_msg.to_string()))
            .await
            .map_err(|e| {
                crate::error::BotError::WebSocketError(format!("Subscribe failed: {}", e))
            })?;

        info!("Subscribed to ticker updates for {}", symbol);

        // Handle incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("WebSocket message: {}", text);
                    
                    if let Err(e) = Self::handle_message(&text, cache).await {
                        warn!("Failed to handle message: {}", e);
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Respond with pong
                    write.send(Message::Pong(data)).await.ok();
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    return Err(crate::error::BotError::WebSocketError(format!(
                        "Message error: {}",
                        e
                    )));
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_message(text: &str, cache: &MarketDataCache) -> BotResult<()> {
        // Try to parse as generic message first
        let value: serde_json::Value = serde_json::from_str(text)?;

        // Check if it's a success response
        if value.get("success").is_some() {
            debug!("WebSocket operation succeeded: {}", text);
            return Ok(());
        }

        // Check for ticker data
        if let Some(topic) = value.get("topic").and_then(|t| t.as_str()) {
            if topic.starts_with("tickers.") {
                if let Some(data) = value.get("data") {
                    let ticker_data: WsTickerData = serde_json::from_value(data.clone())?;
                    let ticker: Ticker = ticker_data.into();

                    // Update cache
                    *cache.ticker.write().await = Some(ticker);
                    *cache.last_update.write().await = Instant::now();
                }
            }
        }

        Ok(())
    }

    /// Check if we should fall back to REST polling
    pub async fn is_stale(&self) -> bool {
        self.cache.is_stale(self.stale_threshold).await
    }
}

/// Market data manager that combines WebSocket and REST polling
pub struct MarketDataManager {
    ws_client: BybitWebSocketClient,
    rest_client: crate::api::rest::BybitRestClient,
    poll_interval: Duration,
}

impl MarketDataManager {
    pub fn new(
        ws_url: impl Into<String>,
        symbol: impl Into<String>,
        rest_client: crate::api::rest::BybitRestClient,
    ) -> Self {
        Self {
            ws_client: BybitWebSocketClient::new(ws_url, symbol),
            rest_client,
            poll_interval: Duration::from_secs(5),
        }
    }

    pub async fn run(self, shutdown: mpsc::Receiver<()>) {
        let symbol = self.ws_client.symbol.clone();
        let cache = self.ws_client.get_cache();
        let poll_interval = self.poll_interval;
        let rest_client = self.rest_client;

        // Start WebSocket in background
        let ws_handle = tokio::spawn(async move {
            self.ws_client.run(shutdown).await;
        });

        // Start polling loop as backup
        let poll_handle = tokio::spawn(async move {
            loop {
                sleep(poll_interval).await;

                // Check if WebSocket data is stale
                if cache.is_stale(Duration::from_secs(10)).await {
                    warn!("WebSocket data is stale, falling back to REST polling");

                    // Fetch from REST API
                    match rest_client.get_ticker(&symbol).await {
                        Ok(ticker) => {
                            *cache.ticker.write().await = Some(ticker);
                            *cache.last_update.write().await = Instant::now();
                            debug!("Updated ticker via REST polling");
                        }
                        Err(e) => {
                            warn!("REST polling failed: {}", e);
                        }
                    }
                }
            }
        });

        let _ = tokio::join!(ws_handle, poll_handle);
    }

    pub fn get_cache(&self) -> MarketDataCache {
        self.ws_client.get_cache()
    }
}
