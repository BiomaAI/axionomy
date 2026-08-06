use crate::store::{MemorySnapshotStore, SnapshotStore};
use crate::wire::{
    ApplyRequest, ApplyResponse, AssessRequest, AssessResponse, EconomyPutRequest,
    EconomyPutResponse, ProblemCatalogRequest, ProblemCatalogResponse, ReplayRequest,
    ReplayResponse, SearchOutcome, SearchRequest, SearchResponse, WireEconomy,
};
use axionomy_search::{
    BfsSession, GraphSearchProgress,
    session::{Continue, SearchStatus, WorkBudget},
};
use axionomy_service::{ReferenceService, RunControl, RunRequest, ServiceProgress};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        tool::ToolCallContext,
        wrapper::{Json, Parameters},
    },
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams,
        CreateTaskResult, GetTaskParams, GetTaskResult, Implementation, ProtocolVersion,
        ServerCapabilities, ServerInfo, UpdateTaskParams,
    },
    service::{RequestContext, RoleServer},
    task_manager::{TaskContext, TaskExit, TaskManager, TaskOptions},
    tool, tool_handler, tool_router,
};
use std::{borrow::Cow, sync::Arc};

const TASK_POLL_INTERVAL_MS: u64 = 100;

#[derive(Clone)]
pub struct AxionomyMcp<S = MemorySnapshotStore>
where
    S: SnapshotStore,
{
    snapshots: S,
    tasks: TaskManager,
    task_options: TaskOptions,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<S> AxionomyMcp<S>
where
    S: SnapshotStore,
{
    pub fn new(snapshots: S) -> Self {
        Self {
            snapshots,
            tasks: TaskManager::new(),
            task_options: TaskOptions::new()
                .with_poll_interval_ms(TASK_POLL_INTERVAL_MS)
                .with_status_message("queued"),
            tool_router: Self::tool_router(),
        }
    }

    /// Overrides retention, polling, and initial-status policy for new tasks.
    pub fn with_task_options(mut self, task_options: TaskOptions) -> Self {
        self.task_options = task_options;
        self
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
            .snapshots
            .put(request.economy)
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
        let mut economy = self.require_economy(&source_economy_id).await?.fork();
        let receipt = economy
            .apply(request.exchange)
            .map_err(|error| error.to_string())?;
        let stored = self
            .snapshots
            .put(economy)
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
            .snapshots
            .put(next)
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
        description = "Start a cancellable breadth-first search task over an explicit candidate exchange universe. Requires MCP Tasks support."
    )]
    async fn search_requires_tasks(
        &self,
        Parameters(_request): Parameters<SearchRequest>,
    ) -> Result<Json<serde_json::Value>, String> {
        Err("axionomy_search requires the MCP io.modelcontextprotocol/tasks capability".to_owned())
    }

    #[tool(
        name = "axionomy_problem_catalog",
        description = "List all canonical Axionomy problem models, strategies, and demonstrated capabilities."
    )]
    async fn problem_catalog(
        &self,
        Parameters(_request): Parameters<ProblemCatalogRequest>,
    ) -> Result<Json<ProblemCatalogResponse>, String> {
        Ok(Json(ProblemCatalogResponse {
            problems: ReferenceService.catalog(),
        }))
    }

    #[tool(
        name = "axionomy_problem_run",
        description = "Run one canonical problem through the shared interface-neutral service and return its complete replay artifact. Requires MCP Tasks support."
    )]
    async fn problem_run_requires_tasks(
        &self,
        Parameters(_request): Parameters<RunRequest>,
    ) -> Result<Json<serde_json::Value>, String> {
        Err(
            "axionomy_problem_run requires the MCP io.modelcontextprotocol/tasks capability"
                .to_owned(),
        )
    }

    async fn require_economy(&self, economy_id: &str) -> Result<Arc<WireEconomy>, String> {
        self.snapshots
            .get(economy_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("economy snapshot `{economy_id}` does not exist"))
    }
}

impl Default for AxionomyMcp<MemorySnapshotStore> {
    fn default() -> Self {
        Self::new(MemorySnapshotStore::default())
    }
}

