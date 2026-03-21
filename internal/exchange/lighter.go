package exchange

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"

	"github.com/elliottech/lighter-go/client"
	"github.com/elliottech/lighter-go/types"
	"github.com/elliottech/lighter-go/types/txtypes"
	"github.com/uykb/MartinStrategy/internal/config"
	"github.com/uykb/MartinStrategy/internal/core"
	"github.com/uykb/MartinStrategy/internal/utils"
	"go.uber.org/zap"
)

// MinimalHTTPClient implements client.MinimalHTTPClient interface
type MinimalHTTPClient struct {
	baseURL    string
	httpClient *http.Client
}

func NewMinimalHTTPClient(baseURL string) *MinimalHTTPClient {
	return &MinimalHTTPClient{
		baseURL:    baseURL,
		httpClient: &http.Client{Timeout: 30 * time.Second},
	}
}

func (c *MinimalHTTPClient) Do(req *http.Request) (*http.Response, error) {
	return c.httpClient.Do(req)
}

func (c *MinimalHTTPClient) GetNextNonce(accountIndex int64, apiKeyIndex uint8) (int64, error) {
	url := fmt.Sprintf("%s/api/v1/nextNonce?account_index=%d&api_key_index=%d", c.baseURL, accountIndex, apiKeyIndex)
	resp, err := c.httpClient.Get(url)
	if err != nil {
		return 0, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return 0, fmt.Errorf("failed to get nonce: %s - %s", resp.Status, string(body))
	}

	var result struct {
		Nonce int64 `json:"nonce"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return 0, err
	}
	return result.Nonce, nil
}

func (c *MinimalHTTPClient) GetApiKey(accountIndex int64, apiKeyIndex uint8) (string, error) {
	url := fmt.Sprintf("%s/api/v1/apikeys?account_index=%d&api_key_index=%d", c.baseURL, accountIndex, apiKeyIndex)
	resp, err := c.httpClient.Get(url)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("failed to get api key: %s - %s", resp.Status, string(body))
	}

	var result struct {
		ApiKeys []struct {
			PublicKey string `json:"public_key"`
		} `json:"api_keys"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", err
	}

	if len(result.ApiKeys) == 0 {
		return "", fmt.Errorf("no api keys returned")
	}
	return result.ApiKeys[0].PublicKey, nil
}

// Position represents a trading position
type Position struct {
	Symbol           string  `json:"symbol"`
	PositionAmt      string  `json:"position_amt"`
	EntryPrice       string  `json:"entry_price"`
	UnrealizedProfit string  `json:"unrealized_profit"`
	Leverage         string  `json:"leverage"`
}

// Order represents an order
type Order struct {
	OrderID       int64   `json:"order_id"`
	Symbol        string  `json:"symbol"`
	Side          string  `json:"side"` // BUY or SELL
	Type          string  `json:"type"` // MARKET or LIMIT
	Price         string  `json:"price"`
	Quantity      string  `json:"quantity"`
	ExecutedQty   string  `json:"executed_qty"`
	Status        string  `json:"status"`
	Time          int64   `json:"time"`
}

// Kline represents a candlestick
type Kline struct {
	OpenTime  int64   `json:"open_time"`
	Open      float64 `json:"open"`
	High      float64 `json:"high"`
	Low       float64 `json:"low"`
	Close     float64 `json:"close"`
	Volume    float64 `json:"volume"`
	CloseTime int64   `json:"close_time"`
}

// ExchangeInfo represents exchange information
type ExchangeInfo struct {
	Symbols []SymbolInfo `json:"symbols"`
}

// SymbolInfo represents symbol information
type SymbolInfo struct {
	Symbol     string `json:"symbol"`
	Status     string `json:"status"`
	BaseAsset  string `json:"base_asset"`
	QuoteAsset string `json:"quote_asset"`
}

// LighterOrderResponse represents the response from Lighter API
type LighterOrderResponse struct {
	OrderID int64  `json:"order_id"`
	Status  string `json:"status"`
	TxHash  string `json:"tx_hash"`
}

// OneUSDC represents 1 USDC in Lighter's 6 decimal format
const OneUSDC = 1000000

// float64ToUSDC converts float64 USDC amount to int64 format (6 decimals)
func float64ToUSDC(amount float64) int64 {
	return int64(amount * OneUSDC)
}

// float64ToPrice converts float64 price to uint32 format
func float64ToPrice(price float64) uint32 {
	return uint32(price * 100) // Assuming 2 decimal places for price
}

