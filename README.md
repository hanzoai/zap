# Hanzo ZAP

ZAP — **Zero-copy Application Protocol**. Cap'n-Proto-style binary RPC
used for service-to-service communication across the Hanzo / Lux /
Liquidity stacks. Wire-compatible Go, Rust, and TypeScript
implementations.

## Wire format

```
┌─ Header (16 bytes) ────────────────────────────────────────────┐
│  Magic       4   "ZAP\0"                                       │
│  Version     2   u16 LE                                        │
│  Flags       2   u16 LE  (compressed, encrypted, signed, …)    │
│  RootOffset  4   u32 LE  offset to root within data segment    │
│  Size        4   u32 LE  total message size including header   │
└────────────────────────────────────────────────────────────────┘
┌─ Data segment (variable, 8-byte aligned) ──────────────────────┐
│  Cap'n-Proto-style structs / lists / text / bytes              │
└────────────────────────────────────────────────────────────────┘
```

All multi-byte integers are little-endian. The data segment uses the
canonical capnp segment layout — readers walk it with zero allocation
via `capnp::message::Reader` (Rust), `capnp.Parse` (Go), or `@zap-proto`
(TypeScript).

## Implementations

| Language | Path | Status |
|----------|------|--------|
| Rust     | `rust/`                        | this crate (v2.0) |
| Go       | `github.com/luxfi/zap`         | reference |
| TypeScript | `@zap-proto/zap`             | reference |

The three impls MUST stay wire-compatible. Schema lives at
`rust/schema/zap.capnp` (canonical) — bump the file id (`@0x…;`) for
wire-incompatible changes.

## v1 → v2

Versions 1.x of the Rust crate were a JSON-payload-in-binary-frame
WebSocket experiment that called itself ZAP but was incompatible with
the Go reference. **v2.0 is wire-incompatible with v1.x.** Upgrade
together with the Go and TS deps; the protocol version byte (`1` in
both v1 and v2 headers) is intentionally not bumped because v1 was
never deployed in production.

## JWKS-via-ZAP

ZAP carries its own JWKS push protocol (`KeysSubscribe` →
`KeysSnapshot` → `KeysRotate` / `KeysRevoke`). Consumers subscribe
once, cache locally, and skip the `/.well-known/jwks` HTTP polling
that otherwise dominates IAM load. See `schema/zap.capnp`.

## License

MIT.