#[tool_handler(router = self.tool_router)]
impl<S> ServerHandler for AxionomyMcp<S>
where
    S: SnapshotStore,
{
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name == "axionomy_problem_run"
            && context
                .client_capabilities()
                .is_some_and(|capabilities| capabilities.supports_tasks())
        {
            let run = match serde_json::from_value::<RunRequest>(serde_json::Value::Object(
                request.arguments.clone().unwrap_or_default(),
            )) {
                Ok(run) => run,
                Err(error) => {
                    return Ok(CallToolResult::structured_error(serde_json::json!({
                        "error": "invalid_problem_run_request",
                        "message": error.to_string()
                    }))
                    .into());
                }
            };
            if let Err(error) = validate_problem_run(&run) {
                return Ok(CallToolResult::structured_error(serde_json::json!({
                    "error": "problem_run_not_started",
                    "message": error
                }))
                .into());
            }
            let task = self.tasks.spawn(self.task_options.clone(), move |context| {
                Box::pin(run_problem(context, run))
            });
            return Ok(CreateTaskResult::new(task).into());
        }

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
            if let Err(error) = validate_search_request(&search) {
                return Ok(CallToolResult::structured_error(serde_json::json!({
                    "error": "search_not_started",
                    "message": error
                }))
                .into());
            }
            let economy = match self.require_economy(&search.economy_id).await {
                Ok(economy) => economy,
                Err(error) => {
                    return Ok(CallToolResult::structured_error(serde_json::json!({
                        "error": "search_not_started",
                        "message": error
                    }))
                    .into());
                }
            };
            let task = self.tasks.spawn(self.task_options.clone(), move |context| {
                Box::pin(run_search(context, economy, search))
            });
            return Ok(CreateTaskResult::new(task).into());
        }

        let context = ToolCallContext::new(self, request, context);
        self.tool_router.call(context).await
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, McpError> {
        Ok(GetTaskResult::new(self.tasks.get_task(&request.task_id)?))
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.tasks
            .update_task(&request.task_id, request.input_responses)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.tasks.cancel_task(&request.task_id)
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

fn validate_problem_run(request: &RunRequest) -> Result<(), String> {
    if request.budget == 0 {
        return Err("budget must be greater than zero".into());
    }
    let problem = ReferenceService
        .problem(&request.problem)
        .ok_or_else(|| format!("unknown problem `{}`", request.problem))?;
    if let Some(strategy) = &request.strategy
        && !problem
            .strategies
            .iter()
            .any(|known| &known.key == strategy)
    {
        return Err(format!(
            "unknown strategy `{strategy}` for problem `{}`",
            request.problem
        ));
    }
    Ok(())
}

async fn run_problem(
    context: TaskContext,
    request: RunRequest,
) -> Result<CallToolResult, TaskExit> {
    let control = Arc::new(RunControl::default());
    let worker_control = Arc::clone(&control);
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<ServiceProgress>();
    let mut worker = tokio::task::spawn_blocking(move || {
        let mut observer = move |progress| {
            let _ = progress_tx.send(progress);
        };
        ReferenceService.run_with(&request, &worker_control, &mut observer)
    });

    let artifact = loop {
        tokio::select! {
            progress = progress_rx.recv() => {
                if let Some(progress) = progress {
                    context.set_status_message(format!("{}: {} ({}/{})", progress.phase, progress.message, progress.completed, progress.total));
                }
            }
            result = &mut worker => {
                break result
                    .map_err(|error| TaskExit::Error(McpError::internal_error(error.to_string(), None)))?
                    .map_err(|error| TaskExit::Error(McpError::internal_error(error.to_string(), None)))?;
            }
            () = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                if context.is_cancel_requested() {
                    control.cancel();
                    return Err(TaskExit::Cancelled);
                }
            }
        }
    };
    context.set_status_message(format!(
        "completed: {} alternatives, selected {}",
        artifact.documents.len(),
        artifact.selected_document_id
    ));
    let structured = serde_json::to_value(artifact)
        .map_err(|error| TaskExit::Error(McpError::internal_error(error.to_string(), None)))?;
    Ok(CallToolResult::structured(structured))
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
    Ok(())
}

async fn run_search(
    context: TaskContext,
    economy: Arc<WireEconomy>,
    request: SearchRequest,
) -> Result<CallToolResult, TaskExit> {
    let SearchRequest {
        economy_id,
        goal,
        candidates,
        max_expansions,
        chunk_size,
    } = request;
    let mut session = BfsSession::new(economy.as_ref(), goal, move |_| candidates.clone());
    let mut remaining = max_expansions;

    loop {
        let status = session.status();
        if status.is_terminal() {
            let outcome = match status {
                SearchStatus::Solved => SearchOutcome::Solved,
                SearchStatus::Exhausted => SearchOutcome::Exhausted,
                SearchStatus::Running | SearchStatus::Interrupted => unreachable!(),
            };
            return complete_search(&context, economy_id, outcome, session);
        }
        if context.is_cancel_requested() {
            return Err(TaskExit::Cancelled);
        }
        if remaining == 0 {
            return complete_search(&context, economy_id, SearchOutcome::ExpansionLimit, session);
        }

        let budget = remaining.min(chunk_size);
        let report = session.advance(WorkBudget::new(budget), &mut Continue);
        remaining = remaining.saturating_sub(report.work_completed());
        context.set_status_message(progress_message(
            *report.progress(),
            max_expansions,
            remaining,
        ));
        tokio::task::yield_now().await;
    }
}

