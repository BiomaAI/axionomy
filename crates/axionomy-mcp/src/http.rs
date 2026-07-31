use crate::{AxionomyMcp, MemorySnapshotStore, SnapshotStore};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub type StatelessHttpService<S = MemorySnapshotStore> =
    StreamableHttpService<AxionomyMcp<S>, NeverSessionManager>;

/// Strict MCP 2026-07-28 configuration with legacy sessions disabled.
pub fn stateless_http_config(cancellation_token: CancellationToken) -> StreamableHttpServerConfig {
    StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_stateless_protocol_metadata_required(true)
        .with_cancellation_token(cancellation_token)
}

pub fn stateless_http_service<S>(
    snapshots: S,
    cancellation_token: CancellationToken,
) -> StatelessHttpService<S>
where
    S: SnapshotStore,
{
    let server = AxionomyMcp::new(snapshots);
    StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(NeverSessionManager::default()),
        stateless_http_config(cancellation_token),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_transport_is_strictly_stateless() {
        let config = stateless_http_config(CancellationToken::new());
        assert!(!config.legacy_session_mode);
        assert!(config.json_response);
        assert!(config.stateless_protocol_metadata_required);
        assert!(config.sse_keep_alive.is_none());
    }
}
