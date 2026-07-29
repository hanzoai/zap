//! The ZAP call envelope — the method + capability framing that carries
//! one interface method call and its answer over a ZAP transport.
//!
//! This is the canonical wire contract, mirrored byte-for-byte from
//! `github.com/zap-proto/go@v1.8.2`'s `rpc` package. A request built here
//! is decoded by Go's `ParseRequest`, and a response built by Go's
//! `BuildResponse` is decoded by [`parse_response`] here.
//! `tests/crosswire.rs` pins Go's actual output as the assertion.
//!
//! Request object (fixed size 28):
//!
//! ```text
//! Method    u32   @0    which interface method (the .zap ordinal, 1-based)
//! PromiseID u32   @4    caller-assigned id this call's answer resolves to
//! Target    u32   @8    promise this call pipelines off (0 = root)
//! Cap       bytes @12   opaque capability buffer (may be empty)
//! Payload   bytes @20   ZAP-encoded method params
//! ```
//!
//! Response object (fixed size 20):
//!
//! ```text
//! Status    u32   @0    200 ok, else an error code
//! PromiseID u32   @4    echoes the request's PromiseID
//! Body      bytes @12   ZAP-encoded results (empty for a void method)
//! ```
//!
//! Both envelopes are finished with header flags = `MSG_TYPE_ROUTER_BASE
//! << 8` so a ZAP transport routes them to this service's handler
//! (`msg_type = flags >> 8`).

use crate::codec::{Builder, Message};
use crate::wire::WireError;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// This service's ZAP message-type slot, carried in the high byte of
/// the header flags word.
pub const MSG_TYPE_ROUTER_BASE: u16 = 200;

/// The `target` of a call that does not pipeline off an earlier promise
/// (it targets the bootstrap object).
pub const NO_TARGET: u32 = 0;

/// Status codes carried in a response.
pub const STATUS_OK: u32 = 200;
pub const STATUS_BAD_REQUEST: u32 = 400;
pub const STATUS_UNAUTHORIZED: u32 = 401;
pub const STATUS_FORBIDDEN: u32 = 403;
pub const STATUS_NOT_FOUND: u32 = 404;
pub const STATUS_INTERNAL: u32 = 500;

const REQ_METHOD: usize = 0;
const REQ_PROMISE_ID: usize = 4;
const REQ_TARGET: usize = 8;
const REQ_CAP: usize = 12;
const REQ_PAYLOAD: usize = 20;
const REQ_FIXED: usize = 28;

const RESP_STATUS: usize = 0;
const RESP_PROMISE_ID: usize = 4;
const RESP_BODY: usize = 12;
const RESP_FIXED: usize = 20;

/// One outbound request's fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Call {
    pub method: u32,
    pub promise_id: u32,
    pub target: u32,
    pub cap: Vec<u8>,
    pub payload: Vec<u8>,
}

/// A decoded response envelope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Response {
    pub status: u32,
    pub promise_id: u32,
    pub body: Vec<u8>,
}

impl Response {
    /// True when the answer is usable (`status == 200`).
    pub fn is_ok(&self) -> bool {
        self.status == STATUS_OK
    }
}

/// Encode a [`Call`] into a router-tagged ZAP message.
pub fn build_request(c: &Call) -> Vec<u8> {
    let mut b = Builder::v2(c.cap.len() + c.payload.len() + REQ_FIXED + 64);
    {
        let mut ob = b.object(REQ_FIXED);
        ob.set_u32(REQ_METHOD, c.method);
        ob.set_u32(REQ_PROMISE_ID, c.promise_id);
        ob.set_u32(REQ_TARGET, c.target);
        ob.set_bytes(REQ_CAP, &c.cap);
        ob.set_bytes(REQ_PAYLOAD, &c.payload);
        ob.finish_as_root();
    }
    b.finish_with_flags(MSG_TYPE_ROUTER_BASE << 8)
}

