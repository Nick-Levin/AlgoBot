use anyhow::{Context, Result};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotConfig {
    pub name: String,
    pub version: String,
    pub log_level: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiCredentials {
    pub key: String,
    pub secret: String,
    pub base_url: String,
    #[serde(default)]
    pub ws_url: Option<String>,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_requests: u32,
    #[serde(default = "default_rate_window")]
    pub rate_limit_window_ms: u64,
}

fn default_rate_limit() -> u32 {
    50
}

fn default_rate_window() -> u64 {
    1000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    pub production: ApiCredentials,
    pub demo: ApiCredentials,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub path: String,
    #[serde(default = "default_backup_enabled")]
    pub backup_enabled: bool,
    #[serde(default = "default_backup_interval")]
    pub backup_interval_hours: u32,
    #[serde(default = "default_backup_retention")]
    pub backup_retention_days: u32,
}

fn default_backup_enabled() -> bool {
    true
}

fn default_backup_interval() -> u32 {
    24
}

fn default_backup_retention() -> u32 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RiskConfig {
    #[serde(default = "default_max_daily_loss")]
    pub max_daily_loss_pct: f64,
    #[serde(default = "default_max_exposure")]
    pub max_total_exposure_pct: f64,
    #[serde(default = "default_emergency_sl")]
    pub emergency_stop_loss_pct: f64,
}

fn default_max_daily_loss() -> f64 {
    2.0
}

fn default_max_exposure() -> f64 {
    50.0
}

fn default_emergency_sl() -> f64 {
    5.0
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ExitConfig {
    #[serde(default = "default_partial_exit_enabled")]
    pub partial_exit_enabled: bool,
    #[serde(default = "default_partial_exit_levels")]
    pub partial_exit_levels: usize,
    #[serde(default = "default_partial_exit_percentages")]
    pub partial_exit_percentages: Vec<u32>,
    #[serde(default = "default_partial_exit_multipliers")]
    pub partial_exit_multipliers: Vec<f64>,
}

fn default_partial_exit_enabled() -> bool {
    true
}

fn default_partial_exit_levels() -> usize {
    3
}

fn default_partial_exit_percentages() -> Vec<u32> {
    vec![30, 30, 40]
}

fn default_partial_exit_multipliers() -> Vec<f64> {
    vec![1.0, 2.0, 3.5]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntryConfig {
    #[serde(default = "default_entry_mode")]
    pub mode: EntryMode,
    #[serde(default = "default_ema_candles")]
    pub ema_candles: usize,
    #[serde(default = "default_ema_timeframe")]
    pub ema_timeframe: String,
    #[serde(default = "default_ema_fallback")]
    pub ema_fallback: bool,
}

impl Default for EntryConfig {
    fn default() -> Self {
        Self {
            mode: default_entry_mode(),
            ema_candles: default_ema_candles(),
            ema_timeframe: default_ema_timeframe(),
            ema_fallback: default_ema_fallback(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryMode {
    EmaTrend,
    Immediate,
    WaitForZone,
}

impl Default for EntryMode {
    fn default() -> Self {
        EntryMode::Immediate
    }
}

fn default_entry_mode() -> EntryMode {
    EntryMode::Immediate
}

fn default_ema_candles() -> usize {
    20
}

fn default_ema_timeframe() -> String {
    "15".to_string()
}

fn default_ema_fallback() -> bool {
    true
}

impl ExitConfig {
    pub fn validate(&self) -> Result<()> {
        if self.partial_exit_levels < 2 || self.partial_exit_levels > 5 {
            anyhow::bail!(
                "partial_exit_levels must be between 2 and 5, got {}",
                self.partial_exit_levels
            );
        }

        if self.partial_exit_percentages.len() != self.partial_exit_levels {
            anyhow::bail!(
                "partial_exit_percentages must have {} elements, got {}",
                self.partial_exit_levels,
                self.partial_exit_percentages.len()
            );
        }

        if self.partial_exit_multipliers.len() != self.partial_exit_levels {
            anyhow::bail!(
                "partial_exit_multipliers must have {} elements, got {}",
                self.partial_exit_levels,
                self.partial_exit_multipliers.len()
            );
        }

        let sum: u32 = self.partial_exit_percentages.iter().sum();
        if sum != 100 {
            anyhow::bail!(
                "partial_exit_percentages must sum to 100, got {}",
                sum
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DynaGridConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub symbol: Option<String>,
    pub position_risk_percentage: Option<f64>,
    #[serde(default = "default_grid_range")]
    pub grid_range_pct: f64,
    #[serde(default = "default_sizing_factor")]
    pub position_sizing_factor: f64,
    #[serde(default = "default_max_grid_levels")]
    pub max_grid_levels: u8,
    #[serde(default = "default_min_entry_interval")]
    pub min_entry_interval_minutes: u64,
    #[serde(default = "default_max_hold_time")]
    pub max_hold_time_hours: u32,
    #[serde(default = "default_leverage")]
    pub leverage: u8,
    pub exit: ExitConfig,
    #[serde(default)]
    pub entry: EntryConfig,
}

fn default_enabled() -> bool {
    true
}

fn default_grid_range() -> f64 {
    2.0
}

fn default_sizing_factor() -> f64 {
    1.5
}

fn default_max_grid_levels() -> u8 {
    4
}

fn default_min_entry_interval() -> u64 {
    60
}

fn default_max_hold_time() -> u32 {
    168
}

fn default_leverage() -> u8 {
    5
}

impl DynaGridConfig {
    pub fn validate(&self) -> Result<()> {
        // Symbol is required
        let symbol = self
            .symbol
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("'strategy.dynagrid.symbol' is required but not found. \
                Please set the trading symbol in your config file or environment variable: \
                BOT_STRATEGY_DYNAGRID_SYMBOL=ETHUSDT"))?;

        if symbol.is_empty() {
            anyhow::bail!("'strategy.dynagrid.symbol' cannot be empty");
        }

        // Position risk percentage is required
        let risk_pct = self
            .position_risk_percentage
            .ok_or_else(|| anyhow::anyhow!("'strategy.dynagrid.position_risk_percentage' is required but not found. \
                Please set the risk percentage in your config file or environment variable: \
                BOT_STRATEGY_DYNAGRID_POSITION_RISK_PERCENTAGE=0.02"))?;

        if risk_pct <= 0.0 || risk_pct > 0.5 {
            anyhow::bail!(
                "'strategy.dynagrid.position_risk_percentage' must be between 0 and 0.5 (0% to 50%), got {}",
                risk_pct
            );
        }

        if self.grid_range_pct <= 0.0 || self.grid_range_pct > 10.0 {
            anyhow::bail!(
                "'strategy.dynagrid.grid_range_pct' must be between 0 and 10%, got {}",
                self.grid_range_pct
            );
        }

        if self.position_sizing_factor < 1.1 || self.position_sizing_factor > 2.0 {
            anyhow::bail!(
                "'strategy.dynagrid.position_sizing_factor' must be between 1.1 and 2.0, got {}",
                self.position_sizing_factor
            );
        }

        if self.max_grid_levels == 0 || self.max_grid_levels > 10 {
            anyhow::bail!(
                "'strategy.dynagrid.max_grid_levels' must be between 1 and 10, got {}",
                self.max_grid_levels
            );
        }

        if self.leverage == 0 || self.leverage > 100 {
            anyhow::bail!(
                "'strategy.dynagrid.leverage' must be between 1 and 100, got {}",
                self.leverage
            );
        }

        // Validate exit config
        self.exit.validate()?;

        // Validate entry config
        self.validate_entry()?;

        Ok(())
    }

    fn validate_entry(&self) -> Result<()> {
        // Validate EMA candles
        if self.entry.ema_candles < 5 || self.entry.ema_candles > 100 {
            anyhow::bail!(
                "'strategy.dynagrid.entry.ema_candles' must be between 5 and 100, got {}",
                self.entry.ema_candles
            );
        }

        // Validate timeframe
        let valid_timeframes = ["5", "15", "60", "240"];
        if !valid_timeframes.contains(&self.entry.ema_timeframe.as_str()) {
            anyhow::bail!(
                "'strategy.dynagrid.entry.ema_timeframe' must be one of: 5, 15, 60, 240 (minutes), got '{}'",
                self.entry.ema_timeframe
            );
        }

        Ok(())
    }

    pub fn symbol(&self) -> &str {
        self.symbol.as_ref().unwrap()
    }

    pub fn risk_percentage(&self) -> f64 {
        self.position_risk_percentage.unwrap()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StrategyConfig {
    #[serde(default)]
    pub dynagrid: DynaGridConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub bot: BotConfig,
    pub api: ApiConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub risk: RiskConfig,
    #[serde(default)]
    pub strategy: StrategyConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let config = Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(File::with_name("config/production").required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(
                Environment::with_prefix("BOT")
                    .separator("_")
                    .try_parsing(true),
            )
            .build()?;

        let app_config: AppConfig = config.try_deserialize()?;

        // Validate the configuration
        app_config.validate()?;

        Ok(app_config)
    }

    pub fn validate(&self) -> Result<()> {
        // Validate API credentials are present
        if self.api.production.key.is_empty() || self.api.production.secret.is_empty() {
            anyhow::bail!(
                "Production API credentials are required. \
                Set BOT_API_PRODUCTION_KEY and BOT_API_PRODUCTION_SECRET environment variables \
                or add them to your config file."
            );
        }

        if self.api.demo.key.is_empty() || self.api.demo.secret.is_empty() {
            anyhow::bail!(
                "Demo API credentials are required. \
                Set BOT_API_DEMO_KEY and BOT_API_DEMO_SECRET environment variables \
                or add them to your config file."
            );
        }

        // Validate strategy config
        if self.strategy.dynagrid.enabled {
            self.strategy.dynagrid.validate()?;
        }

        // Ensure data directory exists
        let data_dir = Path::new(&self.bot.data_dir);
        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir)
                .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_config_validation() {
        let valid_config = ExitConfig {
            partial_exit_enabled: true,
            partial_exit_levels: 3,
            partial_exit_percentages: vec![30, 30, 40],
            partial_exit_multipliers: vec![1.0, 2.0, 3.5],
        };
        assert!(valid_config.validate().is_ok());

        let invalid_sum = ExitConfig {
            partial_exit_enabled: true,
            partial_exit_levels: 3,
            partial_exit_percentages: vec![30, 30, 30], // Sum = 90
            partial_exit_multipliers: vec![1.0, 2.0, 3.5],
        };
        assert!(invalid_sum.validate().is_err());

        let mismatched_levels = ExitConfig {
            partial_exit_enabled: true,
            partial_exit_levels: 3,
            partial_exit_percentages: vec![30, 30, 40],
            partial_exit_multipliers: vec![1.0, 2.0], // Only 2 multipliers
        };
        assert!(mismatched_levels.validate().is_err());
    }

    #[test]
    fn test_dynagrid_symbol_required() {
        let config = DynaGridConfig {
            enabled: true,
            symbol: None,
            position_risk_percentage: Some(0.02),
            grid_range_pct: 2.0,
            position_sizing_factor: 1.5,
            max_grid_levels: 4,
            min_entry_interval_minutes: 60,
            max_hold_time_hours: 168,
            leverage: 5,
            exit: ExitConfig::default(),
            entry: EntryConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_dynagrid_risk_percentage_required() {
        let config = DynaGridConfig {
            enabled: true,
            symbol: Some("ETHUSDT".to_string()),
            position_risk_percentage: None,
            grid_range_pct: 2.0,
            position_sizing_factor: 1.5,
            max_grid_levels: 4,
            min_entry_interval_minutes: 60,
            max_hold_time_hours: 168,
            leverage: 5,
            exit: ExitConfig::default(),
            entry: EntryConfig::default(),
        };
        assert!(config.validate().is_err());
    }
}
