//! ZAP server — raw TCP listener that hands each frame to a `Handler`.
//!
//! ZAP framing IS the framing; we don't wrap it in WebSocket / HTTP.
//! The transport is TCP (or TLS-over-TCP for cross-host links). Each
//! accepted connection runs a per-frame loop:
//!
//!   1. Read 16-byte header.
//!   2. Read header.size - 16 bytes of capnp data segment.
//!   3. Hand to `Handler::handle(header, body) -> ResponseBytes`.
//!   4. Write the handler's response frame back.
//!
//! Authentication, framing-level encryption, and per-frame routing
//! are the handler's responsibility — this layer is intentionally
//! transport-only.

use crate::wire::{self, Flags, Header, WireError};
use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

/// Default ports the server tries in order until one binds.
pub const DEFAULT_PORTS: [u16; 5] = [9999, 9998, 9997, 9996, 9995];

/// A frame coming off the wire, fully read.
pub struct Frame {
    pub header: Header,
    /// Raw capnp data segment. Hand to `capnp::serialize::read_message_from_flat_slice`
    /// for zero-copy reads.
    pub body: Vec<u8>,
}

/// Per-connection handler trait. Implementations should be cheap to
/// clone (use `Arc` internally if state is shared).
#[async_trait]
pub trait Handler: Send + Sync + 'static {
    /// Called once per inbound frame. Return the body of the response
    /// frame (capnp segment); the server wraps it with a fresh header.
    /// Return `None` to skip the response (e.g. fire-and-forget PUSH).
    async fn handle(&self, frame: Frame) -> Result<Option<Vec<u8>>>;
}

/// Server configuration.
pub struct Config {
    /// Bind address. `0.0.0.0` for cross-container reach in compose / k8s.
    pub bind: SocketAddr,
    /// Server name (logged on startup).
    pub name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: ([0, 0, 0, 0], DEFAULT_PORTS[0]).into(),
            name: "hanzo-zap".to_string(),
        }
    }
}

/// Running server handle. Drop to shut down.
pub struct Server {
    pub bound: SocketAddr,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl Server {
    pub fn stop(&self) {
        let _ = self.shutdown.send(true);
    }

    pub async fn start<H: Handler>(handler: Arc<H>, cfg: Config) -> Result<Self> {
        let listener = TcpListener::bind(cfg.bind).await?;
        let bound = listener.local_addr()?;
        info!("[ZAP] {} listening on {}", cfg.name, bound);

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => match accept {
                        Ok((stream, peer)) => {
                            debug!("[ZAP] accepted {}", peer);
                            let h = handler.clone();
                            tokio::spawn(handle_conn(stream, h));
                        }
                        Err(e) => error!("[ZAP] accept error: {}", e),
                    },
                    _ = shutdown_rx.changed() => {
                        info!("[ZAP] shutting down");
                        break;
                    }
                }
            }
        });

        Ok(Server {
            bound,
            shutdown: shutdown_tx,
        })
    }
}

async fn handle_conn<H: Handler>(mut stream: TcpStream, handler: Arc<H>) {
    if let Err(e) = handle_conn_inner(&mut stream, &handler).await {
        debug!("[ZAP] conn closed: {}", e);
    }
}

async fn handle_conn_inner<H: Handler>(
    stream: &mut TcpStream,
    handler: &Arc<H>,
) -> Result<(), WireError> {
    let mut hdr_buf = [0u8; wire::HEADER_SIZE];
    loop {
        // Read header
        match stream.read_exact(&mut hdr_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(WireError::Io(e)),
        }
        let header = Header::decode(&hdr_buf)?;

        // Read body
        let body_len = header.size as usize - wire::HEADER_SIZE;
        let mut body = vec![0u8; body_len];
        stream.read_exact(&mut body).await?;

        // Dispatch
        let frame = Frame { header, body };
        match handler.handle(frame).await {
            Ok(Some(resp_body)) => {
                wire::write_frame_async(stream, Flags::NONE, 0, &resp_body).await?;
            }
            Ok(None) => {} // fire-and-forget
            Err(e) => {
                error!("[ZAP] handler error: {}", e);
                // Connection-level errors close the stream; per-message
                // errors should be encoded by the handler in its response.
                return Ok(());
            }
        }
    }
}