/// Decode a router-tagged request message.
pub fn parse_request(msg: &[u8]) -> Result<Call, WireError> {
    let m = Message::parse(msg)?;
    let r = m.root();
    Ok(Call {
        method: r.u32(REQ_METHOD),
        promise_id: r.u32(REQ_PROMISE_ID),
        target: r.u32(REQ_TARGET),
        cap: r.bytes(REQ_CAP).to_vec(),
        payload: r.bytes(REQ_PAYLOAD).to_vec(),
    })
}

/// Encode a status + body into a router-tagged response.
pub fn build_response(status: u32, promise_id: u32, body: &[u8]) -> Vec<u8> {
    let mut b = Builder::v2(body.len() + RESP_FIXED + 64);
    {
        let mut ob = b.object(RESP_FIXED);
        ob.set_u32(RESP_STATUS, status);
        ob.set_u32(RESP_PROMISE_ID, promise_id);
        ob.set_bytes(RESP_BODY, body);
        ob.finish_as_root();
    }
    b.finish_with_flags(MSG_TYPE_ROUTER_BASE << 8)
}

/// Decode a router-tagged response message.
pub fn parse_response(msg: &[u8]) -> Result<Response, WireError> {
    let m = Message::parse(msg)?;
    let r = m.root();
    Ok(Response {
        status: r.u32(RESP_STATUS),
        promise_id: r.u32(RESP_PROMISE_ID),
        body: r.bytes(RESP_BODY).to_vec(),
    })
}

// ── promise pipelining ──────────────────────────────────────────────
//
// A call carries a caller-assigned PromiseID (the id its answer
// resolves to). A DEPENDENT call sets `target` to a prior call's
// PromiseID, meaning "before you dispatch me, substitute the resolved
// body of the call that answered to that PromiseID as my payload". The
// result of A is the input to B, so B ships back-to-back with A — no
// round trip threads A's answer through the caller first.
//
// Two pieces, each in one place: [`Session`] (client) allocates
// PromiseIDs and stamps `target`; [`Pipeliner`] (server) is the promise
// table that resolves `target` before dispatch. Both are
// transport-agnostic — the same model works in-process and over a
// socket. `NO_TARGET` (0) is a plain call, so a pipelining and a
// non-pipelining peer are byte-compatible.

/// A handle to the answer of an in-flight call. Use its `id` as the
/// `target` of a dependent call to pipeline on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Promise {
    pub id: u32,
}

/// The client half of pipelining: a monotonic PromiseID allocator
/// scoped to one connection. IDs are unique and never `NO_TARGET`.
#[derive(Debug, Default)]
pub struct Session {
    next: AtomicU32,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a fresh, unique, non-zero PromiseID.
    pub fn next(&self) -> Promise {
        let mut id = self.next.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        if id == NO_TARGET {
            // wrapped past 2^32-1 back to 0 — skip it.
            id = self.next.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        }
        Promise { id }
    }

    /// The originating call of a pipeline: fresh PromiseID, no target.
    pub fn origin(&self, p: Promise, method: u32, cap: &[u8], payload: &[u8]) -> Call {
        Call {
            method,
            promise_id: p.id,
            target: NO_TARGET,
            cap: cap.to_vec(),
            payload: payload.to_vec(),
        }
    }

    /// A dependent call pipelining on `target`'s answer. The server
    /// substitutes `target`'s resolved body for this call's payload, so
    /// `payload` here is only the part NOT supplied upstream (often
    /// empty — the whole input is the upstream result).
    pub fn pipeline(
        &self,
        p: Promise,
        target: Promise,
        method: u32,
        cap: &[u8],
        payload: &[u8],
    ) -> Call {
        Call {
            method,
            promise_id: p.id,
            target: target.id,
            cap: cap.to_vec(),
            payload: payload.to_vec(),
        }
    }
}

/// Turns a request envelope into a response envelope — the server-side
/// contract, and the ONE thing [`crate::transport`] serves.
///
/// A plain `Fn(&[u8]) -> Result<Vec<u8>, WireError>` already satisfies
/// it (that is the shape a generated `dispatch_<iface>` has), so most
/// handlers are written as closures. The trait is async because a
/// [`Pipeliner`] genuinely has to WAIT — a dependent call that arrives
/// before its target parks until the target answers.
#[async_trait]
pub trait Dispatch: Send + Sync + 'static {
    async fn dispatch(&self, envelope: &[u8]) -> Result<Vec<u8>, WireError>;
}

