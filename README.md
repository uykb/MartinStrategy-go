# MartinStrategy

基于 **Rust** 的高性能马丁格尔策略交易机器人，采用 **事件驱动 + 有限状态机 (ED-FSM)** 架构，专为 Lighter 交易所设计。

## 特性

- **Rust 高性能**: 零成本抽象，内存安全，媲美 C/C++ 的性能
- **异步事件驱动**: 基于 Tokio 的异步消息处理，高并发低延迟
- **Lighter SDK 集成**: 直接使用 lighter-rust SDK 进行安全交易签名
- **有限状态机**: 清晰的状态流转，避免逻辑混乱
- **并发安全**: 使用 RwLock/Mutex 保护关键操作，防止重复下单
- **生产就绪**: 完善的日志 (tracing)、错误处理和监控计数器
- **Docker 支持**: 一键部署，跨平台兼容

## 目录结构

```
.
├── Cargo.toml              # Rust 项目配置
├── src/
│   ├── main.rs             # 程序入口
│   ├── lib.rs              # 库入口
│   ├── config/             # 配置管理 (serde + config crate)
│   │   └── mod.rs
│   ├── core/               # 核心组件 (EventBus)
│   │   └── mod.rs
│   ├── exchange/           # 交易所适配 (Lighter SDK)
│   │   └── mod.rs
│   ├── strategy/           # 策略逻辑 (Martingale FSM)
│   │   └── mod.rs
│   ├── storage/            # 数据存储 (SQLite + Redis)
│   │   └── mod.rs
│   └── utils/              # 工具库 (Logger, ATR)
│       ├── mod.rs
│       ├── logger.rs
│       └── indicators.rs
├── lighter-rust/           # Lighter Rust SDK (已集成)
│   ├── api-client/         # HTTP API 客户端
│   ├── signer/             # 交易签名
│   ├── crypto/             # 加密算法 (Goldilocks)
│   └── poseidon-hash/      # Poseidon2 哈希
├── config.yaml             # 配置文件
├── docker-compose.yml      # Docker Compose
└── Dockerfile              # 构建文件
```

## 快速开始

### 环境要求

- Rust 1.75+ (推荐最新稳定版)
- SQLite (内嵌)
- Redis (可选，用于分布式锁)

### 方式一: Docker Compose (推荐)

```bash
# 1. 创建配置文件
cp config.yaml.example config.yaml

# 2. 编辑配置 (填入API密钥)
vim config.yaml

# 3. 启动服务
docker-compose up -d

# 4. 查看日志
docker-compose logs -f
```

### 方式二: 本地运行

```bash
# 1. 安装 Rust (如果尚未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 克隆仓库
git clone <repo-url>
cd MartinStrategy

# 3. 构建项目
cargo build --release

# 4. 编辑配置
vim config.yaml

# 5. 运行
cargo run --release
# 或
./target/release/bot
```

### 方式三: 开发模式

```bash
# 快速构建 (无优化，用于开发)
cargo build

# 运行测试
cargo test

# 检查代码
cargo clippy

# 格式化代码
cargo fmt
```

## 配置说明

### config.yaml

```yaml
exchange:
  symbol: "HYPEUSDC"                  # 交易对
  private_key: ""                     # Lighter 私钥 (hex格式，必须配置)
  chain_id: 1                         # Lighter 链ID (1为主网)
  api_url: "https://mainnet.zklighter.elliot.ai"  # Lighter API地址
  account_index: 1                    # 账户索引
  api_key_index: 0                    # API密钥索引
  market_index: 0                     # 市场索引

strategy:
  max_safety_orders: 9                # 最大加仓层数
  base_qty: 0.5                       # 底仓数量
  safety_qtys: [0.5, 0.5, 1.0, 1.5, 2.5, 4.0, 6.5, 10.5, 17.0]  # 加仓数量
  atr_period: 14                      # ATR 周期

storage:
  sqlite_path: "bot.db"               # SQLite 数据库路径
  redis_addr: "localhost:6379"        # Redis 地址
  redis_pass: ""                      # Redis 密码
  redis_db: 0                         # Redis 数据库

log:
  level: "info"                       # 日志级别: trace, debug, info, warn, error
```

### 环境变量

支持通过环境变量覆盖配置，前缀为 `MARTIN_`：

```bash
export MARTIN_EXCHANGE_SYMBOL="HYPEUSDC"
export MARTIN_EXCHANGE_PRIVATE_KEY="your_private_key"
export MARTIN_EXCHANGE_CHAIN_ID="1"
export MARTIN_EXCHANGE_API_URL="https://mainnet.zklighter.elliot.ai"
export MARTIN_EXCHANGE_ACCOUNT_INDEX="1"
export MARTIN_EXCHANGE_API_KEY_INDEX="0"
export MARTIN_EXCHANGE_MARKET_INDEX="0"
export MARTIN_STRATEGY_MAX_SAFETY_ORDERS="9"
export MARTIN_STRATEGY_BASE_QTY="0.5"
export MARTIN_LOG_LEVEL="info"
```

## 策略逻辑

### 状态机

```
┌─────────┐     Tick(IDLE)      ┌──────────────┐
│  IDLE   │────────────────────▶│ PLACING_GRID │
│ (空仓)   │                     │  (挂网格单)   │
└─────────┘                     └──────────────┘│
     ▲                                 │
     │                                 │ OrderFilled
     │                                 ▼
     │                         ┌──────────────┐
     │      TPFilled           │ IN_POSITION  │
     └────────────────────────│   (持仓中)    │──────────────┐
                               └──────────────┘              │
                                     │                      │
                                     │ SafetyFilled         │
                                     ▼                      │
                               更新TP止盈单                  │
```

