//! Serving the problems over MCP and to Studio until the server is stopped.

use crate::CliError;
use axionomy_engine::Engine;
use axionomy_mcp::stateless_http_service;
use axionomy_store::Store;
use axionomy_studio_server::{StudioState, api, with_studio_frontend};
use axionomy_study::Registry;
use axum::{Router, routing::get};
use std::{
    future::Future,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Serves MCP over streamable HTTP at `/mcp`, with `/health`, keeping
/// economies in `store` when given, in memory otherwise.
pub(crate) fn mcp(
    registry: Registry,
    bind: SocketAddr,
    store: Option<PathBuf>,
) -> Result<(), CliError> {
    logging("axionomy_cli=info,axionomy_mcp=info,rmcp=info");
    let store = match store {
        Some(path) => {
            info!(store = %path.display(), "keeping economies on disk");
            Store::open(path)?
        }
        None => Store::memory()?,
    };
    let engine = Engine::new(registry, store)?;
    runtime(async move {
        let cancellation = CancellationToken::new();
        let service = stateless_http_service(engine, cancellation.clone())?;
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
    })
}

/// Serves Studio's API, and the frontend built in `frontend` when there is
/// one.
pub(crate) fn studio(
    registry: Registry,
    bind: SocketAddr,
    frontend: &Path,
) -> Result<(), CliError> {
    logging("axionomy_cli=info,axionomy_studio_server=info,tower_http=info");
    let serves_frontend = frontend.join("index.html").is_file();
    let (router, _) = api(StudioState::new(registry));
    let router = with_studio_frontend(router, frontend);
    let frontend = frontend.to_owned();
    runtime(async move {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        let address = listener.local_addr()?;
        let url = browser_url(address);
        print_startup(&url, address, &frontend, serves_frontend);
        info!(%address, %url, serves_frontend, "Axionomy Studio listening");
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                if tokio::signal::ctrl_c().await.is_ok() {
                    info!("shutdown requested");
                }
            })
            .await?;
        Ok(())
    })
}

fn logging(default: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default)),
        )
        .with_target(false)
        .init();
}

fn runtime(serve: impl Future<Output = Result<(), CliError>>) -> Result<(), CliError> {
    tokio::runtime::Runtime::new()?.block_on(serve)
}

fn browser_url(address: SocketAddr) -> String {
    let host = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".into(),
        IpAddr::V6(ip) if ip.is_unspecified() => "[::1]".into(),
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    format!("http://{host}:{}/", address.port())
}

fn print_startup(url: &str, address: SocketAddr, frontend: &Path, serves_frontend: bool) {
    println!();
    println!("Axionomy Studio is ready");
    println!();
    if serves_frontend {
        println!("  Studio     {url}");
        println!("  Frontend   {}", frontend.display());
    } else {
        println!(
            "  Studio UI  not built (missing {})",
            frontend.join("index.html").display()
        );
        println!("  Build UI   cd studio && pnpm install && pnpm build");
    }
    println!("  API        {url}api/health");
    println!("  OpenAPI    {url}api/openapi.json");
    println!("  Listening  {address}");
    println!("  Stop       Ctrl+C");
    println!();
}

#[cfg(test)]
mod tests {
    use super::browser_url;

    #[test]
    fn browser_url_replaces_unspecified_ipv4_with_loopback() {
        assert_eq!(
            browser_url("0.0.0.0:3000".parse().unwrap()),
            "http://127.0.0.1:3000/"
        );
    }

    #[test]
    fn browser_url_brackets_ipv6_hosts() {
        assert_eq!(
            browser_url("[::1]:3000".parse().unwrap()),
            "http://[::1]:3000/"
        );
    }

    #[test]
    fn browser_url_preserves_bound_port() {
        assert_eq!(
            browser_url("127.0.0.1:43127".parse().unwrap()),
            "http://127.0.0.1:43127/"
        );
    }
}
