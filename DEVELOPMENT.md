# 开发指南

## 项目结构

```
src/
├── main.rs           # 程序入口
├── lib.rs            # 库入口
├── config/           # 配置模块
├── core/             # 事件总线
├── exchange/         # 交易所适配
├── storage/          # 数据存储
├── strategy/         # 策略逻辑
└── utils/            # 工具函数
```

## 添加新功能

### 1. 添加新的事件类型

编辑 `src/core/mod.rs`:

```rust
pub enum EventType {
    Tick,
    OrderUpdate,
    PositionUpdate,
    // 添加新事件
    PriceAlert,
}
```

### 2. 添加新的策略状态

编辑 `src/strategy/mod.rs`:

```rust
pub enum State {
    Idle,
    InPosition,
    PlacingGrid,
    // 添加新状态
    EmergencyExit,
}
```

### 3. 添加新的交易所 API

编辑 `src/exchange/mod.rs`:

```rust
impl LighterExchange {
    pub async fn get_balance(&self) -> Result<Balance> {
        // 实现 API 调用
    }
}
```

## 测试

### 运行单元测试

```bash
# 所有测试
cargo test

# 特定模块
cargo test config::
cargo test utils::

# 显示输出
cargo test -- --nocapture
```

### 编写测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_strategy() {
        // 测试代码
    }
}
```

## 调试技巧

### 1. 使用 tracing

```rust
use tracing::{debug, info, warn, error};

info!("Processing order: id={}", order_id);
debug!("Detailed data: {:?}", data);
```

### 2. 日志过滤

```bash
# 仅查看策略日志
RUST_LOG=martin_strategy::strategy=debug cargo run

# 排除某些模块
RUST_LOG=martin_strategy,info cargo run
```

### 3. 使用 dbg! 宏

```rust
let result = some_function();
dbg!(&result);
```

## 性能优化

### 1. 发布构建

```bash
cargo build --release
```

### 2. 分析性能

```bash
# 安装 flamegraph
cargo install flamegraph

# 生成火焰图
cargo flamegraph --release
```

### 3. 优化依赖

编辑 `Cargo.toml`:

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

## 代码规范

### 1. 格式化

```bash
cargo fmt
```

### 2. Lint

```bash
cargo clippy -- -D warnings
```

### 3. 文档

```bash
# 生成文档
cargo doc --open

# 检查文档链接
cargo doc --document-private-items
```

## 发布

### 1. 版本更新

编辑 `Cargo.toml`:

```toml
[package]
version = "0.1.1"
```

### 2. 构建发布

```bash
cargo build --release

# 验证二进制
./target/release/bot --version
```

### 3. Docker 镜像

```bash
docker build -t martin-strategy:v0.1.1 .
docker tag martin-strategy:v0.1.1 martin-strategy:latest
```

## 贡献指南

1. Fork 项目
2. 创建分支: `git checkout -b feature/your-feature`
3. 提交更改: `git commit -am 'Add feature'`
4. 推送分支: `git push origin feature/your-feature`
5. 创建 Pull Request

## 参考资料

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Tokio 文档](https://tokio.rs/tokio/tutorial)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
