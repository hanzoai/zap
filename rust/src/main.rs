//! hanzo-zap — standalone smoke-test daemon.
//!
//! Echoes every inbound frame back as a fresh ZAP message. Used for
//! integration tests against the Go and TypeScript implementations.
//! Production callers embed `hanzo_zap::server::Server` directly with
//! their own `Handler`.

use anyhow::Result;
use async_trait::async_trait;
use hanzo_zap::server::{Config, Frame, Handler, Server, DEFAULT_PORTS};
use std::sync::Arc;

struct Echo;

#[async_trait]
impl Handler for Echo {
    async fn handle(&self, frame: Frame) -> Result<Option<Vec<u8>>> {
        // Echo the body back verbatim.
        Ok(Some(frame.body))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cfg = Config {
        bind: ([0, 0, 0, 0], DEFAULT_PORTS[0]).into(),
        name: "hanzo-zap-echo".to_string(),
    };
    let server = Server::start(Arc::new(Echo), cfg).await?;
    log::info!("[ZAP] echo server bound at {}", server.bound);

    tokio::signal::ctrl_c().await?;
    server.stop();
    Ok(())
}
