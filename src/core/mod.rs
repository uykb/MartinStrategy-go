use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Tick,
    OrderUpdate,
    PositionUpdate,
    Log,
    Start,
    Stop,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::Tick => write!(f, "TICK"),
            EventType::OrderUpdate => write!(f, "ORDER_UPDATE"),
            EventType::PositionUpdate => write!(f, "POSITION_UPDATE"),
            EventType::Log => write!(f, "LOG"),
            EventType::Start => write!(f, "START"),
            EventType::Stop => write!(f, "STOP"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub data: EventData,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum EventData {
    Tick { price: f64 },
    OrderUpdate(Order),
    PositionUpdate(Position),
    Log(String),
    Start,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: i64,
    pub symbol: String,
    pub side: String, // BUY or SELL
    #[serde(rename = "type")]
    pub order_type: String, // MARKET or LIMIT
    pub price: String,
    pub quantity: String,
    #[serde(rename = "executed_qty")]
    pub executed_qty: String,
    pub status: String,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    #[serde(rename = "position_amt")]
    pub position_amt: String,
    #[serde(rename = "entry_price")]
    pub entry_price: String,
    #[serde(rename = "unrealized_profit")]
    pub unrealized_profit: String,
    pub leverage: String,
}

pub type EventHandler = Arc<dyn Fn(Event) -> futures::future::BoxFuture<'static, ()> + Send + Sync>;

use futures::future::BoxFuture;

pub struct EventBus {
    handlers: Arc<RwLock<HashMap<EventType, Vec<EventHandler>>>>,
    sender: broadcast::Sender<Event>,
}

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus")
            .field("handlers", &"...")
            .field("sender", &"...")
            .finish()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _receiver) = broadcast::channel(1000);
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            sender,
        }
    }

    pub async fn subscribe<F, Fut>(&self, event_type: EventType, handler: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let handler: EventHandler = Arc::new(move |event| {
            let fut = handler(event);
            Box::pin(fut) as BoxFuture<'static, ()>
        });

        let mut handlers = self.handlers.write().await;
        handlers.entry(event_type).or_default().push(handler);
        debug!("Subscribed to event type: {}", event_type);
    }

    pub fn publish(&self, event_type: EventType, data: EventData) {
        let event = Event {
            event_type,
            data,
            timestamp: chrono::Utc::now(),
        };

        if let Err(e) = self.sender.send(event.clone()) {
            warn!("Failed to send event to broadcast channel: {}", e);
        }

        // Also process through registered handlers
        let handlers = self.handlers.clone();
        tokio::spawn(async move {
            let handlers_read = handlers.read().await;
            if let Some(handlers_list) = handlers_read.get(&event_type) {
                for handler in handlers_list {
                    let event = event.clone();
                    let handler = handler.clone();
                    tokio::spawn(async move {
                        handler(event).await;
                    });
                }
            }
        });
    }

    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub async fn start(&self) {
        debug!("EventBus started");
    }

    pub async fn stop(&self) {
        debug!("EventBus stopped");
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
