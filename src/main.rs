use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

use crate::cmd::{AppCommand, CmdOptions};

mod cmd;
mod doc_gen;
mod index;
mod markdown;
mod server;
mod types;
mod workspace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up logging
    let file_appender = tracing_appender::rolling::daily("/tmp/rustdoc-mcp", "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::INFO.into()),
                ),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_filter(tracing_subscriber::EnvFilter::from_default_env()),
        )
        .init();

    let cmd = CmdOptions::parse();

    match cmd.command {
        AppCommand::Version => {
            println!("RustDoc MCP Server, Version: {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }

        AppCommand::Start { cwd } => {
            tracing::info!("Starting RustDoc MCP Server...");
            let server = match server::RustDocMCPServer::new(cwd) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to start server: {}", e);
                    return Err(anyhow::anyhow!("Failed to start server: {}", e));
                }
            };

            tracing::info!("Server initialized successfully");

            let service = server
                .serve(stdio())
                .await
                .inspect_err(|e| tracing::error!("Server error during startup: {}", e))?;

            tracing::info!("Service started, waiting for requests...");

            service
                .waiting()
                .await
                .map_err(|e| anyhow::anyhow!("Server encountered an error during execution: {}", e))
                .map(|_| {
                    tracing::info!("Server stopped gracefully");
                })
        }
    }
}
