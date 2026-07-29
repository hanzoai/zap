@0xc0a8b5e0e5f0a000;

# Canonical ZAP schema. Compiled by zapc (../zapc-rs) at build time
# into `OUT_DIR/zap_schema.rs`, surfaced via `zap::proto`.
#
# Layout policy: fields packed in declaration order with primitive
# alignment, struct slot region 8-byte aligned. Text/Data fields use
# an i32 LE relative offset + u32 LE length at the field slot;
# payload bytes follow the slot region. Matches Go luxfi/zap.

# ── Connection lifecycle ───────────────────────────────────────────
struct Init {
    clientId @0 :Text;
    browser  @1 :Text;
    version  @2 :Text;
}

struct InitAck {
    serverId @0 :Text;
    name     @1 :Text;
    manifest @2 :Data;
}

# ── RPC ────────────────────────────────────────────────────────────
struct Push {
    id     @0 :Text;
    method @1 :Text;
    params @2 :Data;
}

struct Resolve {
    id     @0 :Text;
    result @1 :Data;
}

struct Reject {
    id      @0 :Text;
    code    @1 :Int32;
    message @2 :Text;
}

# ── JWKS-via-ZAP push ─────────────────────────────────────────────
# Replaces /.well-known/jwks polling. Subscribers cache locally and
# receive deltas on every Cert mutation in IAM.

struct KeysSubscribe {
    application @0 :Text;
    cursor      @1 :UInt64;
}

struct KeysSnapshot {
    application @0 :Text;
    seq         @1 :UInt64;
    jwksJson    @2 :Data;
}

struct KeysRotate {
    application @0 :Text;
    seq         @1 :UInt64;
    newKey      @2 :Data;
}

struct KeysRevoke {
    application @0 :Text;
    seq         @1 :UInt64;
    kid         @2 :Text;
}

# ── Control ────────────────────────────────────────────────────────
struct Ping { nonce @0 :UInt64; }
struct Pong { nonce @0 :UInt64; }

struct ZapError {
    code    @0 :Int32;
    message @1 :Text;
}
