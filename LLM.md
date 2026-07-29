# LLM.md — hanzoai/zap

## What this repo IS

The **Rust runtime for the ZAP protocol**, published to crates.io as
**`zap-proto`** and imported as `zap`. Two crates ship as a unit:

- `rust/` — the runtime: `wire` (header) · `codec` (data segment) ·
  `rpc` (call envelope + promise pipelining) · `transport` (Conn/Server).
- `zapc-rs/` — `zapc`, the schema compiler `rust/build.rs` calls.

`interop/main.go` is a live Go ZAP peer used only by the Rust test
suite; it is not shipped.

## What this repo IS NOT

- Not the sidecar. `cmd/zap-sidecar` was removed (Apr 2026); ZAP is
  embedded natively in every backend service (sql, kv, docdb, …), each
  exposing it on TCP 9999 from its own process. No sidecar container,
  image, or CRD field. `ghcr.io/hanzoai/zap` is no longer published.
- Not `hanzo-zap`. See below.

## The crate-name collision, resolved

`hanzo-zap` on crates.io has TWO source lineages and never had one home:

| crate | source | state |
|---|---|---|
| `hanzo-zap` 0.6.75 (max) | `~/work/hanzo/net/hanzo-zap` (repo hanzonet/network) | published; pinned by `hanzo-node` + `hanzo/net` |
| `hanzo-zap` 0.2.1 | `~/work/zap/spec` (repo zap-proto/spec) | published; `Client::connect` / `call_tool` were TODO stubs |
| `hanzo-zap` 1.0.1 | THIS repo, `rust/` | **never published** — declared the name without owning it |
| `hanzo-zap` (no version) | `~/work/hanzo/evm/crates/hanzo-zap` | private path crate, never published |

Resolution:

1. **This repo renamed to `zap-proto`** (a crates.io name we already
   own; 1.0.0 was a reservation, so 1.0.1 is latest+1 patch). Imported
   as `zap`, mirroring `github.com/zap-proto/go` / package `zap`.
2. **`zap-proto/spec` retired its Rust crate.** 12 of its 14 Rust files
   were byte-identical to `hanzo/net/hanzo-zap`; the two that differed
   were a stub `lib.rs` and a 26-line stub `server.rs`. A spec repo
   ships a spec, not a runtime.
3. **`hanzo-zap` stays with `hanzo/net/hanzo-zap`** — it published the
   max version and has the real consumers. 0.2.1 is NOT yanked: ~25
   engine forks pin it exactly, and yanking would break their resolve.

Still open (hand-off): `hanzo/evm/crates/hanzo-zap` is a fourth crate
with the same name. It is path-only, so nothing resolves it from the
registry — but two crates with one name still cannot coexist in one
cargo graph, so the evm one should be renamed before either is
published.

## Wire agreement with Go — how it is proved

- `rust/tests/crosswire.rs` — golden hex generated FROM
  `github.com/zap-proto/go@v1.8.2`'s `rpc.BuildRequest` /
  `rpc.BuildResponse`. Rust must both parse those bytes and emit them.
- `rust/tests/interop.rs` — builds and runs `interop/main.go` (a real
  `transport.Listen` server) on a Unix socket and drives it from
  `zap::transport::Conn`. This is what pins BEHAVIOUR: framing,
  direction tag, PromiseID correlation, connection lifecycle.

The gap that made this necessary: the Rust reader accepted wire version
1 only, and every Go RPC envelope is version 2 (`NewBuilderV2`). Rust
could not read — or even re-parse its own — RPC envelope. Reverting that
one condition turns both interop tests red with `InvalidVersion(2)`.

## Session crypto — the honest state

Rust ZAP is PLAINTEXT. Local over a Unix socket is correct; remote must
stay on HTTPS. The Go plane already pins X25519MLKEM768 in
`transport.DialTLS`/`ListenTLS`; Rust has no TLS sibling yet. The four
concrete steps to close it are in `README.md` § Session crypto.

`zap-proto/ws` (multi-stream pubsub) is still a README + schema.
Multi-message streams (`dir` 3/4/5) exist in the Go transport only;
Rust ignores those frames so they cannot break unary traffic. Unary
server PUSH does work, via the symmetric connection.

## Test gate

`make test` → `cargo fmt --check` + `cargo test` (which includes the Go
interop peer). `cargo clippy` currently fails on this machine with
`multiple input filenames provided` from the rustup clippy shim — an
environment fault, not a code fault; it is not part of the gate until
that is fixed.

## See also

- `~/work/zap/go` — `github.com/zap-proto/go`, the reference runtime.
- `~/work/hanzo/sql`, `~/work/hanzo/kv`, `~/work/hanzo/docdb` — services
  that bake ZAP into their primary container.
