//! Cross-wire conformance against the canonical Go runtime.
//!
//! Every constant below is the VERBATIM hex output of
//! `github.com/zap-proto/go@v1.8.2`'s `rpc.BuildRequest` /
//! `rpc.BuildResponse` for the stated inputs. Regenerate with
//! `tests/gen_golden.go` (see the file header) and paste — never
//! hand-edit. If Go changes the envelope, these fail here; if Rust
//! changes it, these fail here too. That is the point.

use zapwire::rpc::{self, Call, NO_TARGET, STATUS_NOT_FOUND, STATUS_OK};
use zapwire::{Message, HEADER_SIZE};

/// rpc.Call{Method: 7, PromiseID: 1, Target: NO_TARGET, Cap: "cap-token", Payload: "hello-zap"}
const GO_REQ: &str = "5a415000020000c8100000003e000000070000000100000000000000100000000900000011000000090000006361702d746f6b656e68656c6c6f2d7a6170";

/// rpc.Call{Method: 2, PromiseID: 9, Target: 1, Cap: nil, Payload: nil}
const GO_REQ_EMPTY: &str =
    "5a415000020000c8100000002c00000002000000090000000100000000000000000000000000000000000000";

/// rpc.BuildResponse(STATUS_OK, 1, "world")
const GO_RESP: &str =
    "5a415000020000c81000000029000000c800000001000000000000000800000005000000776f726c64";

/// rpc.BuildResponse(STATUS_NOT_FOUND, 42, nil)
const GO_RESP_EMPTY: &str =
    "5a415000020000c81000000024000000940100002a000000000000000000000000000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ── the read direction: Go builds, Rust parses ──────────────────────

#[test]
fn parses_go_request() {
    let call = rpc::parse_request(&unhex(GO_REQ)).expect("parse go request");
    assert_eq!(call.method, 7);
    assert_eq!(call.promise_id, 1);
    assert_eq!(call.target, NO_TARGET);
    assert_eq!(call.cap, b"cap-token");
    assert_eq!(call.payload, b"hello-zap");
}

#[test]
fn parses_go_request_empty() {
    let call = rpc::parse_request(&unhex(GO_REQ_EMPTY)).expect("parse go request");
    assert_eq!(call.method, 2);
    assert_eq!(call.promise_id, 9);
    assert_eq!(call.target, 1);
    assert!(call.cap.is_empty());
    assert!(call.payload.is_empty());
}

#[test]
fn parses_go_response() {
    let resp = rpc::parse_response(&unhex(GO_RESP)).expect("parse go response");
    assert_eq!(resp.status, STATUS_OK);
    assert_eq!(resp.promise_id, 1);
    assert_eq!(resp.body, b"world");
}

#[test]
fn parses_go_response_empty() {
    let resp = rpc::parse_response(&unhex(GO_RESP_EMPTY)).expect("parse go response");
    assert_eq!(resp.status, STATUS_NOT_FOUND);
    assert_eq!(resp.promise_id, 42);
    assert!(resp.body.is_empty());
}

// ── the write direction: Rust builds, bytes MUST equal Go's ─────────

#[test]
fn builds_go_identical_request() {
    let got = rpc::build_request(&Call {
        method: 7,
        promise_id: 1,
        target: NO_TARGET,
        cap: b"cap-token".to_vec(),
        payload: b"hello-zap".to_vec(),
    });
    assert_eq!(hex(&got), GO_REQ);
}

#[test]
fn builds_go_identical_request_empty() {
    let got = rpc::build_request(&Call {
        method: 2,
        promise_id: 9,
        target: 1,
        cap: Vec::new(),
        payload: Vec::new(),
    });
    assert_eq!(hex(&got), GO_REQ_EMPTY);
}

#[test]
fn builds_go_identical_response() {
    assert_eq!(hex(&rpc::build_response(STATUS_OK, 1, b"world")), GO_RESP);
}

#[test]
fn builds_go_identical_response_empty() {
    assert_eq!(
        hex(&rpc::build_response(STATUS_NOT_FOUND, 42, b"")),
        GO_RESP_EMPTY
    );
}

// ── header-level facts the envelope depends on ─────────────────────

#[test]
fn go_envelope_is_version2_router_tagged() {
    let bytes = unhex(GO_REQ);
    let m = Message::parse(&bytes).expect("v2 header must parse");
    assert_eq!(m.version(), 2, "the rpc envelope is a Version2 header");
    assert_eq!(
        m.flags() >> 8,
        rpc::MSG_TYPE_ROUTER_BASE,
        "router msgType rides the high byte of flags"
    );
    assert_eq!(
        m.root().offset(),
        HEADER_SIZE,
        "the root object starts immediately after the header"
    );
}

/// The stream case: a Go RPC envelope arriving on a socket must be
/// readable frame-by-frame, header first. Before v2 headers were
/// accepted this failed at the very first byte pair — `InvalidVersion(2)`
/// — which is why the Rust side could not talk to `zap-proto/go` at all.
#[test]
fn reads_a_go_envelope_off_a_stream() {
    let bytes = unhex(GO_REQ);
    let mut cur = std::io::Cursor::new(&bytes);
    let (hdr, body) = zapwire::read_frame(&mut cur).expect("must read the Go rpc envelope");
    assert_eq!(hdr.version, 2);
    assert_eq!(hdr.size as usize, bytes.len());
    assert_eq!(body.len(), bytes.len() - HEADER_SIZE);
}
