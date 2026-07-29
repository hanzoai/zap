//! The network plumbing for ZAP RPC: it carries [`crate::rpc`]
//! Call/Response envelopes over a real connection and correlates each
//! answer to its call by PromiseID.
//!
//! This is the Rust half of `github.com/zap-proto/go/transport` — the
//! same frames, in the same order, with the same rules — so a Rust
//! client talks to a Go server and vice versa. `tests/interop.rs`
//! proves it against a live Go peer.
//!
//! Framing is length-prefixed and direction-tagged, so one connection
//! is fully SYMMETRIC — either end may call and serve:
//!
//! ```text
//! [ u32 LE len ][ 1 byte dir ][ rpc envelope bytes ]   len = 1 + envelope
//! ```
//!
//! Correlation lives in the envelope itself (`Call.promise_id` /
//! `Response.promise_id`), so the transport adds no correlation header
//! of its own — it reads the PromiseID out of the bytes it is already
//! shipping.
//!
//! ## The address IS the plumbing
//!
//! There is ONE verb in each direction — [`Conn::connect`] and
//! [`Server::listen`] — and the address shape selects the socket, the
//! way `net.Listen(network, addr)` does with the network as a value:
//!
//! ```text
//! "127.0.0.1:9999"          -> TCP
//! "/run/hanzo/app.sock"     -> Unix domain socket
//! ```
//!
//! ## Confidentiality
//!
//! These frames are PLAINTEXT. A Unix socket is confined to the host
//! and its permissions, which is why LOCAL ZAP is correct today. A TCP
//! peer is NOT protected: do not point this at a remote host until the
//! session-crypto work lands. The Go plane already has its half
//! (`transport.DialTLS` pins the X25519MLKEM768 hybrid and refuses a
//! classical-only peer); Rust has no TLS sibling yet. See `README.md`
//! § Session crypto.

use crate::rpc::{self, Dispatch, Response, Session};
use crate::wire::WireError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::sync::oneshot;

/// A request carries a Call envelope and expects a correlated answer.
const DIR_REQUEST: u8 = 1;
/// A response answers a prior request's PromiseID.
const DIR_RESPONSE: u8 = 2;

/// Bounds one frame so a hostile or corrupt length prefix cannot drive
/// an unbounded allocation. Matches the Go transport.
pub const MAX_FRAME: usize = 64 << 20;

/// Bounds the inbound requests dispatched concurrently on one
/// connection. At the cap the read loop stops reading — socket
/// backpressure — instead of spawning unbounded tasks.
pub const MAX_IN_FLIGHT: usize = 256;

/// The canonical ZAP port. Like 80 means HTTP, 9999 means ZAP.
pub const DEFAULT_PORT: u16 = 9999;

type Pipe = Box<dyn Duplex>;

/// The byte pipe a [`Conn`] muxes frames over. Blanket-implemented for
/// `TcpStream` and `UnixStream`; the trait exists so ONE connection
/// implementation serves both, instead of a generic that has to be
/// instantiated once per socket type.
pub trait Duplex: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> Duplex for T {}

