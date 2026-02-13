// Hanzo ZAP sidecar bridges ZAP protocol to backend services (SQL, KV, Datastore).
//
// It runs as a sidecar container alongside each service, accepting ZAP
// connections from the Hanzo Gateway and translating them to native
// protocol calls against the co-located backend.
//
// Any service implementing the ZAP schema gets MCP for free via zapd.
package main

import (
	"context"
	"flag"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"github.com/hanzoai/zap-sidecar/internal/datastore"
	"github.com/hanzoai/zap-sidecar/internal/kv"
	"github.com/hanzoai/zap-sidecar/internal/sql"
)

func main() {
	mode := flag.String("mode", "", "sidecar mode: sql, kv, or datastore")
	nodeID := flag.String("node-id", "", "ZAP node ID (default: mode name)")
	port := flag.Int("port", 9651, "ZAP listen port")
	serviceType := flag.String("service-type", "_hanzo._tcp", "mDNS service type")
	backend := flag.String("backend", "", "backend address (e.g. localhost:5432)")
	password := flag.String("password", "", "backend password (KV/Datastore)")
	flag.Parse()

	if *mode == "" {
		*mode = os.Getenv("ZAP_MODE")
	}
	if *backend == "" {
		*backend = os.Getenv("ZAP_BACKEND")
	}
	if *password == "" {
		*password = os.Getenv("ZAP_PASSWORD")
	}
	if *nodeID == "" {
		*nodeID = *mode
	}

	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	}))

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, syscall.SIGINT, syscall.SIGTERM)

	var svc Sidecar
	var err error

	switch *mode {
	case "sql":
		svc, err = sql.New(ctx, logger, sql.Config{
			NodeID:      *nodeID,
			Port:        *port,
			ServiceType: *serviceType,
			DSN:         *backend,
		})
	case "kv":
		svc, err = kv.New(ctx, logger, kv.Config{
			NodeID:      *nodeID,
			Port:        *port,
			ServiceType: *serviceType,
			Addr:        *backend,
			Password:    *password,
		})
	case "datastore":
		svc, err = datastore.New(ctx, logger, datastore.Config{
			NodeID:      *nodeID,
			Port:        *port,
			ServiceType: *serviceType,
			Addr:        *backend,
			User:        os.Getenv("ZAP_USER"),
			Password:    *password,
		})
	default:
		logger.Error("unknown mode, use: sql, kv, or datastore", "mode", *mode)
		os.Exit(1)
	}

	if err != nil {
		logger.Error("failed to start sidecar", "error", err)
		os.Exit(1)
	}

	logger.Info("hanzo zap sidecar started", "mode", *mode, "node_id", *nodeID, "port", *port, "backend", *backend)

	<-sig
	logger.Info("shutting down")
	svc.Stop()
}

// Sidecar is the interface for all ZAP sidecar backends.
type Sidecar interface {
	Stop()
}
