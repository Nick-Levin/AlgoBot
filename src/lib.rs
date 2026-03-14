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
        let grid_config = &self.config.strategy.dynagrid;
        
        info!("");
        info!("╔══════════════════════════════════════════════════════════════════╗");
        info!("║                    AlgoTrader Starting                           ║");
        info!("╠══════════════════════════════════════════════════════════════════╣");
        info!("║  Symbol:          {:<45} ║", symbol);
        info!("║  Grid Range:       {:.2}%{:>42}", grid_config.grid_range_pct, "║");
        info!("║  Max Levels:       {:<45} ║", grid_config.max_grid_levels);
        info!("║  Position Factor:   {:.2}x{:>41}", grid_config.position_sizing_factor, "║");
        info!("║  Risk Per Trade:    {:.2}%{:>42}", grid_config.risk_percentage() * 100.0, "║");
        info!("║  Entry Mode:        {:?}{:>36}", grid_config.entry.mode, "║");
        info!("╚══════════════════════════════════════════════════════════════════╝");
        info!("");
        
        // Create API manager
        let mut api = api::ApiManager::new(
            &self.config.api,
            &symbol,
        )?;
        
        // Start market data streaming
        let ws_url = self.config.api.production.ws_url.clone()
            .unwrap_or_else(|| "wss://stream.bybit.com/v5/public/linear".to_string());
        
        api.start_market_data(&ws_url).await?;
        
        // Wrap in Arc after market data is started
        let api = Arc::new(api);
        
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
