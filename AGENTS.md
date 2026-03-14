# AlgoTrader - Agent Documentation

## Project Overview

AlgoTrader is a Rust-based algorithmic trading bot implementing the **Dynamic Hedge Grid (DynaGrid)** strategy on Bybit's Demo/Production hybrid environment. The bot uses a unique dual-API architecture:

- **Production API**: Read-only access for market data (prices, orderbook, funding rates)
- **Demo API**: Trading access for executing orders and managing positions

The DynaGrid strategy profits from price oscillations within a range by building asymmetric positions (1.5x ratio) with partial exits.

## Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust (Edition 2021) |
| Async Runtime | Tokio |
| HTTP Client | reqwest |
| WebSocket | tokio-tungstenite |
| Serialization | serde, serde_json |
| Configuration | config + dotenvy |
| Database | SQLite (sqlx) |
| Time Handling | chrono |
| Crypto (HMAC) | hmac + sha2 |
| Logging | tracing + tracing-subscriber |
| Error Handling | thiserror + anyhow |
| Retry Logic | backoff |
| Decimal Math | rust_decimal |

## Project Structure

```
algotrader/
├── Cargo.toml              # Rust package configuration
├── setup.sh                # Interactive configuration wizard
├── src/
│   ├── main.rs             # Application entry point
│   ├── lib.rs              # Library exports and AlgoTrader struct
│   ├── error.rs            # BotError enum and error handling
│   ├── api/                # Bybit API clients
│   │   ├── mod.rs          # Module exports
│   │   ├── client.rs       # ApiManager (dual API coordinator)
│   │   ├── rest.rs         # Bybit REST client
│   │   ├── websocket.rs    # WebSocket market data
│   │   └── types.rs        # API DTOs
│   ├── config/             # Configuration management
│   │   └── mod.rs          # AppConfig with layered loading
│   ├── db/                 # SQLite persistence
│   │   ├── mod.rs          # Database struct and migrations
│   │   └── models.rs       # Data models (StrategyState, TradeRecord, etc.)
│   ├── risk/               # Risk management
│   │   ├── mod.rs          # Module exports
│   │   └── manager.rs      # RiskManager with limits
│   └── strategy/           # Trading strategy
│       ├── mod.rs          # Module exports
│       ├── dynagrid.rs     # DynaGridEngine implementation
│       └── types.rs        # Strategy types (Side, Zone, GridConfig, etc.)
├── config/
│   ├── default.toml        # Default configuration values
│   └── production.toml     # Local overrides (API keys, symbol)
├── data/                   # SQLite database (runtime)
└── logs/                   # Log output directory
```

## Configuration System

Configuration is loaded in priority order (later overrides earlier):

1. `config/default.toml` - Base defaults
2. `config/production.toml` - Local overrides (not in git)
3. `config/local.toml` - Optional additional overrides
4. Environment variables with `BOT_` prefix

### Required Configuration

```toml
[api.production]
key = "YOUR_PRODUCTION_API_KEY"
secret = "YOUR_PRODUCTION_API_SECRET"

[api.demo]
key = "YOUR_DEMO_API_KEY"
secret = "YOUR_DEMO_API_SECRET"

[strategy.dynagrid]
symbol = "ETHUSDT"
position_risk_percentage = 0.02  # 2% of free USDT per trade
```

### Environment Variables

All config values can be set via environment variables:
- `BOT_API_PRODUCTION_KEY`
- `BOT_API_PRODUCTION_SECRET`
- `BOT_API_DEMO_KEY`
- `BOT_API_DEMO_SECRET`
- `BOT_STRATEGY_DYNAGRID_SYMBOL`
- `BOT_STRATEGY_DYNAGRID_POSITION_RISK_PERCENTAGE`

Naming: `BOT_` + section + field (uppercase, underscores for nesting).

### Configuration Validation

The `AppConfig::load()` method validates:
- API credentials are present (non-empty)
- Trading symbol is specified
- Risk percentage is between 0 and 50%
- Grid range is between 0 and 10%
- Position sizing factor is between 1.1 and 2.0
- Leverage is between 1 and 100
- Exit percentages sum to 100
- Timeframe is one of: 5, 15, 60, 240 minutes

## Build and Run Commands

```bash
# Development build and run
cargo run

# Production build
cargo build --release
./target/release/algotrader

# Run with custom config path
BOT_CONFIG_PATH="./config/myconfig.toml" ./target/release/algotrader

# Run with debug logging
RUST_LOG=debug cargo run

# Interactive setup wizard
./setup.sh
```

## Test Commands

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_exit_config_validation
```

Current test coverage:
- Config validation (`src/config/mod.rs`)
- API manager creation (`src/api/client.rs`)
- Risk checks (`src/risk/manager.rs`)
- Strategy helpers (`src/strategy/types.rs`)

## Code Style Guidelines

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Files/modules | `snake_case` | `dynagrid.rs`, `risk_manager` |
| Functions | `snake_case` | `calculate_position_size` |
| Variables | `snake_case` | `grid_range_pct` |
| Structs/Enums | `PascalCase` | `DynaGridEngine`, `RiskStatus` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_GRID_RANGE` |
| TOML keys | `snake_case` | `position_risk_percentage` |