/// One ZAP-RPC connection. Symmetric: either end may call and serve.
/// Safe for concurrent use — clone the `Arc` and call from anywhere.
pub struct Conn {
    w: tokio::sync::Mutex<WriteHalf<Pipe>>,
    pending: Mutex<HashMap<u32, oneshot::Sender<Response>>>,
    session: Session,
    closed: AtomicBool,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl Conn {
    /// Dial `addr` and return a call-only connection.
    ///
    /// A path (`/run/hanzo/app.sock`) is a Unix socket; anything else
    /// is `host:port`.
    pub async fn connect(addr: &str) -> Result<Arc<Conn>, WireError> {
        Self::connect_serve(addr, None::<NoDispatch>).await
    }

    /// [`Conn::connect`] plus an inbound [`Dispatch`], for a peer that
    /// both calls and serves over one socket.
    ///
    /// This is the client half of SERVER PUSH: the server calls back
    /// over the same connection and this dispatcher answers, so a
    /// subscription notice needs no second wire and no second protocol.
    pub async fn connect_serve<D: Dispatch>(
        addr: &str,
        dispatch: Option<D>,
    ) -> Result<Arc<Conn>, WireError> {
        let pipe: Pipe = if is_path(addr) {
            Box::new(UnixStream::connect(addr).await?)
        } else {
            Box::new(TcpStream::connect(addr).await?)
        };
        let (conn, rd, rx) = Conn::split(pipe);
        tokio::spawn(read_loop(conn.clone(), rd, dispatch.map(Arc::new), rx));
        Ok(conn)
    }

    /// Build a connection over an established byte pipe WITHOUT
    /// starting its read loop, and hand back the pieces the loop needs.
    ///
    /// The split exists so a server can hand the live `Conn` to the
    /// code that builds its dispatcher — a handler that can call BACK
    /// must exist before the first inbound frame is read, or the push
    /// path would race the first request.
    fn split(
        pipe: Pipe,
    ) -> (
        Arc<Conn>,
        ReadHalf<Pipe>,
        tokio::sync::watch::Receiver<bool>,
    ) {
        let (rd, w) = tokio::io::split(pipe);
        let (shutdown, rx) = tokio::sync::watch::channel(false);
        let conn = Arc::new(Conn {
            w: tokio::sync::Mutex::new(w),
            pending: Mutex::new(HashMap::new()),
            session: Session::new(),
            closed: AtomicBool::new(false),
            shutdown,
        });
        (conn, rd, rx)
    }

    /// The PromiseID allocator for this connection. A generated client
    /// carries its own; a hand-written caller pipelines with it:
    ///
    /// ```ignore
    /// let a = conn.session().next();
    /// let b = conn.session().next();
    /// let fa = conn.call(&build_request(&conn.session().origin(a, LOAD, &[], key)));
    /// let fb = conn.call(&build_request(&conn.session().pipeline(b, a, RENDER, &[], &[])));
    /// let (ra, rb) = tokio::join!(fa, fb);   // both ship before either answers
    /// ```
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Ship a pre-built request envelope and wait for its answer. This
    /// is the channel contract a generated typed client consumes, and
    /// it is byte-identical to Go's `Conn.Call`.
    pub async fn call(&self, envelope: &[u8]) -> Result<Response, WireError> {
        let call = rpc::parse_request(envelope)?;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(call.promise_id, tx);
        if let Err(e) = self.write_frame(DIR_REQUEST, envelope).await {
            self.pending.lock().unwrap().remove(&call.promise_id);
            return Err(e);
        }
        // A closed connection drops every pending sender, so a waiter is
        // released with `Closed` rather than hanging.
        rx.await.map_err(|_| WireError::Closed)
    }

    /// Call `method` with `payload`, allocating this call's PromiseID
    /// from the connection's [`Session`]. This is the hand-written
    /// path; generated clients build their own envelope and use
    /// [`Conn::call`].
    pub async fn invoke(
        &self,
        method: u32,
        cap: &[u8],
        payload: &[u8],
    ) -> Result<Response, WireError> {
        let p = self.session.next();
        let env = rpc::build_request(&self.session.origin(p, method, cap, payload));
        self.call(&env).await
    }

    /// Whether the connection has been torn down — locally by
    /// [`Conn::close`], or by the peer / a read error. A pool uses it
    /// to evict and redial dead entries.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Shut the connection down and fail every in-flight call with
    /// [`WireError::Closed`]. Idempotent.
    pub fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.shutdown.send(true);
        // Dropping the senders releases every waiter with `Closed`.
        self.pending.lock().unwrap().clear();
    }

