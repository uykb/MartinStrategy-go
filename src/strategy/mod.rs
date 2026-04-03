use crate::config::StrategyConfig;
use crate::core::{Event, EventBus, EventData, EventType, Order, Position};
use crate::exchange::LighterExchange;
use crate::storage::Database;
use crate::utils::indicators::calculate_atr;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    InPosition,
    PlacingGrid,
    Closing,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Idle => write!(f, "IDLE"),
            State::InPosition => write!(f, "IN_POSITION"),
            State::PlacingGrid => write!(f, "PLACING_GRID"),
            State::Closing => write!(f, "CLOSING"),
        }
    }
}

pub struct MartingaleStrategy {
    config: StrategyConfig,
    exchange: Arc<LighterExchange>,
    storage: Arc<Database>,
    event_bus: Arc<EventBus>,
    
    state: RwLock<State>,
    position: RwLock<Option<Position>>,
    active_orders: RwLock<HashMap<i64, Order>>,
    current_tp_order_id: RwLock<i64>,
    
    current_atr: RwLock<f64>,
    
    // Symbol info
    quantity_precision: i32,
    price_precision: i32,
    min_qty: f64,
    step_size: f64,
    tick_size: f64,
    
    // Concurrency locks
    grid_mutex: Mutex<()>,
    tp_mutex: Mutex<()>,
    
    // Counters
    grid_skip_count: AtomicI64,
    tp_skip_count: AtomicI64,
}

