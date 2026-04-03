# MartinStrategy 使用指南

## 目录

1. [安装](#安装)
2. [配置](#配置)
3. [运行](#运行)
4. [监控](#监控)
5. [故障排除](#故障排除)

## 安装

### 1. 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. 克隆项目

```bash
git clone <your-repo-url>
cd MartinStrategy
```

### 3. 构建项目

```bash
# 开发构建
cargo build

# 发布构建 (优化性能)
cargo build --release
```

## 配置

### 1. 创建配置文件

```bash
cp config.yaml config.local.yaml
```

### 2. 编辑配置

编辑 `config.local.yaml`：

```yaml
exchange:
  symbol: "HYPEUSDC"                  # 交易对
  private_key: "YOUR_PRIVATE_KEY_HERE" # ⚠️ 重要: 填入你的 Lighter 私钥
  chain_id: 1                          # 链ID (1=主网)
  api_url: "https://mainnet.zklighter.elliot.ai"
  account_index: 1                     # 账户索引
  api_key_index: 0                     # API密钥索引
  market_index: 0                      # 市场索引

strategy:
  max_safety_orders: 9
  base_qty: 0.5                        # 底仓数量
  safety_qtys: [0.5, 0.5, 1.0, 1.5, 2.5, 4.0, 6.5, 10.5, 17.0]
  atr_period: 14

storage:
  sqlite_path: "bot.db"
  redis_addr: "localhost:6379"
  redis_pass: ""
  redis_db: 0

log:
  level: "info"                        # debug/info/warn/error
```

### 3. 环境变量 (可选)

通过环境变量覆盖配置：

```bash
export MARTIN_EXCHANGE_PRIVATE_KEY="your_private_key"
export MARTIN_EXCHANGE_SYMBOL="HYPEUSDC"
export MARTIN_LOG_LEVEL="debug"
```

## 运行

### 本地运行

```bash
# 使用默认配置 (config.yaml)
cargo run --release

# 使用自定义配置
MARTIN_CONFIG=config.local.yaml cargo run --release
```

### Docker 运行

```bash
# 构建镜像
docker build -t martin-strategy .

# 运行
docker run -v $(pwd)/config.yaml:/app/config.yaml martin-strategy

# 或使用 docker-compose
docker-compose up -d
```

## 监控

### 查看日志

```bash
# 本地运行
cargo run --release 2>&1 | tee bot.log

# Docker
docker-compose logs -f

# 查看特定级别日志
RUST_LOG=martin_strategy=debug cargo run --release
```

### 日志级别

通过 `RUST_LOG` 环境变量控制：

```bash
# 仅显示错误
RUST_LOG=error cargo run --release

# 显示 debug 及以上
RUST_LOG=debug cargo run --release

# 详细追踪
RUST_LOG=trace cargo run --release
```

### 关键日志指标

| 日志消息 | 含义 |
|----------|------|
| `Entering Long Position` | 开始开仓流程 |
| `Placing Base Order` | 正在下底仓市价单 |
| `Buy Order Filled` | 买单已成交 |
| `Placing Grid Orders` | 正在挂网格加仓单 |
| `Placing Safety Order` | 正在挂某一层加仓单 |
| `Updating TP` | 正在更新止盈单 |
| `Sell Order Filled` | 卖单已成交 (止盈或平仓) |
| `State Synced` | 状态同步完成 |
| `skip_count` | 防重入跳过的次数 |

## 故障排除

### 1. 编译错误

**问题**: `cargo build` 失败

**解决**:
```bash
# 更新工具链
rustup update

# 清理并重新构建
cargo clean
cargo build --release
```

### 2. 连接错误

**问题**: 无法连接到 Lighter API

**解决**:
- 检查 `api_url` 配置
- 检查网络连接
- 确认 API 服务状态

### 3. 签名错误

**问题**: `Invalid signature` 或 `nonce` 错误

**解决**:
- 确认 `private_key` 正确 (hex 格式)
- 确认 `account_index` 和 `api_key_index` 正确
- SDK 会自动重试 nonce 错误

### 4. 数据库错误

**问题**: SQLite 权限错误

**解决**:
```bash
# 确保目录有写权限
chmod 755 .

# 或更改数据库路径
mkdir -p ~/.martin
cp config.yaml config.local.yaml
# 编辑 sqlite_path: "~/.martin/bot.db"
```

### 5. Redis 连接失败

**问题**: 无法连接到 Redis

**解决**:
- Redis 是可选的，可以留空 `redis_addr`
- 或启动本地 Redis: `docker run -d -p 6379:6379 redis`

### 6. 程序崩溃

**问题**: 程序意外退出

**解决**:
```bash
# 查看详细日志
RUST_LOG=debug RUST_BACKTRACE=1 cargo run --release 2>&1 | tee crash.log
```

## 进阶配置

### 自定义网格参数

```yaml
strategy:
  max_safety_orders: 5           # 减少层数降低风险
  base_qty: 0.1                  # 减小底仓
  safety_qtys: [0.1, 0.2, 0.3, 0.5, 0.8]  # 自定义加仓量
  atr_period: 10                 # 更敏感的 ATR
```

### 多实例部署

使用 Redis 锁确保单实例运行：

```bash
# 实例 1
MARTIN_EXCHANGE_ACCOUNT_INDEX=1 cargo run --release

# 实例 2
MARTIN_EXCHANGE_ACCOUNT_INDEX=2 cargo run --release
```

## 安全建议

1. **私钥管理**
   - 使用环境变量或配置文件，不要硬编码
   - 设置文件权限: `chmod 600 config.yaml`
   - 使用 `.gitignore` 避免提交密钥

2. **资金安全**
   - 先用小额测试
   - 设置最大加仓层数
   - 定期检查持仓

3. **系统安全**
   - 及时更新依赖: `cargo update`
   - 使用防火墙限制出站连接
   - 定期备份 SQLite 数据库

## 获取帮助

- 查看日志: `RUST_LOG=debug cargo run`
- 运行测试: `cargo test`
- 检查配置: `cargo run -- --dry-run` (待实现)

## 更新日志

### v0.1.0 (2024-04-03)

- 从 Go 迁移到 Rust
- 集成 Lighter Rust SDK
- 实现完整马丁格尔策略
- 支持 SQLite + Redis 存储
- 异步事件驱动架构
