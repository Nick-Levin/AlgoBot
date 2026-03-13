use crate::api::ApiManager;
use crate::config::{DynaGridConfig, RiskConfig};
use crate::db::{Database, PartialExitRecord, StrategyState, TradeRecord};
use crate::strategy::types::Zone;
use crate::db::OrderRequest;
use crate::error::{BotError, BotResult};
use crate::risk::{RiskManager, RiskStatus};
use crate::strategy::types::*;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct DynaGridEngine {
    api: Arc<ApiManager>,
    db: Arc<Database>,
    risk_manager: RiskManager,
    config: DynaGridConfig,
    state: StrategyState,
    initial_position_size: f64,
    grid_config: Option<GridConfig>,
    partial_exit_config: PartialExitConfig,
}

impl DynaGridEngine {
    pub async fn new(
        api: Arc<ApiManager>,
        db: Arc<Database>,
        config: DynaGridConfig,
        risk_config: RiskConfig,
    ) -> BotResult<Self> {
        // Validate exit config
        let partial_exit_config = Self::build_exit_config(&config.exit);
        partial_exit_config.validate().map_err(|e| {
            BotError::ConfigError(format!("Invalid exit config: {}", e))
        })?;

        // Try to load existing state or create new
        let (state, grid_config, initial_position_size) = match db.load_strategy_state().await? {
            Some(state) if state.is_active => {
                info!("Resuming active strategy for {}", state.symbol);
                // Reconstruct grid config from saved state
                let grid = if let (Some(upper), Some(lower)) = (state.grid_upper_price, state.grid_lower_price) {
                    info!("Restoring grid: upper={:.2}, lower={:.2}", upper, lower);
                    Some(GridConfig {
                        upper_price: upper,
                        lower_price: lower,
                        range_pct: config.grid_range_pct,
                    })
                } else {
                    None
                };
                // Calculate initial position size from saved state
                let position_size = if state.initial_position_value_usdt > 0.0 && state.grid_level > 0 {
                    // We have existing position - calculate size per level
                    state.initial_position_value_usdt / state.grid_level as f64
                } else {
                    0.0 // Will be calculated on first price update
                };
                (state, grid, position_size)
            }
            _ => {
                // New strategy - will be initialized on first run
                (StrategyState::new(config.symbol()), None, 0.0)
            }
        };

        // Initialize risk manager
        let risk_manager = RiskManager::new(risk_config);

        Ok(Self {
            api,
            db,
            risk_manager,
            config,
            state,
            initial_position_size,
            grid_config,
            partial_exit_config,
        })
    }

    fn build_exit_config(exit: &crate::config::ExitConfig) -> PartialExitConfig {
        let levels = exit
            .partial_exit_percentages
            .iter()
            .zip(exit.partial_exit_multipliers.iter())
            .map(|(&pct, &mult)| ExitLevel {
                percentage: pct as f64,
                distance_multiplier: mult,
            })
            .collect();

        PartialExitConfig {
            enabled: exit.partial_exit_enabled,
            levels,
        }
    }

