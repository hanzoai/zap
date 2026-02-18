// Package datastore implements a ZAP-to-ClickHouse sidecar using the native
// ClickHouse TCP protocol (port 9000). Zero HTTP — pure native driver.
//
// Accepts ZAP connections and translates to ClickHouse native protocol.
// Optimized for bulk insert of AI telemetry, ad-tech analytics, and traces.
// Exposes MCP-compatible tools: datastore_query, datastore_insert, datastore_exec.
package datastore

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"github.com/ClickHouse/clickhouse-go/v2"
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
	Addr        string // host:9000 (native TCP)
	User        string
	Password    string
	Database    string
}

type Proxy struct {
	node     *zap.Node
	conn     clickhouse.Conn
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

	// Connect via native TCP protocol (port 9000)
	opts := &clickhouse.Options{
		Addr: []string{cfg.Addr},
		Auth: clickhouse.Auth{
			Database: cfg.Database,
			Username: cfg.User,
			Password: cfg.Password,
		},
		DialTimeout:      10 * time.Second,
		MaxOpenConns:     10,
		MaxIdleConns:     5,
		ConnMaxLifetime:  time.Hour,
		ConnOpenStrategy: clickhouse.ConnOpenInOrder,
	}

	var conn clickhouse.Conn
	var err error

	// Retry loop — wait for ClickHouse to be ready (Docker startup order)
	for i := 0; i < 30; i++ {
		conn, err = clickhouse.Open(opts)
		if err == nil {
			pingCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
			err = conn.Ping(pingCtx)
			cancel()
			if err == nil {
				break
			}
			conn.Close()
		}
		logger.Warn("datastore not ready, retrying", "attempt", i+1, "error", err)
		time.Sleep(2 * time.Second)
	}
	if err != nil {
		return nil, fmt.Errorf("datastore: connect failed after 30 retries: %w", err)
	}

	p := &Proxy{
		conn:     conn,
		database: cfg.Database,
		logger:   logger,
	}

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
		conn.Close()
		return nil, fmt.Errorf("datastore: node start failed: %w", err)
	}

	p.node = node
	logger.Info("datastore sidecar ready (native TCP)", "addr", cfg.Addr, "db", cfg.Database)
	return p, nil
}

func (p *Proxy) Stop() {
	if p.node != nil {
		p.node.Stop()
	}
	if p.conn != nil {
		p.conn.Close()
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
	case "/exec":
		return p.exec(body)
	case "/insert":
		return p.insert(body)
	case "/tables":
		return p.tables(body)
	default:
		if len(body) > 0 {
			return p.query(body)
		}
		return respond(http.StatusNotFound, map[string]string{"error": "unknown: " + path})
	}
}

// ================================================================
// /query — SELECT via native protocol, return rows as JSON
// ================================================================

type dsQuery struct {
	SQL      string `json:"sql"`
	Database string `json:"database,omitempty"`
}

func (p *Proxy) query(body []byte) *zap.Message {
	var req dsQuery
	if err := json.Unmarshal(body, &req); err != nil {
		req.SQL = string(body)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	rows, err := p.conn.Query(ctx, req.SQL)
	if err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}
	defer rows.Close()

	columns := rows.ColumnTypes()
	colNames := make([]string, len(columns))
	for i, col := range columns {
		colNames[i] = col.Name()
	}

	var results []map[string]interface{}
	for rows.Next() {
		vals := make([]interface{}, len(columns))
		valPtrs := make([]interface{}, len(columns))
		for i := range vals {
			valPtrs[i] = &vals[i]
		}
		if err := rows.Scan(valPtrs...); err != nil {
			return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
		}
		row := make(map[string]interface{}, len(columns))
		for i, name := range colNames {
			row[name] = vals[i]
		}
		results = append(results, row)
	}
	if err := rows.Err(); err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}

	return respond(http.StatusOK, map[string]interface{}{
		"rows":  results,
		"count": len(results),
	})
}

