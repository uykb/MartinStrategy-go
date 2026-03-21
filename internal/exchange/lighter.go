package exchange

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"

	"github.com/elliottech/lighter-go/client"
	"github.com/elliottech/lighter-go/types"
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
	url := fmt.Sprintf("%s/v1/nonce?account_index=%d&api_key_index=%d", c.baseURL, accountIndex, apiKeyIndex)
	resp, err := c.httpClient.Get(url)
	if err != nil {
		return 0, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return 0, fmt.Errorf("failed to get nonce: %s", resp.Status)
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
	url := fmt.Sprintf("%s/v1/api_key?account_index=%d&api_key_index=%d", c.baseURL, accountIndex, apiKeyIndex)
	resp, err := c.httpClient.Get(url)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("failed to get api key: %s", resp.Status)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", err
	}
	return string(body), nil
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

type LighterClient struct {
	client *client.TxClient
	cfg    *config.ExchangeConfig
	bus    *core.EventBus
	http   *MinimalHTTPClient
}

func NewLighterClient(cfg *config.ExchangeConfig, bus *core.EventBus) (*LighterClient, error) {
	httpClient := NewMinimalHTTPClient(cfg.APIURL)

	// Create lighter client
	// Default values: accountIndex=1, apiKeyIndex=0, chainId from config
	accountIndex := cfg.AccountIndex
	if accountIndex <= 0 {
		accountIndex = 1
	}
	apiKeyIndex := cfg.APIKeyIndex

	txClient, err := client.CreateClient(httpClient, cfg.PrivateKey, cfg.ChainID, apiKeyIndex, accountIndex)
	if err != nil {
		return nil, fmt.Errorf("failed to create lighter client: %w", err)
	}

	// Check client
	if err := txClient.Check(); err != nil {
		return nil, fmt.Errorf("lighter client check failed: %w", err)
	}

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
	url := fmt.Sprintf("%s/v1/position?account_index=%d&symbol=%s",
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

	// For Lighter, we need to use the HTTP client to submit the signed transaction
	// The actual implementation depends on Lighter's API
	// This is a simplified version

	orderReq := map[string]interface{}{
		"symbol":   lc.cfg.Symbol,
		"side":     side,
		"type":     orderType,
		"quantity": strconv.FormatFloat(quantity, 'f', -1, 64),
		"price":    strconv.FormatFloat(price, 'f', -1, 64),
		"nonce":    *filledOpts.Nonce,
	}

	// Submit order via HTTP API
	url := fmt.Sprintf("%s/v1/order", lc.cfg.APIURL)
	_ = url // URL will be used when implementing actual API call
	_ = orderReq // Request body will be used when implementing actual API call

	// TODO: Implement actual Lighter API integration
	// - Sign the request with lighter signer
	// - Submit via HTTP client
	// - Parse response

	utils.Logger.Info("Placing order on Lighter",
		zap.String("symbol", lc.cfg.Symbol),
		zap.String("side", side),
		zap.String("type", orderType),
		zap.Float64("quantity", quantity),
		zap.Float64("price", price),
	)

	// Return a mock order for now
	return &Order{
		OrderID:  time.Now().UnixNano(),
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

	url := fmt.Sprintf("%s/v1/cancel_all_orders?account_index=%d&symbol=%s&nonce=%d",
		lc.cfg.APIURL, lc.client.GetAccountIndex(), lc.cfg.Symbol, *filledOpts.Nonce)

	req, err := http.NewRequest("DELETE", url, nil)
	if err != nil {
		return err
	}

	resp, err := lc.http.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to cancel all orders: %s", resp.Status)
	}

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

	url := fmt.Sprintf("%s/v1/order?account_index=%d&order_id=%d&nonce=%d",
		lc.cfg.APIURL, lc.client.GetAccountIndex(), orderID, *filledOpts.Nonce)

	req, err := http.NewRequest("DELETE", url, nil)
	if err != nil {
		return err
	}

	resp, err := lc.http.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to cancel order: %s", resp.Status)
	}

	return nil
}

func (lc *LighterClient) GetOpenOrders() ([]*Order, error) {
	url := fmt.Sprintf("%s/v1/open_orders?account_index=%d&symbol=%s",
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
	url := fmt.Sprintf("%s/v1/klines?symbol=%s&interval=%s&limit=%d",
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