### 马丁策略

1. **开仓**: IDLE 状态收到 Tick 事件，市价开多头底仓
2. **网格挂单**: 根据 Fibonacci 序列计算各层加仓数量，按 ATR 距离递进挂单
   - Level 1-2: 30m ATR
   - Level 3-4: 1h / 2h ATR
   - Level 5-9: 4h / 6h / 8h / 12h / 1d ATR
3. **止盈**: 基于 15m ATR 设置止盈价位

### 固定数量模式

直接配置数量，无需计算名义价值：

| 类型 | 配置项 | 说明 |
|------|--------|------|
| 底仓 | `base_qty` | 市价开仓数量 (如: 0.5 HYPE) |
| 加仓 | `safety_qtys` | 每层限价单数量列表 |

**默认配置示例**:
```yaml
strategy:
  base_qty: 0.5
  safety_qtys: [0.5, 0.5, 1.0, 1.5, 2.5, 4.0, 6.5, 10.5, 17.0]
```

## 架构设计

### 事件驱动架构

```rust
// EventBus 用于组件间通信
pub struct EventBus {
    handlers: Arc<RwLock<HashMap<EventType, Vec<EventHandler>>>>,
    sender: broadcast::Sender<Event>,
}

// 事件类型
pub enum EventType {
    Tick,
    OrderUpdate,
    PositionUpdate,
    Log,
    Start,
    Stop,
}
```

### 并发安全机制

```rust
// 1. 使用 RwLock 实现读写分离
pub struct MartingaleStrategy {
    state: RwLock<State>,
    position: RwLock<Option<Position>>,
    current_tp_order_id: RwLock<i64>,
}

// 2. 防重入锁
let _guard = match self.grid_mutex.try_lock() {
    Ok(guard) => guard,
    Err(_) => {
        // 已经在执行，跳过
        return;
    }
};

// 3. 状态原子操作
{
    let mut state_guard = self.state.write().await;
    *state_guard = State::PlacingGrid;
}
// 网络请求在锁外执行
let result = exchange.place_order(...).await;
```

### Lighter SDK 集成

```rust
// 创建客户端
let client = LighterClient::new(
    config.api_url.clone(),
    &config.private_key,
    config.account_index,
    config.api_key_index as u8,
)?;

// 下单
let order = api_client::CreateOrderRequest {
    account_index: self.config.account_index,
    order_book_index: self.config.market_index as u8,
    client_order_index,
    base_amount,
    price: price_int,
    is_ask,
    order_type,
    time_in_force,
    reduce_only: false,
    trigger_price: 0,
};
let response = client.create_order(order).await?;
```

## 监控指标

### 日志关键字段

| 字段 | 说明 |
|------|------|
| `skip_count` | 因并发冲突跳过的次数 |
| `entryPrice` | 入场价格 |
| `ATR30m` | 30分钟 ATR |
| `state` | 当前状态 (IDLE, IN_POSITION, PLACING_GRID) |

### 示例日志

```
2024-01-15T08:30:00.123Z  INFO bot: Starting MartinStrategy Bot: symbol=HYPEUSDC
2024-01-15T08:30:00.456Z  INFO martin_strategy::exchange: Lighter client initialized: account_index=1, api_key_index=0, market_index=0
2024-01-15T08:30:01.001Z  INFO martin_strategy::strategy: Entering Long Position...
2024-01-15T08:30:01.234Z  INFO martin_strategy::exchange: Placing order on Lighter: symbol=HYPEUSDC, side=BUY, type=MARKET, qty=0.5, price=0
2024-01-15T08:30:02.567Z  INFO martin_strategy::strategy: Buy Order Filled: type=MARKET
```

## 技术栈

| 组件 | 技术 |
|------|------|
| 语言 | Rust 1.75+ |
| 异步运行时 | Tokio |
| 交易所 | Lighter Rust SDK |
| 存储 | SQLite (sqlx) / Redis |
| 配置 | config + serde |
| 日志 | tracing + tracing-subscriber |
| 序列化 | serde + serde_json + serde_yaml |
| 数学 | rust_decimal |
| HTTP | reqwest |

## 开发指南

```bash
# 运行所有测试
cargo test

# 运行指定测试
cargo test test_name

# 构建发布版本
cargo build --release

# 代码检查
cargo clippy

# 格式化
cargo fmt

# 生成文档
cargo doc --open
```

## 从 Go 版本迁移

如果你之前使用 Go 版本，以下是主要变化：

| 特性 | Go 版本 | Rust 版本 |
|------|---------|-----------|
| 并发模型 | goroutines + channels | Tokio tasks + broadcast |
| 锁 | sync.Mutex | tokio::sync::RwLock/Mutex |
| 错误处理 | error interface | Result<T, E> + anyhow |
| JSON | encoding/json | serde_json |
| 配置 | Viper | config + serde |
| 日志 | Zap | tracing |
| 数据库 | GORM | sqlx |
| HTTP | net/http | reqwest |

## 风险提示

⚠️ **重要提示**

- 马丁格尔策略在单边下跌行情中风险极高，可能损失全部本金
- 建议设置止损或限制最大持仓层数
- 请确保已正确配置 Lighter API 私钥，**切勿将私钥提交到 Git**
- 建议先用小额资金测试验证策略
- 交易有风险，入市需谨慎

## License

MIT License

## 致谢

- [Lighter Rust SDK](https://github.com/your-quantguy/lighter-rust) - 提供交易签名和 API 客户端
- [Tokio](https://tokio.rs/) - Rust 异步运行时
- [sqlx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包
