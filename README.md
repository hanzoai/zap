# ZAP: Zero-copy Agent Protocol

> **"One ZAP endpoint to rule all MCP servers."**

ZAP is the **MCP Killer**—a single ZAP server that auto-wraps any number of MCP servers into one unified, consensus-aware, zero-copy tool mesh. Agents speak ZAP once; everything else is implementation detail.

## Key Features

- **Drop-in Claude Code replacement** — All canonical tools (Read, Write, Bash, etc.)
- **Capability aggregation layer** — Unified Catalog across all MCP servers
- **Consensus-backed routing** — Lux metastable consensus for stable discovery
- **Zero-copy performance** — Wire format = memory format
- **Post-quantum ready** — PQ signatures via Ringtail/ML-DSA

## Performance

| Metric | MCP (JSON-RPC) | ZAP (Cap'n Proto) |
|--------|----------------|-------------------|
| Local latency | ~500μs | <1μs |
| Throughput | 2,200/s | 1,200,000/s |
| Message overhead | ~40% | ~5% |
| Infrastructure cost | Baseline | **40-50× lower** |

## Quick Start

```bash
# Install ZAP daemon
cargo install zapd

# Start the gateway
zapd serve --port 9999

# Add MCP servers
zapd add mcp --name github --url stdio://gh-mcp
zapd add mcp --name slack --url http://localhost:8080
zapd add mcp --name db --url zap+unix:///tmp/postgres.sock

# Agents connect once
# zap://localhost:9999 → all tools unified
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Agent (speaks ZAP)                      │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                     ZAP Gateway                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Catalog   │  │ Coordination│  │    Tool Router      │  │
│  │  (unified)  │  │  (Lux cons) │  │ (load-bal/failover) │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└──────┬─────────────────┬─────────────────┬──────────────────┘
       │                 │                 │
┌──────▼─────┐    ┌──────▼─────┐    ┌──────▼─────┐
│ MCP Server │    │ MCP Server │    │ MCP Server │
│  (GitHub)  │    │  (Slack)   │    │ (Postgres) │
└────────────┘    └────────────┘    └────────────┘
```

## Transport URIs

| Scheme | Transport | Use Case |
|--------|-----------|----------|
| `zap://` | TCP | Remote servers |
| `zap+unix://` | Unix socket | Local IPC |
| `zap+tls://` | TLS over TCP | Secure remote |
| `zap+quic://` | QUIC | High-performance remote |
| `zap+mem://` | Shared memory | In-process |

## Documentation

- [Getting Started](https://zap.hanzo.ai/docs)
- [Whitepaper](./docs/ZAP-WHITEPAPER.md)
- [Schema](./schema/zap.zap)

## Built With

- [Cap'n Proto](https://capnproto.org/) — Zero-copy serialization
- [Lux Network](https://lux.network/) — Metastable consensus
- [Hanzo AI](https://hanzo.ai/) — AI infrastructure

## License

MIT License - Hanzo AI Inc.