fn complete_search(
    context: &TaskContext,
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
) -> Result<CallToolResult, TaskExit> {
    let progress = session.progress();
    context.set_status_message(terminal_message(progress));
    let response = SearchResponse {
        economy_id,
        outcome,
        progress,
        solution: session.into_solution(),
    };
    let structured = serde_json::to_value(response)
        .map_err(|error| TaskExit::Error(McpError::internal_error(error.to_string(), None)))?;
    Ok(CallToolResult::structured(structured))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SnapshotStore;
    use axionomy::{Account, Basket, EconomyBuilder, Exchange, Goal, Quantity, Rate, Trace};
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
        };
        (economy, request)
    }

    async fn terminal_task(tasks: &TaskManager, task_id: &str) -> rmcp::model::DetailedTask {
        for _ in 0..100 {
            let task = tasks.get_task(task_id).unwrap();
            if task.status().is_terminal() {
                return task;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task did not reach a terminal state")
    }

    #[tokio::test]
    async fn every_reference_tool_exposes_input_and_output_schemas() {
        let server = AxionomyMcp::default();
        let tools = server.tool_router.list_all();

        assert_eq!(tools.len(), 7);
        for tool in tools {
            assert!(!tool.input_schema.is_empty(), "{} input schema", tool.name);
            assert!(tool.output_schema.is_some(), "{} output schema", tool.name);
        }
    }

    #[tokio::test]
    async fn bfs_task_returns_a_structured_solution() {
        let snapshots = MemorySnapshotStore::default();
        let (economy, mut request) = search_fixture();
        request.economy_id = snapshots.put(economy.clone()).await.unwrap().economy_id;
        let tasks = TaskManager::new();
        let task = tasks.spawn(TaskOptions::new(), move |context| {
            Box::pin(run_search(context, Arc::new(economy), request))
        });

        let detailed = terminal_task(&tasks, &task.task_id).await;
        assert_eq!(detailed.status(), TaskStatus::Completed);
        assert!(
            detailed
                .task
                .status_message
                .as_deref()
                .is_some_and(|message| message.starts_with("finished:"))
        );
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
    async fn bfs_task_observes_task_manager_cancellation() {
        let (economy, request) = search_fixture();
        let tasks = TaskManager::new();
        let task = tasks.spawn(TaskOptions::new(), move |context| {
            Box::pin(run_search(context, Arc::new(economy), request))
        });
        tasks.cancel_task(&task.task_id).unwrap();

        let detailed = terminal_task(&tasks, &task.task_id).await;
        assert_eq!(detailed.status(), TaskStatus::Cancelled);
        assert!(matches!(detailed.payload, TaskPayload::Cancelled));
    }

    #[tokio::test]
    async fn problem_task_returns_the_shared_service_artifact() {
        let request = RunRequest::new("maze").with_strategy("a_star");
        let tasks = TaskManager::new();
        let task_request = request.clone();
        let task = tasks.spawn(TaskOptions::new(), move |context| {
            Box::pin(run_problem(context, task_request))
        });
        let detailed = terminal_task(&tasks, &task.task_id).await;
        assert_eq!(detailed.status(), TaskStatus::Completed);
        let TaskPayload::Completed { result } = detailed.payload else {
            panic!("problem run should complete with a tool result")
        };
        let tool_result: CallToolResult =
            serde_json::from_value(serde_json::Value::Object(result)).unwrap();
        let mcp_artifact: axionomy_service::RunArtifact =
            serde_json::from_value(tool_result.structured_content.unwrap()).unwrap();
        let service_artifact = ReferenceService.run(request).unwrap();
        assert_eq!(mcp_artifact, service_artifact);
    }

    #[test]
    fn search_request_rejects_removed_idempotency_policy() {
        let (_, request) = search_fixture();
        let mut value = serde_json::to_value(request).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("idempotency_key".to_owned(), serde_json::json!("retry"));

        assert!(serde_json::from_value::<SearchRequest>(value).is_err());
    }

    #[tokio::test]
    async fn apply_and_replay_create_new_snapshots_without_replacing_the_source() {
        let snapshots = MemorySnapshotStore::default();
        let server = AxionomyMcp::new(snapshots.clone());
        let (economy, request) = search_fixture();
        let exchange = request.candidates[0].clone();
        let source_id = snapshots.put(economy).await.unwrap().economy_id;

        let Json(applied) = server
            .exchange_apply(Parameters(ApplyRequest {
                economy_id: source_id.clone(),
                exchange: exchange.clone(),
            }))
            .await
            .unwrap();
        let mut trace = Trace::new();
        trace.push(exchange);
        let Json(replayed) = server
            .trace_replay(Parameters(ReplayRequest {
                economy_id: source_id.clone(),
                trace,
            }))
            .await
            .unwrap();

        assert_ne!(source_id, applied.economy_id);
        assert_eq!(applied.economy_id, replayed.economy_id);
        assert_eq!(replayed.receipts.len(), 1);
        let source = snapshots.get(&source_id).await.unwrap().unwrap();
        let next = snapshots.get(&applied.economy_id).await.unwrap().unwrap();
        assert_eq!(
            source.balance(&"source".to_owned(), &"token".to_owned()),
            Quantity::new(1)
        );
        assert_eq!(
            next.balance(&"sink".to_owned(), &"token".to_owned()),
            Quantity::new(1)
        );
    }
}
