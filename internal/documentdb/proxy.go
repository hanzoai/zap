// Package documentdb implements a ZAP-to-DocumentDB/FerretDB sidecar.
//
// Accepts ZAP connections and translates to MongoDB wire protocol via
// the official Go driver. FerretDB provides MongoDB compatibility on top
// of PostgreSQL, so this proxy enables document-store operations through
// the ZAP zero-copy protocol.
// Exposes MCP-compatible tools: documentdb_find, documentdb_insert,
// documentdb_update, documentdb_delete, documentdb_health.
package documentdb

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"github.com/luxfi/zap"
	"go.mongodb.org/mongo-driver/v2/bson"
	"go.mongodb.org/mongo-driver/v2/mongo"
	"go.mongodb.org/mongo-driver/v2/mongo/options"
)

const MsgTypeDocumentDB uint16 = 303

const (
	fieldPath   = 4
	fieldBody   = 12
	respStatus  = 0
	respBody    = 4
	respHeaders = 8
)

type Config struct {
	NodeID      string
	Port        int
	ServiceType string
	Addr        string // MongoDB-compatible connection string (e.g. mongodb://localhost:27017)
	Database    string // Default database name
}

type Proxy struct {
	node   *zap.Node
	client *mongo.Client
	db     string
	logger *slog.Logger
}

func New(ctx context.Context, logger *slog.Logger, cfg Config) (*Proxy, error) {
	var client *mongo.Client
	var err error

	for i := 0; i < 30; i++ {
		client, err = mongo.Connect(options.Client().ApplyURI(cfg.Addr))
		if err == nil {
			if pingErr := client.Ping(ctx, nil); pingErr == nil {
				break
			} else {
				_ = client.Disconnect(ctx)
				err = pingErr
			}
		}
		if i == 29 {
			return nil, fmt.Errorf("documentdb: connect failed after retries: %w", err)
		}
		logger.Info("documentdb: waiting for backend", "attempt", i+1, "addr", cfg.Addr)
		time.Sleep(2 * time.Second)
	}

	db := cfg.Database
	if db == "" {
		db = "hanzo"
	}

	p := &Proxy{client: client, db: db, logger: logger}

	node := zap.NewNode(zap.NodeConfig{
		NodeID:      cfg.NodeID,
		ServiceType: cfg.ServiceType,
		Port:        cfg.Port,
		Logger:      logger,
	})

	node.Handle(MsgTypeDocumentDB, func(_ context.Context, _ string, msg *zap.Message) (*zap.Message, error) {
		return p.handle(ctx, msg), nil
	})

	if err := node.Start(); err != nil {
		_ = client.Disconnect(ctx)
		return nil, fmt.Errorf("documentdb: node start failed: %w", err)
	}

	p.node = node
	logger.Info("documentdb sidecar ready", "addr", cfg.Addr, "database", db)
	return p, nil
}

func (p *Proxy) Stop() {
	if p.node != nil {
		p.node.Stop()
	}
	if p.client != nil {
		_ = p.client.Disconnect(context.Background())
	}
}

func (p *Proxy) handle(ctx context.Context, msg *zap.Message) *zap.Message {
	root := msg.Root()
	path := root.Text(fieldPath)
	body := root.Bytes(fieldBody)

	switch path {
	case "/find":
		return p.find(ctx, body)
	case "/insert":
		return p.insert(ctx, body)
	case "/update":
		return p.update(ctx, body)
	case "/delete":
		return p.del(ctx, body)
	case "/health":
		return p.health(ctx)
	default:
		return respond(http.StatusNotFound, map[string]string{"error": "unknown path: " + path})
	}
}

type findReq struct {
	Collection string `json:"collection"`
	Filter     bson.M `json:"filter"`
	Limit      int64  `json:"limit,omitempty"`
	Database   string `json:"database,omitempty"`
}

func (p *Proxy) find(ctx context.Context, body []byte) *zap.Message {
	var req findReq
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	db := p.db
	if req.Database != "" {
		db = req.Database
	}

	coll := p.client.Database(db).Collection(req.Collection)
	opts := options.Find()
	if req.Limit > 0 {
		opts.SetLimit(req.Limit)
	}

	cursor, err := coll.Find(ctx, req.Filter, opts)
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}
	defer cursor.Close(ctx)

	var results []bson.M
	if err := cursor.All(ctx, &results); err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}

	return respond(http.StatusOK, map[string]interface{}{
		"documents": results,
		"count":     len(results),
	})
}

type insertReq struct {
	Collection string   `json:"collection"`
	Documents  []bson.M `json:"documents"`
	Database   string   `json:"database,omitempty"`
}

func (p *Proxy) insert(ctx context.Context, body []byte) *zap.Message {
	var req insertReq
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	db := p.db
	if req.Database != "" {
		db = req.Database
	}

	coll := p.client.Database(db).Collection(req.Collection)

	docs := make([]interface{}, len(req.Documents))
	for i, d := range req.Documents {
		docs[i] = d
	}

	result, err := coll.InsertMany(ctx, docs)
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}

	return respond(http.StatusOK, map[string]interface{}{
		"inserted_ids": result.InsertedIDs,
		"count":        len(result.InsertedIDs),
	})
}

type updateReq struct {
	Collection string `json:"collection"`
	Filter     bson.M `json:"filter"`
	Update     bson.M `json:"update"`
	Database   string `json:"database,omitempty"`
}

func (p *Proxy) update(ctx context.Context, body []byte) *zap.Message {
	var req updateReq
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	db := p.db
	if req.Database != "" {
		db = req.Database
	}

	coll := p.client.Database(db).Collection(req.Collection)
	result, err := coll.UpdateMany(ctx, req.Filter, req.Update)
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}

	return respond(http.StatusOK, map[string]interface{}{
		"matched_count":  result.MatchedCount,
		"modified_count": result.ModifiedCount,
	})
}

type deleteReq struct {
	Collection string `json:"collection"`
	Filter     bson.M `json:"filter"`
	Database   string `json:"database,omitempty"`
}

func (p *Proxy) del(ctx context.Context, body []byte) *zap.Message {
	var req deleteReq
	if err := json.Unmarshal(body, &req); err != nil {
		return respond(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	db := p.db
	if req.Database != "" {
		db = req.Database
	}

	coll := p.client.Database(db).Collection(req.Collection)
	result, err := coll.DeleteMany(ctx, req.Filter)
	if err != nil {
		return respond(http.StatusInternalServerError, map[string]string{"error": err.Error()})
	}

	return respond(http.StatusOK, map[string]interface{}{
		"deleted_count": result.DeletedCount,
	})
}

func (p *Proxy) health(ctx context.Context) *zap.Message {
	if err := p.client.Ping(ctx, nil); err != nil {
		return respond(http.StatusServiceUnavailable, map[string]string{"error": err.Error()})
	}
	return respond(http.StatusOK, map[string]string{"status": "ok", "service": "hanzo-documentdb"})
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