impl MartingaleStrategy {
    pub fn new(
        config: StrategyConfig,
        exchange: Arc<LighterExchange>,
        storage: Arc<Database>,
        event_bus: Arc<EventBus>,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            exchange,
            storage,
            event_bus,
            state: RwLock::new(State::Idle),
            position: RwLock::new(None),
            active_orders: RwLock::new(HashMap::new()),
            current_tp_order_id: RwLock::new(0),
            current_atr: RwLock::new(0.0),
            quantity_precision: 8,
            price_precision: 8,
            min_qty: 0.0001,
            step_size: 0.00000001,
            tick_size: 0.01,
            grid_mutex: Mutex::new(()),
            tp_mutex: Mutex::new(()),
            grid_skip_count: AtomicI64::new(0),
            tp_skip_count: AtomicI64::new(0),
        })
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Initialize symbol info
        self.init_symbol_info().await?;
        
        // Subscribe to events
        let strategy_clone = Arc::clone(&self);
        self.event_bus.subscribe(EventType::Tick, move |event| {
            let strategy = Arc::clone(&strategy_clone);
            async move {
                if let Err(e) = strategy.handle_tick(event).await {
                    error!("Error handling tick: {}", e);
                }
            }
        }).await;
        
        let strategy_clone = Arc::clone(&self);
        self.event_bus.subscribe(EventType::OrderUpdate, move |event| {
            let strategy = Arc::clone(&strategy_clone);
            async move {
                if let Err(e) = strategy.handle_order_update(event).await {
                    error!("Error handling order update: {}", e);
                }
            }
        }).await;
        
        // Sync initial state
        self.sync_state().await?;
        
        info!("MartingaleStrategy started");
        Ok(())
    }

    async fn init_symbol_info(&self) -> Result<()> {
        let info = self.exchange.get_exchange_info().await?;
        
        let symbol = self.exchange.get_symbol();
        let found = info.symbols.iter().any(|s| s.symbol == symbol);
        
        if !found {
            return Err(anyhow::anyhow!("Symbol {} not found in exchange info", symbol));
        }
        
        info!(
            "Symbol Info Initialized: symbol={}, price_prec={}, qty_prec={}",
            symbol, self.price_precision, self.quantity_precision
        );
        
        Ok(())
    }

    async fn sync_state(&self) -> Result<()> {
        // Get position
        let pos = self.exchange.get_position().await?;
        
        let mut position_guard = self.position.write().await;
        *position_guard = pos.clone();
        
        if let Some(ref pos) = pos {
            let amt: f64 = pos.position_amt.parse().unwrap_or(0.0);
            
            if amt.abs() > 0.0 {
                let mut state_guard = self.state.write().await;
                *state_guard = State::InPosition;
                drop(state_guard);
                
                // Update ATR
                self.update_atr().await;
                
                // Check open orders
                match self.exchange.get_open_orders().await {
                    Ok(orders) => {
                        let mut has_tp = false;
                        for order in &orders {
                            if order.side == "SELL" && order.order_type == "LIMIT" {
                                has_tp = true;
                                let mut tp_guard = self.current_tp_order_id.write().await;
                                *tp_guard = order.order_id;
                                break;
                            }
                        }
                        
                        if !has_tp {
                            warn!("Detected position without TP order. Restoring TP...");
                            let exchange = Arc::clone(&self.exchange);
                            tokio::spawn(async move {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                // TP update will be triggered on next position check
                                let _ = exchange.get_position().await;
                            });
                        } else {
                            info!("State restored with existing TP. Open orders: {}", orders.len());
                        }
                    }
                    Err(e) => {
                        error!("Failed to get open orders: {}", e);
                    }
                }
            } else {
                let mut state_guard = self.state.write().await;
                *state_guard = State::Idle;
            }
        }
        
        info!("State Synced: state={}, amt={:?}", self.state.read().await, 
              pos.as_ref().map(|p| p.position_amt.clone()).unwrap_or_default());
        
        Ok(())
    }

    async fn handle_tick(&self, event: Event) -> Result<()> {
        let price = match event.data {
            EventData::Tick { price } => price,
            _ => return Ok(()),
        };
        
        // Check state
        let state = *self.state.read().await;
        if state != State::Idle {
            return Ok(());
        }
        
        // Set state to placing grid
        {
            let mut state_guard = self.state.write().await;
            *state_guard = State::PlacingGrid;
        }
        
        // Place entry order
        if let Err(e) = self.enter_long(price).await {
            // Rollback state on failure
            let mut state_guard = self.state.write().await;
            *state_guard = State::Idle;
            error!("enterLong failed, resetting to IDLE: {}", e);
            return Err(e);
        }
        
        Ok(())
    }

    async fn handle_order_update(&self, event: Event) -> Result<()> {
        let order = match event.data {
            EventData::OrderUpdate(order) => order,
            _ => return Ok(()),
        };
        
        info!(
            "Order Update Received: id={}, status={}, type={}",
            order.order_id, order.status, order.order_type
        );
        
        if order.status == "FILLED" {
            if order.side == "BUY" {
                info!("Buy Order Filled: type={}", order.order_type);
                
                let prev_state = {
                    let mut state_guard = self.state.write().await;
                    let prev = *state_guard;
                    *state_guard = State::InPosition;
                    prev
                };
                
                if prev_state == State::Idle || prev_state == State::PlacingGrid {
                    // Base order filled -> Place Grid
                    let exec_price: f64 = order.price.parse().unwrap_or(0.0);
                    let exchange = Arc::clone(&self.exchange);
                    let config = self.config.clone();
                    tokio::spawn(async move {
                        // Grid orders logic - simplified for now
                        info!("Base order filled at {}, placing grid orders...", exec_price);
                    });
                } else {
                    // Safety order filled -> Update TP
                    info!("Safety Order Filled. Re-calculating TP.");
                    let exchange = Arc::clone(&self.exchange);
                    tokio::spawn(async move {
                        // TP update logic - simplified for now
                        let _ = exchange.get_position().await;
                    });
                }
            } else if order.side == "SELL" {
                // Sell order filled (TP, Manual, or Stop)
                info!(
                    "Sell Order Filled (TP/Manual): type={}, status={}",
                    order.order_type, order.status
                );
                
                {
                    let mut state_guard = self.state.write().await;
                    *state_guard = State::Idle;
                }
                {
                    let mut tp_guard = self.current_tp_order_id.write().await;
                    *tp_guard = 0;
                }
                
                if let Err(e) = self.exchange.cancel_all_orders().await {
                    error!("Failed to cancel orders: {}", e);
                }
                
                // Wait before next cycle
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        }
        
        Ok(())
    }

    async fn enter_long(&self, current_price: f64) -> Result<()> {
        info!("Entering Long Position...");
        
        // Update ATR
        self.update_atr().await;
        
        // Use configured base quantity
        let base_qty = if self.config.base_qty > 0.0 {
            self.config.base_qty
        } else {
            0.5
        };
        
        info!(
            "Placing Base Order (Fixed Qty): price={}, base_qty={}",
            current_price, base_qty
        );
        
        self.exchange.place_order("BUY", "MARKET", base_qty, 0.0).await?;
        
        Ok(())
    }

    async fn place_grid_orders(&self, exec_price: f64) {
        // Try to acquire grid mutex
        let _guard = match self.grid_mutex.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                let count = self.grid_skip_count.fetch_add(1, Ordering::SeqCst) + 1;
                warn!("placeGridOrders skipped: already running, skip_count={}", count);
                return;
            }
        };
        
        let entry_price = if exec_price > 0.0 {
            exec_price
        } else {
            match self.exchange.get_position().await {
                Ok(Some(pos)) => pos.entry_price.parse().unwrap_or(0.0),
                _ => {
                    error!("Failed to get position for grid orders");
                    return;
                }
            }
        };
        
        if entry_price <= 0.0 {
            error!("Invalid entry price: {}", entry_price);
            return;
        }
        
        // Fetch ATRs for different timeframes
        let atr_30m = self.fetch_atr("30m").await;
        let atr_1h = self.fetch_atr("1h").await;
        let atr_2h = self.fetch_atr("2h").await;
        let atr_4h = self.fetch_atr("4h").await;
        let atr_6h = self.fetch_atr("6h").await;
        let atr_8h = self.fetch_atr("8h").await;
        let atr_12h = self.fetch_atr("12h").await;
        let atr_1d = self.fetch_atr("1d").await;
        
        // Fallback to 1% of entry price if ATR is 0
        let atr_30m = if atr_30m == 0.0 { entry_price * 0.01 } else { atr_30m };
        let atr_1h = if atr_1h == 0.0 { entry_price * 0.01 } else { atr_1h };
        let atr_2h = if atr_2h == 0.0 { entry_price * 0.01 } else { atr_2h };
        let atr_4h = if atr_4h == 0.0 { entry_price * 0.01 } else { atr_4h };
        let atr_6h = if atr_6h == 0.0 { entry_price * 0.01 } else { atr_6h };
        let atr_8h = if atr_8h == 0.0 { entry_price * 0.01 } else { atr_8h };
        let atr_12h = if atr_12h == 0.0 { entry_price * 0.01 } else { atr_12h };
        let atr_1d = if atr_1d == 0.0 { entry_price * 0.01 } else { atr_1d };
        
        // Use configured safety quantities or default
        let safety_qtys = if self.config.safety_qtys.is_empty() {
            vec![0.5, 0.5, 1.0, 1.5, 2.5, 4.0, 6.5, 10.5, 17.0]
        } else {
            self.config.safety_qtys.clone()
        };
        
        info!(
            "Placing Grid Orders (Fixed Qty): Entry={}, ATR30m={}, max_orders={}",
            entry_price, atr_30m, self.config.max_safety_orders
        );
        
        let grid_distances = vec![
            atr_30m, atr_30m, atr_1h, atr_2h, atr_4h, 
            atr_6h, atr_8h, atr_12h, atr_1d
        ];
        
        let mut current_price_level = entry_price;
        
        for i in 1..=self.config.max_safety_orders {
            let step_dist = grid_distances.get((i - 1) as usize)
                .copied()
                .unwrap_or(*grid_distances.last().unwrap());
            
            let price = current_price_level - step_dist;
            current_price_level = price;
            
            let qty_index = (i - 1) as usize;
            let qty = safety_qtys.get(qty_index)
                .copied()
                .unwrap_or(*safety_qtys.last().unwrap());
            
            info!(
                "Placing Safety Order: index={}, price={}, qty={}, dist_atr={}",
                i, price, qty, step_dist
            );
            
            match self.exchange.place_order("BUY", "LIMIT", qty, price).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to place safety order {}: {}", i, e);
                }
            }
            
            // Rate limit protection
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
        
        // Place initial TP
        self.update_tp().await;
    }

    async fn update_tp(&self) {
        // Try to acquire TP mutex
        let _guard = match self.tp_mutex.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                let count = self.tp_skip_count.fetch_add(1, Ordering::SeqCst) + 1;
                warn!("updateTP skipped: already running, skip_count={}", count);
                return;
            }
        };
        
        // Get updated position
        let pos = match self.exchange.get_position().await {
            Ok(Some(pos)) => pos,
            _ => {
                error!("Failed to get position for TP update");
                return;
            }
        };
        
        let avg_price: f64 = pos.entry_price.parse().unwrap_or(0.0);
        let amt: f64 = pos.position_amt.parse().unwrap_or(0.0);
        
        // If position is closed, no need for TP
        if amt.abs() == 0.0 {
            let mut tp_guard = self.current_tp_order_id.write().await;
            *tp_guard = 0;
            return;
        }
        
        // Check if state is Idle
        {
            let state = *self.state.read().await;
            if state == State::Idle {
                return;
            }
        }
        
        // Get ATR for TP calculation
        let atr_15m = self.fetch_atr("15m").await;
        let atr_15m = if atr_15m == 0.0 { avg_price * 0.01 } else { atr_15m };
        
        let old_tp_id = *self.current_tp_order_id.read().await;
        let tp_price = avg_price + atr_15m;
        
        // Cancel old TP
        if old_tp_id != 0 {
            info!("Cancelling old TP: id={}", old_tp_id);
            if let Err(e) = self.exchange.cancel_order(old_tp_id).await {
                warn!("Failed to cancel old TP: {}", e);
            }
        }
        
        // Round TP price to tick size
        let tp_price = crate::utils::round_to_tick_size(tp_price, self.tick_size);
        let tp_price = crate::utils::to_fixed(tp_price, self.price_precision);
        
        // Round quantity to precision
        let tp_qty = crate::utils::to_fixed(amt.abs(), self.quantity_precision);
        
        info!("Updating TP: Price={}, Qty={}", tp_price, tp_qty);
        
        match self.exchange.place_order("SELL", "LIMIT", tp_qty, tp_price).await {
            Ok(order) => {
                // Check state again
                {
                    let state = *self.state.read().await;
                    if state == State::Idle {
                        info!("Cycle finished during TP update, cancelling new TP: id={}", order.order_id);
                        let exchange = Arc::clone(&self.exchange);
                        let order_id = order.order_id;
                        tokio::spawn(async move {
                            let _ = exchange.cancel_order(order_id).await;
                        });
                        return;
                    }
                }
                
                let mut tp_guard = self.current_tp_order_id.write().await;
                *tp_guard = order.order_id;
            }
            Err(e) => {
                error!("Failed to place TP order: {}", e);
            }
        }
    }

    async fn update_atr(&self) {
        let atr = self.fetch_atr("15m").await;
        let mut atr_guard = self.current_atr.write().await;
        *atr_guard = atr;
        info!("ATR Updated (Default 15m): ATR={}", atr);
    }

    async fn fetch_atr(&self, interval: &str) -> f64 {
        match self.exchange.get_klines(interval, 50).await {
            Ok(klines) => {
                let highs: Vec<f64> = klines.iter().map(|k| k.high).collect();
                let lows: Vec<f64> = klines.iter().map(|k| k.low).collect();
                let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
                
                calculate_atr(&highs, &lows, &closes, self.config.atr_period)
            }
            Err(e) => {
                error!("Failed to get klines for interval {}: {}", interval, e);
                0.0
            }
        }
    }

    pub async fn shutdown(&self) {
        info!("Shutting down strategy...");
        
        if let Err(e) = self.exchange.cancel_all_orders().await {
            error!("Failed to cancel orders on shutdown: {}", e);
        } else {
            info!("All orders cancelled");
        }
    }
}
