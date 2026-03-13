use crate::config::RiskConfig;
use crate::db::StrategyState;
use crate::error::{BotError, BotResult};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Global risk manager that monitors and enforces risk limits
pub struct RiskManager {
    config: RiskConfig,
    daily_stats: Arc<RwLock<DailyStats>>,
}

#[derive(Debug, Clone)]
struct DailyStats {
    date: chrono::NaiveDate,
    starting_balance: f64,
    current_balance: f64,
    total_pnl: f64,
    total_exposure: f64,
    trade_count: u32,
    loss_streak: u32,
}

impl DailyStats {
    fn new(starting_balance: f64) -> Self {
        Self {
            date: Utc::now().date_naive(),
            starting_balance,
            current_balance: starting_balance,
            total_pnl: 0.0,
            total_exposure: 0.0,
            trade_count: 0,
            loss_streak: 0,
        }
    }

    fn reset_if_new_day(&mut self, current_balance: f64) {
        let today = Utc::now().date_naive();
        if self.date != today {
            info!("New trading day detected, resetting daily stats");
            *self = Self::new(current_balance);
        }
    }

    fn update_balance(&mut self, new_balance: f64) {
        self.total_pnl = new_balance - self.starting_balance;
        self.current_balance = new_balance;
        
        if self.total_pnl < 0.0 {
            self.loss_streak += 1;
        } else {
            self.loss_streak = 0;
        }
    }
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            daily_stats: Arc::new(RwLock::new(DailyStats::new(0.0))),
        }
    }

    /// Initialize daily stats with current balance
    pub async fn initialize(&self, current_balance: f64) -> BotResult<()> {
        let mut stats = self.daily_stats.write().await;
        *stats = DailyStats::new(current_balance);
        
        info!(
            "RiskManager initialized. Daily loss limit: {:.2}%, Max exposure: {:.2}%",
            self.config.max_daily_loss_pct,
            self.config.max_total_exposure_pct
        );
        
        Ok(())
    }

    /// Check if a new trade should be allowed
    pub async fn check_trade_allowed(
        &self,
        current_balance: f64,
        new_position_value: f64,
        current_exposure: f64,
    ) -> BotResult<()> {
        let mut stats = self.daily_stats.write().await;
        
        // Reset stats if it's a new day
        stats.reset_if_new_day(current_balance);
        stats.update_balance(current_balance);

        // Check 1: Daily loss limit
        let daily_loss_pct = (-stats.total_pnl / stats.starting_balance) * 100.0;
        if daily_loss_pct > self.config.max_daily_loss_pct {
            error!(
                "DAILY LOSS LIMIT BREACHED: {:.2}% / {:.2}%",
                daily_loss_pct, self.config.max_daily_loss_pct
            );
            return Err(BotError::ApiError {
                message: format!(
                    "Daily loss limit reached: {:.2}%",
                    daily_loss_pct
                ),
                retryable: false,
            });
        }

        // Check 2: Max total exposure
        let new_total_exposure = current_exposure + new_position_value;
        let exposure_pct = (new_total_exposure / current_balance) * 100.0;
        
        if exposure_pct > self.config.max_total_exposure_pct {
            warn!(
                "Max exposure would be exceeded: {:.2}% > {:.2}%",
                exposure_pct, self.config.max_total_exposure_pct
            );
            return Err(BotError::ApiError {
                message: format!(
                    "Max exposure limit would be exceeded: {:.2}%",
                    exposure_pct
                ),
                retryable: false,
            });
        }

        // Check 3: Loss streak (consecutive losses)
        if stats.loss_streak >= 5 {
            warn!(
                "High loss streak detected: {} consecutive losses",
                stats.loss_streak
            );
            // Don't block, but warn
        }

        // Update stats
        stats.total_exposure = current_exposure;
        stats.trade_count += 1;

        Ok(())
    }

    /// Check strategy-level risk conditions
    pub async fn check_strategy_risk(
        &self,
        state: &StrategyState,
        current_price: f64,
        allocated_capital: f64,
    ) -> BotResult<RiskStatus> {
        // Check 1: Strategy timeout
        if state.is_timed_out() {
            warn!("Strategy timeout reached");
            return Ok(RiskStatus::ExitRequired {
                reason: "Strategy timeout".to_string(),
            });
        }

        // Check 2: Emergency stop loss
        if allocated_capital > 0.0 {
            let total_pnl = state.total_pnl(current_price);
            let loss_pct = (-total_pnl / allocated_capital) * 100.0;
            
            if loss_pct >= self.config.emergency_stop_loss_pct {
                error!(
                    "EMERGENCY STOP LOSS TRIGGERED: {:.2}% loss",
                    loss_pct
                );
                return Ok(RiskStatus::EmergencyExit {
                    reason: format!("Stop loss: {:.2}%", loss_pct),
                });
            }

            // Warning at 50% of stop loss
            if loss_pct >= self.config.emergency_stop_loss_pct * 0.5 {
                warn!(
                    "Approaching stop loss: {:.2}% / {:.2}%",
                    loss_pct, self.config.emergency_stop_loss_pct
                );
            }
        }

        // Check 3: Max grid levels reached
        if state.grid_level >= state.max_grid_levels {
            warn!("Max grid levels reached: {}", state.grid_level);
            // This is handled in the strategy, but we flag it here too
        }

        // Check 4: High funding fee accumulation
        let funding_pct = if allocated_capital > 0.0 {
            (state.funding_fees_paid / allocated_capital) * 100.0
        } else {
            0.0
        };
        
        if funding_pct > 1.0 {
            warn!(
                "High funding fee accumulation: {:.2}% of allocated capital",
                funding_pct
            );
        }

        Ok(RiskStatus::Ok)
    }

    /// Get current risk metrics
    pub async fn get_risk_metrics(&self, current_balance: f64) -> RiskMetrics {
        let mut stats = self.daily_stats.write().await;
        stats.reset_if_new_day(current_balance);
        stats.update_balance(current_balance);

        RiskMetrics {
            daily_pnl: stats.total_pnl,
            daily_pnl_pct: (stats.total_pnl / stats.starting_balance) * 100.0,
            daily_loss_limit_pct: self.config.max_daily_loss_pct,
            current_exposure: stats.total_exposure,
            max_exposure_pct: self.config.max_total_exposure_pct,
            trade_count: stats.trade_count,
            loss_streak: stats.loss_streak,
        }
    }

    /// Update exposure tracking
    pub async fn update_exposure(&self, exposure: f64) {
        let mut stats = self.daily_stats.write().await;
        stats.total_exposure = exposure;
    }

    /// Record a completed trade for stats
    pub async fn record_trade(&self, pnl: f64) {
        let mut stats = self.daily_stats.write().await;
        stats.total_pnl += pnl;
        stats.trade_count += 1;
        
        if pnl < 0.0 {
            stats.loss_streak += 1;
        } else {
            stats.loss_streak = 0;
        }
    }
}

