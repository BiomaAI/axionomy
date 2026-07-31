use axionomy_mcp::{SqliteStore, stateless_http_service};
use axum::{Router, routing::get};
use std::{env, error::Error, net::SocketAddr, path::PathBuf};
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
    let database = env::var_os("AXIONOMY_MCP_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("axionomy-mcp.sqlite3"));
    let store = SqliteStore::open(&database).await?;
    let cancellation = CancellationToken::new();
    let service = stateless_http_service(store, cancellation.clone());
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(bind).await?;

    info!(address = %bind, database = %database.display(), "Axionomy MCP server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancellation.cancel();
            }
        })
        .await?;
    Ok(())
}