// LighterClient wraps the Lighter SDK client
type LighterClient struct {
	client *client.TxClient
	cfg    *config.ExchangeConfig
	bus    *core.EventBus
	http   *MinimalHTTPClient
}

// NewLighterClient creates a new Lighter client
func NewLighterClient(cfg *config.ExchangeConfig, bus *core.EventBus) (*LighterClient, error) {
	httpClient := NewMinimalHTTPClient(cfg.APIURL)

	accountIndex := cfg.AccountIndex
	if accountIndex <= 0 {
		accountIndex = 1
	}
	apiKeyIndex := cfg.APIKeyIndex

	txClient, err := client.CreateClient(httpClient, cfg.PrivateKey, cfg.ChainID, apiKeyIndex, accountIndex)
	if err != nil {
		return nil, fmt.Errorf("failed to create lighter client: %w", err)
	}

	if err := txClient.Check(); err != nil {
		return nil, fmt.Errorf("lighter client check failed: %w", err)
	}

	utils.Logger.Info("Lighter client initialized",
		zap.Int64("account_index", accountIndex),
		zap.Uint8("api_key_index", apiKeyIndex),
		zap.Int16("market_index", cfg.MarketIndex),
	)

	return &LighterClient{
		client: txClient,
		cfg:    cfg,
		bus:    bus,
		http:   httpClient,
	}, nil
}

// StartWS starts WebSocket connections
// Lighter doesn't have native WebSocket, so we use polling
func (lc *LighterClient) StartWS() error {
	utils.Logger.Info("Starting Lighter polling loop (no native WebSocket)")

	// Start polling for order updates and price ticks
	go lc.pollLoop()

	return nil
}

func (lc *LighterClient) pollLoop() {
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		// Poll position updates
		pos, err := lc.GetPosition()
		if err == nil && pos != nil {
			lc.bus.Publish(core.EventPositionUpdate, pos)
		}

		// Poll open orders
		orders, err := lc.GetOpenOrders()
		if err == nil {
			for _, order := range orders {
				lc.bus.Publish(core.EventOrderUpdate, order)
			}
		}
	}
}

func (lc *LighterClient) GetPosition() (*Position, error) {
	url := fmt.Sprintf("%s/api/v1/position?account_index=%d&symbol=%s",
		lc.cfg.APIURL, lc.client.GetAccountIndex(), lc.cfg.Symbol)

	resp, err := lc.http.httpClient.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to get position: %s", resp.Status)
	}

	var position Position
	if err := json.NewDecoder(resp.Body).Decode(&position); err != nil {
		return nil, err
	}

	return &position, nil
}

func (lc *LighterClient) GetSymbol() string {
	return lc.cfg.Symbol
}

func (lc *LighterClient) GetExchangeInfo() (*ExchangeInfo, error) {
	url := fmt.Sprintf("%s/v1/exchange_info", lc.cfg.APIURL)

	resp, err := lc.http.httpClient.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to get exchange info: %s", resp.Status)
	}

	var info ExchangeInfo
	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		return nil, err
	}

	return &info, nil
}

func (lc *LighterClient) PlaceOrder(side string, orderType string, quantity, price float64) (*Order, error) {
	// Prepare transaction options
	opts := &types.TransactOpts{
		ExpiredAt: time.Now().Add(10 * time.Minute).UnixMilli(),
	}

	// Fill default options (gets nonce automatically)
	filledOpts, err := lc.client.FullFillDefaultOps(opts)
	if err != nil {
		return nil, fmt.Errorf("failed to fill opts: %w", err)
	}

	// Convert to Lighter format
	isAsk := uint8(0)
	if side == "SELL" {
		isAsk = 1
	}

	var orderTypeUint uint8
	var timeInForce uint8
	if orderType == "MARKET" {
		orderTypeUint = txtypes.MarketOrder
		timeInForce = txtypes.ImmediateOrCancel
	} else {
		orderTypeUint = txtypes.LimitOrder
		timeInForce = txtypes.GoodTillTime
	}

	// Build order request
	orderReq := &types.CreateOrderTxReq{
		MarketIndex:            lc.cfg.MarketIndex,
		ClientOrderIndex:       time.Now().UnixNano(),
		BaseAmount:             float64ToUSDC(quantity),
		Price:                  float64ToPrice(price),
		IsAsk:                  isAsk,
		Type:                   orderTypeUint,
		TimeInForce:            timeInForce,
		ReduceOnly:             0,
		TriggerPrice:           0,
		OrderExpiry:            opts.ExpiredAt,
		IntegratorAccountIndex: 0,
		IntegratorMakerFee:     0,
		IntegratorTakerFee:     0,
	}

	// Sign the order
	txInfo, err := types.ConstructCreateOrderTx(
		lc.client.GetKeyManager(),
		lc.client.GetChainId(),
		orderReq,
		filledOpts,
	)
	if err != nil {
		return nil, fmt.Errorf("failed to sign order: %w", err)
	}

	utils.Logger.Info("Placing order on Lighter",
		zap.String("symbol", lc.cfg.Symbol),
		zap.String("side", side),
		zap.String("type", orderType),
		zap.Float64("quantity", quantity),
		zap.Float64("price", price),
		zap.String("tx_hash", txInfo.GetTxHash()),
	)

	// TODO: Submit to Lighter API
	// For now return a mock order
	return &Order{
		OrderID:  txInfo.ClientOrderIndex,
		Symbol:   lc.cfg.Symbol,
		Side:     side,
		Type:     orderType,
		Price:    strconv.FormatFloat(price, 'f', -1, 64),
		Quantity: strconv.FormatFloat(quantity, 'f', -1, 64),
		Status:   "NEW",
		Time:     time.Now().UnixMilli(),
	}, nil
}

