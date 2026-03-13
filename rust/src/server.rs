//! ZAP WebSocket server — serves tools over ZAP binary protocol.
//!
//! Uses a `ToolHandler` trait so any tool implementation can plug in.
//! hanzo-mcp, hanzo-dev, or any other crate just implements the trait.

use crate::protocol::{msg, encode, decode};
use anyhow::Result;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

/// Default ports to try (same as TypeScript @zap-proto/zap)
pub const DEFAULT_PORTS: [u16; 5] = [9999, 9998, 9997, 9996, 9995];

// ── Tool Handler Trait ──────────────────────────────────────────────────

/// Tool definition for manifests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Result from tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentBlock>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Trait for tool dispatch — implement this to plug your tools into ZAP.
#[async_trait]
pub trait ToolHandler: Send + Sync + 'static {
    /// List available tools
    fn list_tools(&self) -> Vec<ToolDef>;

    /// Call a tool by name with JSON arguments
    async fn call_tool(&self, name: &str, args: Value) -> Result<ToolResult>;
}

// ── Server ──────────────────────────────────────────────────────────────

pub struct ZapServerOptions {
    pub name: String,
    pub version: String,
    pub preferred_port: Option<u16>,
}

impl Default for ZapServerOptions {
    fn default() -> Self {
        Self {
            name: "hanzo-zap".into(),
            version: "1.0.0".into(),
            preferred_port: None,
        }
    }
}

/// Running ZAP server handle
pub struct ZapServer {
    pub port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl ZapServer {
    pub fn stop(&self) {
        let _ = self.shutdown.send(true);
    }

    /// Start a ZAP server with the given tool handler.
    pub async fn start<H: ToolHandler>(
        handler: Arc<H>,
        opts: ZapServerOptions,
    ) -> Result<Self> {
        let server_id = format!("zap-{}", chrono::Utc::now().timestamp_millis());

        let ports: Vec<u16> = if let Some(preferred) = opts.preferred_port {
            let mut p = vec![preferred];
            p.extend(DEFAULT_PORTS.iter().filter(|&&pp| pp != preferred));
            p
        } else {
            DEFAULT_PORTS.to_vec()
        };

        for port in &ports {
            let addr: SocketAddr = ([127, 0, 0, 1], *port).into();
            match TcpListener::bind(addr).await {
                Ok(listener) => {
                    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
                    let port = *port;
                    let handler = handler.clone();
                    let name = opts.name.clone();
                    let server_id = server_id.clone();

                    let tool_count = handler.list_tools().len();
                    info!("[ZAP] Server listening on ws://127.0.0.1:{} ({} tools)", port, tool_count);

                    tokio::spawn(async move {
                        let mut shutdown_rx = shutdown_rx;
                        loop {
                            tokio::select! {
                                result = listener.accept() => {
                                    match result {
                                        Ok((stream, addr)) => {
                                            debug!("[ZAP] Connection from {}", addr);
                                            let h = handler.clone();
                                            let n = name.clone();
                                            let sid = server_id.clone();
                                            tokio::spawn(handle_connection(stream, h, n, sid));
                                        }
                                        Err(e) => error!("[ZAP] Accept error: {}", e),
                                    }
                                }
                                _ = shutdown_rx.changed() => {
                                    info!("[ZAP] Server shutting down");
                                    break;
                                }
                            }
                        }
                    });

                    return Ok(ZapServer { port, shutdown: shutdown_tx });
                }
                Err(_) => {
                    debug!("[ZAP] Port {} busy, trying next", port);
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!("[ZAP] Could not bind to any port (9999-9995)"))
    }
}

async fn handle_connection<H: ToolHandler>(
    stream: TcpStream,
    handler: Arc<H>,
    name: String,
    server_id: String,
) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("[ZAP] WebSocket upgrade failed: {}", e);
            return;
        }
    };

    let (mut tx, mut rx) = ws.split();
    let manifest: Vec<Value> = handler
        .list_tools()
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .collect();
    let mut client_id = String::from("unknown");

    while let Some(msg) = rx.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                debug!("[ZAP] Read error: {}", e);
                break;
            }
        };

        let data = match &msg {
            Message::Binary(b) => b.as_slice(),
            _ => continue,
        };

        let frame = match decode(data) {
            Some(f) => f,
            None => continue,
        };

        match frame.msg_type {
            msg::INIT => {
                let p = frame.payload.as_object().cloned().unwrap_or_default();
                client_id = p.get("clientId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let browser = p.get("browser").and_then(|v| v.as_str()).unwrap_or("unknown");
                let version = p.get("version").and_then(|v| v.as_str()).unwrap_or("0");
                info!("[ZAP] Client connected: {} ({} v{})", client_id, browser, version);

                let resp = encode(msg::INIT_ACK, &json!({
                    "serverId": server_id,
                    "name": name,
                    "tools": manifest,
                }));
                if tx.send(Message::Binary(resp.into())).await.is_err() { break; }
            }

            msg::PUSH => {
                let p = frame.payload.as_object().cloned().unwrap_or_default();
                let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let method = p.get("method").and_then(|v| v.as_str()).unwrap_or("");
                let params = p.get("params").cloned().unwrap_or(Value::Null);

                let response = handle_request(&id, method, params, &handler, &manifest).await;
                let resp_frame = encode(msg::RESOLVE, &response);
                if tx.send(Message::Binary(resp_frame.into())).await.is_err() { break; }
            }

            msg::PING => {
                let resp = encode(msg::PONG, &json!({}));
                if tx.send(Message::Binary(resp.into())).await.is_err() { break; }
            }

            _ => {}
        }
    }

    info!("[ZAP] Client disconnected: {}", client_id);
}

async fn handle_request<H: ToolHandler>(
    id: &str,
    method: &str,
    params: Value,
    handler: &Arc<H>,
    manifest: &[Value],
) -> Value {
    match method {
        "tools/list" => {
            json!({ "id": id, "result": { "tools": manifest } })
        }

        "tools/call" => {
            let tool_name = params.get("name")
                .and_then(|v| v.as_str());
            let tool_args = params.get("arguments")
                .cloned()
                .unwrap_or(json!({}));

            match tool_name {
                Some(name) => {
                    match handler.call_tool(name, tool_args).await {
                        Ok(result) => {
                            json!({
                                "id": id,
                                "result": result,
                            })
                        }
                        Err(e) => {
                            json!({
                                "id": id,
                                "error": { "code": -1, "message": e.to_string() }
                            })
                        }
                    }
                }
                None => {
                    json!({
                        "id": id,
                        "error": { "code": -1, "message": "Missing tool name" }
                    })
                }
            }
        }

        "resources/list" => json!({ "id": id, "result": { "resources": [] } }),
        "prompts/list" => json!({ "id": id, "result": { "prompts": [] } }),

        _ => {
            json!({
                "id": id,
                "error": { "code": -1, "message": format!("Unsupported method: {}", method) }
            })
        }
    }
}