    /// Main strategy loop
    pub async fn run(mut self) -> StrategyResult {
        info!("Starting DynaGrid strategy for {}", self.config.symbol());

        // Initialize risk manager with current balance
        match self.api.get_wallet_balance("USDT").await {
            Ok(balance) => {
                if let Err(e) = self.risk_manager.initialize(balance.wallet_balance).await {
                    error!("Failed to initialize risk manager: {}", e);
                    return StrategyResult::Error { 
                        reason: format!("Risk manager init failed: {}", e) 
                    };
                }
            }
            Err(e) => {
                error!("Failed to fetch initial balance: {}", e);
                return StrategyResult::Error { 
                    reason: format!("Cannot fetch balance: {}", e) 
                };
            }
        }

        loop {
            match self.run_iteration().await {
                Ok(true) => {
                    // Strategy completed
                    info!("Strategy completed successfully");
                    return StrategyResult::Completed {
                        final_pnl: self.state.realized_pnl,
                    };
                }
                Ok(false) => {
                    // Continue running
                }
                Err(e) => {
                    error!("Strategy error: {}", e);
                    
                    if e.is_critical() {
                        // Try to close positions on critical error
                        warn!("Critical error, attempting emergency close");
                        let _ = self.emergency_close().await;
                        return StrategyResult::Error { reason: e.to_string() };
                    }
                    
                    // Log non-critical error and continue
                    let _ = self.db.log_event("ERROR", &format!("Strategy error: {}", e), None).await;
                }
            }

            // Sleep before next iteration
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Single iteration of the strategy loop
    /// Returns Ok(true) if strategy completed, Ok(false) to continue
    async fn run_iteration(&mut self) -> BotResult<bool> {
        // Get current market price
        let ticker = self.api.get_ticker().await?;
        let current_price = ticker.last_price;
        
        debug!(
            "Strategy iteration: price={:.2}, is_active={}, has_positions={}, grid_level={}",
            current_price, self.state.is_active, self.state.has_positions(), self.state.grid_level
        );

        // Check if we need to initialize
        if !self.state.is_active {
            info!("Initializing strategy at price {:.2}", current_price);
            self.initialize_strategy(current_price).await?;
            return Ok(false);
        }

        // Update state
        self.state.last_action_time = Some(Utc::now());
        
        // Get current balance for risk checks
        let current_balance = self.api.get_wallet_balance("USDT").await?.wallet_balance;
        let allocated_capital = self.state.initial_position_value_usdt;
        let current_exposure = self.state.total_exposure() * current_price;

        // Check risk conditions using RiskManager
        match self.risk_manager.check_strategy_risk(
            &self.state,
            current_price,
            allocated_capital,
        ).await? {
            RiskStatus::EmergencyExit { reason } => {
                error!("Emergency exit triggered by risk manager: {}", reason);
                self.emergency_close().await?;
                self.finalize_strategy().await?;
                return Ok(true);
            }
            RiskStatus::ExitRequired { reason } => {
                warn!("Exit required by risk manager: {}", reason);
                self.exit_all_positions(current_price).await?;
                self.finalize_strategy().await?;
                return Ok(true);
            }
            RiskStatus::Warning { message } => {
                warn!("Risk warning: {}", message);
            }
            RiskStatus::Ok => {}
        }

        // Check exit conditions first (highest priority)
        if let Some(exit_action) = self.check_exit_conditions(current_price).await? {
            self.execute_exit(exit_action, current_price).await?;
            
            // Record trade P&L with risk manager
            self.risk_manager.record_trade(self.state.realized_pnl).await;

            // Check if fully closed
            if !self.state.has_positions() {
                self.finalize_strategy().await?;
                return Ok(true);
            }
            
            return Ok(false);
        }

        // Check entry conditions
        if let Some(enter_action) = self.check_entry_conditions(current_price).await? {
            let new_position_value = enter_action.qty * current_price;
            
            // Check risk limits before executing entry
            if let Err(e) = self.risk_manager.check_trade_allowed(
                current_balance,
                new_position_value,
                current_exposure,
            ).await {
                warn!("Trade blocked by risk manager: {}", e);
                return Ok(false);
            }
            
            self.execute_entry(enter_action, current_price).await?;
            
            // Update risk manager with new exposure
            let new_exposure = self.state.total_exposure() * current_price;
            self.risk_manager.update_exposure(new_exposure).await;
        }

        // Update P&L tracking
        self.update_pnl(current_price).await?;

        // Persist state
        self.db.save_strategy_state(&self.state).await?;

        // Log risk metrics periodically
        if self.state.grid_level % 2 == 0 {
            let metrics = self.risk_manager.get_risk_metrics(current_balance).await;
            info!(
                "Risk Metrics - Daily P&L: {:.2}% ({:.2} USDT), Exposure: {:.2}%, Trades: {}, Loss Streak: {}",
                metrics.daily_pnl_pct,
                metrics.daily_pnl,
                (metrics.current_exposure / current_balance) * 100.0,
                metrics.trade_count,
                metrics.loss_streak
            );
        }

        Ok(false)
    }

    /// Initialize a new strategy
    async fn initialize_strategy(&mut self, current_price: f64) -> BotResult<()> {
        info!("Initializing DynaGrid strategy at price {}", current_price);

        // Calculate initial position size based on account balance
        self.initial_position_size = self
            .api
            .calculate_position_size(self.config.risk_percentage())
            .await?;

        // Validate minimum size
        self.api.validate_position_size(self.initial_position_size, 10.0)?;

        // Set up grid
        let grid = GridConfig::new(current_price, self.config.grid_range_pct);
        info!(
            "Grid configured: upper={:.2}, lower={:.2}, range={:.2}%",
            grid.upper_price, grid.lower_price, self.config.grid_range_pct
        );

        // Initialize state
        self.state.is_active = true;
        self.state.grid_upper_price = Some(grid.upper_price);
        self.state.grid_lower_price = Some(grid.lower_price);
        self.state.max_grid_levels = self.config.max_grid_levels;
        self.state.initial_position_value_usdt =
            self.initial_position_size * current_price;
        self.state.initial_risk_percentage = self.config.risk_percentage();
        self.state.entry_time = Some(Utc::now());
        self.state.max_hold_until = Some(
            Utc::now() + chrono::Duration::hours(self.config.max_hold_time_hours as i64),
        );

        self.grid_config = Some(grid);

        // Determine initial entry based on configured mode
        let initial_side = match self.config.entry.mode {
            crate::config::EntryMode::EmaTrend => {
                self.determine_entry_by_ema().await?
            }
            crate::config::EntryMode::Immediate => {
                info!("Immediate entry mode - entering LONG");
                Side::Buy
            }
            crate::config::EntryMode::WaitForZone => {
                // Wait for price to enter a zone
                if current_price >= grid.upper_price {
                    info!("Price in upper zone, entering LONG");
                    Side::Buy
                } else if current_price <= grid.lower_price {
                    info!("Price in lower zone, entering SHORT");
                    Side::Sell
                } else {
                    info!("Price {:.2} in neutral zone ({:.2} - {:.2}), waiting for entry...", 
                          current_price, grid.lower_price, grid.upper_price);
                    // Don't mark as active yet - wait for next iteration
                    self.state.is_active = false;
                    return Ok(());
                }
            }
        };

        // Execute initial entry
        let action = EnterAction {
            side: initial_side,
            qty: self.initial_position_size,
            zone: if initial_side == Side::Buy { Zone::Upper } else { Zone::Lower },
            price: None, // Market order
        };

        self.execute_entry(action, current_price).await?;
        
        // Save initial state
        self.db.save_strategy_state(&self.state).await?;
        
        info!("Strategy initialized successfully");
        Ok(())
    }

    /// Determine entry direction based on EMA trend
    async fn determine_entry_by_ema(&self) -> BotResult<Side> {
        let timeframe = &self.config.entry.ema_timeframe;
        let candles = self.config.entry.ema_candles;
        
        info!(
            "Analyzing EMA trend with {} candles on {}m timeframe",
            candles, timeframe
        );

        // Fetch klines - limit to exact number needed to avoid response truncation
        let klines = self.api.get_klines(timeframe, candles as u32).await?;
        
        if klines.len() < candles {
            return Err(BotError::ApiError {
                message: format!(
                    "Not enough klines data: got {}, need {}",
                    klines.len(),
                    candles
                ),
                retryable: true,
            });
        }

        // Calculate EMA
        let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
        let ema = Self::calculate_ema(&closes, candles);
        
        if ema.is_empty() {
            return Err(BotError::ApiError {
                message: "Failed to calculate EMA".to_string(),
                retryable: false,
            });
        }

        // Get recent EMA values for trend analysis
        let recent_ema = &ema[ema.len().saturating_sub(5)..];
        let _current_price = closes[closes.len() - 1];
        let current_ema = recent_ema[recent_ema.len() - 1];
        let previous_ema = recent_ema[0];

        // Determine trend
        let ema_slope = current_ema - previous_ema;
        let ema_slope_pct = (ema_slope / previous_ema) * 100.0;
        
        info!(
            "EMA Analysis - Current: {:.2}, Previous: {:.2}, Slope: {:.4}%",
            current_ema, previous_ema, ema_slope_pct
        );

        // Decision logic
        let min_slope_pct = 0.05; // Minimum 0.05% slope to consider a trend
        
        if ema_slope_pct > min_slope_pct {
            info!("Uptrend detected ({:.4}% slope) - entering LONG", ema_slope_pct);
            Ok(Side::Buy)
        } else if ema_slope_pct < -min_slope_pct {
            info!("Downtrend detected ({:.4}% slope) - entering SHORT", ema_slope_pct);
            Ok(Side::Sell)
        } else {
            // No clear trend
            if self.config.entry.ema_fallback {
                info!(
                    "No clear trend detected ({:.4}% slope) - falling back to wait-for-zone",
                    ema_slope_pct
                );
                // Return error to trigger fallback behavior
                Err(BotError::ApiError {
                    message: "No clear EMA trend, fallback to zone entry".to_string(),
                    retryable: true,
                })
            } else {
                info!("No clear trend, entering LONG as default");
                Ok(Side::Buy)
            }
        }
    }

    /// Calculate Exponential Moving Average
    fn calculate_ema(prices: &[f64], period: usize) -> Vec<f64> {
        if prices.len() < period {
            return Vec::new();
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = Vec::with_capacity(prices.len() - period + 1);

        // First EMA is SMA
        let sma: f64 = prices[0..period].iter().sum::<f64>() / period as f64;
        ema.push(sma);

        // Calculate subsequent EMAs
        for price in prices.iter().skip(period) {
            let new_ema = (price - ema.last().unwrap()) * multiplier + ema.last().unwrap();
            ema.push(new_ema);
        }

        ema
    }

    /// Check if we should enter a new position
    async fn check_entry_conditions(&self, current_price: f64) -> BotResult<Option<EnterAction>> {
        let grid = self.grid_config.as_ref().ok_or_else(|| {
            BotError::Unknown("Grid not initialized".to_string())
        })?;

        // Check if we've reached max grid levels
        if self.state.grid_level >= self.config.max_grid_levels {
            debug!("Max grid levels reached, skipping entry");
            return Ok(None);
        }

        // Check for timeout (handled by risk manager, but double-check here)
        if self.state.is_timed_out() {
            warn!("Strategy timeout approaching, avoiding new entries");
            return Ok(None);
        }

        // Determine current zone
        let current_zone = match grid.get_zone(current_price) {
            Some(zone) => {
                debug!("Price {:.2} is in {:?} zone", current_price, zone);
                zone
            }
            None => {
                debug!("Price {:.2} is in neutral zone (no entry)", current_price);
                return Ok(None);
            }
        };

        // Check if we already have the correct dominant position
        let (long_value, short_value) = (
            self.state.long_size * current_price,
            self.state.short_size * current_price,
        );

        debug!(
            "Position values - Long: {:.2} USDT, Short: {:.2} USDT, Target factor: {:.2}x",
            long_value, short_value, self.config.position_sizing_factor
        );

        let target_zone = match current_zone {
            Zone::Upper => {
                // Want Long to be 1.5x Short
                if long_value >= short_value * self.config.position_sizing_factor {
                    debug!("Already balanced in upper zone (long: {:.2} >= {:.2}), skipping", 
                           long_value, short_value * self.config.position_sizing_factor);
                    return Ok(None);
                }
                Zone::Upper
            }
            Zone::Lower => {
                // Want Short to be 1.5x Long
                if short_value >= long_value * self.config.position_sizing_factor {
                    debug!("Already balanced in lower zone (short: {:.2} >= {:.2}), skipping",
                           short_value, long_value * self.config.position_sizing_factor);
                    return Ok(None);
                }
                Zone::Lower
            }
        };

        // Calculate required position size
        let sizing = PositionSizing {
            initial_value_usdt: self.state.initial_position_value_usdt,
            risk_percentage: self.config.risk_percentage(),
            sizing_factor: self.config.position_sizing_factor,
        };

        let qty = sizing.calculate_next_size(
            self.state.long_size,
            self.state.short_size,
            target_zone,
            current_price,
        );

        // Validate minimum size
        if qty * current_price < 10.0 {
            // Below minimum order value
            return Ok(None);
        }

        Ok(Some(EnterAction {
            side: target_zone.to_side(),
            qty,
            zone: target_zone,
            price: None, // Market order for entries
        }))
    }

    /// Execute an entry action
    async fn execute_entry(&mut self, action: EnterAction, current_price: f64) -> BotResult<()> {
        info!(
            "Entering {} position: {:.6} {} @ ~{:.2}",
            action.side, action.qty, self.config.symbol(), current_price
        );

        let symbol = self.config.symbol().to_string();
        let order = OrderRequest::market(&symbol, action.side.to_string(), action.qty);
        let response = self.api.place_order(&order).await?;

        // Record trade
        let trade = TradeRecord::new(
            response.order_id,
            symbol.clone(),
            action.side.to_string(),
            "Market",
            action.qty,
        );
        self.db.record_trade(&trade).await?;

        // Update state
        match action.side {
            Side::Buy => {
                // Update long position with weighted average
                let total_value =
                    self.state.long_size * self.state.long_avg_price + action.qty * current_price;
                self.state.long_size += action.qty;
                if self.state.long_size > 0.0 {
                    self.state.long_avg_price = total_value / self.state.long_size;
                }
            }
            Side::Sell => {
                // Update short position with weighted average
                let total_value =
                    self.state.short_size * self.state.short_avg_price + action.qty * current_price;
                self.state.short_size += action.qty;
                if self.state.short_size > 0.0 {
                    self.state.short_avg_price = total_value / self.state.short_size;
                }
            }
        }

        self.state.grid_level += 1;
        self.state.last_action_time = Some(Utc::now());

        info!(
            "Entry executed. Grid level: {}, Long: {:.6} @ {:.2}, Short: {:.6} @ {:.2}",
            self.state.grid_level,
            self.state.long_size,
            self.state.long_avg_price,
            self.state.short_size,
            self.state.short_avg_price
        );

        Ok(())
    }

    /// Check if we should exit positions
    async fn check_exit_conditions(&self, current_price: f64) -> BotResult<Option<ExitAction>> {
        let grid = self.grid_config.as_ref().ok_or_else(|| {
            BotError::Unknown("Grid not initialized".to_string())
        })?;

        // Note: Timeout and max grid levels are handled by RiskManager
        // We only check for profitable breakout here

        if !self.partial_exit_config.enabled || self.partial_exit_config.levels.is_empty() {
            return Ok(None);
        }

        // Determine which side is winning
        let (winning_side, entry_price, winning_size, losing_size) =
            if self.state.long_size > self.state.short_size {
                (
                    Zone::Upper,
                    self.state.long_avg_price,
                    self.state.long_size,
                    self.state.short_size,
                )
            } else if self.state.short_size > self.state.long_size {
                (
                    Zone::Lower,
                    self.state.short_avg_price,
                    self.state.short_size,
                    self.state.long_size,
                )
            } else {
                return Ok(None); // Equal sizes, no clear direction
            };

        // Calculate grid range
        let grid_range = grid.upper_price - grid.lower_price;

        // Check each exit level
        for (idx, level) in self.partial_exit_config.levels.iter().enumerate() {
            let exit_price = self.partial_exit_config.get_exit_price(
                idx,
                winning_side,
                grid_range,
                entry_price,
            );

            let should_trigger = match winning_side {
                Zone::Upper => current_price >= exit_price,
                Zone::Lower => current_price <= exit_price,
            };

            if should_trigger {
                let close_pct = level.percentage / 100.0;
                let close_winning = winning_size * close_pct;
                let close_losing = losing_size * close_pct;

                let reason = if idx == self.partial_exit_config.levels.len() - 1 {
                    ExitReason::FinalTakeProfit
                } else {
                    ExitReason::PartialTakeProfit { level: idx + 1 }
                };

                return Ok(Some(ExitAction {
                    level: idx + 1,
                    close_long_qty: if winning_side == Zone::Upper {
                        close_winning
                    } else {
                        close_losing
                    },
                    close_short_qty: if winning_side == Zone::Upper {
                        close_losing
                    } else {
                        close_winning
                    },
                    reason,
                }));
            }
        }

        Ok(None)
    }

    /// Execute an exit action
    async fn execute_exit(&mut self, action: ExitAction, current_price: f64) -> BotResult<()> {
        let symbol = self.config.symbol().to_string();
        
        info!(
            "Executing exit (level {}): Close Long {:.6}, Close Short {:.6} - {:?}",
            action.level, action.close_long_qty, action.close_short_qty, action.reason
        );

        let mut realized_pnl = 0.0;

        // Close long position
        if action.close_long_qty > 0.0 {
            let order = OrderRequest::market(
                &symbol,
                "Sell",
                action.close_long_qty,
            )
            .reduce_only();
            
            let response = self.api.place_order(&order).await?;
            
            // Calculate P&L
            let long_pnl = action.close_long_qty * (current_price - self.state.long_avg_price);
            realized_pnl += long_pnl;
            
            // Record
            let trade = TradeRecord::new(
                response.order_id,
                symbol.clone(),
                "Sell",
                "Market",
                action.close_long_qty,
            );
            self.db.record_trade(&trade).await?;

            // Update state
            self.state.long_size -= action.close_long_qty;
        }

        // Close short position
        if action.close_short_qty > 0.0 {
            let order = OrderRequest::market(
                &symbol,
                "Buy",
                action.close_short_qty,
            )
            .reduce_only();
            
            let response = self.api.place_order(&order).await?;
            
            // Calculate P&L
            let short_pnl = action.close_short_qty * (self.state.short_avg_price - current_price);
            realized_pnl += short_pnl;
            
            // Record
            let trade = TradeRecord::new(
                response.order_id,
                symbol.clone(),
                "Buy",
                "Market",
                action.close_short_qty,
            );
            self.db.record_trade(&trade).await?;

            // Update state
            self.state.short_size -= action.close_short_qty;
        }

        // Update realized P&L
        self.state.realized_pnl += realized_pnl;

        // Record partial exit
        let exit_record = PartialExitRecord {
            level: action.level as i32,
            symbol: symbol.clone(),
            long_closed_qty: if action.close_long_qty > 0.0 {
                Some(action.close_long_qty)
            } else {
                None
            },
            long_avg_close_price: if action.close_long_qty > 0.0 {
                Some(current_price)
            } else {
                None
            },
            short_closed_qty: if action.close_short_qty > 0.0 {
                Some(action.close_short_qty)
            } else {
                None
            },
            short_avg_close_price: if action.close_short_qty > 0.0 {
                Some(current_price)
            } else {
                None
            },
            realized_pnl,
        };
        self.db.record_partial_exit(&exit_record).await?;

        info!(
            "Exit executed. Realized P&L: {:.2}, Total Realized: {:.2}",
            realized_pnl, self.state.realized_pnl
        );

        Ok(())
    }

    /// Update P&L tracking
    async fn update_pnl(&mut self, current_price: f64) -> BotResult<()> {
        let unrealized = self.state.unrealized_pnl(current_price);
        let total = self.state.total_pnl(current_price);

        debug!(
            "P&L Update: Unrealized={:.2}, Realized={:.2}, Total={:.2}",
            unrealized, self.state.realized_pnl, total
        );

        // Note: Emergency stop loss is now handled by RiskManager
        // This is kept for logging/debugging purposes

        Ok(())
    }

    /// Exit all positions at market
    async fn exit_all_positions(&mut self, current_price: f64) -> BotResult<()> {
        info!("Exiting all positions at market");

        let action = ExitAction {
            level: 0,
            close_long_qty: self.state.long_size,
            close_short_qty: self.state.short_size,
            reason: ExitReason::Manual,
        };

        self.execute_exit(action, current_price).await
    }

    /// Emergency close all positions
    async fn emergency_close(&self) -> BotResult<()> {
        warn!("Executing emergency close of all positions");
        
        if self.state.long_size > 0.0 {
            let _ = self.api.close_position("Buy").await;
        }
        if self.state.short_size > 0.0 {
            let _ = self.api.close_position("Sell").await;
        }
        
        Ok(())
    }

    /// Finalize strategy and clean up
    async fn finalize_strategy(&mut self) -> BotResult<()> {
        info!("Finalizing strategy. Total Realized P&L: {:.2}", self.state.realized_pnl);
        
        self.state.is_active = false;
        self.db.save_strategy_state(&self.state).await?;
        
        // Log final summary
        let msg = format!(
            "Strategy completed. Total P&L: {:.2} USDT",
            self.state.realized_pnl
        );
        self.db.log_event("INFO", &msg, None).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require mocking the API and database
    // For now, we just verify the logic compiles
    
    #[test]
    fn test_exit_config_building() {
        let config = crate::config::ExitConfig {
            partial_exit_enabled: true,
            partial_exit_levels: 3,
            partial_exit_percentages: vec![30, 30, 40],
            partial_exit_multipliers: vec![1.0, 2.0, 3.5],
        };

        let exit_config = DynaGridEngine::build_exit_config(&config);
        
        assert_eq!(exit_config.levels.len(), 3);
        assert_eq!(exit_config.levels[0].percentage, 30.0);
        assert_eq!(exit_config.levels[0].distance_multiplier, 1.0);
    }
}
