# ZAP: Zero-copy Agent Protocol

**Version 0.2.1 (Cap'n Proto RPC Edition)**
**Hanzo AI**
**January 2026**

---

## Abstract

> **"One ZAP endpoint to rule all MCP servers."**

ZAP (Zero-copy Agent Protocol) is the **MCP Killer**—a single ZAP server that auto-wraps any number of MCP servers into one unified, consensus-aware, zero-copy tool mesh. Agents speak ZAP once; everything else is implementation detail.

Built entirely on Cap'n Proto RPC, ZAP provides:
- **Drop-in Claude Code replacement** - All canonical tools (Read, Write, Bash, etc.)
- **Capability aggregation layer** - Unified Catalog across all MCP servers
- **Consensus-backed routing** - Lux metastable consensus for stable discovery
- **Zero-copy performance** - Wire format = memory format

ZAP is zero-copy, low-allocation, and capability-secure—no JSON-RPC, no schema parsing at runtime.

---

## 1. Introduction

### 1.1 The Problem with MCP

The Model Context Protocol (MCP) was designed for human-AI interaction using JSON-RPC. For agent swarms coordinating in real-time, JSON-RPC has fundamental limitations:

1. **Serialization overhead**: JSON parsing adds 100-500μs per message
2. **Verbose wire format**: ~40% message overhead from JSON encoding
3. **Runtime schema validation**: Silent failures, no compile-time checking
4. **No capability security**: Ambient auth model, no object capabilities
5. **Weak streaming**: Bolted-on, not first-class
6. **No native consensus**: Multi-agent agreement requires external coordination

### 1.2 The ZAP Solution

ZAP replaces JSON-RPC entirely with Cap'n Proto RPC:

| Feature | MCP (JSON-RPC) | ZAP (capnp-rpc) |
|---------|----------------|-----------------|
| Wire format | JSON | Cap'n Proto |
| Serialization | O(n) parse time | Zero-copy |
| Schema | Runtime validation | Compile-time types |
| Message overhead | ~40% | ~5% |
| Security model | Ambient auth | Capability-based |
| Streaming | Bolted-on | First-class (capabilities) |
| Pipelining | None | Native promise pipelining |
| Consensus | External | Native (metastable) |
| Latency (local) | ~500μs | <1μs |

### 1.3 Design Principles

1. **Zero-copy by default**: Wire format IS the memory format
2. **Capability-secure**: Object references, not ambient authority
3. **MCP-adjacent**: Familiar primitives (tools/resources/prompts)
4. **Consensus-first**: Multi-agent agreement is a primitive
5. **Effect-aware**: Operations declare their side-effect level
6. **Low-allocation**: Minimize heap allocations on hot paths

### 1.4 Precise Performance Claims

- **Zero-copy**: Cap'n Proto messages are read without decoding/rehydrating object graphs; traverse directly in-place
- **Low-allocation**: Hot paths minimize allocations (language/runtime dependent)
- **No JSON**: No schema parsing at runtime

---

## 2. Protocol Architecture

### 2.1 Layer Model

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│      (Tools, Resources, Prompts, Tasks, Consensus)          │
├─────────────────────────────────────────────────────────────┤
│                    Capability Layer                          │
│      (Object references, Progress sinks, Subscriptions)     │
├─────────────────────────────────────────────────────────────┤
│                      ZAP Layer                               │
│      (Zap interface, Hello/Welcome, Effect annotations)     │
├─────────────────────────────────────────────────────────────┤
│                  Cap'n Proto RPC Layer                       │
│      (Pipelining, Streaming, Promise resolution)            │
├─────────────────────────────────────────────────────────────┤
│                    Transport Layer                           │
│      (stdio, TCP, QUIC, Unix socket, in-process)            │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Core Roles

- **Host**: The app embedding the model (MCP term, keeps adjacency)
- **Agent**: CLI/daemon that runs the inference loop
- **Endpoint**: A ZAP server providing capabilities
- **Peer**: Symmetric ZAP↔ZAP connection
- **Provider**: Thing that offers tools/resources/prompts
- **Gateway**: MCP-bridging component (ZAP outward, MCP inward)

### 2.3 Connection Model

One Cap'n Proto RPC connection between Host and Endpoint over any bytestream transport:
- stdio (subprocess)
- TCP/QUIC (network)
- Unix socket (local)
- In-process (zero-copy shared memory)

The protocol IS the RPC interface—no JSON-RPC framing.

### 2.4 Security Model

Capability-based security using object references:

```
┌──────────────┐          ┌──────────────┐
│    Client    │──Hello──▶│   Endpoint   │
│              │◀─Welcome─│              │
│              │          │              │
│   Zap cap    │◀─────────│   Zap impl   │
│              │          │              │
│  Tools cap   │◀─tools()─│  Tools impl  │
│              │          │              │
│ Resources cap│◀resources│ Resources    │
└──────────────┘          └──────────────┘
```

Instead of ambient auth tokens, capabilities are passed as object references. If you need auth, model it as:
- Bootstrap `initialize(auth :Auth)` call, or
- Transport-level mutual auth + explicit `AuthContext` struct

---

## 3. Top-Level Interface

### 3.1 Bootstrap Entry Point

The client obtains a `Zap` capability from the transport bootstrap:

```capnp
interface Zap {
    # Bootstrap and capability negotiation
    initialize @0 (hello :Hello) -> (welcome :Welcome);

    # Core MCP-adjacent capabilities
    tools      @1 () -> (tools :Tools);
    resources  @2 () -> (resources :Resources);
    prompts    @3 () -> (prompts :Prompts);

    # Operational capabilities
    tasks      @4 () -> (tasks :Tasks);
    log        @5 () -> (log :Log);

    # Client callback registration (bidirectional)
    setClient  @6 (client :ClientCaps) -> ();

    # ZAP-native extensions
    consensus  @7 () -> (consensus :Consensus);
    mesh       @8 () -> (mesh :Mesh);

    # Health check
    ping       @9 () -> (latencyNs :UInt64, serverTime :UInt64);
}
```

### 3.2 Capability Negotiation

```capnp
struct Hello {
    protocolVersion @0 :Text;    # e.g. "2026-01-22"
    clientInfo      @1 :Implementation;
    capabilities    @2 :ClientCaps;
    schemaHash      @3 :Data;    # zaplock hash for reproducibility
}

struct Welcome {
    protocolVersion @0 :Text;
    endpointInfo    @1 :Implementation;
    capabilities    @2 :EndpointCaps;
    instructions    @3 :Text;
    schemaHash      @4 :Data;
}
```

This replaces JSON-RPC `initialize` while keeping the same semantics.

---

## 4. ZAP Primitives as Capabilities

### 4.1 Tools

```capnp
interface Tools {
    list @0 () -> (tools :List(Tool));
    call @1 (id :Text, args :AnyPointer, ctx :CallContext) -> (result :ToolResult);
}
```

- `args :AnyPointer` allows zero-copy structured args (schema-checked via tool-defined `inputType`/`outputType`)
- `ToolResult` includes content, error, or Task capability for long-running ops
- Native ZAP tools use concrete structs; gateway tools use AnyPointer with schema hash

### 4.2 Resources

```capnp
interface Resources {
    list      @0 (cursor :Cursor) -> (page :ResourcePage);
    read      @1 (uri :Text) -> (content :ResourceContent);
    subscribe @2 (uri :Text) -> (sub :ResourceSubscription);
}

interface ResourceSubscription {
    next   @0 () -> (update :ResourceUpdate, done :Bool);
    cancel @1 () -> ();
}
```

Streaming via capability, not polling notifications.

### 4.3 Prompts

```capnp
interface Prompts {
    list @0 (cursor :Cursor) -> (page :PromptPage);
    get  @1 (name :Text, args :AnyPointer) -> (prompt :PromptInstance);
}
```

### 4.4 Client Callbacks

For Host/Client-mediated features, the endpoint calls back into the client:

```capnp
interface ClientCaps {
    # Roots (workspace/project roots)
    roots          @0 () -> (roots :List(Root));
    onRootsChanged @1 () -> ();

    # Sampling (model invocation via client)
    createMessage  @2 (request :SamplingRequest) -> (result :SamplingResult);

    # Elicitation (user input via client)
    elicit         @3 (prompt :Text, schema :Data) -> (response :AnyPointer);
}
```

---

## 5. Content Model

Zero-copy for large payloads:

```capnp
struct ContentBlock {
    union {
        text         @0 :Text;
        image        @1 :Blob;
        audio        @2 :Blob;
        video        @3 :Blob;
        resourceLink @4 :ResourceLink;
        embedded     @5 :ResourceContent;
    }
    annotations @6 :Annotations;
    meta        @7 :Meta;
}

struct Blob {
    mimeType @0 :Text;
    bytes    @1 :Data;
}
```

For large resources, return a `ByteStream` capability:

```capnp
interface ByteStream {
    readNext @0 (maxBytes :UInt64) -> (data :Data, done :Bool);
    cancel   @1 () -> ();
}
```

---

## 6. Progress, Tasks, and Cancellation

### 6.1 Progress via Capability

```capnp
interface ProgressSink {
    report @0 (progress :Progress) -> ();
}

struct Progress {
    done    @0 :UInt64;
    total   @1 :UInt64;
    message @2 :Text;
}
```

Pass `progress :ProgressSink` in `CallContext` when the caller wants progress updates.

### 6.2 Tasks as Capabilities

Long-running operations return a Task capability instead of blocking:

```capnp
interface Task {
    status    @0 () -> (status :TaskStatus);
    result    @1 () -> (data :AnyPointer);
    cancel    @2 () -> ();
    subscribe @3 () -> (updates :Subscription(TaskStatus));
}
```

No polling JSON-RPC—use the capability directly.

### 6.3 Cancellation

Cap'n Proto RPC has cancellation semantics via dropped promises. Explicit cancellation via `Task.cancel()` for long-running operations.

---

## 7. Effect System and .zap Profile

### 7.1 Effect Lattice

Every ZAP operation declares its effect level:

```
EFFECT LATTICE
│
├─ PURE (⊥)
│   No side effects. Deterministic. Cacheable.
│   Examples: id.hash, code.parse, Tools.list
│
├─ DETERMINISTIC
│   Side effects, but reproducible given same input.
│   Examples: fs.read, vcs.status, Resources.read
│
└─ NONDETERMINISTIC (⊤)
    May vary between calls. Network, time, randomness.
    Examples: Tools.call, consensus.propose, ClientCaps.createMessage
```

### 7.2 .zap Profile Annotations

**File-level annotations:**
```capnp
$namespace("ai.hanzo.zap");
$version("2.0.0");
$protocol("capnp-rpc");
$profile("zap-capnp-1");
```

**Method-level annotations:**
```capnp
interface Tools {
    list @0 () -> (tools :List(Tool))
        $effect(pure)
        $idempotent(true);

    call @1 (id :Text, args :AnyPointer, ctx :CallContext) -> (result :ToolResult)
        $effect(nondeterministic)
        $scope(span)
        $witness(minimal);
}
```

**Available annotations:**
- `$effect(pure|deterministic|nondeterministic)`
- `$idempotent(true|false)`
- `$replayable(true|false)`
- `$scope(span|file|repo|workspace|node|chain|global)`
- `$witness(none|minimal|full)`
- `$costModel(free|metered|gas|quota)`

### 7.3 Lint Rules (zap-capnp-1 profile)

1. **No Text for opaque blobs**: Must use `Data`/`Blob`/`ByteStream`
2. **Streaming required for unbounded outputs**: Return subscription/stream capability
3. **No stringly-typed args for tools**: Tool schemas must bind to capnp structs (or AnyPointer + schema hash)
4. **Determinism contract**: Deterministic methods must accept `DeterminismContext` if they touch time/chain state

---

## 8. Consensus Protocol

### 8.1 Metastable Consensus (ZAP-Native)

ZAP integrates the Hanzo metastable consensus protocol for multi-agent agreement:

```capnp
interface Consensus {
    propose @0 (prompt :Text, participants :List(Text), config :ConsensusConfig)
        -> (result :ConsensusResult);

    vote @1 (sessionId :Text, vote :Text, confidence :Float64)
        -> (accepted :Bool);
}
```

### 8.2 Two-Phase Protocol

**Phase I: Sampling**
```
for round in 1..max_rounds:
    peers = random_sample(agents, k)
    for peer in peers:
        vote = peer.vote(prompt, context)
        update_confidence(vote)

    if confidence >= β₁:
        transition to Phase II
```

**Phase II: Finality**
```
for round in current..max_rounds:
    if confidence >= β₂:
        finalize(winner, synthesize(votes))
        return
```

### 8.3 Luminance Weighting

Faster agents get higher weight in consensus:

```
luminance = 1 / (1 + latency_ms / 1000)
weighted_vote = vote.confidence * luminance
```

---

## 9. Mesh Discovery

### 9.1 Peer-to-Peer Mesh

```capnp
interface Mesh {
    register   @0 (info :PeerInfo) -> (accepted :Bool, reason :Text);
    deregister @1 () -> ();
    topology   @2 () -> (mesh :MeshTopology);
    subscribe  @3 () -> (updates :Subscription(MeshTopology));
}
```

### 9.2 Endpoint URIs

```
zap://host:port              # Cap'n Proto RPC over TCP
zap+unix:///path/to/socket   # Unix domain socket
zap+quic://host:port         # QUIC transport
zap+tls://host:port          # TLS encrypted
zap+mem://segment            # Shared memory (in-process)
```

---

## 10. MCP Gateway

For interop with MCP-only tools:

```capnp
interface Gateway {
    listMcpTools @0 () -> (tools :List(McpTool));
    callMcpTool  @1 (name :Text, jsonArgs :Text) -> (jsonResult :Text);

    # Format conversion
    zapToMcp @2 (content :List(ContentBlock)) -> (json :Text);
    mcpToZap @3 (json :Text) -> (content :List(ContentBlock));
}
```

The Gateway adds ~200μs latency for JSON translation but preserves compatibility.

---

## 11. Performance Analysis

### 11.1 Microbenchmarks

**Single tool call (same machine):**

| Protocol | Latency (p50) | Latency (p99) | Throughput |
|----------|---------------|---------------|------------|
| MCP JSON-RPC | 450μs | 1.2ms | 2,200/s |
| ZAP Unix Socket | 12μs | 45μs | 83,000/s |
| ZAP Shared Mem | 0.8μs | 2.1μs | 1,200,000/s |

**Consensus (10 agents, 10 rounds):**

| Protocol | Time to Finality | Messages |
|----------|------------------|----------|
| MCP + External | 850ms | 200 |
| ZAP Native | 45ms | 100 |

### 11.2 Why Zero-Copy Matters

Cap'n Proto messages are read without:
- Parsing/tokenizing
- Object graph construction
- Heap allocations for message data
- Schema validation at runtime

You traverse the wire bytes directly, accessing fields by computed offset.

### 11.3 Promise Pipelining

Cap'n Proto RPC eliminates round-trips:

```
# Without pipelining (3 round trips):
tools = await endpoint.tools()
tool_list = await tools.list()
result = await tools.call("fs.read", args)

# With pipelining (1 round trip):
result = endpoint.tools().call("fs.read", args)
# All calls batched into one round trip
```

---

## 12. Migration from MCP

### 12.1 Concept Mapping

| MCP (JSON-RPC) | ZAP (capnp-rpc) |
|----------------|-----------------|
| `{jsonrpc, id, method, params}` | Direct RPC method call |
| Request ID uniqueness | Handled by RPC promises |
| `notifications/*` | ProgressSink, subscriptions, callbacks |
| `tools/list`, `tools/call` | `Tools.list()`, `Tools.call()` |
| `resources/subscribe` + notifications | Return `ResourceSubscription` capability |
| `tasks/*` polling | Return `Task` capability |

### 12.2 Gradual Migration

1. **Phase 1**: Run ZAP endpoint with MCP Gateway
2. **Phase 2**: Convert hot-path tools to native ZAP
3. **Phase 3**: Remove Gateway for pure-ZAP deployment

### 12.3 Type System

Use capnp structs as the tool input/output types. JSON Schema export for UI/forms as a derived artifact (not the runtime contract).

---

## 13. Implementation

### 13.1 Reference Implementation

```
hanzo-mcp/rust/
├── src/
│   └── zap/
│       ├── mod.rs          # Module root
│       ├── endpoint.rs     # Zap interface impl
│       ├── tools.rs        # Tools capability
│       ├── resources.rs    # Resources capability
│       ├── consensus.rs    # Metastable consensus
│       ├── mesh.rs         # Peer discovery
│       └── gateway.rs      # MCP bridge
├── schema/
│   └── zap.capnp           # Wire format schema
```

### 13.2 Language Support

| Language | Status | Package |
|----------|--------|---------|
| Rust | Reference | `hanzo-zap` |
| Python | Production | `hanzo-zap` |
| TypeScript | Planned | `@hanzo/zap` |
| Go | Planned | `github.com/hanzoai/zap` |

### 13.3 Quick Start

**Rust:**
```rust
use hanzo_zap::{ZapEndpoint, Tools, ConsensusConfig};

// Create endpoint
let endpoint = ZapEndpoint::new();

// Get tools capability
let tools = endpoint.tools().await?;
let tool_list = tools.list().await?;

// Call a tool
let result = tools.call("fs.read", &args, ctx).await?;

// Run consensus
let consensus = endpoint.consensus().await?;
let result = consensus.propose("Best approach?", &participants, config).await?;
```

---

## 14. Conclusion

ZAP v2.0 is a complete reimagining of agent communication:

- **No JSON-RPC**: Pure Cap'n Proto RPC throughout
- **Zero-copy**: Wire format = memory format
- **Capability-secure**: Object references, not tokens
- **MCP-adjacent**: Familiar primitives, superior performance
- **Consensus-native**: Multi-agent agreement built in

The future of AI is not single models—it's agent swarms coordinating in real-time. ZAP makes this possible with the performance characteristics required for true agent-to-agent collaboration.

---

## References

1. Cap'n Proto. https://capnproto.org/
2. Cap'n Proto RPC. https://capnproto.org/rpc.html
3. Hanzo Metastable Consensus. https://github.com/luxfi/consensus
4. Model Context Protocol. https://modelcontextprotocol.io/

---

## Appendix A: Full Cap'n Proto Schema

See `/schema/zap.capnp` for the complete wire format specification.

## Appendix B: JSON-RPC to capnp-rpc Migration Table

| JSON-RPC ZAP concept | capnp-rpc replacement |
|---------------------|----------------------|
| `{jsonrpc, id, method, params}` | Direct RPC method call |
| Request ID uniqueness | Handled by RPC promises; no protocol-level IDs |
| `notifications/*` | ProgressSink, subscriptions, or callback interfaces |
| `tools/list`, `tools/call` | `Tools.list()`, `Tools.call()` |
| `resources/subscribe` + notifications | Return `ResourceSubscription` capability with `next()` |
| `tasks/*` polling | Return `Task` capability (`status()`, `result()`, `cancel()`) |

---

**Document Version:** 2.0
**Last Updated:** January 2025
**Authors:** Hanzo AI Engineering
**License:** MIT
