use crate::config::ExchangeConfig;
use crate::core::{EventBus, EventData, EventType, Order, Position};
use anyhow::Result;
use api_client::LighterClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeInfo {
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub symbol: String,
    pub status: String,
    #[serde(rename = "base_asset")]
    pub base_asset: String,
    #[serde(rename = "quote_asset")]
    pub quote_asset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    #[serde(rename = "open_time")]
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    #[serde(rename = "close_time")]
    pub close_time: i64,
}

pub struct LighterExchange {
    client: Arc<RwLock<LighterClient>>,
    config: ExchangeConfig,
    event_bus: Arc<EventBus>,
    http_client: reqwest::Client,
}

impl LighterExchange {
    pub async fn new(config: ExchangeConfig, event_bus: Arc<EventBus>) -> Result<Self> {
        let client = LighterClient::new(
            config.api_url.clone(),
            &config.private_key,
            config.account_index,
            config.api_key_index as u8,
        )?;

        info!(
            "Lighter client initialized: account_index={}, api_key_index={}, market_index={}",
            config.account_index, config.api_key_index, config.market_index
        );

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            config,
            event_bus,
            http_client,
        })
    }

    pub async fn start_polling(&self) {
        let exchange = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
            
            loop {
                interval.tick().await;
                
                // Poll position updates
                match exchange.get_position().await {
                    Ok(Some(position)) => {
                        exchange.event_bus.publish(
                            EventType::PositionUpdate,
                            EventData::PositionUpdate(position),
                        );
                    }
                    Ok(None) => {}
                    Err(e) => {
                        debug!("Failed to get position: {}", e);
                    }
                }

                // Poll open orders
                match exchange.get_open_orders().await {
                    Ok(orders) => {
                        for order in orders {
                            exchange.event_bus.publish(
                                EventType::OrderUpdate,
                                EventData::OrderUpdate(order),
                            );
                        }
                    }
                    Err(e) => {
                        debug!("Failed to get open orders: {}", e);
                    }
                }
            }
        });

        info!("Started Lighter polling loop");
    }

    pub async fn get_position(&self) -> Result<Option<Position>> {
        let url = format!(
            "{}/api/v1/position?account_index={}&symbol={}",
            self.config.api_url,
            self.config.account_index,
            self.config.symbol
        );

        let resp = self.http_client.get(&url).send().await?;
        
        if resp.status().is_success() {
            let position: Position = resp.json().await?;
            Ok(Some(position))
        } else if resp.status().as_u16() == 404 {
            Ok(None)
        } else {
            Err(anyhow::anyhow!("Failed to get position: {}", resp.status()))
        }
    }

    pub fn get_symbol(&self) -> &str {
        &self.config.symbol
    }

    pub async fn get_exchange_info(&self) -> Result<ExchangeInfo> {
        let url = format!("{}/v1/exchange_info", self.config.api_url);
        
        let resp = self.http_client.get(&url).send().await?;
        
        if resp.status().is_success() {
            let info: ExchangeInfo = resp.json().await?;
            Ok(info)
        } else {
            Err(anyhow::anyhow!("Failed to get exchange info: {}", resp.status()))
        }
    }

    pub async fn place_order(
        &self,
        side: &str,
        order_type: &str,
        quantity: f64,
        price: f64,
    ) -> Result<Order> {
        let client = self.client.read().await;
        
        let is_ask = side == "SELL";
        let order_type_num = if order_type == "MARKET" { 1 } else { 0 };
        let time_in_force = if order_type == "MARKET" { 0 } else { 1 };
        
        // Convert quantity to base_amount (USDC with 6 decimals)
        let base_amount = (quantity * 1_000_000.0) as i64;
        
        // Convert price to Lighter format (price * 100 for 2 decimals)
        let price_int = (price * 100.0) as i64;
        
        let client_order_index = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        
        let order = api_client::CreateOrderRequest {
            account_index: self.config.account_index,
            order_book_index: self.config.market_index as u8,
            client_order_index,
            base_amount,
            price: price_int,
            is_ask,
            order_type: order_type_num,
            time_in_force,
            reduce_only: false,
            trigger_price: 0,
        };
        
        info!(
            "Placing order on Lighter: symbol={}, side={}, type={}, qty={}, price={}",
            self.config.symbol, side, order_type, quantity, price
        );
        
        let response = client.create_order(order).await?;
        
        let order_id = response["data"]["order_id"]
            .as_i64()
            .unwrap_or(client_order_index as i64);
        
        let order = Order {
            order_id,
            symbol: self.config.symbol.clone(),
            side: side.to_string(),
            order_type: order_type.to_string(),
            price: price.to_string(),
            quantity: quantity.to_string(),
            executed_qty: "0".to_string(),
            status: "NEW".to_string(),
            time: chrono::Utc::now().timestamp_millis(),
        };
        
        Ok(order)
    }

    pub async fn cancel_all_orders(&self) -> Result<()> {
        let client = self.client.read().await;
        
        info!("Canceling all orders");
        
        let time = chrono::Utc::now().timestamp_millis();
        client.cancel_all_orders(0, time).await?;
        
        Ok(())
    }

    pub async fn cancel_order(&self, order_id: i64) -> Result<()> {
        let client = self.client.read().await;
        
        info!("Canceling order: {}", order_id);
        
        client.cancel_order(self.config.market_index as u8, order_id).await?;
        
        Ok(())
    }

    pub async fn get_open_orders(&self) -> Result<Vec<Order>> {
        let url = format!(
            "{}/api/v1/open_orders?account_index={}&symbol={}",
            self.config.api_url,
            self.config.account_index,
            self.config.symbol
        );

        let resp = self.http_client.get(&url).send().await?;
        
        if resp.status().is_success() {
            let orders: Vec<Order> = resp.json().await?;
            Ok(orders)
        } else {
            Err(anyhow::anyhow!("Failed to get open orders: {}", resp.status()))
        }
    }

    pub async fn get_klines(&self, interval: &str, limit: i32) -> Result<Vec<Kline>> {
        let url = format!(
            "{}/api/v1/klines?symbol={}&interval={}&limit={}",
            self.config.api_url,
            self.config.symbol,
            interval,
            limit
        );

        let resp = self.http_client.get(&url).send().await?;
        
        if resp.status().is_success() {
            let klines: Vec<Kline> = resp.json().await?;
            Ok(klines)
        } else {
            Err(anyhow::anyhow!("Failed to get klines: {}", resp.status()))
        }
    }
}

impl Clone for LighterExchange {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            config: self.config.clone(),
            event_bus: Arc::clone(&self.event_bus),
            http_client: self.http_client.clone(),
        }
    }
}
