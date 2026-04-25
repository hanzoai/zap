# Hanzo ZAP — deprecated repository

This repository previously hosted `zap-sidecar`, a standalone Go binary
that ran as a sidecar container next to each backend service (PostgreSQL,
Valkey, ClickHouse, FerretDB) and bridged the ZAP protocol to the
co-located backend.

**That abstraction has been removed.** ZAP is now implemented natively
inside each backend service and exposed on TCP `9999` by the service
itself. There is no separate sidecar container, no `ghcr.io/hanzoai/zap`
image, and no `ZAPSidecar` CRD field.

## What was deleted

- `cmd/zap-sidecar/main.go` — the sidecar binary entrypoint
- `internal/sql/`, `internal/kv/`, `internal/datastore/`,
  `internal/documentdb/` — sidecar-only protocol bridges
- `internal/mcp.go` — MCP shim for the sidecar
- `Dockerfile` — produced `ghcr.io/hanzoai/zap:*`

## What remains

- `LICENSE`, `LLM.md`, this `README.md`
- `Makefile`, `go.mod`, `go.sum` — kept so the repo still builds clean
- `rust/` — the Rust ZAP library (still active, embedded into Rust
  services such as `hanzo/base`)
- `out/`, `public/` — historical assets

## Where ZAP lives now

Backend services bake ZAP into their primary container:

| Service | Repo | ZAP location |
|---------|------|--------------|
| `sql` | `hanzo/sql` | embedded in `ghcr.io/hanzoai/sql:*` |
| `kv` | `hanzo/kv` | embedded in `ghcr.io/hanzoai/kv:*` |
| `docdb` | `hanzo/docdb` | embedded in `ghcr.io/hanzoai/docdb:*` |
| `datastore` | `hanzo/datastore` | embedded in `ghcr.io/hanzoai/datastore:*` |
| `base` | `hanzo/base` | embedded in `ghcr.io/hanzoai/base:*` (Rust) |

The ZAP protocol itself is unchanged. Existing ZAP clients
(zapd / hanzoai/gateway) connect to the same TCP `9999` port — the
container layout is what changed.

## Migration timeline

- **Apr 2026**: sidecar binary and protocol bridges deleted from this
  repo; CRD fields and CR `zap:` blocks stripped from operators and
  k8s manifests; native ZAP exposure verified on `sql`, `kv`, `docdb`,
  `datastore` StatefulSets.
