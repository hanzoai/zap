# Hanzo Zap (deprecated)

## Status

**DEPRECATED.** This repository previously hosted the standalone
`zap-sidecar` binary that bridged the ZAP protocol to backend services as
a co-located container. That abstraction has been removed.

ZAP is now embedded **natively inside every backend service** (sql, kv,
docdb, datastore, …). Each service exposes ZAP on TCP `9999` from its own
process — there is no separate sidecar container, image, or CRD field.

## What remains in this repo

- `LICENSE`, `LLM.md`, `README.md`, `Makefile`, `go.mod`, `go.sum`
- `rust/` — the Rust ZAP implementation (separate, still active for
  embedding into Rust services)
- `out/`, `public/` — historical assets

## What was removed (Apr 2026)

- `cmd/zap-sidecar/main.go` — the sidecar binary entrypoint
- `internal/sql`, `internal/kv`, `internal/datastore`, `internal/documentdb`
  — sidecar-only protocol bridges to PostgreSQL, Valkey, ClickHouse, FerretDB
- `internal/mcp.go` — MCP shim for the sidecar
- `Dockerfile` — produced `ghcr.io/hanzoai/zap:*`, no longer published

## Migration

- Operators (`hanzo/operator`, `hanzo/hanzo-operator`): the `zap` /
  `ZAPSidecar` field is gone from `HanzoDatastore.Spec` and the inline
  datastore spec on `HanzoPlatform.Spec`. Services expose ZAP themselves.
- K8s manifests (`hanzo/universe/infra/k8s`): no `zap:` blocks on CRs;
  StatefulSets/Deployments expose `containerPort: 9999` on the main
  container of services that ship native ZAP.

## See also

- `~/work/hanzo/sql`, `~/work/hanzo/kv`, `~/work/hanzo/docdb` — services
  that bake ZAP into their primary container.
- `rust/` — Rust ZAP library used by Rust services (e.g. `hanzo/base`).
