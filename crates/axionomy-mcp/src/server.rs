use crate::store::SqliteStore;
use crate::wire::{
    ApplyRequest, ApplyResponse, AssessRequest, AssessResponse, EconomyPutRequest,
    EconomyPutResponse, ReplayRequest, ReplayResponse, SearchRequest,
};
use rmcp::{
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct AxionomyMcp {
    store: SqliteStore,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AxionomyMcp {
    pub fn new(store: SqliteStore) -> Self {
        Self {
            store,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "axionomy_economy_put",
        description = "Validate and store an immutable Axionomy economy snapshot. Returns its content-addressed economy_id."
    )]
    async fn economy_put(
        &self,
        Parameters(request): Parameters<EconomyPutRequest>,
    ) -> Result<Json<EconomyPutResponse>, String> {
        let stored = self
            .store
            .put_economy(&request.economy)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Json(EconomyPutResponse {
            economy_id: stored.economy_id,
            deduplicated: stored.deduplicated,
        }))
    }

    #[tool(
        name = "axionomy_exchange_assess",
        description = "Explain whether a proposed exchange is applicable, infeasible, or invalid without changing its economy snapshot."
    )]
    async fn exchange_assess(
        &self,
        Parameters(request): Parameters<AssessRequest>,
    ) -> Result<Json<AssessResponse>, String> {
        let economy = self.require_economy(&request.economy_id).await?;
        let assessment = economy.assess(&request.exchange);
        let status = assessment.status();
        let assessment = serde_json::to_value(assessment).map_err(|error| error.to_string())?;
        Ok(Json(AssessResponse {
            economy_id: request.economy_id,
            status,
            assessment,
        }))
    }

    #[tool(
        name = "axionomy_exchange_apply",
        description = "Apply an exchange atomically to an immutable snapshot and return a receipt plus a new economy_id."
    )]
    async fn exchange_apply(
        &self,
        Parameters(request): Parameters<ApplyRequest>,
    ) -> Result<Json<ApplyResponse>, String> {
        let source_economy_id = request.economy_id;
        let mut economy = self.require_economy(&source_economy_id).await?;
        let receipt = economy
            .apply(request.exchange)
            .map_err(|error| error.to_string())?;
        let stored = self
            .store
            .put_economy(&economy)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Json(ApplyResponse {
            source_economy_id,
            economy_id: stored.economy_id,
            receipt,
        }))
    }

    #[tool(
        name = "axionomy_trace_replay",
        description = "Replay a trace atomically on an immutable snapshot and return receipts plus a new economy_id."
    )]
    async fn trace_replay(
        &self,
        Parameters(request): Parameters<ReplayRequest>,
    ) -> Result<Json<ReplayResponse>, String> {
        let source_economy_id = request.economy_id;
        let economy = self.require_economy(&source_economy_id).await?;
        let mut next = economy.fork();
        let receipts = next
            .replay(&request.trace)
            .map_err(|error| error.to_string())?;
        let stored = self
            .store
            .put_economy(&next)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Json(ReplayResponse {
            source_economy_id,
            economy_id: stored.economy_id,
            receipts,
        }))
    }

    #[tool(
        name = "axionomy_search",
        description = "Start a durable, cancellable breadth-first search task over an explicit candidate exchange universe. Requires MCP Tasks support."
    )]
    async fn search_requires_tasks(
        &self,
        Parameters(_request): Parameters<SearchRequest>,
    ) -> Result<Json<serde_json::Value>, String> {
        Err("axionomy_search requires the MCP io.modelcontextprotocol/tasks capability".to_owned())
    }

    async fn require_economy(&self, economy_id: &str) -> Result<crate::wire::WireEconomy, String> {
        self.store
            .get_economy(economy_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("economy snapshot `{economy_id}` does not exist"))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AxionomyMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
        .with_server_info(Implementation::new("axionomy-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Axionomy models every valid state transition as an exchange over explicit accounts, assets, rates, and role bindings. Store an economy first, then pass its immutable economy_id to every other tool.",
        )
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }
}
