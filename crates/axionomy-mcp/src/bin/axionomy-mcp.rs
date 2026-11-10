use axionomy_mcp::stateless_http_service;
use axionomy_store::Store;
use axum::{Router, routing::get};
use std::{env, error::Error, net::SocketAddr};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("axionomy_mcp=info,rmcp=info")),
        )
        .with_target(false)
        .init();

    let bind = env::var("AXIONOMY_MCP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8000".to_owned())
        .parse::<SocketAddr>()?;
    // A store file keeps economies across restarts; without one they last
    // until the server stops.
    let store = match env::var_os("AXIONOMY_MCP_STORE") {
        Some(path) => {
            info!(store = %path.to_string_lossy(), "keeping economies on disk");
            Store::open(path)?
        }
        None => Store::memory()?,
    };
    let cancellation = CancellationToken::new();
    let service = stateless_http_service(store, cancellation.clone())?;
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(bind).await?;

    info!(address = %bind, "Axionomy MCP server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancellation.cancel();
            }
        })
        .await?;
    Ok(())
}
