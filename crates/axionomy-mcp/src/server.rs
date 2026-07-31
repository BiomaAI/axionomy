use crate::store::{CreatedTask, SqliteStore};
use crate::wire::{
    ApplyRequest, ApplyResponse, AssessRequest, AssessResponse, EconomyPutRequest,
    EconomyPutResponse, ReplayRequest, ReplayResponse, SearchOutcome, SearchRequest,
    SearchResponse, WireEconomy,
};
use axionomy_search::{
    BfsSession, GraphSearchProgress,
    session::{Continue, SearchStatus, WorkBudget},
};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        tool::ToolCallContext,
        wrapper::{Json, Parameters},
    },
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams,
        CreateTaskResult, GetTaskParams, GetTaskResult, Implementation, JsonObject,
        ProtocolVersion, ServerCapabilities, ServerInfo, UpdateTaskParams,
    },
    service::{RequestContext, RoleServer},
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

    async fn create_search_task(
        &self,
        request: &SearchRequest,
    ) -> Result<(CreatedTask, WireEconomy), String> {
        validate_search_request(request)?;
        let economy = self.require_economy(&request.economy_id).await?;
        let request_json = serde_json::to_string(request).map_err(|error| error.to_string())?;
        let task = self
            .store
            .create_search_task(request_json, request.idempotency_key.clone())
            .await
            .map_err(|error| error.to_string())?;
        Ok((task, economy))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AxionomyMcp {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name == "axionomy_search"
            && context
                .client_capabilities()
                .is_some_and(|capabilities| capabilities.supports_tasks())
        {
            let search = match serde_json::from_value::<SearchRequest>(serde_json::Value::Object(
                request.arguments.clone().unwrap_or_default(),
            )) {
                Ok(search) => search,
                Err(error) => {
                    return Ok(CallToolResult::structured_error(serde_json::json!({
                        "error": "invalid_search_request",
                        "message": error.to_string()
                    }))
                    .into());
                }
            };
            let (created, economy) = match self.create_search_task(&search).await {
                Ok(created) => created,
                Err(error) => {
                    return Ok(CallToolResult::structured_error(serde_json::json!({
                        "error": "search_not_started",
                        "message": error
                    }))
                    .into());
                }
            };
            if created.created {
                spawn_search(
                    self.store.clone(),
                    created.task.task_id.clone(),
                    economy,
                    search,
                );
            }
            return Ok(CreateTaskResult::new(created.task).into());
        }

        let context = ToolCallContext::new(self, request, context);
        self.tool_router.call(context).await
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, McpError> {
        let task = self
            .store
            .get_task(&request.task_id)
            .await
            .map_err(internal_store_error)?
            .ok_or_else(|| unknown_task(&request.task_id))?;
        Ok(GetTaskResult::new(task))
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        if self
            .store
            .task_exists(&request.task_id)
            .await
            .map_err(internal_store_error)?
        {
            Ok(())
        } else {
            Err(unknown_task(&request.task_id))
        }
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        if self
            .store
            .request_cancellation(&request.task_id)
            .await
            .map_err(internal_store_error)?
        {
            Ok(())
        } else {
            Err(unknown_task(&request.task_id))
        }
    }

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

fn validate_search_request(request: &SearchRequest) -> Result<(), String> {
    if request.max_expansions == 0 {
        return Err("max_expansions must be greater than zero".to_owned());
    }
    if request.chunk_size == 0 {
        return Err("chunk_size must be greater than zero".to_owned());
    }
    if request.chunk_size > 10_000 {
        return Err(
            "chunk_size must not exceed 10000 so cancellation remains responsive".to_owned(),
        );
    }
    if request.idempotency_key.as_deref() == Some("") {
        return Err("idempotency_key must not be empty".to_owned());
    }
    Ok(())
}

fn spawn_search(store: SqliteStore, task_id: String, economy: WireEconomy, request: SearchRequest) {
    tokio::spawn(async move {
        if let Err(error) = run_search(&store, &task_id, economy, request).await {
            tracing::error!(task_id, %error, "Axionomy search task failed");
            if let Err(store_error) = store.fail_task(&task_id, error).await {
                tracing::error!(task_id, %store_error, "could not persist Axionomy task failure");
            }
        }
    });
}

async fn run_search(
    store: &SqliteStore,
    task_id: &str,
    economy: WireEconomy,
    request: SearchRequest,
) -> Result<(), String> {
    let SearchRequest {
        economy_id,
        goal,
        candidates,
        max_expansions,
        chunk_size,
        idempotency_key: _,
    } = request;
    let mut session = BfsSession::new(&economy, goal, move |_| candidates.clone());
    let mut remaining = max_expansions;

    loop {
        let status = session.status();
        if status.is_terminal() {
            let outcome = match status {
                SearchStatus::Solved => SearchOutcome::Solved,
                SearchStatus::Exhausted => SearchOutcome::Exhausted,
                SearchStatus::Running | SearchStatus::Interrupted => unreachable!(),
            };
            return complete_search(store, task_id, economy_id, outcome, session).await;
        }
        if store
            .cancellation_requested(task_id)
            .await
            .map_err(|error| error.to_string())?
        {
            return store
                .cancel_task(task_id)
                .await
                .map_err(|error| error.to_string());
        }
        if remaining == 0 {
            return complete_search(
                store,
                task_id,
                economy_id,
                SearchOutcome::ExpansionLimit,
                session,
            )
            .await;
        }

        let budget = remaining.min(chunk_size);
        let report = session.advance(WorkBudget::new(budget), &mut Continue);
        remaining = remaining.saturating_sub(report.work_completed());
        let progress = *report.progress();
        store
            .update_task_progress(
                task_id,
                progress_message(progress, max_expansions, remaining),
            )
            .await
            .map_err(|error| error.to_string())?;
        tokio::task::yield_now().await;
    }
}

async fn complete_search(
    store: &SqliteStore,
    task_id: &str,
    economy_id: String,
    outcome: SearchOutcome,
    session: BfsSession<
        String,
        String,
        String,
        String,
        u64,
        impl FnMut(&WireEconomy) -> Vec<crate::wire::WireExchange>,
    >,
) -> Result<(), String> {
    let progress = session.progress();
    let response = SearchResponse {
        economy_id,
        outcome,
        progress,
        solution: session.into_solution(),
    };
    let result = CallToolResult::structured(
        serde_json::to_value(response).map_err(|error| error.to_string())?,
    );
    let result = json_object(result)?;
    store
        .complete_task(task_id, terminal_message(progress), result)
        .await
        .map_err(|error| error.to_string())
}

fn progress_message(
    progress: GraphSearchProgress,
    max_expansions: usize,
    remaining: usize,
) -> String {
    format!(
        "expanded={}/{} generated={} frontier={} visited={} remaining={}",
        progress.expanded(),
        max_expansions,
        progress.generated(),
        progress.frontier(),
        progress.visited(),
        remaining
    )
}

fn terminal_message(progress: GraphSearchProgress) -> String {
    format!(
        "finished: expanded={} generated={} frontier={} visited={}",
        progress.expanded(),
        progress.generated(),
        progress.frontier(),
        progress.visited()
    )
}

fn json_object(value: impl serde::Serialize) -> Result<JsonObject, String> {
    serde_json::to_value(value)
        .map_err(|error| error.to_string())?
        .as_object()
        .cloned()
        .ok_or_else(|| "task result must serialize as a JSON object".to_owned())
}

fn unknown_task(task_id: &str) -> McpError {
    McpError::invalid_params(format!("unknown task: {task_id}"), None)
}

fn internal_store_error(error: crate::StoreError) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Account, Basket, EconomyBuilder, Exchange, Goal, Quantity, Rate};
    use rmcp::model::{TaskPayload, TaskStatus};

    fn one(asset: &str) -> Basket<String> {
        [(asset.to_owned(), Quantity::new(1))].into_iter().collect()
    }

    fn search_fixture() -> (WireEconomy, SearchRequest) {
        let economy = EconomyBuilder::new()
            .account("source".to_owned(), Account::new(one("token")))
            .account("sink".to_owned(), Account::default())
            .rate(
                "transfer".to_owned(),
                Rate::new()
                    .consume("giver".to_owned(), one("token"))
                    .produce("receiver".to_owned(), one("token"))
                    .distinct("giver".to_owned(), "receiver".to_owned()),
            )
            .build()
            .unwrap();
        let exchange = Exchange::new("transfer".to_owned(), Quantity::new(1))
            .bind("giver".to_owned(), "source".to_owned())
            .bind("receiver".to_owned(), "sink".to_owned());
        let goal = Goal::new().require("sink".to_owned(), one("token"));
        let request = SearchRequest {
            economy_id: String::new(),
            goal,
            candidates: vec![exchange],
            max_expansions: 8,
            chunk_size: 1,
            idempotency_key: None,
        };
        (economy, request)
    }

    #[tokio::test]
    async fn every_reference_tool_exposes_input_and_output_schemas() {
        let server = AxionomyMcp::new(SqliteStore::open_in_memory().await.unwrap());
        let tools = server.tool_router.list_all();

        assert_eq!(tools.len(), 5);
        for tool in tools {
            assert!(!tool.input_schema.is_empty(), "{} input schema", tool.name);
            assert!(tool.output_schema.is_some(), "{} output schema", tool.name);
        }
    }

    #[tokio::test]
    async fn bfs_task_persists_a_structured_solution() {
        let store = SqliteStore::open_in_memory().await.unwrap();
        let (economy, mut request) = search_fixture();
        request.economy_id = store.put_economy(&economy).await.unwrap().economy_id;
        let created = store
            .create_search_task(serde_json::to_string(&request).unwrap(), None)
            .await
            .unwrap();

        run_search(&store, &created.task.task_id, economy, request)
            .await
            .unwrap();

        let detailed = store
            .get_task(&created.task.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detailed.status(), TaskStatus::Completed);
        let TaskPayload::Completed { result } = detailed.payload else {
            panic!("search should complete with a tool result")
        };
        let tool_result: CallToolResult =
            serde_json::from_value(serde_json::Value::Object(result)).unwrap();
        let response: SearchResponse =
            serde_json::from_value(tool_result.structured_content.unwrap()).unwrap();
        assert_eq!(response.outcome, SearchOutcome::Solved);
        assert_eq!(response.solution.unwrap().cost(), 1);
    }

    #[tokio::test]
    async fn bfs_task_observes_persisted_cancellation() {
        let store = SqliteStore::open_in_memory().await.unwrap();
        let (economy, mut request) = search_fixture();
        request.economy_id = store.put_economy(&economy).await.unwrap().economy_id;
        let created = store
            .create_search_task(serde_json::to_string(&request).unwrap(), None)
            .await
            .unwrap();
        store
            .request_cancellation(&created.task.task_id)
            .await
            .unwrap();

        run_search(&store, &created.task.task_id, economy, request)
            .await
            .unwrap();

        let detailed = store
            .get_task(&created.task.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detailed.status(), TaskStatus::Cancelled);
        assert!(matches!(detailed.payload, TaskPayload::Cancelled));
    }
}
