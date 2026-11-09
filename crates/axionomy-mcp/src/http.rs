use crate::AxionomyMcp;
use axionomy_store::{Store, StoreError};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub type StatelessHttpService = StreamableHttpService<AxionomyMcp, NeverSessionManager>;

/// Strict MCP 2026-07-28 configuration with legacy sessions disabled.
pub fn stateless_http_config(cancellation_token: CancellationToken) -> StreamableHttpServerConfig {
    StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_stateless_protocol_metadata_required(true)
        .with_cancellation_token(cancellation_token)
}

/// The server over stateless HTTP, keeping its snapshots in `store`.
pub fn stateless_http_service(
    store: Store,
    cancellation_token: CancellationToken,
) -> Result<StatelessHttpService, StoreError> {
    let server = AxionomyMcp::new(store)?;
    Ok(StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(NeverSessionManager::default()),
        stateless_http_config(cancellation_token),
    ))
}
