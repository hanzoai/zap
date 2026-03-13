//! hanzo-zap — standalone ZAP server binary.
//!
//! Starts a ZAP WebSocket server. Plug in tools via the ToolHandler trait.
//! For hanzo-mcp integration, use hanzo-mcp which implements ToolHandler.

use anyhow::Result;
use async_trait::async_trait;
use hanzo_zap::{ZapServer, ZapServerOptions, ToolHandler, ToolDef, ToolResult, ContentBlock};
use std::sync::Arc;

/// Minimal built-in tools for standalone mode
struct BuiltinTools;

#[async_trait]
impl ToolHandler for BuiltinTools {
    fn list_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "ping".into(),
                description: "Health check".into(),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
            },
            ToolDef {
                name: "echo".into(),
                description: "Echo back input".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"]
                }),
            },
        ]
    }

    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
        match name {
            "ping" => Ok(ToolResult {
                content: vec![ContentBlock {
                    content_type: "text".into(),
                    text: "pong".into(),
                }],
                is_error: None,
            }),
            "echo" => {
                let msg = args.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(empty)");
                Ok(ToolResult {
                    content: vec![ContentBlock {
                        content_type: "text".into(),
                        text: msg.to_string(),
                    }],
                    is_error: None,
                })
            }
            _ => Ok(ToolResult {
                content: vec![ContentBlock {
                    content_type: "text".into(),
                    text: format!("Unknown tool: {}", name),
                }],
                is_error: Some(true),
            }),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let handler = Arc::new(BuiltinTools);
    let server = ZapServer::start(handler, ZapServerOptions::default()).await?;

    log::info!("[ZAP] Running on ws://127.0.0.1:{}", server.port);
    log::info!("[ZAP] Press Ctrl+C to stop");

    tokio::signal::ctrl_c().await?;
    server.stop();

    Ok(())
}