#[async_trait]
impl<F> Dispatch for F
where
    F: Fn(&[u8]) -> Result<Vec<u8>, WireError> + Send + Sync + 'static,
{
    async fn dispatch(&self, envelope: &[u8]) -> Result<Vec<u8>, WireError> {
        self(envelope)
    }
}

/// The server-side promise table for one connection: a [`Dispatch`]
/// that resolves `target` references before handing the call on.
///
/// Wrap the real dispatcher and serve the Pipeliner instead — it is a
/// `Dispatch` itself, so nothing else in the stack changes:
///
/// ```ignore
/// let handler = Arc::new(Pipeliner::new(dispatch_echo));
/// Server::listen(addr, handler).await?;
/// ```
///
/// A call with `target == NO_TARGET` passes straight through.
/// Otherwise the target decides:
///
/// - resolved OK — its body replaces the dependent's payload and the
///   call dispatches immediately;
/// - answered non-OK, or [`Pipeliner::finish`]ed — refused with
///   `STATUS_BAD_REQUEST`, because the target can never produce a
///   result to pipeline on;
/// - unknown — the call PARKS until a later dispatch resolves it (the
///   dependent legitimately arrived first), then answers normally.
///
/// A parked call is released by its target's resolution, its target's
/// failure, or [`Pipeliner::finish`] — so it never hangs forever.
pub struct Pipeliner<D: Dispatch> {
    inner: D,
    state: Mutex<Table>,
}

#[derive(Default)]
struct Table {
    resolved: HashMap<u32, Vec<u8>>,
    failed: HashSet<u32>,
    finished: HashSet<u32>,
    parked: HashMap<u32, Vec<Parked>>,
}

/// A dependent call waiting on its target, plus the channel its own
/// answer is delivered on.
struct Parked {
    call: Call,
    answer: tokio::sync::oneshot::Sender<Vec<u8>>,
}

#[async_trait]
impl<D: Dispatch> Dispatch for Pipeliner<D> {
    async fn dispatch(&self, envelope: &[u8]) -> Result<Vec<u8>, WireError> {
        let call = parse_request(envelope)?;
        if call.target == NO_TARGET {
            return self.run(call).await;
        }
        let waiting = {
            let mut st = self.state.lock().unwrap();
            if let Some(body) = st.resolved.get(&call.target) {
                let mut c = call;
                c.payload = body.clone();
                Some(Ok(c))
            } else if st.failed.contains(&call.target) || st.finished.contains(&call.target) {
                // The target can never produce a result to pipeline on.
                return Ok(build_response(STATUS_BAD_REQUEST, call.promise_id, &[]));
            } else {
                let (tx, rx) = tokio::sync::oneshot::channel();
                st.parked
                    .entry(call.target)
                    .or_default()
                    .push(Parked { call, answer: tx });
                Some(Err(rx))
            }
        };
        match waiting {
            Some(Ok(c)) => self.run(c).await,
            Some(Err(rx)) => rx.await.map_err(|_| WireError::Closed),
            None => unreachable!(),
        }
    }
}

impl<D: Dispatch> Pipeliner<D> {
    pub fn new(inner: D) -> Self {
        Pipeliner {
            inner,
            state: Mutex::new(Table::default()),
        }
    }

    /// Drop the cached answer for `id` once no further call will
    /// pipeline on it (the ZAP analogue of capnp's Finish). Calling it
    /// is optional: without it, every OK answer is retained for the
    /// connection's lifetime so a late dependent still finds its
    /// target. A long-lived connection that pipelines heavily should
    /// finish each promise it is done with, to bound the table.
    ///
    /// After this, `id` is terminal: any dependent targeting it —
    /// already parked or arriving later — is refused rather than hung.
    pub fn finish(&self, id: u32) {
        let woken = {
            let mut st = self.state.lock().unwrap();
            st.resolved.remove(&id);
            st.failed.remove(&id);
            st.finished.insert(id);
            st.parked.remove(&id).unwrap_or_default()
        };
        for p in woken {
            let _ = p
                .answer
                .send(build_response(STATUS_BAD_REQUEST, p.call.promise_id, &[]));
        }
    }

