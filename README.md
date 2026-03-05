# MartinStrategy 交易机器人

基于 **Python + Go** 混合架构的高性能马丁策略交易机器人，专为 **HYPEUSDT** 永续合约设计。

该项目结合了 Go 语言在 WebSocket 长连接与高并发处理上的优势，以及 Python 在数据分析与策略逻辑上的灵活性。

## 核心特性

*   **混合架构 (Hybrid Architecture)**:
    *   **Go 执行层**: 负责维护与币安 (Binance Futures) 的 WebSocket 实时行情连接，以及订单的高速发送与撤单，确保极低的延迟和极高的稳定性。
    *   **Python 策略层**: 负责复杂的马丁策略逻辑运算、ATR 指标计算 (pandas-ta) 以及仓位管理。
*   **斐波那契仓位管理**: 严格按照斐波那契数列 (1, 1, 2, 3, 5...) 进行加仓，科学分配资金。
*   **动态 ATR 网格**: 网格间距不再固定，而是根据市场波动率 (15m ATR) 实时动态调整，通过自定义倍数 (1x, 2x, 4x...) 适应极端行情。
*   **动态追踪止盈**: 止盈点位随加仓均价和市场波动 (ATR) 实时动态调整，确保在回调中快速获利离场。
*   **Docker 化部署**: 提供开箱即用的 Docker 镜像，支持一键部署到任何支持 Docker 的环境。

## 快速开始 (Docker)

本项目已构建并发布至 GitHub Container Registry (GHCR)，你可以直接拉取并运行。

### 1. 准备 API Key
请确保你已在币安合约账户中创建了 API Key，并开启了 **"允许合约交易" (Enable Futures)** 权限。

### 2. 拉取镜像
```bash
docker pull ghcr.io/uykb/martinstrategy:main
```

### 3. 启动机器人
使用以下命令启动容器，请替换 `<YOUR_API_KEY>` 和 `<YOUR_SECRET>` 为你的真实密钥。

```bash
docker run -d \
  --name martin-bot \
  --restart always \
  -e BINANCE_API_KEY="<YOUR_API_KEY>" \
  -e BINANCE_API_SECRET="<YOUR_SECRET>" \
  -e SYMBOL="HYPEUSDT" \
  ghcr.io/uykb/martinstrategy:main
```

### 环境变量配置

| 变量名 | 必填 | 默认值 | 描述 |
| :--- | :---: | :--- | :--- |
| `BINANCE_API_KEY` | ✅ | - | 币安 API Key |
| `BINANCE_API_SECRET` | ✅ | - | 币安 API Secret |
| `SYMBOL` | ❌ | `HYPEUSDT` | 交易对名称 (目前针对 HYPE 优化) |
| `PORT` | ❌ | `8080` | Go 服务内部监听端口 (无需映射) |

## 策略逻辑详解

### 1. 入场信号
*   **首单 (Base Order)**: 机器人启动或上一轮止盈结束后，自动获取当前 15分钟 K线计算 ATR，并立即市价买入首单。

### 2. 加仓逻辑 (Safety Orders)
*   **触发条件**: 价格下跌超过动态网格间距。
*   **网格间距**: 基于当前 **15m ATR** 值 × 自定义倍数列表。
    *   第 1-4 单: 1.0 × ATR
    *   第 5-6 单: 2.0 × ATR
    *   第 7-8 单: 4.0 × ATR
    *   ...以此类推，适应深度回调。
*   **加仓数量**: 基于 **斐波那契数列** (1, 1, 2, 3, 5, 8...) 计算倍投量。

### 3. 止盈逻辑 (Take Profit)
*   **动态止盈**: 每次加仓成交后，止盈价格会立即重新计算。
*   **公式**: `目标止盈价 = 当前持仓均价 + (1.0 × 当前ATR)`
*   **全平模式**: 一旦触及止盈价，平掉所有持仓，并撤销所有未成交的加仓挂单，随后进入下一轮循环。

## 本地开发与构建

如果你想在本地修改代码或进行调试：

### 前置要求
*   Go 1.21+
*   Python 3.12+
*   Docker (可选)

### 本地运行
1.  **启动 Go 执行层**:
    ```bash
    cd go
    export BINANCE_API_KEY="your_key"
    export BINANCE_API_SECRET="your_secret"
    go run main.go
    ```
2.  **启动 Python 策略层** (在另一个终端):
    ```bash
    cd python
    pip install -r requirements.txt
    export SYMBOL="HYPEUSDT"
    python main.py
    ```
    *(注意: `python/main.py` 默认会尝试启动 Go 子进程。在开发模式下，你可能需要修改 `main.py` 以连接到独立运行的 Go 服务，或者直接让它启动编译好的 Go 二进制文件)*

## 免责声明

本软件仅供学习和研究使用，不构成任何投资建议。加密货币交易风险极高，使用本策略产生的任何盈亏均由使用者自行承担。请务必在实盘前进行充分的测试。