    async fn write_frame(&self, dir: u8, envelope: &[u8]) -> Result<(), WireError> {
        if self.is_closed() {
            return Err(WireError::Closed);
        }
        let n = 1 + envelope.len();
        if n > MAX_FRAME {
            return Err(WireError::SizeExceeded {
                got: n,
                max: MAX_FRAME,
            });
        }
        let mut hdr = [0u8; 5];
        hdr[0..4].copy_from_slice(&(n as u32).to_le_bytes());
        hdr[4] = dir;

        // One writer at a time, so concurrent calls and inbound
        // answers never interleave on the wire.
        let mut w = self.w.lock().await;
        w.write_all(&hdr).await?;
        w.write_all(envelope).await?;
        w.flush().await?;
        Ok(())
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// A [`Dispatch`] that can never be constructed — the type argument
/// for a call-only [`Conn::connect_serve`].
pub enum NoDispatch {}

#[async_trait::async_trait]
impl Dispatch for NoDispatch {
    async fn dispatch(&self, _envelope: &[u8]) -> Result<Vec<u8>, WireError> {
        match *self {}
    }
}

async fn read_loop<D: Dispatch>(
    conn: Arc<Conn>,
    mut rd: ReadHalf<Pipe>,
    dispatch: Option<Arc<D>>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let sem = Arc::new(tokio::sync::Semaphore::new(MAX_IN_FLIGHT));
    loop {
        let frame = tokio::select! {
            f = read_frame(&mut rd) => f,
            _ = shutdown.changed() => break,
        };
        let (dir, body) = match frame {
            Ok(v) => v,
            Err(_) => break, // EOF or read error tears the connection down
        };
        match dir {
            DIR_RESPONSE => {
                let Ok(resp) = rpc::parse_response(&body) else {
                    continue; // skip a malformed answer, keep the connection
                };
                let waiter = conn.pending.lock().unwrap().remove(&resp.promise_id);
                if let Some(tx) = waiter {
                    let _ = tx.send(resp);
                }
            }
            DIR_REQUEST => {
                let Some(d) = dispatch.clone() else {
                    continue; // call-only endpoint: ignore inbound requests
                };
                // Take the in-flight slot BEFORE spawning: at the cap
                // this awaits, so the peer feels backpressure instead of
                // our task count growing without bound.
                let Ok(permit) = sem.clone().acquire_owned().await else {
                    break;
                };
                let conn = conn.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    serve(conn, d, body).await;
                });
            }
            // Stream frames (3/4/5 in the Go transport) have no Rust
            // realisation yet; ignoring them keeps a peer that opens one
            // from killing the unary traffic on the same connection.
            _ => continue,
        }
    }
    conn.close();
}

/// Dispatch one inbound request and write its answer. A dispatch error
/// still produces a `STATUS_INTERNAL` answer, so the caller unblocks
/// rather than waiting out its timeout.
async fn serve<D: Dispatch>(conn: Arc<Conn>, dispatch: Arc<D>, envelope: Vec<u8>) {
    let resp = match dispatch.dispatch(&envelope).await {
        Ok(bytes) => bytes,
        Err(_) => match rpc::parse_request(&envelope) {
            Ok(call) => rpc::build_response(rpc::STATUS_INTERNAL, call.promise_id, &[]),
            Err(_) => return, // unparseable: nothing to answer to
        },
    };
    let _ = conn.write_frame(DIR_RESPONSE, &resp).await;
}

/// Read one direction-tagged, length-prefixed frame.
async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<(u8, Vec<u8>), WireError> {
    let mut hdr = [0u8; 5];
    r.read_exact(&mut hdr).await?;
    let n = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    if n < 1 || n > MAX_FRAME {
        return Err(WireError::SizeExceeded {
            got: n,
            max: MAX_FRAME,
        });
    }
    let mut body = vec![0u8; n - 1]; // n counts the direction byte
    r.read_exact(&mut body).await?;
    Ok((hdr[4], body))
}

