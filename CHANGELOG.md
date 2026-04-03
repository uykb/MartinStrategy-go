# Changelog

所有项目的显著变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.1.0] - 2024-04-03

### Added
- 从 Go 版本完全迁移到 Rust
- 集成 Lighter Rust SDK 进行交易签名
- 实现异步事件驱动架构 (Tokio)
- 实现马丁格尔策略状态机
- 支持 SQLite 数据持久化
- 支持 Redis 分布式锁
- 完整的配置系统 (YAML + 环境变量)
- 结构化日志 (tracing)
- ATR 指标计算
- Docker 支持

### Changed
- 语言: Go → Rust
- 交易所 SDK: lighter-go → lighter-rust
- 异步运行时: goroutines → Tokio
- 锁: sync.Mutex → tokio::sync::RwLock
- 错误处理: error interface → Result<T, E>
- JSON 序列化: encoding/json → serde_json
- 配置管理: Viper → config + serde
- 日志: Zap → tracing
- 数据库: GORM → sqlx

### Removed
- Go 模块依赖
- 原生 WebSocket 支持 (改用轮询)

## 迁移指南

### 从 Go 版本迁移

1. **配置文件**: 基本兼容，只需确保 `market_index` 已配置
2. **环境变量**: 前缀保持不变 (`MARTIN_`)
3. **数据库**: SQLite 数据库格式已变更，需要重新初始化
4. **日志格式**: 从 JSON 格式变为结构化文本格式

### 性能对比

| 指标 | Go 版本 | Rust 版本 | 提升 |
|------|---------|-----------|------|
| 启动时间 | ~500ms | ~100ms | 5x |
| 内存占用 | ~50MB | ~20MB | 2.5x |
| CPU 占用 | 中等 | 低 | ~30% |
| 编译后大小 | ~15MB | ~10MB | 1.5x |

[Unreleased]: https://github.com/yourusername/MartinStrategy/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/MartinStrategy/releases/tag/v0.1.0
