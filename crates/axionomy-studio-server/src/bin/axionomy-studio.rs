use axionomy_studio_server::{StudioState, api, with_studio_frontend};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("axionomy_studio=info,axionomy_studio_server=info,tower_http=info")
        }))
        .init();
    let bind = std::env::var("AXIONOMY_STUDIO_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let frontend = std::env::var_os("AXIONOMY_STUDIO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("studio/dist"));
    let frontend_index = frontend.join("index.html");
    let serves_frontend = frontend_index.is_file();
    let (router, _) = api(StudioState::default());
    let router = with_studio_frontend(router, &frontend);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
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

fn print_startup(
    url: &str,
    address: SocketAddr,
    frontend: &std::path::Path,
    serves_frontend: bool,
) {
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