    /// Dispatch one resolved call, record its answer under its own
    /// PromiseID, and release everything parked on that id.
    fn run<'a>(
        &'a self,
        call: Call,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, WireError>> + Send + 'a>>
    {
        Box::pin(async move {
            let id = call.promise_id;
            // Re-encode: the payload may have been substituted, and the
            // dispatcher must see what it is actually being asked.
            let bytes = match self.inner.dispatch(&build_request(&call)).await {
                Ok(b) => b,
                Err(e) => {
                    self.poison(id);
                    return Err(e);
                }
            };
            let resp = match parse_response(&bytes) {
                Ok(r) => r,
                Err(e) => {
                    self.poison(id);
                    return Err(e);
                }
            };
            self.record(id, resp).await;
            Ok(bytes)
        })
    }

    /// Store the answer under `id` and release every dependent parked
    /// on it. An OK answer caches its body (what dependents pipeline
    /// on); a non-OK answer marks `id` failed so its dependents are
    /// refused, not hung.
    async fn record(&self, id: u32, resp: Response) {
        let woken = {
            let mut st = self.state.lock().unwrap();
            if resp.status == STATUS_OK {
                st.resolved.insert(id, resp.body.clone());
            } else {
                st.failed.insert(id);
            }
            st.parked.remove(&id).unwrap_or_default()
        };
        for p in woken {
            if resp.status == STATUS_OK {
                let mut c = p.call;
                c.payload = resp.body.clone();
                if let Ok(bytes) = self.run(c).await {
                    let _ = p.answer.send(bytes);
                }
            } else {
                let _ = p
                    .answer
                    .send(build_response(STATUS_BAD_REQUEST, p.call.promise_id, &[]));
            }
        }
    }

