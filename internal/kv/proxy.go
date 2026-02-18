// Package kv implements a ZAP-to-Valkey/Redis sidecar.
//
// Accepts ZAP connections and translates to Redis RESP protocol.
// Optimized for zero-copy GET/SET/MGET bulk operations.
// Exposes MCP-compatible tools: kv_get, kv_set, kv_mget, kv_cmd.
package kv

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"github.com/luxfi/zap"

	kv "github.com/hanzoai/kv-go/v9"
)

const MsgTypeKV uint16 = 301

const (
	fieldPath = 4
	fieldBody = 12
	respStatus  = 0
	respBody    = 4
	respHeaders = 8
)

type Config struct {
	NodeID      string
	Port        int
	ServiceType string
	Addr        string
	Password    string
	DB          int
}

type Proxy struct {
	node   *zap.Node
	client *kv.Client
	logger *slog.Logger
}

func New(ctx context.Context, logger *slog.Logger, cfg Config) (*Proxy, error) {
	client := kv.NewClient(&kv.Options{
		Addr:     cfg.Addr,
		Password: cfg.Password,
		DB:       cfg.DB,
	})
	// Retry ping — Redis may still be loading data (AOF/RDB replay)
	for i := 0; i < 30; i++ {
		if err := client.Ping(ctx).Err(); err == nil {
			break
		} else if i == 29 {
			client.Close()
			return nil, fmt.Errorf("kv: ping failed after retries: %w", err)
		}
		logger.Info("kv: waiting for backend", "attempt", i+1, "addr", cfg.Addr)
		time.Sleep(2 * time.Second)
	}

	p := &Proxy{client: client, logger: logger}

	node := zap.NewNode(zap.NodeConfig{
		NodeID:      cfg.NodeID,
		ServiceType: cfg.ServiceType,
		Port:        cfg.Port,
		Logger:      logger,
	})

	node.Handle(MsgTypeKV, func(_ context.Context, _ string, msg *zap.Message) (*zap.Message, error) {
		return p.handle(ctx, msg), nil
	})

	if err := node.Start(); err != nil {
		client.Close()
		return nil, fmt.Errorf("kv: node start failed: %w", err)
	}

	p.node = node
	logger.Info("kv sidecar ready", "addr", cfg.Addr)
	return p, nil
}

func (p *Proxy) Stop() {
	if p.node != nil {
		p.node.Stop()
	}
	if p.client != nil {
		p.client.Close()
	}
}

func (p *Proxy) handle(ctx context.Context, msg *zap.Message) *zap.Message {
	root := msg.Root()
	path := root.Text(fieldPath)
	body := root.Bytes(fieldBody)

	switch path {
	case "/health":
		return p.health(ctx)
	case "/get":
		return p.get(ctx, body)
	case "/set":
		return p.set(ctx, body)
	case "/mget":
		return p.mget(ctx, body)
	case "/cmd":
		return p.cmd(ctx, body)
	default:
		if len(body) > 0 {
			return p.cmd(ctx, body)
		}
		return respond(http.StatusNotFound, map[string]string{"error": "unknown: " + path})
	}
}

type kvCmd struct {
	Cmd  string   `json:"cmd"`
	Args []string `json:"args"`
}

func (p *Proxy) cmd(ctx context.Context, body []byte) *zap.Message {
	var req kvCmd
	if err := json.Unmarshal(body, &req); err != nil {
		parts := strings.Fields(string(body))
		if len(parts) == 0 {
			return respond(http.StatusBadRequest, map[string]string{"error": "empty command"})
		}
		req.Cmd = parts[0]
		req.Args = parts[1:]
	}

	args := make([]interface{}, len(req.Args)+1)
	args[0] = req.Cmd
	for i, a := range req.Args {
		args[i+1] = a
	}

	result, err := p.client.Do(ctx, args...).Result()
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]interface{}{"result": result})
}

func (p *Proxy) get(ctx context.Context, body []byte) *zap.Message {
	var req struct{ Key string `json:"key"` }
	if err := json.Unmarshal(body, &req); err != nil {
		req.Key = string(body)
	}

	val, err := p.client.Get(ctx, req.Key).Result()
	if err == kv.Nil {
		return respond(http.StatusOK, map[string]interface{}{"value": nil})
	}
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]interface{}{"value": val})
}

func (p *Proxy) set(ctx context.Context, body []byte) *zap.Message {
	var req struct {
		Key   string `json:"key"`
		Value string `json:"value"`
	}
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	if err := p.client.Set(ctx, req.Key, req.Value, 0).Err(); err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]string{"status": "OK"})
}

func (p *Proxy) mget(ctx context.Context, body []byte) *zap.Message {
	var req struct{ Keys []string `json:"keys"` }
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	vals, err := p.client.MGet(ctx, req.Keys...).Result()
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]interface{}{"values": vals})
}

func (p *Proxy) health(ctx context.Context) *zap.Message {
	if err := p.client.Ping(ctx).Err(); err != nil {
		return respond(http.StatusServiceUnavailable, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]string{"status": "ok", "service": "hanzo-kv"})
}

func respond(status int, data interface{}) *zap.Message {
	b := zap.NewBuilder(4096)
	ob := b.StartObject(12)
	ob.SetUint32(respStatus, uint32(status))
	body, _ := json.Marshal(data)
	ob.SetBytes(respBody, body)
	ob.SetBytes(respHeaders, []byte(`{"Content-Type":["application/json"]}`))
	ob.FinishAsRoot()
	msg, _ := zap.Parse(b.Finish())
	return msg
}