// ================================================================
// /exec — DDL / non-SELECT statements
// ================================================================

type dsExec struct {
	SQL  string        `json:"sql"`
	Args []interface{} `json:"args,omitempty"`
}

func (p *Proxy) exec(body []byte) *zap.Message {
	var req dsExec
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	err := p.conn.Exec(ctx, req.SQL, req.Args...)
	if err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]string{"status": "ok"})
}

// ================================================================
// /insert — bulk insert via native batch protocol
// ================================================================

type insertReq struct {
	Table    string                   `json:"table"`
	Database string                   `json:"database,omitempty"`
	Columns  []string                 `json:"columns,omitempty"`
	Rows     []map[string]interface{} `json:"rows"`
}

func (p *Proxy) insert(body []byte) *zap.Message {
	var req insertReq
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}
	if len(req.Rows) == 0 {
		return respond(http.StatusBadRequest, map[string]string{"error": "no rows"})
	}

	// Build column list from first row if not provided
	cols := req.Columns
	if len(cols) == 0 && len(req.Rows) > 0 {
		for k := range req.Rows[0] {
			cols = append(cols, k)
		}
	}

	// Build INSERT statement
	table := req.Table
	if req.Database != "" && req.Database != p.database {
		table = req.Database + "." + req.Table
	}

	colList := ""
	for i, c := range cols {
		if i > 0 {
			colList += ", "
		}
		colList += c
	}
	sql := fmt.Sprintf("INSERT INTO %s (%s)", table, colList)

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	batch, err := p.conn.PrepareBatch(ctx, sql)
	if err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}

	for _, row := range req.Rows {
		vals := make([]interface{}, len(cols))
		for i, col := range cols {
			vals[i] = row[col]
		}
		if err := batch.Append(vals...); err != nil {
			return respond(http.StatusBadRequest, map[string]string{
				"error": fmt.Sprintf("row append: %s", err),
			})
		}
	}

	if err := batch.Send(); err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}

	return respond(http.StatusOK, map[string]interface{}{
		"status":   "ok",
		"inserted": len(req.Rows),
	})
}

// ================================================================
// /tables — introspect schema
// ================================================================

type tablesReq struct {
	Database string `json:"database,omitempty"`
}

func (p *Proxy) tables(body []byte) *zap.Message {
	var req tablesReq
	json.Unmarshal(body, &req)
	db := req.Database
	if db == "" {
		db = p.database
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	rows, err := p.conn.Query(ctx,
		"SELECT name, engine, total_rows, total_bytes FROM system.tables WHERE database = ?", db)
	if err != nil {
		return respond(http.StatusBadGateway, map[string]string{"error": err.Error()})
	}
	defer rows.Close()

	type tableInfo struct {
		Name       string  `json:"name"`
		Engine     string  `json:"engine"`
		TotalRows  *uint64 `json:"total_rows"`
		TotalBytes *uint64 `json:"total_bytes"`
	}
	var tables []tableInfo
	for rows.Next() {
		var t tableInfo
		if err := rows.Scan(&t.Name, &t.Engine, &t.TotalRows, &t.TotalBytes); err != nil {
			continue
		}
		tables = append(tables, t)
	}

	return respond(http.StatusOK, map[string]interface{}{
		"database": db,
		"tables":   tables,
	})
}

// ================================================================
// /health — native ping
// ================================================================

func (p *Proxy) health() *zap.Message {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := p.conn.Ping(ctx); err != nil {
		return respond(http.StatusServiceUnavailable, map[string]string{"error": err.Error()})
	}

	info, _ := p.conn.ServerVersion()
	ver := ""
	if info != nil {
		ver = fmt.Sprintf("%d.%d.%d", info.Version.Major, info.Version.Minor, info.Version.Patch)
	}

	return respond(http.StatusOK, map[string]interface{}{
		"status":  "ok",
		"service": "hanzo-datastore",
		"native":  true,
		"version": ver,
	})
}

// ================================================================
// ZAP response builders
// ================================================================

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
