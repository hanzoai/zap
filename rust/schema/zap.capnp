@0xc0a8b5e0e5f0a000;  # randomly chosen file id

# ZAP canonical schema — matches @zap-proto/zap (TypeScript) and the
# Go luxfi/zap reference. Keep these struct ids stable; bump the file
# id when making wire-incompatible changes.

# ── Connection lifecycle ───────────────────────────────────────────

struct Init @0xa001 {
  clientId  @0 :Text;
  browser   @1 :Text;
  version   @2 :Text;
}

struct InitAck @0xa002 {
  serverId  @0 :Text;
  name      @1 :Text;
  manifest  @2 :Data;     # opaque tool-manifest blob (capnp segment)
}

# ── RPC (object-capability) ────────────────────────────────────────

struct Push @0xa010 {
  id       @0 :Text;
  method   @1 :Text;
  params   @2 :Data;      # method-specific capnp segment
}

struct Resolve @0xa012 {
  id       @0 :Text;
  result   @1 :Data;
}

struct Reject @0xa013 {
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

struct KeysSubscribe @0xa050 {
  application @0 :Text;     # empty = global
  cursor      @1 :UInt64;   # last seen rotation seq, 0 = full snapshot
}

struct KeysSnapshot @0xa051 {
  application @0 :Text;
  seq         @1 :UInt64;   # current rotation seq
  jwksJson    @2 :Data;     # canonical JWKS bytes (RFC 7517)
}

struct KeysRotate @0xa052 {
  application @0 :Text;
  seq         @1 :UInt64;
  newKey      @2 :Data;     # JSONWebKey for the new signing cert
}

struct KeysRevoke @0xa053 {
  application @0 :Text;
  seq         @1 :UInt64;
  kid         @2 :Text;     # key id of the revoked entry
}

# ── Control ────────────────────────────────────────────────────────

struct Ping  @0xa0f0 { nonce @0 :UInt64; }
struct Pong  @0xa0f1 { nonce @0 :UInt64; }

struct ZapError @0xa0ff {
  code    @0 :Int32;
  message @1 :Text;
}