/// Accepts ZAP-RPC connections and serves a [`Dispatch`] on each.
///
/// The argument is a CONNECTION HANDLER: given the live [`Conn`], it
/// returns the dispatcher that connection serves. A handler that
/// ignores the connection is the ordinary unary server —
/// `Server::listen(addr, |_| dispatch_echo)` — while one that keeps it
/// can call the peer back, which is what a server push IS. One verb,
/// both shapes.
pub struct Server {
    addr: String,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl Server {
    /// Bind `addr` and serve every accepted connection with the
    /// dispatcher `handler` builds for it. A path is a Unix socket (a
    /// stale socket file from a crashed predecessor is cleared first);
    /// anything else is `host:port`.
    pub async fn listen<D, F>(addr: &str, handler: F) -> Result<Server, WireError>
    where
        D: Dispatch,
        F: Fn(Arc<Conn>) -> D + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        let (shutdown, mut rx) = tokio::sync::watch::channel(false);

        let bound = if is_path(addr) {
            remove_stale_socket(addr)?;
            let ln = UnixListener::bind(addr)?;
            let bound = addr.to_string();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        a = ln.accept() => match a {
                            Ok((s, _)) => accept(Box::new(s), &handler),
                            Err(_) => break,
                        },
                        _ = rx.changed() => break,
                    }
                }
            });
            bound
        } else {
            let ln = TcpListener::bind(addr).await?;
            let bound = ln.local_addr()?.to_string();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        a = ln.accept() => match a {
                            Ok((s, _)) => accept(Box::new(s), &handler),
                            Err(_) => break,
                        },
                        _ = rx.changed() => break,
                    }
                }
            });
            bound
        };

        Ok(Server {
            addr: bound,
            shutdown,
        })
    }

    /// The bound address — the resolved one, so `127.0.0.1:0` reports
    /// the port the OS picked.
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Stop accepting. Live connections finish their in-flight work and
    /// end when the peer disconnects.
    pub fn close(&self) {
        let _ = self.shutdown.send(true);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.close();
    }
}

/// Wire one accepted pipe: build the Conn, let the handler see it
/// (so it can push), THEN start reading. The order matters — a
/// dispatcher built after the read loop started could miss the first
/// inbound frame.
fn accept<D, F>(pipe: Pipe, handler: &Arc<F>)
where
    D: Dispatch,
    F: Fn(Arc<Conn>) -> D + Send + Sync + 'static,
{
    let (conn, rd, rx) = Conn::split(pipe);
    let dispatch = Arc::new(handler(conn.clone()));
    tokio::spawn(read_loop(conn, rd, Some(dispatch), rx));
}

/// A filesystem path is a Unix socket; anything else is `host:port`.
/// The same rule the Go plane and `zip` use, so an address means the
/// same thing everywhere.
fn is_path(addr: &str) -> bool {
    addr.starts_with('/') || addr.starts_with("./") || addr.starts_with("../")
}

