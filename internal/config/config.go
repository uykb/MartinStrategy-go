package config

import (
	"strings"

	"github.com/spf13/viper"
)

type Config struct {
	Exchange ExchangeConfig `mapstructure:"exchange"`
	Strategy StrategyConfig `mapstructure:"strategy"`
	Storage  StorageConfig  `mapstructure:"storage"`
	Log      LogConfig      `mapstructure:"log"`
}

type ExchangeConfig struct {
	// Lighter Exchange Configuration
	Symbol     string `mapstructure:"symbol"`
	PrivateKey string `mapstructure:"private_key"`
	ChainID    uint32 `mapstructure:"chain_id"`
	APIURL     string `mapstructure:"api_url"`

	// Account Settings
	AccountIndex int64 `mapstructure:"account_index"`
	APIKeyIndex  uint8 `mapstructure:"api_key_index"`
	MarketIndex  int16 `mapstructure:"market_index"`
}

type StrategyConfig struct {
	// Grid Settings
	MaxSafetyOrders int `mapstructure:"max_safety_orders"`

	// Fixed Quantity Mode
	BaseQty        float64   `mapstructure:"base_qty"`        // 底仓数量 (e.g., 0.5 HYPE)
	SafetyQtys     []float64 `mapstructure:"safety_qtys"`     // 每层加仓数量列表

	// ATR Settings
	AtrPeriod int `mapstructure:"atr_period"`
}

type StorageConfig struct {
	SqlitePath string `mapstructure:"sqlite_path"`
	RedisAddr  string `mapstructure:"redis_addr"`
	RedisPass  string `mapstructure:"redis_pass"`
	RedisDB    int    `mapstructure:"redis_db"`
}

type LogConfig struct {
	Level string `mapstructure:"level"`
}

func LoadConfig(path string) (*Config, error) {
	viper.SetConfigFile(path)
	viper.SetConfigType("yaml")

	// Environment variables
	viper.SetEnvPrefix("MARTIN")
	viper.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	viper.AutomaticEnv()

	if err := viper.ReadInConfig(); err != nil {
		return nil, err
	}

	var cfg Config
	if err := viper.Unmarshal(&cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}