func (lc *LighterClient) CancelAllOrders() error {
	opts := &types.TransactOpts{
		ExpiredAt: time.Now().Add(10 * time.Minute).UnixMilli(),
	}

	filledOpts, err := lc.client.FullFillDefaultOps(opts)
	if err != nil {
		return fmt.Errorf("failed to fill opts: %w", err)
	}

	// Build cancel all orders request
	cancelReq := &types.CancelAllOrdersTxReq{
		TimeInForce: txtypes.ImmediateCancelAll,
		Time:        opts.ExpiredAt,
	}

	// Sign the transaction
	txInfo, err := types.ConstructL2CancelAllOrdersTx(
		lc.client.GetKeyManager(),
		lc.client.GetChainId(),
		cancelReq,
		filledOpts,
	)
	if err != nil {
		return fmt.Errorf("failed to sign cancel all orders: %w", err)
	}

	utils.Logger.Info("Canceling all orders",
		zap.String("tx_hash", txInfo.GetTxHash()),
	)

	// TODO: Submit to Lighter API
	return nil
}

func (lc *LighterClient) CancelOrder(orderID int64) error {
	opts := &types.TransactOpts{
		ExpiredAt: time.Now().Add(10 * time.Minute).UnixMilli(),
	}

	filledOpts, err := lc.client.FullFillDefaultOps(opts)
	if err != nil {
		return fmt.Errorf("failed to fill opts: %w", err)
	}

	// Build cancel order request
	cancelReq := &types.CancelOrderTxReq{
		MarketIndex: lc.cfg.MarketIndex,
		Index:       orderID,
	}

	// Sign the transaction
	txInfo, err := types.ConstructL2CancelOrderTx(
		lc.client.GetKeyManager(),
		lc.client.GetChainId(),
		cancelReq,
		filledOpts,
	)
	if err != nil {
		return fmt.Errorf("failed to sign cancel order: %w", err)
	}

	utils.Logger.Info("Canceling order",
		zap.Int64("order_id", orderID),
		zap.String("tx_hash", txInfo.GetTxHash()),
	)

	// TODO: Submit to Lighter API
	return nil
}

func (lc *LighterClient) GetOpenOrders() ([]*Order, error) {
	url := fmt.Sprintf("%s/api/v1/open_orders?account_index=%d&symbol=%s",
		lc.cfg.APIURL, lc.client.GetAccountIndex(), lc.cfg.Symbol)

	resp, err := lc.http.httpClient.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to get open orders: %s", resp.Status)
	}

	var orders []*Order
	if err := json.NewDecoder(resp.Body).Decode(&orders); err != nil {
		return nil, err
	}

	return orders, nil
}

func (lc *LighterClient) GetKlines(interval string, limit int) ([]*Kline, error) {
	url := fmt.Sprintf("%s/api/v1/klines?symbol=%s&interval=%s&limit=%d",
		lc.cfg.APIURL, lc.cfg.Symbol, interval, limit)

	resp, err := lc.http.httpClient.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to get klines: %s", resp.Status)
	}

	var klines []*Kline
	if err := json.NewDecoder(resp.Body).Decode(&klines); err != nil {
		return nil, err
	}

	return klines, nil
}