#[derive(Debug, Clone)]
pub enum RiskStatus {
    Ok,
    Warning { message: String },
    ExitRequired { reason: String },
    EmergencyExit { reason: String },
}

impl RiskStatus {
    pub fn is_critical(&self) -> bool {
        matches!(self, RiskStatus::EmergencyExit { .. })
    }

    pub fn requires_exit(&self) -> bool {
        matches!(self, RiskStatus::ExitRequired { .. } | RiskStatus::EmergencyExit { .. })
    }
}

#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub daily_pnl: f64,
    pub daily_pnl_pct: f64,
    pub daily_loss_limit_pct: f64,
    pub current_exposure: f64,
    pub max_exposure_pct: f64,
    pub trade_count: u32,
    pub loss_streak: u32,
}

impl RiskMetrics {
    pub fn is_within_limits(&self) -> bool {
        self.daily_pnl_pct > -self.daily_loss_limit_pct
            && (self.current_exposure / self.max_exposure_pct) < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> RiskConfig {
        RiskConfig {
            max_daily_loss_pct: 2.0,
            max_total_exposure_pct: 50.0,
            emergency_stop_loss_pct: 5.0,
        }
    }

    #[tokio::test]
    async fn test_daily_loss_limit() {
        let manager = RiskManager::new(create_test_config());
        manager.initialize(10000.0).await.unwrap();

        // Simulate losses
        let result = manager
            .check_trade_allowed(9800.0, 1000.0, 0.0)
            .await;
        
        // Should still be allowed (2% loss == limit, not exceeding)
        assert!(result.is_ok());

        // Exceed the limit
        let result = manager
            .check_trade_allowed(9700.0, 1000.0, 0.0)
            .await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_max_exposure() {
        let manager = RiskManager::new(create_test_config());
        manager.initialize(10000.0).await.unwrap();

        // 60% exposure should fail (limit is 50%)
        let result = manager
            .check_trade_allowed(10000.0, 6000.0, 0.0)
            .await;
        
        assert!(result.is_err());

        // 40% exposure should pass
        let result = manager
            .check_trade_allowed(10000.0, 4000.0, 0.0)
            .await;
        
        assert!(result.is_ok());
    }
}
