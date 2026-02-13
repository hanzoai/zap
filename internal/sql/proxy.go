// Package sql implements a ZAP-to-PostgreSQL sidecar.
//
// Accepts ZAP connections and translates to PostgreSQL wire protocol
// via pgx. Optimized for vector operations and session management.
// Exposes MCP-compatible tools: sql_query, sql_exec, sql_health.
package sql

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/luxfi/zap"
)

const MsgTypeSQL uint16 = 300

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
	DSN         string
}

type Proxy struct {
	node   *zap.Node
	pool   *pgxpool.Pool
	logger *slog.Logger
}

func New(ctx context.Context, logger *slog.Logger, cfg Config) (*Proxy, error) {
	var pool *pgxpool.Pool
	var err error
	for i := 0; i < 30; i++ {
		pool, err = pgxpool.New(ctx, cfg.DSN)
		if err == nil {
			if pingErr := pool.Ping(ctx); pingErr == nil {
				break
			} else {
				pool.Close()
				err = pingErr
			}
		}
		if i == 29 {
			return nil, fmt.Errorf("sql: connect failed after retries: %w", err)
		}
		logger.Info("sql: waiting for backend", "attempt", i+1)
		time.Sleep(2 * time.Second)
	}

	p := &Proxy{pool: pool, logger: logger}

	node := zap.NewNode(zap.NodeConfig{
		NodeID:      cfg.NodeID,
		ServiceType: cfg.ServiceType,
		Port:        cfg.Port,
		Logger:      logger,
	})

	node.Handle(MsgTypeSQL, func(_ context.Context, _ string, msg *zap.Message) (*zap.Message, error) {
		return p.handle(ctx, msg), nil
	})

	if err := node.Start(); err != nil {
		pool.Close()
		return nil, fmt.Errorf("sql: node start failed: %w", err)
	}

	p.node = node
	logger.Info("sql sidecar ready")
	return p, nil
}

func (p *Proxy) Stop() {
	if p.node != nil {
		p.node.Stop()
	}
	if p.pool != nil {
		p.pool.Close()
	}
}

func (p *Proxy) handle(ctx context.Context, msg *zap.Message) *zap.Message {
	root := msg.Root()
	path := root.Text(fieldPath)
	body := root.Bytes(fieldBody)

	switch path {
	case "/query":
		return p.query(ctx, body)
	case "/exec":
		return p.exec(ctx, body)
	case "/health":
		return p.health(ctx)
	default:
		if len(body) > 0 {
			return p.query(ctx, body)
		}
		return respond(http.StatusNotFound, map[string]string{"error": "unknown: " + path})
	}
}

type sqlReq struct {
	SQL  string        `json:"sql"`
	Args []interface{} `json:"args,omitempty"`
}

func (p *Proxy) query(ctx context.Context, body []byte) *zap.Message {
	var req sqlReq
	if err := json.Unmarshal(body, &req); err != nil {
		req.SQL = string(body)
	}

	rows, err := p.pool.Query(ctx, req.SQL, req.Args...)
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	defer rows.Close()

	descs := rows.FieldDescriptions()
	var results []map[string]interface{}
	for rows.Next() {
		vals, err := rows.Values()
		if err != nil {
			return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
		}
		row := make(map[string]interface{}, len(descs))
		for i, d := range descs {
			row[string(d.Name)] = vals[i]
		}
		results = append(results, row)
	}

	return respond(http.StatusOK, map[string]interface{}{
		"rows":  results,
		"count": len(results),
	})
}

func (p *Proxy) exec(ctx context.Context, body []byte) *zap.Message {
	var req sqlReq
	if err := json.Unmarshal(body, &req); err != nil {
		req.SQL = string(body)
	}

	tag, err := p.pool.Exec(ctx, req.SQL, req.Args...)
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]interface{}{
		"rows_affected": tag.RowsAffected(),
		"command":       tag.String(),
	})
}

func (p *Proxy) health(ctx context.Context) *zap.Message {
	if err := p.pool.Ping(ctx); err != nil {
		return respond(http.StatusServiceUnavailable, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]string{"status": "ok", "service": "hanzo-sql"})
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
