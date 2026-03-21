# MartinStrategy

基于 Go 语言的高性能马丁格尔策略交易机器人，采用 **事件驱动 + 有限状态机 (ED-FSM)** 架构，专为 Lighter 设计。

## 特性

- **事件驱动架构**: 基于 EventBus 的异步消息处理，高并发低延迟
- **有限状态机**: 清晰的状态流转，避免逻辑混乱
- **并发安全**: 互斥锁保护关键操作，防止重复下单
- **生产就绪**: 完善的日志、错误处理和监控计数器
- **Docker 支持**: 一键部署，跨平台兼容

## 目录结构

```
.
├── cmd/
│   └── bot/
│       └── main.go        # 入口文件
├── internal/
│   ├── config/            # 配置管理 (Viper)
│   ├── core/               # 核心组件 (EventBus)
│   ├── exchange/           # 交易所适配 (Lighter HTTP)
│   ├── strategy/           # 策略逻辑 (Martingale FSM)
│   ├── storage/            # 数据存储 (SQLite, Redis)
│   └── utils/              # 工具库 (Logger, ATR)
├── config.yaml             # 配置文件
├── docker-compose.yml      # Docker Compose
├── Dockerfile              # 构建文件
└── go.mod                  # 依赖管理
```

## 快速开始

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
# 1. 安装依赖
go mod tidy

# 2. 编辑配置
vim config.yaml

# 3. 运行
go run cmd/bot/main.go
```

## 配置说明

### config.yaml

```yaml
exchange:
  symbol: "HYPEUSDC"       # 交易对
  private_key: ""          # Lighter 私钥 (hex格式)
  chain_id: 1              # Lighter 链ID
  api_url: "https://api.lighter.xyz"  # Lighter API地址
  account_index: 1         # 账户索引
  api_key_index: 0         # API密钥索引

strategy:
  max_safety_orders: 9     # 最大加仓层数 (Fibonacci)
  atr_period: 14           # ATR 周期

storage:
  sqlite_path: "bot.db"    # SQLite 数据库路径
  redis_addr: "localhost:6379"
  redis_pass: ""
  redis_db: 0

log:
  level: "info"            # 日志级别: debug, info, warn, error
```

### 环境变量

支持通过环境变量覆盖配置，前缀为 `MARTIN_`：

```bash
export MARTIN_EXCHANGE_SYMBOL="HYPEUSDC"
export MARTIN_EXCHANGE_PRIVATE_KEY="your_private_key"
export MARTIN_EXCHANGE_CHAIN_ID="1"
export MARTIN_EXCHANGE_API_URL="https://api.lighter.xyz"
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
2. **网格挂单**: 根据 Fibonacci 序列计算各层加仓数量，按ATR 距离递进挂单
   - Level 1-2: 30m ATR
   - Level 3-4: 1h / 2h ATR
   - Level 5-9: 4h / 6h / 8h / 12h / 1d ATR
3. **止盈**: 基于 15m ATR 设置止盈价位

### Fibonacci 加仓倍数

| 层数 | 倍数 | 数量 (假设底仓=1) |
|------|------|-------------------|
| 底仓 | 1    | 1                 |
| 1    | 1    | 1                 |
| 2    | 2    | 2                 |
| 3    | 3    | 3                 |
| 4    | 5    | 5                 |
| 5    | 8    | 8                 |
| 6    | 13   | 13                |
| 7    | 21   | 21                |
| 8    | 34   | 34                |
| 9    | 55   | 55                |

## 并发安全机制

为防止并发执行导致的问题，已实现以下保护：

### 1. 防重入锁

```go
// placeGridOrders 防并发
if !s.gridMu.TryLock() {
    s.gridSkipCount++  // 监控计数
    return
}
defer s.gridMu.Unlock()

// updateTP 防并发
if !s.tpMu.TryLock() {
    s.tpSkipCount++    // 监控计数
    return
}
defer s.tpMu.Unlock()
```

### 2. 状态原子操作

```go
// handleTick: 状态检查与网络请求分离
s.mu.Lock()
if s.currentState != StateIdle {
    s.mu.Unlock()
    return
}
s.currentState = StatePlacingGrid
s.mu.Unlock()

// 网络请求在锁外执行，避免阻塞
if err := s.enterLong(price); err != nil {
    s.mu.Lock()
    s.currentState = StateIdle  // 失败恢复
    s.mu.Unlock()
}
```

### 3. 执行价格优化

从订单成交事件直接获取执行价格，避免 Position API 的竞态条件：

```go
// 从 WebSocket 事件获取 (推荐)
execPrice, _ := strconv.ParseFloat(order.AveragePrice, 64)
go s.placeGridOrders(execPrice)
```

## 监控指标

### 日志关键字段

| 字段 | 说明 |
|------|------|
| `skip_count` | 因并发冲突跳过的次数 |
| `entryPrice` | 入场价格 |
| `ATR30m` | 30分钟 ATR |
| `UnitQty` | 单位数量 |

### 示例日志

```json
{"level":"info","msg":"Order Update Received","id":123456,"status":"FILLED","type":"LIMIT"}
{"level":"info","msg":"Using execution price from order event","entryPrice":39.638}
{"level":"warn","msg":"placeGridOrders skipped: already running","skip_count":5}
{"level":"warn","msg":"updateTP skipped: already running","skip_count":12}
```

## 风险提示

-马丁格尔策略在单边行情中风险极高
- 建议设置止损或限制最大持仓
- 请确保已正确配置 Lighter API 私钥
- 建议先小额测试验证策略

## 技术栈

| 组件 | 技术 |
|------|------|
| 语言 | Go 1.21+ |
| 交易所 | Lighter HTTP |
| 存储 | SQLite / Redis |
| 配置 | Viper |
| 日志 | Zap (结构化) |
| 指标 | go-talib (ATR) |

## 开发

```bash
# 运行测试
go test ./...

# 构建
go build -o bot cmd/bot/main.go

# 代码检查
go vet ./...
```

## License

MIT License