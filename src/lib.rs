pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod risk;
pub mod strategy;

use std::sync::Arc;
use tracing::{error, info};

pub use config::AppConfig;
pub use error::{BotError, BotResult};

/// Main bot application
pub struct AlgoTrader {
    config: AppConfig,
    db: Arc<db::Database>,
}

impl AlgoTrader {
    pub async fn new(config: AppConfig) -> BotResult<Self> {
        // Initialize database
        let db = Arc::new(db::Database::new(&config.database.path).await?);
        
        info!("AlgoTrader initialized with database at {}", config.database.path);
        
        Ok(Self {
            config,
            db,
        })
    }

    /// Run the trading bot
    pub async fn run(self) -> BotResult<()> {
        let symbol = self.config.strategy.dynagrid.symbol().to_string();
        
        info!("Starting AlgoTrader for symbol: {}", symbol);
        
        // Create API manager
        let api = Arc::new(api::ApiManager::new(
            &self.config.api,
            &symbol,
        )?);
        
        // Start market data streaming
        let ws_url = self.config.api.production.ws_url.clone()
            .unwrap_or_else(|| "wss://stream.bybit.com/v5/public/linear".to_string());
        
        let mut api_clone = Arc::clone(&api);
        Arc::make_mut(&mut api_clone).start_market_data(&ws_url).await?;
        
        // Create and run strategy with risk manager
        let engine = strategy::DynaGridEngine::new(
            api,
            self.db.clone(),
            self.config.strategy.dynagrid.clone(),
            self.config.risk.clone(),
        ).await?;
        
        let result = engine.run().await;
        
        match result {
            strategy::StrategyResult::Completed { final_pnl } => {
                info!("Strategy completed successfully with P&L: {:.2} USDT", final_pnl);
                Ok(())
            }
            strategy::StrategyResult::Stopped { reason } => {
                info!("Strategy stopped: {}", reason);
                Ok(())
            }
            strategy::StrategyResult::Error { reason } => {
                error!("Strategy failed: {}", reason);
                Err(BotError::Unknown(reason))
            }
        }
    }
}