    /// Dispatch itself failed (not a normal non-OK answer): the id can
    /// never resolve, so refuse everything parked on it.
    fn poison(&self, id: u32) {
        let woken = {
            let mut st = self.state.lock().unwrap();
            st.failed.insert(id);
            st.parked.remove(&id).unwrap_or_default()
        };
        for p in woken {
            let _ = p
                .answer
                .send(build_response(STATUS_BAD_REQUEST, p.call.promise_id, &[]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let c = Call {
            method: 3,
            promise_id: 11,
            target: 7,
            cap: b"cap".to_vec(),
            payload: b"body".to_vec(),
        };
        assert_eq!(parse_request(&build_request(&c)).unwrap(), c);
    }

    #[test]
    fn response_roundtrip() {
        let bytes = build_response(STATUS_FORBIDDEN, 5, b"nope");
        let r = parse_response(&bytes).unwrap();
        assert_eq!(r.status, STATUS_FORBIDDEN);
        assert_eq!(r.promise_id, 5);
        assert_eq!(r.body, b"nope");
        assert!(!r.is_ok());
    }

    #[test]
    fn session_ids_are_unique_and_non_zero() {
        let s = Session::new();
        let a = s.next();
        let b = s.next();
        assert_eq!((a.id, b.id), (1, 2));
        assert_ne!(a.id, NO_TARGET);
    }

    fn echo_upper() -> impl Dispatch {
        |env: &[u8]| -> Result<Vec<u8>, WireError> {
            let c = parse_request(env)?;
            let body = String::from_utf8_lossy(&c.payload).to_uppercase();
            Ok(build_response(STATUS_OK, c.promise_id, body.as_bytes()))
        }
    }

    #[tokio::test]
    async fn pipelines_a_dependent_on_a_resolved_target() {
        let p = Pipeliner::new(echo_upper());
        let s = Session::new();
        let a = s.next();
        let b = s.next();

        let r1 = p
            .dispatch(&build_request(&s.origin(a, 1, &[], b"hello")))
            .await
            .unwrap();
        assert_eq!(parse_response(&r1).unwrap().body, b"HELLO");

        // The dependent carries NO payload — the server substitutes the
        // resolved answer of `a`.
        let r2 = p
            .dispatch(&build_request(&s.pipeline(b, a, 1, &[], &[])))
            .await
            .unwrap();
        assert_eq!(parse_response(&r2).unwrap().body, b"HELLO");
    }

    /// The dependent arrives FIRST. It must park, not fail, and answer
    /// as soon as its target resolves — this is what makes back-to-back
    /// shipping safe when the two calls race on the wire.
    #[tokio::test]
    async fn parks_a_dependent_that_arrives_before_its_target() {
        let p = std::sync::Arc::new(Pipeliner::new(echo_upper()));
        let s = std::sync::Arc::new(Session::new());
        let a = s.next();
        let b = s.next();

        let dep = {
            let (p, s) = (p.clone(), s.clone());
            tokio::spawn(async move {
                p.dispatch(&build_request(&s.pipeline(b, a, 1, &[], &[])))
                    .await
            })
        };
        // Give the dependent a chance to park before the origin lands.
        tokio::task::yield_now().await;

        let r1 = p
            .dispatch(&build_request(&s.origin(a, 1, &[], b"late")))
            .await
            .unwrap();
        assert_eq!(parse_response(&r1).unwrap().body, b"LATE");

        let r2 = parse_response(&dep.await.unwrap().unwrap()).unwrap();
        assert_eq!(r2.promise_id, b.id);
        assert_eq!(r2.body, b"LATE");
    }

    #[tokio::test]
    async fn refuses_a_dependent_on_a_finished_target() {
        let p = Pipeliner::new(echo_upper());
        let s = Session::new();
        let a = s.next();
        let b = s.next();

        p.dispatch(&build_request(&s.origin(a, 1, &[], b"x")))
            .await
            .unwrap();
        p.finish(a.id);

        let r = p
            .dispatch(&build_request(&s.pipeline(b, a, 1, &[], &[])))
            .await
            .unwrap();
        assert_eq!(parse_response(&r).unwrap().status, STATUS_BAD_REQUEST);
    }

    #[tokio::test]
    async fn refuses_a_dependent_on_a_failed_target() {
        let deny = |env: &[u8]| -> Result<Vec<u8>, WireError> {
            let c = parse_request(env)?;
            Ok(build_response(STATUS_FORBIDDEN, c.promise_id, &[]))
        };
        let p = Pipeliner::new(deny);
        let s = Session::new();
        let a = s.next();
        let b = s.next();

        p.dispatch(&build_request(&s.origin(a, 1, &[], b"x")))
            .await
            .unwrap();
        let r = p
            .dispatch(&build_request(&s.pipeline(b, a, 1, &[], &[])))
            .await
            .unwrap();
        assert_eq!(parse_response(&r).unwrap().status, STATUS_BAD_REQUEST);
    }

    /// A parked dependent must be released — with a refusal — when its
    /// target is finished before it ever resolves. Otherwise the call
    /// hangs for the life of the connection.
    #[tokio::test]
    async fn finish_releases_an_already_parked_dependent() {
        let p = std::sync::Arc::new(Pipeliner::new(echo_upper()));
        let s = std::sync::Arc::new(Session::new());
        let a = s.next();
        let b = s.next();

        let dep = {
            let (p, s) = (p.clone(), s.clone());
            tokio::spawn(async move {
                p.dispatch(&build_request(&s.pipeline(b, a, 1, &[], &[])))
                    .await
            })
        };
        tokio::task::yield_now().await;
        p.finish(a.id);

        let r = parse_response(&dep.await.unwrap().unwrap()).unwrap();
        assert_eq!(r.status, STATUS_BAD_REQUEST);
        assert_eq!(r.promise_id, b.id);
    }
}
