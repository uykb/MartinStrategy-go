use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub exchange: ExchangeConfig,
    pub strategy: StrategyConfig,
    pub storage: StorageConfig,
    pub log: LogConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExchangeConfig {
    pub symbol: String,
    pub private_key: String,
    pub chain_id: u32,
    pub api_url: String,
    pub account_index: i64,
    pub api_key_index: u8,
    pub market_index: i16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyConfig {
    pub max_safety_orders: i32,
    pub base_qty: f64,
    #[serde(default)]
    pub safety_qtys: Vec<f64>,
    pub atr_period: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub sqlite_path: String,
    pub redis_addr: String,
    #[serde(default)]
    pub redis_pass: String,
    #[serde(default)]
    pub redis_db: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    pub level: String,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            max_safety_orders: 9,
            base_qty: 0.5,
            safety_qtys: vec![0.5, 0.5, 1.0, 1.5, 2.5, 4.0, 6.5, 10.5, 17.0],
            atr_period: 14,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            sqlite_path: "martin.db".to_string(),
            redis_addr: "redis://127.0.0.1:6379".to_string(),
            redis_pass: String::new(),
            redis_db: 0,
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut cfg = Config::builder();

        // Add file if exists
        if path.as_ref().exists() {
            cfg = cfg.add_source(File::from(path.as_ref()));
        }

        // Add environment variables with prefix MARTIN_
        cfg = cfg.add_source(
            Environment::with_prefix("MARTIN")
                .separator("_")
                .try_parsing(true),
        );

        let cfg = cfg.build()?;
        let config: AppConfig = cfg.try_deserialize()?;
        
        Ok(config)
    }

    pub fn from_env() -> anyhow::Result<Self> {
        Self::load("config.yaml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = StrategyConfig::default();
        assert_eq!(config.max_safety_orders, 9);
        assert_eq!(config.base_qty, 0.5);
        assert_eq!(config.safety_qtys.len(), 9);
    }
}
