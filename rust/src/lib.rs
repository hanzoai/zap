//! hanzo-zap — ZAP binary protocol + server in Rust.
//!
//! Canonical Rust implementation of the Zero-copy Agent Protocol.
//! Provides ZAP binary encode/decode and a WebSocket server with
//! pluggable tool dispatch via the `ToolHandler` trait.
//!
//! ```rust,no_run
//! use hanzo_zap::{ZapServer, ZapServerOptions, ToolHandler, ToolDef, ToolResult, ContentBlock};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MyTools;
//!
//! #[async_trait]
//! impl ToolHandler for MyTools {
//!     fn list_tools(&self) -> Vec<ToolDef> {
//!         vec![ToolDef {
//!             name: "hello".into(),
//!             description: "Say hello".into(),
//!             input_schema: serde_json::json!({"type": "object"}),
//!         }]
//!     }
//!     async fn call_tool(&self, name: &str, args: serde_json::Value) -> anyhow::Result<ToolResult> {
//!         Ok(ToolResult {
//!             content: vec![ContentBlock { content_type: "text".into(), text: "Hello!".into() }],
//!             is_error: None,
//!         })
//!     }
//! }
//! ```

pub mod protocol;
pub mod server;

pub use protocol::{encode, decode, ZapFrame, ZAP_MAGIC, HEADER_SIZE, MAX_PAYLOAD_SIZE, PROTOCOL_VERSION};
pub use protocol::msg;
pub use server::{ZapServer, ZapServerOptions, ToolHandler, ToolDef, ToolResult, ContentBlock, DEFAULT_PORTS};
