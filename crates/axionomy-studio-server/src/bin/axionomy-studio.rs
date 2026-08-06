use axionomy_studio_server::{StudioState, api, with_studio_frontend};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("axionomy_studio_server=info,tower_http=info")),
        )
        .init();
    let bind = std::env::var("AXIONOMY_STUDIO_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let frontend = std::env::var_os("AXIONOMY_STUDIO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("studio/dist"));
    let (router, _) = api(StudioState::default());
    let router = with_studio_frontend(router, frontend);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!(%bind, "Axionomy Studio listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            if tokio::signal::ctrl_c().await.is_ok() {
                info!("shutdown requested");
            }
        })
        .await?;
    Ok(())
}
