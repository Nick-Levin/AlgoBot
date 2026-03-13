use crate::error::{BotError, BotResult};
use chrono::{DateTime, Utc};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;

pub mod models;

pub use models::*;

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(database_url: &str) -> BotResult<Self> {
        // Create database if it doesn't exist
        if !Path::new(database_url).exists() {
            tracing::info!("Creating new database at {}", database_url);
            Sqlite::create_database(database_url).await.map_err(|e| {
                BotError::Unknown(format!("Failed to create database: {}", e))
            })?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;

        Ok(db)
    }

    async fn run_migrations(&self) -> BotResult<()> {
        // Create tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS strategy_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                symbol TEXT NOT NULL,
                is_active BOOLEAN DEFAULT FALSE,
                
                grid_upper_price REAL,
                grid_lower_price REAL,
                grid_level INTEGER DEFAULT 0,
                max_grid_levels INTEGER DEFAULT 4,
                
                long_size REAL DEFAULT 0,
                short_size REAL DEFAULT 0,
                long_avg_price REAL DEFAULT 0,
                short_avg_price REAL DEFAULT 0,
                
                initial_position_value_usdt REAL,
                initial_risk_percentage REAL,
                entry_time TIMESTAMP,
                last_action_time TIMESTAMP,
                max_hold_until TIMESTAMP,
                
                realized_pnl REAL DEFAULT 0,
                funding_fees_paid REAL DEFAULT 0,
                
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trade_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                order_id TEXT UNIQUE NOT NULL,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                qty REAL NOT NULL,
                price REAL,
                avg_filled_price REAL,
                filled_qty REAL,
                order_status TEXT NOT NULL,
                closed_pnl REAL,
                exec_fee REAL DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS funding_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                funding_rate REAL NOT NULL,
                fee_paid REAL NOT NULL,
                position_size REAL NOT NULL,
                side TEXT NOT NULL,
                exec_time TIMESTAMP NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS event_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                data TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS partial_exits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                level INTEGER NOT NULL,
                symbol TEXT NOT NULL,
                long_closed_qty REAL,
                long_avg_close_price REAL,
                short_closed_qty REAL,
                short_avg_close_price REAL,
                realized_pnl REAL,
                closed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trade_history(symbol)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_time ON trade_history(created_at)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_time ON event_log(timestamp)")
            .execute(&self.pool)
            .await?;

        tracing::info!("Database migrations completed");
        Ok(())
    }

    /// Initialize or update strategy state
    pub async fn save_strategy_state(&self, state: &StrategyState) -> BotResult<()> {
        sqlx::query(
            r#"
            INSERT INTO strategy_state (
                id, symbol, is_active, grid_upper_price, grid_lower_price,
                grid_level, max_grid_levels, long_size, short_size,
                long_avg_price, short_avg_price, initial_position_value_usdt,
                initial_risk_percentage, entry_time, last_action_time,
                max_hold_until, realized_pnl, funding_fees_paid
            ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(id) DO UPDATE SET
                symbol = excluded.symbol,
                is_active = excluded.is_active,
                grid_upper_price = excluded.grid_upper_price,
                grid_lower_price = excluded.grid_lower_price,
                grid_level = excluded.grid_level,
                max_grid_levels = excluded.max_grid_levels,
                long_size = excluded.long_size,
                short_size = excluded.short_size,
                long_avg_price = excluded.long_avg_price,
                short_avg_price = excluded.short_avg_price,
                initial_position_value_usdt = excluded.initial_position_value_usdt,
                initial_risk_percentage = excluded.initial_risk_percentage,
                entry_time = excluded.entry_time,
                last_action_time = excluded.last_action_time,
                max_hold_until = excluded.max_hold_until,
                realized_pnl = excluded.realized_pnl,
                funding_fees_paid = excluded.funding_fees_paid,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&state.symbol)
        .bind(state.is_active)
        .bind(state.grid_upper_price)
        .bind(state.grid_lower_price)
        .bind(state.grid_level)
        .bind(state.max_grid_levels)
        .bind(state.long_size)
        .bind(state.short_size)
        .bind(state.long_avg_price)
        .bind(state.short_avg_price)
        .bind(state.initial_position_value_usdt)
        .bind(state.initial_risk_percentage)
        .bind(state.entry_time)
        .bind(state.last_action_time)
        .bind(state.max_hold_until)
        .bind(state.realized_pnl)
        .bind(state.funding_fees_paid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load strategy state
    pub async fn load_strategy_state(&self) -> BotResult<Option<StrategyState>> {
        let row = sqlx::query_as::<_, StrategyStateRow>(
            r#"
            SELECT 
                symbol, is_active, grid_upper_price, grid_lower_price,
                grid_level, max_grid_levels, long_size, short_size,
                long_avg_price, short_avg_price, initial_position_value_usdt,
                initial_risk_percentage, entry_time, last_action_time,
                max_hold_until, realized_pnl, funding_fees_paid
            FROM strategy_state
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row.into())),
            None => Ok(None),
        }
    }

    /// Record a trade execution
    pub async fn record_trade(&self, trade: &TradeRecord) -> BotResult<()> {
        sqlx::query(
            r#"
            INSERT INTO trade_history (
                order_id, symbol, side, order_type, qty, price,
                avg_filled_price, filled_qty, order_status, closed_pnl, exec_fee
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(&trade.order_id)
        .bind(&trade.symbol)
        .bind(&trade.side)
        .bind(&trade.order_type)
        .bind(trade.qty)
        .bind(trade.price)
        .bind(trade.avg_filled_price)
        .bind(trade.filled_qty)
        .bind(&trade.order_status)
        .bind(trade.closed_pnl)
        .bind(trade.exec_fee)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a funding fee payment
    pub async fn record_funding(&self, funding: &FundingRecord) -> BotResult<()> {
        sqlx::query(
            r#"
            INSERT INTO funding_records (
                symbol, funding_rate, fee_paid, position_size, side, exec_time
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&funding.symbol)
        .bind(funding.funding_rate)
        .bind(funding.fee_paid)
        .bind(funding.position_size)
        .bind(&funding.side)
        .bind(funding.exec_time)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a partial exit
    pub async fn record_partial_exit(&self, exit: &PartialExitRecord) -> BotResult<()> {
        sqlx::query(
            r#"
            INSERT INTO partial_exits (
                level, symbol, long_closed_qty, long_avg_close_price,
                short_closed_qty, short_avg_close_price, realized_pnl
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(exit.level)
        .bind(&exit.symbol)
        .bind(exit.long_closed_qty)
        .bind(exit.long_avg_close_price)
        .bind(exit.short_closed_qty)
        .bind(exit.short_avg_close_price)
        .bind(exit.realized_pnl)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Log an event
    pub async fn log_event(&self, level: &str, message: &str, data: Option<&str>) -> BotResult<()> {
        sqlx::query(
            r#"
            INSERT INTO event_log (level, message, data)
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(level)
        .bind(message)
        .bind(data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get total funding fees paid for a symbol
    pub async fn get_total_funding_fees(&self, symbol: &str) -> BotResult<f64> {
        let result: Option<(f64,)> = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(fee_paid), 0) 
            FROM funding_records 
            WHERE symbol = ?1
            "#,
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0).unwrap_or(0.0))
    }

    /// Clear strategy state (when closing a strategy)
    pub async fn clear_strategy_state(&self) -> BotResult<()> {
        sqlx::query("DELETE FROM strategy_state WHERE id = 1")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get trade history
    pub async fn get_trade_history(&self, symbol: &str, limit: i64) -> BotResult<Vec<TradeRecord>> {
        let rows = sqlx::query_as::<_, TradeRecordRow>(
            r#"
            SELECT order_id, symbol, side, order_type, qty, price,
                   avg_filled_price, filled_qty, order_status, closed_pnl, exec_fee
            FROM trade_history
            WHERE symbol = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

#[derive(sqlx::FromRow)]
struct StrategyStateRow {
    symbol: String,
    is_active: bool,
    grid_upper_price: Option<f64>,
    grid_lower_price: Option<f64>,
    grid_level: i64,
    max_grid_levels: i64,
    long_size: f64,
    short_size: f64,
    long_avg_price: f64,
    short_avg_price: f64,
    initial_position_value_usdt: f64,
    initial_risk_percentage: f64,
    entry_time: Option<DateTime<Utc>>,
    last_action_time: Option<DateTime<Utc>>,
    max_hold_until: Option<DateTime<Utc>>,
    realized_pnl: f64,
    funding_fees_paid: f64,
}

impl From<StrategyStateRow> for StrategyState {
    fn from(row: StrategyStateRow) -> Self {
        StrategyState {
            symbol: row.symbol,
            is_active: row.is_active,
            grid_upper_price: row.grid_upper_price,
            grid_lower_price: row.grid_lower_price,
            grid_level: row.grid_level as u8,
            max_grid_levels: row.max_grid_levels as u8,
            long_size: row.long_size,
            short_size: row.short_size,
            long_avg_price: row.long_avg_price,
            short_avg_price: row.short_avg_price,
            initial_position_value_usdt: row.initial_position_value_usdt,
            initial_risk_percentage: row.initial_risk_percentage,
            entry_time: row.entry_time,
            last_action_time: row.last_action_time,
            max_hold_until: row.max_hold_until,
            realized_pnl: row.realized_pnl,
            funding_fees_paid: row.funding_fees_paid,
        }
    }
}

#[derive(sqlx::FromRow)]
struct TradeRecordRow {
    order_id: String,
    symbol: String,
    side: String,
    order_type: String,
    qty: f64,
    price: Option<f64>,
    avg_filled_price: Option<f64>,
    filled_qty: Option<f64>,
    order_status: String,
    closed_pnl: Option<f64>,
    exec_fee: Option<f64>,
}

impl From<TradeRecordRow> for TradeRecord {
    fn from(row: TradeRecordRow) -> Self {
        TradeRecord {
            order_id: row.order_id,
            symbol: row.symbol,
            side: row.side,
            order_type: row.order_type,
            qty: row.qty,
            price: row.price,
            avg_filled_price: row.avg_filled_price,
            filled_qty: row.filled_qty,
            order_status: row.order_status,
            closed_pnl: row.closed_pnl,
            exec_fee: row.exec_fee.unwrap_or(0.0),
        }
    }
}
