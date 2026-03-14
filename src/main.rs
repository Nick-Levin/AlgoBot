use algotrader::{AlgoTrader, AppConfig};
use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with file output
    let log_dir = std::path::PathBuf::from("logs");
    std::fs::create_dir_all(&log_dir)?;

    // Create file appender with daily rotation
    let file_appender = tracing_appender::rolling::daily(&log_dir, "algotrader");

    let env_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("algotrader=info".parse()?)
        .add_directive("warn".parse()?);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false),
        )
        .init();

    info!("╔════════════════════════════════════════════════════════════╗");
    info!("║              AlgoTrader - DynaGrid Strategy                ║");
    info!("║              Bybit Demo/Production Hybrid                  ║");
    info!("╚════════════════════════════════════════════════════════════╝");

    // Load configuration
    let config = match AppConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            eprintln!("\nConfiguration Error: {}", e);
            eprintln!("\nPlease ensure:");
            eprintln!("  1. You have a config file in config/default.toml or config/production.toml");
            eprintln!("  2. Required environment variables are set:");
            eprintln!("     - BOT_API_PRODUCTION_KEY");
            eprintln!("     - BOT_API_PRODUCTION_SECRET");
            eprintln!("     - BOT_API_DEMO_KEY");
            eprintln!("     - BOT_API_DEMO_SECRET");
            eprintln!("  3. Required config values are set:");
            eprintln!("     - strategy.dynagrid.symbol (e.g., 'ETHUSDT')");
            eprintln!("     - strategy.dynagrid.position_risk_percentage (e.g., 0.02 for 2%)");
            std::process::exit(1);
        }
    };

    info!("Configuration loaded successfully");
    info!("Trading symbol: {}", config.strategy.dynagrid.symbol());
    info!(
        "Risk percentage: {:.2}%",
        config.strategy.dynagrid.risk_percentage() * 100.0
    );

    // Create and run the bot
    let trader = AlgoTrader::new(config).await?;
    
    match trader.run().await {
        Ok(_) => {
            info!("AlgoTrader shut down gracefully");
            Ok(())
        }
        Err(e) => {
            error!("AlgoTrader failed: {}", e);
            Err(e.into())
        }
    }
}
