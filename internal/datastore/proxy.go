// Package datastore implements a ZAP-to-ClickHouse sidecar.
//
// Accepts ZAP connections and translates to ClickHouse HTTP API.
// Optimized for bulk insert of AI telemetry and traces.
// Exposes MCP-compatible tools: datastore_query, datastore_insert.
package datastore

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"

	"github.com/luxfi/zap"
)

const MsgTypeDatastore uint16 = 302

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
	User        string
	Password    string
	Database    string
}

type Proxy struct {
	node     *zap.Node
	http     *http.Client
	addr     string
	user     string
	password string
	database string
	logger   *slog.Logger
}

func New(ctx context.Context, logger *slog.Logger, cfg Config) (*Proxy, error) {
	if cfg.Database == "" {
		cfg.Database = "default"
	}
	if cfg.User == "" {
		cfg.User = "default"
	}

	p := &Proxy{
		http:     &http.Client{},
		addr:     cfg.Addr,
		user:     cfg.User,
		password: cfg.Password,
		database: cfg.Database,
		logger:   logger,
	}

	// Verify ClickHouse
	resp, err := p.http.Get(fmt.Sprintf("http://%s/ping", cfg.Addr))
	if err != nil {
		return nil, fmt.Errorf("datastore: ping failed: %w", err)
	}
	resp.Body.Close()

	node := zap.NewNode(zap.NodeConfig{
		NodeID:      cfg.NodeID,
		ServiceType: cfg.ServiceType,
		Port:        cfg.Port,
		Logger:      logger,
	})

	node.Handle(MsgTypeDatastore, func(_ context.Context, _ string, msg *zap.Message) (*zap.Message, error) {
		return p.handle(msg), nil
	})

	if err := node.Start(); err != nil {
		return nil, fmt.Errorf("datastore: node start failed: %w", err)
	}

	p.node = node
	logger.Info("datastore sidecar ready", "addr", cfg.Addr, "db", cfg.Database)
	return p, nil
}

func (p *Proxy) Stop() {
	if p.node != nil {
		p.node.Stop()
	}
}

func (p *Proxy) handle(msg *zap.Message) *zap.Message {
	root := msg.Root()
	path := root.Text(fieldPath)
	body := root.Bytes(fieldBody)

	switch path {
	case "/health":
		return p.health()
	case "/query":
		return p.query(body)
	case "/insert":
		return p.insert(body)
	default:
		if len(body) > 0 {
			return p.query(body)
		}
		return respond(http.StatusNotFound, map[string]string{"error": "unknown: " + path})
	}
}

type dsQuery struct {
	SQL      string `json:"sql"`
	Database string `json:"database,omitempty"`
	Format   string `json:"format,omitempty"`
}

func (p *Proxy) query(body []byte) *zap.Message {
	var req dsQuery
	if err := json.Unmarshal(body, &req); err != nil {
		req.SQL = string(body)
	}
	if req.Database == "" {
		req.Database = p.database
	}
	if req.Format == "" {
		req.Format = "JSONEachRow"
	}

	sql := req.SQL + " FORMAT " + req.Format
	url := fmt.Sprintf("http://%s/?database=%s", p.addr, req.Database)

	httpReq, _ := http.NewRequest("POST", url, bytes.NewReader([]byte(sql)))
	httpReq.Header.Set("X-ClickHouse-User", p.user)
	if p.password != "" {
		httpReq.Header.Set("X-ClickHouse-Key", p.password)
	}

	resp, err := p.http.Do(httpReq)
	if err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}
	defer resp.Body.Close()
	respBody, _ := io.ReadAll(resp.Body)

	if resp.StatusCode != http.StatusOK {
		return respond(resp.StatusCode, map[string]string{"error": string(respBody)})
	}
	return respondRaw(http.StatusOK, respBody)
}

type insertReq struct {
	Table    string                   `json:"table"`
	Database string                   `json:"database,omitempty"`
	Rows     []map[string]interface{} `json:"rows"`
}

func (p *Proxy) insert(body []byte) *zap.Message {
	var req insertReq
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}
	if req.Database == "" {
		req.Database = p.database
	}

	var buf bytes.Buffer
	for _, row := range req.Rows {
		line, _ := json.Marshal(row)
		buf.Write(line)
		buf.WriteByte('\n')
	}

	sql := fmt.Sprintf("INSERT INTO %s FORMAT JSONEachRow", req.Table)
	url := fmt.Sprintf("http://%s/?database=%s&query=%s", p.addr, req.Database, sql)

	httpReq, _ := http.NewRequest("POST", url, &buf)
	httpReq.Header.Set("X-ClickHouse-User", p.user)
	if p.password != "" {
		httpReq.Header.Set("X-ClickHouse-Key", p.password)
	}

	resp, err := p.http.Do(httpReq)
	if err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}
	defer resp.Body.Close()
	rb, _ := io.ReadAll(resp.Body)

	if resp.StatusCode != http.StatusOK {
		return respond(resp.StatusCode, map[string]string{"error": string(rb)})
	}
	return respond(http.StatusOK, map[string]interface{}{
		"status":   "ok",
		"inserted": len(req.Rows),
	})
}

func (p *Proxy) health() *zap.Message {
	resp, err := p.http.Get(fmt.Sprintf("http://%s/ping", p.addr))
	if err != nil {
		return respond(http.StatusServiceUnavailable, map[string]string{"error": err.Error()})
	}
	resp.Body.Close()
	return respond(http.StatusOK, map[string]string{"status": "ok", "service": "hanzo-datastore"})
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

func respondRaw(status int, raw []byte) *zap.Message {
	b := zap.NewBuilder(len(raw) + 256)
	ob := b.StartObject(12)
	ob.SetUint32(respStatus, uint32(status))
	ob.SetBytes(respBody, raw)
	ob.SetBytes(respHeaders, []byte(`{"Content-Type":["application/x-ndjson"]}`))
	ob.FinishAsRoot()
	msg, _ := zap.Parse(b.Finish())
	return msg
}
