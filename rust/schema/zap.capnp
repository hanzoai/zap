@0xc0a8b5e0e5f0a000;

# ZAP canonical schema — matches @zap-proto/zap (TypeScript) and the
# Go luxfi/zap reference. Compiled by build.rs via capnpc-rust into
# `OUT_DIR/zap_capnp.rs` and re-exported through `src/schema.rs`.

# ── Connection lifecycle ───────────────────────────────────────────

struct Init {
  clientId  @0 :Text;
  browser   @1 :Text;
  version   @2 :Text;
}

struct InitAck {
  serverId  @0 :Text;
  name      @1 :Text;
  manifest  @2 :Data;     # opaque tool-manifest blob (capnp segment)
}

# ── RPC (object-capability) ────────────────────────────────────────

struct Push {
  id       @0 :Text;
  method   @1 :Text;
  params   @2 :Data;      # method-specific capnp segment
}

struct Resolve {
  id       @0 :Text;
  result   @1 :Data;
}

struct Reject {
  id       @0 :Text;
  code     @1 :Int32;
  message  @2 :Text;
}

# ── JWKS-via-ZAP push ──────────────────────────────────────────────
#
# Replaces the JWKS HTTP polling pattern: consumers send KeysSubscribe
# once, get the current snapshot via KeysSnapshot, then receive
# KeysRotate / KeysRevoke broadcasts whenever IAM mutates a Cert row.
# Eliminates the cache-TTL refetch storm against /.well-known/jwks.

struct KeysSubscribe {
  application @0 :Text;     # empty = global
  cursor      @1 :UInt64;   # last seen rotation seq, 0 = full snapshot
}

struct KeysSnapshot {
  application @0 :Text;
  seq         @1 :UInt64;   # current rotation seq
  jwksJson    @2 :Data;     # canonical JWKS bytes (RFC 7517)
}

struct KeysRotate {
  application @0 :Text;
  seq         @1 :UInt64;
  newKey      @2 :Data;     # JSONWebKey for the new signing cert
}

struct KeysRevoke {
  application @0 :Text;
  seq         @1 :UInt64;
  kid         @2 :Text;     # key id of the revoked entry
}

# ── Control ────────────────────────────────────────────────────────

struct Ping  { nonce @0 :UInt64; }
struct Pong  { nonce @0 :UInt64; }

struct ZapError {
  code    @0 :Int32;
  message @1 :Text;
}
