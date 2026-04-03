use redis::AsyncCommands;
use sqlx::{FromRow, SqlitePool};
use std::time::Duration;
use tracing::{error, info};

#[derive(Debug, Clone, FromRow)]
pub struct Order {
    pub id: i64,
    pub symbol: String,
    pub order_id: i64,
    pub side: String,
    pub order_type: String,
    pub price: f64,
    pub quantity: f64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct BotState {
    pub id: i64,
    pub current_state: String,
    pub position_size: f64,
    pub avg_price: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct Database {
    sqlite: SqlitePool,
    redis: redis::aio::MultiplexedConnection,
}

impl Database {
    pub async fn init(
        sqlite_path: &str,
        redis_addr: &str,
        redis_pass: &str,
        _redis_db: i32,
    ) -> anyhow::Result<Self> {
        // Initialize SQLite
        let sqlite_url = format!("sqlite:{}", sqlite_path);
        let sqlite = SqlitePool::connect(&sqlite_url).await?;

        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                order_id INTEGER UNIQUE NOT NULL,
                side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                price REAL NOT NULL,
                quantity REAL NOT NULL,
                status TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&sqlite)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bot_state (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                current_state TEXT NOT NULL,
                position_size REAL NOT NULL,
                avg_price REAL NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&sqlite)
        .await?;

        info!("SQLite database initialized at: {}", sqlite_path);

        // Initialize Redis
        let redis_client = if redis_pass.is_empty() {
            redis::Client::open(redis_addr)?
        } else {
            let redis_url = format!("redis://:{}@{}", redis_pass, redis_addr.trim_start_matches("redis://"));
            redis::Client::open(redis_url)?
        };

        let redis = redis_client.get_multiplexed_tokio_connection().await?;

        info!("Redis connection established");

        Ok(Self { sqlite, redis })
    }

    pub async fn save_order(&self, order: &Order) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO orders (symbol, order_id, side, order_type, price, quantity, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(order_id) DO UPDATE SET
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&order.symbol)
        .bind(order.order_id)
        .bind(&order.side)
        .bind(&order.order_type)
        .bind(order.price)
        .bind(order.quantity)
        .bind(&order.status)
        .bind(&order.created_at)
        .bind(&order.updated_at)
        .execute(&self.sqlite)
        .await?;

        Ok(())
    }

    pub async fn get_order(&self, order_id: i64) -> anyhow::Result<Option<Order>> {
        let order = sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE order_id = ?1"
        )
        .bind(order_id)
        .fetch_optional(&self.sqlite)
        .await?;

        Ok(order)
    }

    pub async fn get_orders_by_symbol(&self, symbol: &str) -> anyhow::Result<Vec<Order>> {
        let orders = sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE symbol = ?1 ORDER BY created_at DESC"
        )
        .bind(symbol)
        .fetch_all(&self.sqlite)
        .await?;

        Ok(orders)
    }

    pub async fn save_bot_state(&self, state: &BotState) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO bot_state (id, current_state, position_size, avg_price, updated_at)
            VALUES (1, ?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                current_state = excluded.current_state,
                position_size = excluded.position_size,
                avg_price = excluded.avg_price,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&state.current_state)
        .bind(state.position_size)
        .bind(state.avg_price)
        .bind(&state.updated_at)
        .execute(&self.sqlite)
        .await?;

        Ok(())
    }

    pub async fn get_bot_state(&self) -> anyhow::Result<Option<BotState>> {
        let state = sqlx::query_as::<_, BotState>(
            "SELECT * FROM bot_state WHERE id = 1"
        )
        .fetch_optional(&self.sqlite)
        .await?;

        Ok(state)
    }

    pub async fn acquire_lock(&self, key: &str, ttl: Duration) -> anyhow::Result<bool> {
        let mut conn = self.redis.clone();
        let result: redis::RedisResult<bool> = conn.set_nx(key, "locked").await;
        
        match result {
            Ok(acquired) => {
                if acquired {
                    // Set expiration
                    let _: redis::RedisResult<()> = conn.expire(key, ttl.as_secs() as i64).await;
                }
                Ok(acquired)
            }
            Err(e) => {
                error!("Failed to acquire lock: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn release_lock(&self, key: &str) -> anyhow::Result<()> {
        let mut conn = self.redis.clone();
        let _: redis::RedisResult<()> = conn.del(key).await;
        Ok(())
    }
}