/// Remove `path` only if it exists AND is a socket, so a crashed
/// predecessor's inode cannot block the bind — and a misconfigured
/// address can never delete a real file.
fn remove_stale_socket(path: &str) -> Result<(), WireError> {
    use std::os::unix::fs::FileTypeExt;
    if let Ok(md) = std::fs::metadata(path) {
        if md.file_type().is_socket() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::{build_request, build_response, parse_request, STATUS_OK};

    fn echo_upper(env: &[u8]) -> Result<Vec<u8>, WireError> {
        let c = parse_request(env)?;
        let body = String::from_utf8_lossy(&c.payload).to_uppercase();
        Ok(build_response(STATUS_OK, c.promise_id, body.as_bytes()))
    }

    #[tokio::test]
    async fn round_trip_over_tcp() {
        let srv = Server::listen("127.0.0.1:0", |_| echo_upper).await.unwrap();
        let conn = Conn::connect(srv.addr()).await.unwrap();

        let r = conn.invoke(7, &[], b"hello").await.unwrap();
        assert_eq!(r.status, STATUS_OK);
        assert_eq!(r.body, b"HELLO");
        assert_eq!(r.promise_id, 1);
    }

    #[tokio::test]
    async fn round_trip_over_unix_socket() {
        let dir = std::env::temp_dir().join(format!("zap-uds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zap.sock");
        let sock = path.to_str().unwrap();

        let srv = Server::listen(sock, |_| echo_upper).await.unwrap();
        let conn = Conn::connect(sock).await.unwrap();

        let r = conn.invoke(1, &[], b"unix").await.unwrap();
        assert_eq!(r.body, b"UNIX");
        srv.close();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Concurrent calls on ONE connection must each get their OWN
    /// answer — correlation is by PromiseID, not by arrival order.
    #[tokio::test]
    async fn correlates_concurrent_calls_by_promise_id() {
        let srv = Server::listen("127.0.0.1:0", |_| echo_upper).await.unwrap();
        let conn = Conn::connect(srv.addr()).await.unwrap();

        let mut tasks = Vec::new();
        for i in 0..32u32 {
            let conn = conn.clone();
            tasks.push(tokio::spawn(async move {
                let body = format!("call-{i}");
                let r = conn.invoke(1, &[], body.as_bytes()).await.unwrap();
                assert_eq!(r.body, body.to_uppercase().as_bytes());
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
    }

    /// Two calls shipped back-to-back where the second pipelines on the
    /// first: the server substitutes the first answer as the second
    /// payload, so the caller never round-trips the intermediate value.
    #[tokio::test]
    async fn pipelines_two_calls_over_one_connection() {
        let srv = Server::listen("127.0.0.1:0", |_| rpc::Pipeliner::new(echo_upper))
            .await
            .unwrap();
        let conn = Conn::connect(srv.addr()).await.unwrap();

        let a = conn.session().next();
        let b = conn.session().next();
        let origin = build_request(&conn.session().origin(a, 1, &[], b"chain"));
        let dependent = build_request(&conn.session().pipeline(b, a, 1, &[], &[]));
        let (ra, rb) = tokio::join!(conn.call(&origin), conn.call(&dependent));

        assert_eq!(ra.unwrap().body, b"CHAIN");
        assert_eq!(rb.unwrap().body, b"CHAIN");
    }

    /// SERVER PUSH: the server calls the client on the connection the
    /// client dialed. A strict request/response handler could never do
    /// this — it can only answer a frame it just received.
    #[tokio::test]
    async fn server_pushes_a_call_to_the_client() {
        let (tx, rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
        let tx = Arc::new(Mutex::new(Some(tx)));

        // On accept, the server keeps the connection and calls the
        // client on it — unprompted.
        let srv = Server::listen("127.0.0.1:0", |conn: Arc<Conn>| {
            tokio::spawn(async move {
                let _ = conn.invoke(9, &[], b"jwks-rotated").await;
            });
            echo_upper
        })
        .await
        .unwrap();

        // The client serves, so it can receive that push.
        let sink = tx.clone();
        let _client = Conn::connect_serve(
            srv.addr(),
            Some(move |env: &[u8]| {
                let c = parse_request(env)?;
                if let Some(tx) = sink.lock().unwrap().take() {
                    let _ = tx.send(c.payload.clone());
                }
                Ok(build_response(STATUS_OK, c.promise_id, b"ack"))
            }),
        )
        .await
        .unwrap();

        let pushed = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .expect("server push must arrive")
            .expect("push channel must deliver");
        assert_eq!(pushed, b"jwks-rotated");
    }

    #[tokio::test]
    async fn a_closed_connection_fails_its_callers() {
        let srv = Server::listen("127.0.0.1:0", |_| echo_upper).await.unwrap();
        let conn = Conn::connect(srv.addr()).await.unwrap();
        conn.close();
        assert!(conn.is_closed());
        assert!(matches!(
            conn.invoke(1, &[], b"x").await,
            Err(WireError::Closed)
        ));
    }
}