### Code Formatting

Use standard Rust formatting:
```bash
cargo fmt
```

### Linting

```bash
cargo clippy --all-targets --all-features
```

### Domain Terminology

- `ApiManager`: Dual API coordinator
- `DynaGridEngine`: Strategy implementation
- `RiskManager`: Exposure and loss controls
- `StrategyState`: Persistent strategy state
- `Zone::Upper` / `Zone::Lower`: Grid zones (buy/sell)
- `Side::Buy` / `Side::Sell`: Position sides

### API Field Casing

Match Bybit API exactly:
- Request fields: `camelCase`
- Response fields: `camelCase` (use serde rename if needed)

## Testing Guidelines

### Unit Tests

Place tests in the same file using `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        // Arrange
        let input = ...;
        
        // Act
        let result = my_function(input);
        
        // Assert
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_async_function() {
        // Async test
    }
}
```

### Test Patterns

1. **Config validation**: Test that invalid configs fail with appropriate errors
2. **Precision/rounding**: Test financial calculations with explicit rounding rules
3. **Risk boundaries**: Test limit enforcement (daily loss, max exposure)
4. **Strategy logic**: Test grid calculations, position sizing, zone detection

## Security Considerations

### API Key Management

- **NEVER commit real API keys** to git
- `keys.txt` and `config/production.toml` contain secrets - do not commit
- Use environment variables for credentials in production:
  ```bash
  export BOT_API_PRODUCTION_KEY="..."
  export BOT_API_PRODUCTION_SECRET="..."
  ```
- The `.gitignore` currently only ignores `/target` - review staging carefully

### Database Files

Do not commit:
- `data/*.db*` - SQLite database files
- `data/*.db-shm` - Shared memory files
- `data/*.db-wal` - Write-ahead log files
- `logs/` - Log files

### Safe Operations

- Bot only trades on **Demo account** (no real money risk)
- Emergency stop-loss triggers at configurable threshold (default 5%)
- Daily loss limit stops trading after threshold (default 2%)

## Git Workflow

### Commit Message Format

Follow Conventional Commit-style prefixes:
- `feat:` - New feature
- `fix:` - Bug fix
- `refactor:` - Code refactoring
- `test:` - Adding tests
- `docs:` - Documentation updates

Example: `fix: Handle WebSocket delta updates properly`

### Recent History

Recent commits focus on API integration fixes:
- Order placement with hedge mode
- Field name casing for Bybit API
- Position size precision
- WebSocket subscription handling
- Database state restoration

## Architecture Patterns

### Error Handling

- Use `BotError` enum for all domain errors
- Use `BotResult<T>` as alias for `Result<T, BotError>`
- Mark errors as `retryable` for transient failures
- Mark errors as `critical` for emergency shutdown conditions

### Async Patterns

- Use `tokio` for async runtime
- Use `Arc<Database>` for shared database access
- Use `tokio::sync::RwLock` for shared mutable state
- Use `tokio::spawn` for background tasks (market data manager)

### State Persistence

- Strategy state saved to SQLite on every change
- State restored on bot restart
- Database migrations run automatically on startup

### Dual API Pattern

```rust
// Production API - read-only market data
let ticker = api.get_ticker().await?;

// Demo API - trading operations
let order = api.place_order(&request).await?;
```

## Key Files Reference

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, logging init, config load |
| `src/lib.rs` | AlgoTrader struct, main run loop |
| `src/error.rs` | Error types and helpers |
| `src/config/mod.rs` | Configuration structs and validation |
| `src/api/client.rs` | ApiManager (dual API coordinator) |
| `src/api/rest.rs` | REST API implementation |
| `src/api/websocket.rs` | WebSocket market data |
| `src/db/mod.rs` | Database connection and migrations |
| `src/db/models.rs` | Data models |
| `src/risk/manager.rs` | Risk limit enforcement |
| `src/strategy/dynagrid.rs` | Strategy implementation |
| `src/strategy/types.rs` | Strategy types and helpers |

## Debugging Tips

### View Database

```bash
sqlite3 ./data/algotrader.db "SELECT * FROM trade_history ORDER BY created_at DESC LIMIT 10;"
```

### Check Risk Metrics

Risk metrics are logged periodically:
```
Risk Metrics - Daily P&L: -0.50% (-50.00 USDT), Exposure: 25.00%, Trades: 5
```

### Common Issues

- **"Configuration error: 'strategy.dynagrid.symbol' is required"**: Set symbol in config or env
- **"Authentication failed"**: Verify API keys and permissions
- **"Insufficient margin"**: Check demo account balance or reduce risk percentage

## License

MIT License - See LICENSE file

## Disclaimer

⚠️ **Trading Risk Warning**: This bot trades on Demo account only. Cryptocurrency trading carries significant risk. Past performance does not guarantee future results.
