# ZAP — Zero-copy Application Protocol (Rust)

The Rust runtime for ZAP, the binary RPC used for service-to-service
communication across the Hanzo / Lux stacks. Wire-compatible with the Go
reference (`github.com/zap-proto/go`) and the TypeScript reference
(`@zap-proto/zap`).

| | |
|---|---|
| crate | **`zapwire`** |
| path | `rust/` |
| proof | `rust/tests/crosswire.rs` (Go's bytes) · `rust/tests/interop.rs` (a live Go peer) |

```toml
[dependencies]
zapwire = "1.0"
```

```rust
use zapwire::rpc::{build_response, parse_request, STATUS_OK};
use zapwire::transport::{Conn, Server};

let srv = Server::listen("/run/hanzo/app.sock", |_| {
    |env: &[u8]| {
        let call = parse_request(env)?;
        Ok(build_response(STATUS_OK, call.promise_id, &call.payload))
    }
}).await?;

let conn = Conn::connect(srv.addr()).await?;
let answer = conn.invoke(1, &[], b"ping").await?;   // -> b"ping"
```

## Why `zapwire`

One name must mean one thing, and every shorter name in this family
already means something else:

| name | who owns it |
|---|---|
| `zap` | an unrelated crates.io project |
| `hanzo-zap` | `hanzo/net/hanzo-zap` — published 0.6.75, pinned by `hanzo-node` and the engine forks |
| `zap-proto` | a reservation OWNED AND PUBLISHED by `zap-proto/spec` (`reserve/zap-proto`, published by its `release.yml`) |
| `zap-schema` | the ZAP schema compiler, in `zap-proto/spec`; its lib is also imported as `zap` |

This crate is the WIRE RUNTIME — header, data segment, call envelope,
transport — so it is `zapwire`, and its lib name is `zapwire` too, which
keeps it able to sit in the same dependency graph as `zap-schema`.

It previously declared `hanzo-zap` while never publishing under it: a
name owned by one codebase and claimed by another is exactly how a
collision starts.

## The four layers

| module | what it owns |
|---|---|
| `wire` | the 16-byte message header; self-delimiting message I/O |
| `codec` | the data segment — offset-driven readers and a builder |
| `rpc` | the call envelope (method / promise / capability) and promise pipelining |
| `transport` | connections — `Conn::connect`, `Server::listen` |

Each layer is complete alone: a message needs no envelope, an envelope
needs no connection, and a connection adds only the direction tag a
socket requires.

### Message header (16 bytes)

```
Magic       4   "ZAP\0"
Version     2   u16 LE   1 or 2 — BOTH accepted; the RPC envelope is 2
Flags       2   u16 LE   compressed / encrypted / signed; high byte = msgType
RootOffset  4   u32 LE   offset to root within the data segment
Size        4   u32 LE   total message size, header included
```

### Call envelope (`rpc`)

Request object, fixed size 28 — `Method@0`, `PromiseID@4`, `Target@8`,
`Cap@12`, `Payload@20`. Response object, fixed size 20 — `Status@0`,
`PromiseID@4`, `Body@12`. Both are finished with header flags
`200 << 8`. Byte-for-byte identical to Go's `rpc.BuildRequest` /
`rpc.BuildResponse`; the golden hex in `rust/tests/crosswire.rs` is that
runtime's actual output.

### Connection framing (`transport`)

```
[ u32 LE len ][ 1 byte dir ][ rpc envelope ]      len = 1 + envelope
dir: 1 = request, 2 = response      (3/4/5 = streams — Go only, so far)
```

Correlation is the envelope's PromiseID, so the transport adds no
correlation header of its own. A connection is SYMMETRIC: either end may
call and serve, which is what makes server push possible without a
second protocol — see `Conn::connect_serve`.

The address shape selects the socket, exactly as in `zip`: a path
(`/run/hanzo/app.sock`) is a Unix socket, anything else is `host:port`.
`9999` is ZAP's port the way `443` is HTTPS's.

## Session crypto — read this before pointing ZAP at a remote host

**ZAP frames carry no session crypto in Rust today. They are plaintext.**

- **LOCAL (same host / pod — admin → services): ZAP over a Unix socket.**
  Correct and in use. Confidentiality comes from the filesystem: the
  socket is confined to the host and its permissions.
- **REMOTE (laptop → cloud, across the internet): HTTPS.** Pointing
  today's Rust ZAP at a remote peer would ship a developer's prompts and
  source in cleartext. Do not do it.

The Go plane already has its half: `zap-proto/go/transport.DialTLS` /
`ListenTLS` pin the X25519MLKEM768 hybrid ("PQ X-Wing") and REFUSE a
classical-only peer, and the `quic` subpackage reuses that config. So
remote ZAP is not blocked on protocol design — it is blocked on Rust
having no TLS sibling to `Conn::connect` / `Server::listen`.

What closing it takes, concretely:

1. `Conn::connect_tls` / `Server::listen_tls` over `tokio-rustls`, with
   the key exchange pinned to X25519MLKEM768 so a classical-only peer
   fails the handshake instead of silently downgrading. Same `Conn`,
   same `Dispatch` — only the byte pipe changes, since `Conn` is already
   generic over `Duplex`.
2. Trust material from KMS, not from files next to the binary.
3. Peer identity: SO_PEERCRED is what authenticates a local caller, and
   it has no remote equivalent — a remote peer needs a certificate
   identity bound to a workload, checked on every connection.
4. A downgrade test that FAILS if a classical-only peer completes a
   handshake (the Go plane has one; the Rust side needs the same).

Until 1–4 land, remote stays on HTTPS. Both legs already ride the same
one-call seam, so when ZAP gains crypto they move together — there is no
second wire to migrate.

## Streaming / server push

- **Server push works today** through the symmetric connection: the
  server holds the accepted `Conn` and calls the client on it
  (`Server::listen(addr, |conn| …)` hands the live connection to the
  code that builds its dispatcher). Proved by
  `transport::tests::server_pushes_a_call_to_the_client`.
- **Multi-message streams** (`dir` 3/4/5 — open / msg / end) exist in the
  Go transport only. Rust ignores those frames rather than failing the
  connection, so a peer that opens a stream cannot break unary traffic
  on the same socket. `zap-proto/ws`, the multi-stream pubsub
  sub-protocol, is still a README and a schema file.

## Test

```
make test          # cargo fmt --check + cargo test, incl. the live Go interop peer
```

`rust/tests/interop.rs` builds `interop/main.go` against
`github.com/zap-proto/go@v1.8.2`, runs it on a Unix socket, and drives it
from the Rust client. It skips (loudly) when no Go toolchain is present.

## License

MIT.
