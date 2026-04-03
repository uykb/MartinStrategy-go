use anyhow::Result;
use martin_strategy::{
    config::AppConfig,
    core::EventBus,
    exchange::LighterExchange,
    storage::Database,
    strategy::MartingaleStrategy,
    utils::logger::init_logger,
};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load configuration
    let config = match AppConfig::load("config.yaml") {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    // 2. Initialize logger
    init_logger(&config.log.level)?;

    info!(
        "Starting MartinStrategy Bot: symbol={}",
        config.exchange.symbol
    );

    // 3. Initialize storage
    let storage = match Database::init(
        &config.storage.sqlite_path,
        &config.storage.redis_addr,
        &config.storage.redis_pass,
        config.storage.redis_db,
    )
    .await
    {
        Ok(db) => Arc::new(db),
        Err(e) => {
            error!("Failed to init storage: {}", e);
            std::process::exit(1);
        }
    };

    // 4. Create event bus
    let event_bus = Arc::new(EventBus::new());
    event_bus.start().await;

    // 5. Create exchange client
    let exchange = match LighterExchange::new(config.exchange.clone(), Arc::clone(&event_bus)).await
    {
        Ok(ex) => Arc::new(ex),
        Err(e) => {
            error!("Failed to create Lighter client: {}", e);
            std::process::exit(1);
        }
    };

    // Start polling
    exchange.start_polling().await;

    // 6. Create and start strategy
    let strategy = MartingaleStrategy::new(
        config.strategy.clone(),
        Arc::clone(&exchange),
        Arc::clone(&storage),
        Arc::clone(&event_bus),
    );

    let strategy_clone = Arc::clone(&strategy);
    if let Err(e) = strategy.start().await {
        error!("Failed to start strategy: {}", e);
        std::process::exit(1);
    }

    // 7. Wait for shutdown signal
    info!("Bot is running. Press Ctrl+C to stop.");
    
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())?;
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv() => {
            info!("Received SIGINT, shutting down...");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM, shutting down...");
        }
    }

    // Graceful shutdown
    info!("Shutting down...");
    
    strategy_clone.shutdown().await;
    event_bus.stop().await;
    
    info!("Shutdown complete");
    
    Ok(())
}
