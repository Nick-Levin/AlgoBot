use crate::api::types::*;
use crate::db::{AccountBalance, Kline, OrderRequest, OrderResponse, Orderbook, Position, Ticker};
use crate::error::{retryable_api_error, BotError, BotResult};
use crate::config::ApiCredentials;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Client, Method, RequestBuilder};
use sha2::Sha256;
use std::time::Duration;
use tracing::{debug, error, info, warn};

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct BybitRestClient {
    client: Client,
    credentials: ApiCredentials,
    recv_window: u64,
}

impl BybitRestClient {
    pub fn new(credentials: ApiCredentials) -> BotResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            client,
            credentials,
            recv_window: 5000, // 5 seconds
        })
    }

    /// Generate authentication headers for Bybit API
    /// Format: timestamp + api_key + recv_window + payload
    fn sign_request(&self, timestamp: u64, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.credentials.secret.as_bytes())
            .expect("HMAC can take key of any size");
        let sign_data = format!("{}{}{}{}", timestamp, self.credentials.key, self.recv_window, payload);
        debug!("Signing data: {}", sign_data);
        mac.update(sign_data.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// Build a signed request
    fn build_request(&self, method: Method, path: &str, params: Option<&str>) -> RequestBuilder {
        let timestamp = Utc::now().timestamp_millis() as u64;
        let payload = if method == Method::GET {
            params.unwrap_or("")
        } else {
            params.unwrap_or("")
        };

        let signature = self.sign_request(timestamp, payload);

        let url = format!("{}{}", self.credentials.base_url, path);
        let mut request = self.client.request(method, &url);

        // Add headers
        request = request
            .header("X-BAPI-API-KEY", &self.credentials.key)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-RECV-WINDOW", self.recv_window.to_string())
            .header("X-BAPI-SIGN", signature)
            .header("Content-Type", "application/json");

        request
    }

    // ============== Wallet & Account ==============

    /// Get wallet balance (for Demo account)
    pub async fn get_wallet_balance(&self, coin: &str) -> BotResult<AccountBalance> {
        let path = "/v5/account/wallet-balance";
        let params = format!("accountType=UNIFIED&coin={}", coin);
        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch wallet balance: {}", e);
                BotError::NetworkError(e)
            })?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;
        debug!("Wallet balance response: {}", body);

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<WalletBalanceResponse> =
            serde_json::from_str(&body).map_err(|e| {
                error!("Failed to parse wallet balance response: {}", e);
                error!("Response body: {}", body);
                BotError::SerializationError(e)
            })?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006, // Rate limit
            });
        }

        // Check if list is empty
        if result.result.list.is_empty() {
            error!("Wallet balance response has empty list");
            error!("Full response: {}", body);
            return Err(BotError::ApiError {
                message: "Empty wallet balance list".to_string(),
                retryable: false,
            });
        }

        let balance_item = result
            .result
            .list
            .first()
            .and_then(|item| item.coin.first())
            .ok_or_else(|| BotError::ApiError {
                message: "No balance found".to_string(),
                retryable: false,
            })?;

        Ok(AccountBalance {
            coin: balance_item.coin.clone(),
            wallet_balance: balance_item.wallet_balance_f64(),
            available_balance: balance_item.available_balance_f64(),
            unrealized_pnl: balance_item.unrealisedPnl.parse().unwrap_or(0.0),
        })
    }

    // ============== Positions ==============

    /// Get positions (for Demo account)
    pub async fn get_positions(&self, symbol: Option<&str>) -> BotResult<Vec<Position>> {
        let path = "/v5/position/list";
        let mut params = "category=linear&settleCoin=USDT".to_string();
        
        if let Some(sym) = symbol {
            params.push_str(&format!("&symbol={}", sym));
        }

        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch positions: {}", e);
                BotError::NetworkError(e)
            })?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;
        debug!("Positions response: {}", body);

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<PositionResponse> =
            serde_json::from_str(&body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        let positions: Vec<Position> = result
            .result
            .list
            .into_iter()
            .filter(|p| p.size_f64() > 0.0) // Only non-zero positions
            .map(|p| Position {
                symbol: p.symbol.clone(),
                side: p.side.clone(),
                size: p.size_f64(),
                avg_price: p.avg_price_f64(),
                leverage: p.leverage.parse().unwrap_or(1.0),
                unrealized_pnl: p.unrealised_pnl_f64(),
                realized_pnl: p.cum_realised_pnl.parse().unwrap_or(0.0),
            })
            .collect();

        Ok(positions)
    }

    // ============== Market Data ==============

    /// Get ticker (from Production)
    pub async fn get_ticker(&self, symbol: &str) -> BotResult<Ticker> {
        let path = "/v5/market/tickers";
        let params = format!("category=linear&symbol={}", symbol);
        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch ticker: {}", e);
                BotError::NetworkError(e)
            })?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<TickerResponse> =
            serde_json::from_str(&body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        let ticker_item = result.result.list.first().ok_or_else(|| BotError::ApiError {
            message: "No ticker data found".to_string(),
            retryable: false,
        })?;

        Ok(Ticker {
            symbol: ticker_item.symbol.clone(),
            last_price: ticker_item.last_price_f64(),
            bid_price: ticker_item.bid_price_f64(),
            ask_price: ticker_item.ask_price_f64(),
            volume_24h: ticker_item.volume24h.parse().unwrap_or(0.0),
            funding_rate: ticker_item.funding_rate_f64(),
            next_funding_time: ticker_item
                .next_funding_time
                .as_ref()
                .and_then(|t| t.parse::<i64>().ok())
                .map(|ts| chrono::DateTime::from_timestamp(ts / 1000, 0))
                .flatten(),
        })
    }

    /// Get orderbook (from Production)
    pub async fn get_orderbook(&self, symbol: &str, limit: u8) -> BotResult<Orderbook> {
        let path = "/v5/market/orderbook";
        let params = format!("category=linear&symbol={}&limit={}", symbol, limit);
        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(BotError::NetworkError)?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<OrderbookResponse> =
            serde_json::from_str(&body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        let bids: Vec<(f64, f64)> = result
            .result
            .b
            .unwrap_or_default()
            .into_iter()
            .map(|level| (level.price.parse().unwrap_or(0.0), level.qty.parse().unwrap_or(0.0)))
            .collect();

        let asks: Vec<(f64, f64)> = result
            .result
            .a
            .unwrap_or_default()
            .into_iter()
            .map(|level| (level.price.parse().unwrap_or(0.0), level.qty.parse().unwrap_or(0.0)))
            .collect();

        Ok(Orderbook {
            symbol: result.result.s,
            bids,
            asks,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get funding rate (from Production)
    pub async fn get_funding_rate(&self, symbol: &str) -> BotResult<f64> {
        let path = "/v5/market/funding/history";
        let params = format!("category=linear&symbol={}&limit=1", symbol);
        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(BotError::NetworkError)?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<FundingRateResponse> =
            serde_json::from_str(&body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        result
            .result
            .list
            .first()
            .map(|f| f.funding_rate_f64())
            .ok_or_else(|| BotError::ApiError {
                message: "No funding rate found".to_string(),
                retryable: false,
            })
    }

    /// Get klines/candles (from Production)
    pub async fn get_klines(&self, symbol: &str, interval: &str, limit: u32) -> BotResult<Vec<Kline>> {
        let path = "/v5/market/kline";
        let params = format!(
            "category=linear&symbol={}&interval={}&limit={}",
            symbol, interval, limit
        );
        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(BotError::NetworkError)?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<KlineResponse> =
            serde_json::from_str(&body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        let klines: Vec<Kline> = result
            .result
            .list
            .into_iter()
            .map(|k| Kline {
                start_time: chrono::DateTime::from_timestamp(k.start_time_ms() / 1000, 0)
                    .unwrap_or_else(|| chrono::Utc::now()),
                open: k.open.parse().unwrap_or(0.0),
                high: k.high.parse().unwrap_or(0.0),
                low: k.low.parse().unwrap_or(0.0),
                close: k.close_f64(),
                volume: k.volume.parse().unwrap_or(0.0),
            })
            .collect();

        Ok(klines)
    }

    // ============== Trading (Demo API only) ==============

    /// Place an order (to Demo)
    pub async fn place_order(&self, order: &OrderRequest) -> BotResult<OrderResponse> {
        let path = "/v5/order/create";

        let request_body = PlaceOrderRequest {
            category: "linear".to_string(),
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            order_type: order.order_type.clone(),
            qty: order.qty.to_string(),
            price: order.price.map(|p| p.to_string()),
            time_in_force: order.time_in_force.clone(),
            reduce_only: if order.reduce_only { Some(true) } else { None },
            close_on_trigger: if order.close_on_trigger { Some(true) } else { None },
            position_idx: Some(0), // One-way mode
        };

        let body_json = serde_json::to_string(&request_body)?;
        debug!("Placing order: {}", body_json);

        let response = self
            .build_request(Method::POST, path, Some(&body_json))
            .body(body_json)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to place order: {}", e);
                BotError::NetworkError(e)
            })?;

        let status = response.status();
        let response_body = response.text().await.map_err(BotError::NetworkError)?;
        info!("Order response: {}", response_body);

        if !status.is_success() {
            return Err(BotError::OrderRejected {
                reason: format!("HTTP {}: {}", status, response_body),
            });
        }

        let result: BybitResponse<PlaceOrderResult> =
            serde_json::from_str(&response_body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::OrderRejected {
                reason: result.ret_msg,
            });
        }

        Ok(OrderResponse {
            order_id: result.result.order_id,
            order_link_id: Some(result.result.order_link_id),
        })
    }

    /// Cancel an order (on Demo)
    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> BotResult<()> {
        let path = "/v5/order/cancel";

        let request_body = serde_json::json!({
            "category": "linear",
            "symbol": symbol,
            "orderId": order_id,
        });

        let body_json = request_body.to_string();

        let response = self
            .build_request(Method::POST, path, Some(&body_json))
            .body(body_json)
            .send()
            .await
            .map_err(BotError::NetworkError)?;

        let status = response.status();
        let response_body = response.text().await.map_err(BotError::NetworkError)?;

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, response_body
            )));
        }

        let result: BybitResponse<CancelOrderResult> =
            serde_json::from_str(&response_body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        Ok(())
    }

    /// Get order status (from Demo)
    pub async fn get_order_status(&self, symbol: &str, order_id: &str) -> BotResult<OrderStatusItem> {
        let path = "/v5/order/realtime";
        let params = format!(
            "category=linear&symbol={}&orderId={}",
            symbol, order_id
        );
        let url = format!("{}?{}", path, params);

        let response = self
            .build_request(Method::GET, &url, Some(&params))
            .send()
            .await
            .map_err(BotError::NetworkError)?;

        let status = response.status();
        let body = response.text().await.map_err(BotError::NetworkError)?;

        if !status.is_success() {
            return Err(retryable_api_error(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: BybitResponse<OrderStatusResponse> =
            serde_json::from_str(&body).map_err(BotError::SerializationError)?;

        if !result.is_success() {
            return Err(BotError::ApiError {
                message: result.ret_msg,
                retryable: result.ret_code == 10006,
            });
        }

        result
            .result
            .list
            .into_iter()
            .next()
            .ok_or_else(|| BotError::ApiError {
                message: "Order not found".to_string(),
                retryable: false,
            })
    }

    /// Close all positions for a symbol (on Demo)
    pub async fn close_position(&self, symbol: &str, side: &str) -> BotResult<()> {
        // First get current position
        let positions = self.get_positions(Some(symbol)).await?;
        
        let position = positions.iter().find(|p| {
            if side.eq_ignore_ascii_case("buy") {
                p.side == "Sell" // Close short with buy
            } else {
                p.side == "Buy" // Close long with sell
            }
        });

        if let Some(pos) = position {
            let close_side = if pos.side == "Buy" { "Sell" } else { "Buy" };
            
            let order = OrderRequest {
                symbol: symbol.to_string(),
                side: close_side.to_string(),
                order_type: "Market".to_string(),
                qty: pos.size,
                price: None,
                time_in_force: None,
                reduce_only: true,
                close_on_trigger: false,
            };

            self.place_order(&order).await?;
            info!("Closed {} position: {} {}", pos.side, pos.size, symbol);
        } else {
            warn!("No {} position found to close for {}", side, symbol);
        }

        Ok(())
    }
}
