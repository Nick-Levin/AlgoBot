

/// Strategy execution result
#[derive(Debug, Clone)]
pub enum StrategyResult {
    /// Strategy completed successfully (all positions closed)
    Completed { final_pnl: f64 },
    /// Strategy was stopped manually or by system
    Stopped { reason: String },
    /// Strategy hit an error
    Error { reason: String },
}

/// Action to take based on strategy logic
#[derive(Debug, Clone)]
pub enum StrategyAction {
    /// Enter a new position
    Enter(EnterAction),
    /// Exit positions (partial or full)
    Exit(ExitAction),
    /// Do nothing
    Hold,
}

#[derive(Debug, Clone)]
pub struct EnterAction {
    pub side: Side,
    pub qty: f64,
    pub zone: Zone,
    pub price: Option<f64>, // None for market order
}

#[derive(Debug, Clone)]
pub struct ExitAction {
    pub level: usize, // Which partial exit level (0 = all)
    pub close_long_qty: f64,
    pub close_short_qty: f64,
    pub reason: ExitReason,
}

#[derive(Debug, Clone)]
pub enum ExitReason {
    PartialTakeProfit { level: usize },
    FinalTakeProfit,
    StopLoss,
    Timeout,
    MaxGridLevels,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,  // Long
    Sell, // Short
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "Buy"),
            Side::Sell => write!(f, "Sell"),
        }
    }
}

impl From<&str> for Side {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            _ => panic!("Invalid side: {}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Upper, // Buy zone
    Lower, // Sell zone
}

impl Zone {
    pub fn opposite(&self) -> Self {
        match self {
            Zone::Upper => Zone::Lower,
            Zone::Lower => Zone::Upper,
        }
    }

    pub fn to_side(&self) -> Side {
        match self {
            Zone::Upper => Side::Buy,
            Zone::Lower => Side::Sell,
        }
    }
}

/// Grid configuration
#[derive(Debug, Clone, Copy)]
pub struct GridConfig {
    pub upper_price: f64,
    pub lower_price: f64,
    pub range_pct: f64,
}

impl GridConfig {
    pub fn new(center_price: f64, range_pct: f64) -> Self {
        let half_range = center_price * (range_pct / 100.0) / 2.0;
        Self {
            upper_price: center_price + half_range,
            lower_price: center_price - half_range,
            range_pct,
        }
    }

    /// Check which zone the price is in
    pub fn get_zone(&self, price: f64) -> Option<Zone> {
        if price >= self.upper_price {
            Some(Zone::Upper)
        } else if price <= self.lower_price {
            Some(Zone::Lower)
        } else {
            None
        }
    }

    /// Check if price is in the range (not in any zone)
    pub fn is_in_range(&self, price: f64) -> bool {
        self.get_zone(price).is_none()
    }

    /// Calculate distance from a zone
    pub fn distance_from_zone(&self, price: f64, zone: Zone) -> f64 {
        match zone {
            Zone::Upper => price - self.upper_price,
            Zone::Lower => self.lower_price - price,
        }
    }
}

/// Exit configuration for partial closes
#[derive(Debug, Clone)]
pub struct PartialExitConfig {
    pub enabled: bool,
    pub levels: Vec<ExitLevel>,
}

#[derive(Debug, Clone)]
pub struct ExitLevel {
    pub percentage: f64,      // % of position to close
    pub distance_multiplier: f64, // multiplier of grid_range for exit price
}

impl PartialExitConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.levels.is_empty() {
            return Err("Partial exit enabled but no levels configured".to_string());
        }

        let total_pct: f64 = self.levels.iter().map(|l| l.percentage).sum();
        if (total_pct - 100.0).abs() > 0.01 {
            return Err(format!(
                "Exit percentages must sum to 100, got {:.2}",
                total_pct
            ));
        }

        Ok(())
    }

    /// Get exit price for a level
    pub fn get_exit_price(&self, level_idx: usize, zone: Zone, grid_range: f64, entry_price: f64) -> f64 {
        if level_idx >= self.levels.len() {
            panic!("Invalid exit level index: {}", level_idx);
        }

        let distance = grid_range * self.levels[level_idx].distance_multiplier;
        
        match zone {
            Zone::Upper => entry_price + distance,
            Zone::Lower => entry_price - distance,
        }
    }
}

/// Position sizing parameters
#[derive(Debug, Clone)]
pub struct PositionSizing {
    pub initial_value_usdt: f64,
    pub risk_percentage: f64,
    pub sizing_factor: f64,
}

impl PositionSizing {
    /// Calculate the next position size to achieve target ratio
    pub fn calculate_next_size(
        &self,
        current_long: f64,
        current_short: f64,
        target_zone: Zone,
        current_price: f64,
    ) -> f64 {
        let current_long_value = current_long * current_price;
        let current_short_value = current_short * current_price;

        let target_value = match target_zone {
            Zone::Upper => {
                // Want Long = sizing_factor × Short
                let target_long_value = current_short_value * self.sizing_factor;
                target_long_value - current_long_value
            }
            Zone::Lower => {
                // Want Short = sizing_factor × Long
                let target_short_value = current_long_value * self.sizing_factor;
                target_short_value - current_short_value
            }
        };

        // Convert value to qty
        let qty = target_value / current_price;

        // Ensure minimum size
        qty.max(self.initial_value_usdt / current_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_config() {
        let grid = GridConfig::new(3000.0, 2.0);
        
        // With 2% range, half is 1%
        assert_eq!(grid.upper_price, 3030.0); // 3000 + 1%
        assert_eq!(grid.lower_price, 2970.0); // 3000 - 1%
        
        assert!(grid.is_in_range(3000.0));
        assert!(grid.get_zone(3100.0) == Some(Zone::Upper));
        assert!(grid.get_zone(2900.0) == Some(Zone::Lower));
    }

    #[test]
    fn test_partial_exit_config() {
        let config = PartialExitConfig {
            enabled: true,
            levels: vec![
                ExitLevel { percentage: 30.0, distance_multiplier: 1.0 },
                ExitLevel { percentage: 30.0, distance_multiplier: 2.0 },
                ExitLevel { percentage: 40.0, distance_multiplier: 3.5 },
            ],
        };

        assert!(config.validate().is_ok());

        let price = config.get_exit_price(0, Zone::Upper, 60.0, 3000.0);
        assert_eq!(price, 3060.0); // 3000 + (60 * 1.0)

        let price = config.get_exit_price(2, Zone::Upper, 60.0, 3000.0);
        assert_eq!(price, 3210.0); // 3000 + (60 * 3.5)
    }

    #[test]
    fn test_position_sizing() {
        let sizing = PositionSizing {
            initial_value_usdt: 50.0,
            risk_percentage: 0.02,
            sizing_factor: 1.5,
        };

        // Start with no positions
        let size = sizing.calculate_next_size(0.0, 0.0, Zone::Upper, 3000.0);
        assert!((size - 0.01667).abs() < 0.0001); // $50 / $3000

        // Add long to match short
        // Current: Long 0.0, Short 0.025 (worth $75)
        // Want: Long = 1.5 × $75 = $112.5
        // Need: $112.5 more
        let size = sizing.calculate_next_size(0.0, 0.025, Zone::Upper, 3000.0);
        assert!((size - 0.0375).abs() < 0.0001); // $112.5 / $3000
    }
}
