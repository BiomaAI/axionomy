use crate::{
    authoring::{self, AuthoringError},
    handles::{AssessmentId, EconomyId, Handle},
    issues,
    messages::{
        AssessResponse, EconomyCounts, EconomyCreateRequest, EconomyEntries, EconomyExtendRequest,
        EconomyStored, ExchangeRequest, ExchangedResponse, FailureCode, Issue, LineKind,
        ProblemCatalogRequest, ProblemCatalogResponse, QuantityLine, ReplayRequest, SearchOutcome,
        SearchRequest, SearchResponse, SearchSolved, StateRoot, StudySummary, ToolFailure,
        TraceFindRequest, TraceFindResponse, TraceStepRequest, TraceStepResponse,
    },
    resources::{self, Address, IMMUTABLE_TTL_MS, LIST_TTL_MS, Target},
    store::{self, Archive, Assessed, Loaded, Origin, State, checkpoint},
    studies,
};
use axionomy::{AssessmentStatus, Quantity, QuantityComparison, paths};
use axionomy_search::{
    BfsSession, GraphSearchProgress,
    session::{Continue, SearchStatus, WorkBudget},
};
use axionomy_service::{
    ReferenceService, ServiceError, ServiceProgress, StudyControl, StudyRequest,
};
use axionomy_store::{Store, StoreError};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        tool::{IntoCallToolResult, ToolCallContext, schema_for_output},
        wrapper::Parameters,
    },
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams,
        CompleteRequestParams, CompleteResult, ContentBlock, CreateTaskResult, GetTaskParams,
        GetTaskResult, Implementation, JsonObject, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, ServerCapabilities, ServerConfig, UpdateTaskParams,
    },
    service::{RequestContext, RoleServer},
    task_manager::{TaskContext, TaskExit, TaskManager, TaskOptions},
    tool, tool_handler, tool_router,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_path_to_error::Segment;
use std::{borrow::Cow, sync::Arc};

#[cfg(test)]
mod tests;

const TASK_POLL_INTERVAL_MS: u64 = 100;
/// Exchanges one replay or search may carry.
pub(crate) const MAX_EXCHANGES: usize = 1_000;
/// Lines a tool result shows before pointing at their resource.
const PREVIEW_LINES: usize = 10;
/// The most state expansions a search runs between cancellation checks.
const MAX_CHUNK_SIZE: usize = 10_000;
const SEARCH: &str = "axionomy_search";
const PROBLEM_STUDY: &str = "axionomy_problem_study";

const INSTRUCTIONS: &str = "Axionomy is an exact economic kernel. An economy holds accounts with \
balances, rates and conservation invariants, and every change is an exchange: some units of a \
rate, applied atomically between the accounts bound to its roles. Accounts, assets, rates, roles \
and invariants are named by paths: lowercase segments of a-z, 0-9 and '-', separated by '/', such \
as 'agents/7/wallet'. Quantities are decimal strings. A handle, economy_id, names an economy that \
never changes: every call that changes something stores a new economy and returns its handle, so \
older handles are branches to return to.

Start here:
1. axionomy_problem_catalog lists the built-in problems with their instance and strategy keys.
2. axionomy_problem_study studies one and returns its documents, the requested strategy's run \
and the problem's other outcomes, each with initial_economy_id and final_economy_id.
3. axionomy_trace_step with a step on final_economy_id names the economy at that step of the \
study.
4. Read that economy's rates (axionomy://economy/{economy_id}/rates) for candidate exchanges, and \
its accounts (.../accounts).
5. axionomy_exchange_assess says whether an exchange would apply, and why not; \
axionomy_exchange_apply applies it and returns your economy.
6. The diff resource (axionomy://economy/{economy_id}/diff/{other_id}) compares your economy \
with the study's next step.

An exchange looks like {\"rate\": \"transfer\", \"bindings\": {\"giver\": \"source\", \
\"receiver\": \"sink\"}, \"units\": \"2\"}. Bind an account to every role of the rate. A role the \
rate opens binds the path of the account the exchange creates, which must not exist yet; a role \
it closes binds an account its effects leave empty.

Steps: step N of a trace is the economy after its first N exchanges, and step 0 the economy they \
were applied to. Positions of exchanges count from 0 (exchange_index, a frame's view.index): the \
exchange at index i takes step i to step i + 1. Messages count from 1 (\"exchange 2 of 3\"). \
Frame i of a study document shows step i + 1 of its final_economy_id, and its exchange is written \
as that economy writes it: applied to step i with axionomy_exchange_apply, it makes step i + 1.

Build economies of your own with axionomy_economy_create and axionomy_economy_extend, at most \
10,000 entries per call; apply a sequence of exchanges with axionomy_trace_replay; find the first \
step where a balance condition holds with axionomy_trace_find; and search for exchanges that \
reach goal balances with axionomy_search. Read what an economy holds through axionomy:// \
resources: tool results link to them, paths appear in their URIs as is \
(.../accounts/agents/7/wallet), every list is paged with a cursor, and a page never changes. \
Economies with equal roots have equal contents, however they were made.

axionomy_problem_study and axionomy_search run as MCP tasks: declare the \
io.modelcontextprotocol/tasks extension and poll tasks/get for the result. Without it they fail \
with tasks_required; every other tool works without it, so a client can still build, step \
through and branch economies of its own.

A failed call returns isError with {error, message, details}: error is a stable code, an \
invalid_request's details.pointer is the JSON pointer of the argument at fault, and an invalid \
exchange lists its issues, each with a message saying what to fix. Handles last as long as the \
server's store: across restarts when it is on disk, until the server stops when it is in memory.";

#[derive(Clone)]
pub struct AxionomyMcp {
    pub(crate) archive: Archive,
    tasks: TaskManager,
    task_options: TaskOptions,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AxionomyMcp {
    /// A server that keeps its economies in `store`. A store on disk keeps
    /// them, and the secret their handles are keyed with, across restarts.
    pub fn new(store: Store) -> Result<Self, StoreError> {
        Ok(Self {
            archive: Archive::open(store)?,
            tasks: TaskManager::new(),
            task_options: TaskOptions::new()
                .with_poll_interval_ms(TASK_POLL_INTERVAL_MS)
                .with_status_message("queued"),
            tool_router: Self::tool_router(),
        })
    }

    /// Overrides retention, polling, and initial-status policy for new tasks.
    pub fn with_task_options(mut self, task_options: TaskOptions) -> Self {
        self.task_options = task_options;
        self
    }

    #[tool(
        name = "axionomy_economy_create",
        title = "Create an economy",
        description = "Store a new economy with up to 10,000 entries: accounts with their balances, rates, and conservation invariants, each keyed by its path, such as `agents/7/wallet`. Grow it past that with axionomy_economy_extend. Returns the economy's economy_id and root, its content hash; repeating the same call returns the same economy_id.",
        output_schema = schema_for_output::<EconomyStored>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn economy_create(
        &self,
        Parameters(request): Parameters<EconomyCreateRequest>,
    ) -> Result<Linked<EconomyStored>, ToolError> {
        self.author(None, request.entries).await
    }

    #[tool(
        name = "axionomy_economy_extend",
        title = "Extend an economy",
        description = "Add up to 10,000 entries to an economy and store the result as a new economy; the economy it extends is unchanged. New paths add accounts, rates or invariants; existing ones gain balances, amounts or weights, but nothing already present is replaced.",
        output_schema = schema_for_output::<EconomyStored>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn economy_extend(
        &self,
        Parameters(request): Parameters<EconomyExtendRequest>,
    ) -> Result<Linked<EconomyStored>, ToolError> {
        let parent = self.economy(&request.economy_id).await?;
        self.author(Some((request.economy_id, parent)), request.entries)
            .await
    }

    #[tool(
        name = "axionomy_exchange_assess",
        title = "Assess an exchange",
        description = "Explain whether an exchange would apply to an economy, without changing it: applicable with the quantities it would move, infeasible with what each account lacks, or invalid with why. Lists longer than the preview are paged through the linked assessment resource. Rates and the roles to bind are listed by the economy's `rates` resource.",
        output_schema = schema_for_output::<AssessResponse>(),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn exchange_assess(
        &self,
        Parameters(request): Parameters<ExchangeRequest>,
    ) -> Result<Linked<AssessResponse>, ToolError> {
        let source = self.economy(&request.economy_id).await?;
        let assessment = source.state.economy.assess(&request.exchange);
        let (assessment_id, stored) = self
            .keep_assessment(&request.economy_id, &[], &request.exchange, assessment)
            .await?;
        let lines = assessment_lines(&stored.assessment).count();
        let response = AssessResponse {
            issues: issues::of(&stored.assessment, &request.exchange, &request.economy_id),
            economy_id: request.economy_id,
            assessment_id: assessment_id.clone(),
            status: stored.assessment.status().into(),
            lines,
            preview: assessment_lines(&stored.assessment)
                .take(PREVIEW_LINES)
                .collect(),
        };
        Ok(Linked::new(
            response,
            assessment_links(&assessment_id, lines),
        ))
    }

    #[tool(
        name = "axionomy_exchange_apply",
        title = "Apply an exchange",
        description = "Apply one exchange atomically and store the economy it makes under a new economy_id; the economy it was applied to is unchanged. A failure stores nothing and explains itself like axionomy_exchange_assess.",
        output_schema = schema_for_output::<ExchangedResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn exchange_apply(
        &self,
        Parameters(request): Parameters<ExchangeRequest>,
    ) -> Result<Linked<ExchangedResponse>, ToolError> {
        self.exchange(request.economy_id, vec![request.exchange])
            .await
    }

    #[tool(
        name = "axionomy_trace_replay",
        title = "Replay exchanges",
        description = "Apply up to 1,000 exchanges in order and store the economy they make under a new economy_id, with every step of the trace reachable through axionomy_trace_step. The first failure stops the replay, stores nothing, and names the failing exchange: \"exchange 2 of 3\" in words, `details.exchange_index` 1. Chain calls to replay longer traces.",
        output_schema = schema_for_output::<ExchangedResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn trace_replay(
        &self,
        Parameters(request): Parameters<ReplayRequest>,
    ) -> Result<Linked<ExchangedResponse>, ToolError> {
        if request.exchanges.is_empty() {
            return Err(ToolError::argument(
                "/exchanges",
                "a replay needs at least one exchange",
            ));
        }
        if request.exchanges.len() > MAX_EXCHANGES {
            return Err(too_many_exchanges(request.exchanges.len()));
        }
        self.exchange(request.economy_id, request.exchanges).await
    }

    #[tool(
        name = "axionomy_trace_step",
        title = "Step through a trace",
        description = "Name the economy at step `step` of the trace that made an economy: the economy after the first `step` of its exchanges. Step zero is the trace's source, the economy they were applied to, which the result names `source_economy_id`; the last step is the economy passed in, which it names `traced_economy_id`. Read the stepped economy, or diff it against its neighbors, to see what each exchange did.",
        output_schema = schema_for_output::<TraceStepResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn trace_step(
        &self,
        Parameters(request): Parameters<TraceStepRequest>,
    ) -> Result<Linked<TraceStepResponse>, ToolError> {
        let loaded = self.economy(&request.economy_id).await?;
        let (source, exchanges) = traced(&request.economy_id, &loaded)?;
        if request.step > exchanges {
            return Err(ToolError::argument(
                "/step",
                format!(
                    "step {} is past the last step of `{}`, which {exchanges} exchanges made",
                    request.step, request.economy_id
                ),
            ));
        }
        self.stepped(request.economy_id, &loaded, source, request.step)
            .await
    }

    #[tool(
        name = "axionomy_trace_find",
        title = "Find where a balance condition starts",
        description = "Find the first step of the trace that made an economy at which an account's balance of an asset compares with a quantity as asked (`equal`, `greater_than_or_equal` or `less_than_or_equal`), and name that step's economy as axionomy_trace_step does. Step zero is the trace's source, the economy its exchanges were applied to. When no step satisfies the condition, `step` is null and `message` says so; an account no economy of the trace has is `not_found`.",
        output_schema = schema_for_output::<TraceFindResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn trace_find(
        &self,
        Parameters(request): Parameters<TraceFindRequest>,
    ) -> Result<Linked<TraceFindResponse>, ToolError> {
        let loaded = self.economy(&request.economy_id).await?;
        let (source_id, _) = traced(&request.economy_id, &loaded)?;
        let source = self.economy(&source_id).await?;
        let opened = || {
            loaded
                .receipts
                .iter()
                .flat_map(|receipt| receipt.opened())
                .any(|account| account == &request.account)
        };
        if source.state.economy.account(&request.account).is_none() && !opened() {
            return Err(ToolError::new(
                FailureCode::NotFound,
                format!(
                    "no economy of the trace that made `{}` has an account `{}`; the accounts of its source are listed at {}",
                    request.economy_id,
                    request.account,
                    Target::Accounts {
                        economy_id: source_id,
                        prefix: None,
                    }
                    .uri()
                ),
            )
            .at("/account"));
        }
        let holds = |balance: Quantity| match request.comparison {
            QuantityComparison::Equal => balance == request.quantity,
            QuantityComparison::GreaterThanOrEqual => balance >= request.quantity,
            QuantityComparison::LessThanOrEqual => balance <= request.quantity,
        };
        // The receipts say what each exchange moved, so the balance after
        // every step follows from the source's without rebuilding a state.
        let mut balance = source
            .state
            .economy
            .balance(&request.account, &request.asset);
        let mut found = holds(balance).then_some(0);
        for (index, receipt) in loaded.receipts.iter().enumerate() {
            if found.is_some() {
                break;
            }
            for delta in receipt.deltas() {
                if delta.account() == &request.account {
                    balance = balance
                        .checked_sub(delta.consumed().quantity(&request.asset))
                        .and_then(|left| {
                            left.checked_add(delta.produced().quantity(&request.asset))
                        })
                        .ok_or_else(|| {
                            ToolError::internal("a receipt does not follow its state")
                        })?;
                }
            }
            found = holds(balance).then_some(index + 1);
        }
        let Some(step) = found else {
            let comparison = match request.comparison {
                QuantityComparison::Equal => "equal to",
                QuantityComparison::GreaterThanOrEqual => "at least",
                QuantityComparison::LessThanOrEqual => "at most",
            };
            return Ok(Linked::new(
                TraceFindResponse {
                    step: None,
                    economy: None,
                    message: Some(format!(
                        "`{}`'s balance of `{}` is {comparison} {} at none of the {} steps of the trace that made `{}`",
                        request.account,
                        request.asset,
                        request.quantity,
                        loaded.receipts.len() + 1,
                        request.economy_id
                    )),
                },
                Vec::new(),
            ));
        };
        Ok(self
            .stepped(request.economy_id, &loaded, source_id, step)
            .await?
            .map(|economy| TraceFindResponse {
                step: Some(step),
                economy: Some(economy),
                message: None,
            }))
    }

    #[tool(
        name = "axionomy_search",
        title = "Search for a trace",
        description = "Start a cancellable breadth-first search, as an MCP task, for exchanges from up to 1,000 candidates that bring the named accounts to their goal balances. A solution is stored as the economy it reaches; its `exchanges` resource is the trace. Runs only as an MCP task: a client that does not declare the io.modelcontextprotocol/tasks extension gets `tasks_required`.",
        output_schema = schema_for_output::<SearchResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn search_requires_tasks(
        &self,
        Parameters(_request): Parameters<SearchRequest>,
    ) -> Result<Linked<SearchResponse>, ToolError> {
        Err(tasks_required(SEARCH))
    }

    #[tool(
        name = "axionomy_problem_catalog",
        title = "List problems",
        description = "List the built-in Axionomy problems: each problem's `key`, its instances (`micro`, `showcase`, `stress`) and its strategies' keys, which axionomy_problem_study takes, with their defaults and capabilities.",
        output_schema = schema_for_output::<ProblemCatalogResponse>(),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn problem_catalog(
        &self,
        Parameters(_request): Parameters<ProblemCatalogRequest>,
    ) -> Result<Linked<ProblemCatalogResponse>, ToolError> {
        Ok(Linked::new(
            ProblemCatalogResponse {
                problems: ReferenceService.catalog(),
            },
            Vec::new(),
        ))
    }

    #[tool(
        name = "axionomy_problem_study",
        title = "Study a problem",
        description = "Study one built-in problem and return a summary of its documents: the run of the requested `strategy`, which is the `selected_document`, beside the problem's other outcomes, usually the runs of its other strategies and cases such as a deadlock. Take the `problem`, `instance` and `strategy` keys from axionomy_problem_catalog. Each document is two stored economies: initial_economy_id, where its trajectory starts, and final_economy_id, where its exchanges reach. Step into the final one with axionomy_trace_step and branch from any step; frame i's exchange applies to step i. The study's documents and frames are linked resources. Runs only as an MCP task: a client that does not declare the io.modelcontextprotocol/tasks extension gets `tasks_required`.",
        output_schema = schema_for_output::<StudySummary>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn problem_study_requires_tasks(
        &self,
        Parameters(_request): Parameters<StudyRequest>,
    ) -> Result<Linked<StudySummary>, ToolError> {
        Err(tasks_required(PROBLEM_STUDY))
    }
}

impl AxionomyMcp {
    pub(crate) async fn economy(&self, economy_id: &EconomyId) -> Result<Arc<Loaded>, ToolError> {
        self.archive
            .economy(economy_id)
            .await
            .map_err(ToolError::internal)?
            .ok_or_else(|| ToolError::new(FailureCode::NotFound, self.not_stored(economy_id)))
    }

    /// Why the store has no `handle`, and how to get what it named again.
    pub(crate) fn not_stored<H: Handle>(&self, handle: &H) -> String {
        let store = if self.archive.in_memory() {
            "This server keeps its store in memory, so every handle it issued before it last restarted is gone"
        } else {
            "This server keeps its store on disk, where handles outlive restarts, so this one came from another store"
        };
        format!(
            "{} `{handle}` is not in this server's store. {store}: {}, which issues a new handle.",
            H::NAMES,
            H::MADE_AGAIN
        )
    }

    async fn author(
        &self,
        parent: Option<(EconomyId, Arc<Loaded>)>,
        entries: EconomyEntries,
    ) -> Result<Linked<EconomyStored>, ToolError> {
        let size = entries.len();
        let operation = store::operation("entries", &entries);
        let (parent_id, base) = parent.unzip();
        let economy =
            authoring::extend(base.as_deref().map(|loaded| &loaded.state.economy), entries)
                .map_err(|error| {
                    let code = match error {
                        AuthoringError::TooLarge { .. } => FailureCode::TooLarge,
                        AuthoringError::AccountHolds { .. }
                        | AuthoringError::RateHas { .. }
                        | AuthoringError::InvariantWeights { .. } => FailureCode::Conflict,
                    };
                    ToolError::new(code, error.to_string())
                })?;
        let counts = counts(&economy);
        let stored = self
            .archive
            .author(parent_id.clone(), &operation, size, State::new(economy))
            .await
            .map_err(ToolError::internal)?;
        let root = self.economy(&stored.economy_id).await?.state.root();
        let link = Target::Economy {
            economy_id: stored.economy_id.clone(),
        }
        .link("economy", "The stored economy");
        Ok(Linked::new(
            EconomyStored {
                economy_id: stored.economy_id,
                parent_id,
                deduplicated: stored.deduplicated,
                root: StateRoot(root),
                counts,
            },
            vec![link],
        ))
    }

    /// Applies `exchanges` in order to an economy and stores the economy they make.
    async fn exchange(
        &self,
        source_economy_id: EconomyId,
        exchanges: Vec<paths::Exchange>,
    ) -> Result<Linked<ExchangedResponse>, ToolError> {
        let source = self.economy(&source_economy_id).await?;
        let mut state = source.state.clone();
        let mut steps = Vec::with_capacity(exchanges.len());
        for (index, exchange) in exchanges.iter().enumerate() {
            if let Ok(receipt) = state.apply(exchange.clone()) {
                let step = source.record.point.step + index as u64 + 1;
                steps.push((receipt, checkpoint(step).then(|| state.tree.clone())));
                continue;
            }
            // Explain the failure against the state this exchange met.
            let assessment = state.economy.assess(exchange);
            let (assessment_id, stored) = self
                .keep_assessment(
                    &source_economy_id,
                    &exchanges[..index],
                    exchange,
                    assessment,
                )
                .await?;
            return Err(failed_exchange(
                index,
                exchanges.len(),
                &assessment_id,
                &stored,
            ));
        }
        store_exchanged(
            &self.archive,
            source_economy_id,
            &source,
            &exchanges,
            steps,
            state,
        )
        .await
    }

    /// Names step `step` of the trace that made `economy_id` from `source`.
    async fn stepped(
        &self,
        economy_id: EconomyId,
        loaded: &Loaded,
        source: EconomyId,
        step: usize,
    ) -> Result<Linked<TraceStepResponse>, ToolError> {
        let stored = self
            .archive
            .step(&economy_id, loaded, step)
            .await
            .map_err(ToolError::internal)?;
        let stepped = self.economy(&stored.economy_id).await?;
        Ok(
            exchanged_response(source, stored.economy_id, stored.deduplicated, &stepped).map(
                |economy| TraceStepResponse {
                    traced_economy_id: economy_id,
                    step,
                    economy,
                },
            ),
        )
    }

    async fn keep_assessment(
        &self,
        economy_id: &EconomyId,
        before: &[paths::Exchange],
        exchange: &paths::Exchange,
        assessment: paths::Assessment,
    ) -> Result<(AssessmentId, Arc<Assessed>), ToolError> {
        self.archive
            .keep_assessment(economy_id, before, exchange, assessment)
            .await
            .map_err(ToolError::internal)
    }

    async fn start_search(
        &self,
        arguments: Option<&JsonObject>,
    ) -> Result<CallToolResponse, ToolError> {
        let request: SearchRequest = arguments_of(arguments)?;
        if !(1..=MAX_CHUNK_SIZE).contains(&request.chunk_size) {
            return Err(ToolError::argument(
                "/chunk_size",
                format!(
                    "chunk_size must be from 1 to {MAX_CHUNK_SIZE}, so cancellation stays responsive"
                ),
            ));
        }
        if request.candidates.len() > MAX_EXCHANGES {
            return Err(too_many_exchanges(request.candidates.len()).at("/candidates"));
        }
        let goal =
            request
                .goal
                .iter()
                .try_fold(paths::Goal::new(), |goal, (account, amounts)| {
                    let basket = paths::Basket::try_from_entries(
                        amounts
                            .iter()
                            .map(|(asset, quantity)| (asset.clone(), *quantity)),
                    )
                    .map_err(|error| {
                        let mut pointer = "/goal".to_owned();
                        push_token(&mut pointer, account.as_str());
                        ToolError::argument(&pointer, error.to_string())
                    })?;
                    Ok::<_, ToolError>(goal.require(account.clone(), basket))
                })?;
        let source = self.economy(&request.economy_id).await?;
        let archive = self.archive.clone();
        let task = self.tasks.spawn(self.task_options.clone(), move |context| {
            Box::pin(run_search(context, archive, source, request, goal))
        });
        Ok(CreateTaskResult::new(task).into())
    }

    fn start_problem_study(
        &self,
        arguments: Option<&JsonObject>,
    ) -> Result<CallToolResponse, ToolError> {
        let request: StudyRequest = arguments_of(arguments)?;
        if request.budget == 0 {
            return Err(ToolError::argument(
                "/budget",
                "budget must be greater than zero",
            ));
        }
        ReferenceService.resolve(&request).map_err(|error| {
            let refused = ToolError::new(
                FailureCode::InvalidRequest,
                format!(
                    "{error}; axionomy_problem_catalog lists every problem with its instances and strategies"
                ),
            );
            match error {
                ServiceError::UnknownProblem(_) => refused.at("/problem"),
                ServiceError::UnknownInstance { .. } => refused.at("/instance"),
                ServiceError::UnknownStrategy { .. } => refused.at("/strategy"),
                _ => refused,
            }
        })?;
        let archive = self.archive.clone();
        let task = self.tasks.spawn(self.task_options.clone(), move |context| {
            Box::pin(study_problem(context, archive, request))
        });
        Ok(CreateTaskResult::new(task).into())
    }
}

impl Default for AxionomyMcp {
    /// A server that keeps its economies in memory, until it stops.
    ///
    /// # Panics
    ///
    /// If an in-memory store cannot be created, or the operating system
    /// provides no randomness for the handle secret.
    fn default() -> Self {
        Self::new(Store::memory().expect("an in-memory store opens"))
            .expect("an in-memory store keeps the server's tables")
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AxionomyMcp {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if let Err(error) = check_arguments(&request.name, request.arguments.as_ref()) {
            return error.into_call_tool_result();
        }
        let tasks = context
            .client_capabilities()
            .is_some_and(|capabilities| capabilities.supports_tasks());
        if tasks && request.name == SEARCH {
            return self
                .start_search(request.arguments.as_ref())
                .await
                .or_else(ToolError::into_call_tool_result);
        }
        if tasks && request.name == PROBLEM_STUDY {
            return self
                .start_problem_study(request.arguments.as_ref())
                .or_else(ToolError::into_call_tool_result);
        }
        let context = ToolCallContext::new(self, request, context);
        self.tool_router.call(context).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tool_router.list_all())
            .with_ttl_ms(LIST_TTL_MS)
            .with_cache_scope(CacheScope::Public))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        // Snapshots are reached through their handles and the links tool
        // results carry, never through a listing shared by every caller.
        Ok(ListResourcesResult::with_all_items(Vec::new())
            .with_ttl_ms(LIST_TTL_MS)
            .with_cache_scope(CacheScope::Public))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(
            ListResourceTemplatesResult::with_all_items(resources::templates())
                .with_ttl_ms(LIST_TTL_MS)
                .with_cache_scope(CacheScope::Public),
        )
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let address = Address::parse(&request.uri)?;
        let text = self.read(&address).await?;
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, request.uri).with_mime_type("application/json"),
        ])
        .with_ttl_ms(IMMUTABLE_TTL_MS)
        .with_cache_scope(CacheScope::Private)
        .into())
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        self.completion(request).await
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

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_completions()
                .enable_resources()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
        .with_server_info(Implementation::new(
            "axionomy-mcp",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(INSTRUCTIONS)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }
}

/// A structured tool result followed by links to the resources that page
/// through what it summarizes.
pub(crate) struct Linked<T> {
    value: T,
    links: Vec<Resource>,
}

impl<T: Serialize> Linked<T> {
    fn new(value: T, links: Vec<Resource>) -> Self {
        Self { value, links }
    }

    /// The same links with the value wrapped or changed.
    fn map<U>(self, change: impl FnOnce(T) -> U) -> Linked<U> {
        Linked {
            value: change(self.value),
            links: self.links,
        }
    }

    fn into_result(self) -> Result<CallToolResult, McpError> {
        let value = serde_json::to_value(self.value)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let mut result = CallToolResult::structured(value);
        result
            .content
            .extend(self.links.into_iter().map(ContentBlock::resource_link));
        Ok(result)
    }
}

impl<T: Serialize> IntoCallToolResult for Linked<T> {
    fn into_call_tool_result(self) -> Result<CallToolResponse, McpError> {
        self.into_result().map(Into::into)
    }
}

/// A tool execution error the caller can act on: reported as a result with
/// `isError`, a stable code and links to anything too long to inline.
#[derive(Debug)]
pub(crate) struct ToolError {
    failure: ToolFailure,
    links: Vec<Resource>,
}

impl ToolError {
    pub(crate) fn new(error: FailureCode, message: impl Into<String>) -> Self {
        Self {
            failure: ToolFailure {
                error,
                message: message.into(),
                details: None,
            },
            links: Vec::new(),
        }
    }

    pub(crate) fn internal(error: impl ToString) -> Self {
        Self::new(FailureCode::Internal, error.to_string())
    }

    /// An `invalid_request` naming the argument at fault by its JSON pointer.
    fn argument(pointer: &str, message: impl Into<String>) -> Self {
        Self::new(FailureCode::InvalidRequest, message).at(pointer)
    }

    /// Names the argument at fault by its JSON pointer (RFC 6901).
    fn at(self, pointer: &str) -> Self {
        self.with_details(ArgumentDetails { pointer })
    }

    fn with_details(mut self, details: impl Serialize) -> Self {
        self.failure.details = serde_json::to_value(details).ok();
        self
    }

    fn with_links(mut self, links: Vec<Resource>) -> Self {
        self.links = links;
        self
    }
}

impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, McpError> {
        let value = serde_json::to_value(&self.failure)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let mut result = CallToolResult::structured_error(value);
        result
            .content
            .extend(self.links.into_iter().map(ContentBlock::resource_link));
        Ok(result.into())
    }
}

fn tasks_required(tool: &str) -> ToolError {
    ToolError::new(
        FailureCode::TasksRequired,
        format!(
            "{tool} runs only as an MCP task: declare the io.modelcontextprotocol/tasks extension and poll tasks/get for its result. Every other tool works without it."
        ),
    )
}

fn too_many_exchanges(count: usize) -> ToolError {
    ToolError::new(
        FailureCode::TooLarge,
        format!("{count} exchanges exceed the limit of {MAX_EXCHANGES} per call; split them"),
    )
}

/// The details of an `invalid_request`: which argument is at fault.
#[derive(Serialize)]
struct ArgumentDetails<'a> {
    /// The JSON pointer (RFC 6901) of the argument, within the call's
    /// arguments.
    pointer: &'a str,
}

/// Reads a tool's arguments, refusing malformed ones with the JSON pointer of
/// the argument at fault.
fn arguments_of<T: DeserializeOwned>(arguments: Option<&JsonObject>) -> Result<T, ToolError> {
    let none = JsonObject::new();
    serde_path_to_error::deserialize(arguments.unwrap_or(&none)).map_err(|error| {
        let mut pointer = String::new();
        for segment in error.path() {
            let token = match segment {
                Segment::Seq { index } => index.to_string(),
                Segment::Map { key } => key.clone(),
                Segment::Enum { variant } => variant.clone(),
                Segment::Unknown => continue,
            };
            push_token(&mut pointer, &token);
        }
        let message = error.into_inner().to_string();
        // Serde reports a missing field at the object that should have it,
        // naming the field only in its message.
        if let Some(field) = message
            .strip_prefix("missing field `")
            .and_then(|rest| rest.split('`').next())
        {
            push_token(&mut pointer, field);
        }
        let message = if pointer.is_empty() {
            format!("the arguments are invalid: {message}")
        } else {
            format!("argument `{pointer}` is invalid: {message}")
        };
        ToolError::argument(&pointer, message)
    })
}

/// Appends one reference token to a JSON pointer, escaped as RFC 6901 says.
fn push_token(pointer: &mut String, token: &str) {
    pointer.push('/');
    pointer.push_str(&token.replace('~', "~0").replace('/', "~1"));
}

/// Reads a call's arguments as its tool will, so a malformed one is refused
/// with a structured `invalid_request` naming the argument at fault, rather
/// than with the tool router's text alone.
fn check_arguments(tool: &str, arguments: Option<&JsonObject>) -> Result<(), ToolError> {
    fn check<T: DeserializeOwned>(arguments: Option<&JsonObject>) -> Result<(), ToolError> {
        arguments_of::<T>(arguments).map(drop)
    }
    match tool {
        "axionomy_economy_create" => check::<EconomyCreateRequest>(arguments),
        "axionomy_economy_extend" => check::<EconomyExtendRequest>(arguments),
        "axionomy_exchange_assess" | "axionomy_exchange_apply" => {
            check::<ExchangeRequest>(arguments)
        }
        "axionomy_trace_replay" => check::<ReplayRequest>(arguments),
        "axionomy_trace_step" => check::<TraceStepRequest>(arguments),
        "axionomy_trace_find" => check::<TraceFindRequest>(arguments),
        SEARCH => check::<SearchRequest>(arguments),
        "axionomy_problem_catalog" => check::<ProblemCatalogRequest>(arguments),
        PROBLEM_STUDY => check::<StudyRequest>(arguments),
        _ => Ok(()),
    }
}

/// The source a traced economy's exchanges were applied to, and how many
/// there were. An authored economy has no trace.
fn traced(economy_id: &EconomyId, loaded: &Loaded) -> Result<(EconomyId, usize), ToolError> {
    match (loaded.record.origin, &loaded.record.parent) {
        (Origin::Exchanged { exchanges }, Some(source)) => Ok((source.clone(), exchanges)),
        _ => Err(ToolError::argument(
            "/economy_id",
            format!(
                "`{economy_id}` was authored, not made by exchanges, so it has no trace to step through; name an economy that axionomy_exchange_apply, axionomy_trace_replay, axionomy_search or a study made"
            ),
        )),
    }
}

pub(crate) fn counts(economy: &paths::Economy) -> EconomyCounts {
    EconomyCounts {
        accounts: economy.accounts().count(),
        assets: economy.asset_keys().len(),
        rates: economy.rate_ids().count(),
        invariants: economy.invariants().count(),
    }
}

/// Stores the state a call's exchanges reached from `source` and describes it.
/// Applying one exchange and replaying it alone are the same operation, so
/// they share a handle.
async fn store_exchanged(
    archive: &Archive,
    source_economy_id: EconomyId,
    source: &Loaded,
    exchanges: &[paths::Exchange],
    steps: Vec<(paths::Receipt, Option<axionomy_tree::Tree>)>,
    state: State,
) -> Result<Linked<ExchangedResponse>, ToolError> {
    let operation = store::operation("exchanges", &exchanges);
    let stored = archive
        .exchanged(&source_economy_id, source, &operation, steps, state)
        .await
        .map_err(ToolError::internal)?;
    let loaded = archive
        .economy(&stored.economy_id)
        .await
        .map_err(ToolError::internal)?
        .ok_or_else(|| ToolError::internal("a stored economy cannot be read back"))?;
    Ok(exchanged_response(
        source_economy_id,
        stored.economy_id,
        stored.deduplicated,
        &loaded,
    ))
}

fn exchanged_response(
    source_economy_id: EconomyId,
    economy_id: EconomyId,
    deduplicated: bool,
    loaded: &Loaded,
) -> Linked<ExchangedResponse> {
    let receipts = &loaded.receipts;
    let links = exchanged_links(&economy_id);
    Linked::new(
        ExchangedResponse {
            source_economy_id,
            economy_id,
            deduplicated,
            root: StateRoot(loaded.state.root()),
            exchanges: receipts.len(),
            changes: receipt_lines(receipts).count(),
            opened: receipts
                .iter()
                .flat_map(|receipt| receipt.opened())
                .cloned()
                .collect(),
            closed: receipts
                .iter()
                .flat_map(|receipt| receipt.closed())
                .cloned()
                .collect(),
        },
        links,
    )
}

fn exchanged_links(economy_id: &EconomyId) -> Vec<Resource> {
    let economy_id = economy_id.clone();
    vec![
        Target::Economy {
            economy_id: economy_id.clone(),
        }
        .link("economy", "The stored economy"),
        Target::Changes {
            economy_id: economy_id.clone(),
        }
        .link("changes", "Quantities the exchanges moved, paged"),
        Target::Exchanges { economy_id }.link("exchanges", "The exchanges applied, paged"),
    ]
}

fn assessment_links(assessment_id: &AssessmentId, lines: usize) -> Vec<Resource> {
    let assessment = Target::Assessment {
        assessment_id: assessment_id.clone(),
    }
    .link("assessment", "The full assessment");
    if lines <= PREVIEW_LINES {
        return vec![assessment];
    }
    let lines_link = Target::AssessmentLines {
        assessment_id: assessment_id.clone(),
    }
    .link("assessment-lines", format!("All {lines} lines, paged"));
    vec![assessment, lines_link]
}

/// Explains the failure of the exchange at `exchange_index` of a call that
/// applies `exchanges`, assessed against the economy it met.
fn failed_exchange(
    exchange_index: usize,
    exchanges: usize,
    assessment_id: &AssessmentId,
    stored: &Assessed,
) -> ToolError {
    let which = if exchanges == 1 {
        "the exchange".to_owned()
    } else {
        format!("exchange {} of {exchanges}", exchange_index + 1)
    };
    let lines = assessment_lines(&stored.assessment).count();
    let links = assessment_links(assessment_id, lines);
    let assessment = &stored.assessment;
    let error = match assessment.status() {
        AssessmentStatus::Infeasible => {
            let missing: Vec<QuantityLine> = assessment_lines(assessment).collect();
            ToolError::new(
                FailureCode::Infeasible,
                format!(
                    "{which} is infeasible: {}; nothing was stored",
                    issues::shortfalls(&missing)
                ),
            )
            .with_details(FailureDetails {
                exchange_index,
                assessment_id,
                issues: Vec::new(),
                lines,
                preview: missing.into_iter().take(PREVIEW_LINES).collect(),
            })
        }
        AssessmentStatus::Invalid => {
            let issues = issues::of(assessment, &stored.exchange, &stored.economy_id);
            let reason = issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            ToolError::new(
                FailureCode::Invalid,
                format!("{which} is invalid: {reason}; nothing was stored"),
            )
            .with_details(FailureDetails {
                exchange_index,
                assessment_id,
                issues,
                lines,
                preview: Vec::new(),
            })
        }
        AssessmentStatus::Applicable => ToolError::internal(format!(
            "{which} failed although its assessment is applicable"
        )),
    };
    error.with_links(links)
}

#[derive(Serialize)]
struct FailureDetails<'a> {
    /// The failing exchange's position in the call, from zero.
    exchange_index: usize,
    assessment_id: &'a AssessmentId,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    issues: Vec<Issue>,
    lines: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    preview: Vec<QuantityLine>,
}

pub(crate) fn delta_lines(
    exchange_index: Option<usize>,
    deltas: &[paths::AccountDelta],
) -> impl Iterator<Item = QuantityLine> + '_ {
    deltas.iter().flat_map(move |delta| {
        [
            (LineKind::Consumed, delta.consumed()),
            (LineKind::Produced, delta.produced()),
            (LineKind::Preserved, delta.preserved()),
        ]
        .into_iter()
        .flat_map(move |(kind, basket)| {
            basket.iter().map(move |(asset, quantity)| QuantityLine {
                exchange_index,
                account: delta.account().clone(),
                asset: asset.clone(),
                kind,
                quantity: *quantity,
            })
        })
    })
}

pub(crate) fn receipt_lines(
    receipts: &[paths::Receipt],
) -> impl Iterator<Item = QuantityLine> + '_ {
    receipts
        .iter()
        .enumerate()
        .flat_map(|(index, receipt)| delta_lines(Some(index), receipt.deltas()))
}

/// What explains an assessment: the projected changes of an applicable
/// exchange, or what each account lacks for an infeasible one.
pub(crate) fn assessment_lines(
    assessment: &paths::Assessment,
) -> Box<dyn Iterator<Item = QuantityLine> + '_> {
    if let Some(deltas) = assessment.projected_deltas() {
        return Box::new(delta_lines(None, deltas));
    }
    Box::new(assessment.shortfalls().iter().flat_map(|shortfall| {
        shortfall
            .missing()
            .iter()
            .map(|(asset, quantity)| QuantityLine {
                exchange_index: None,
                account: shortfall.account().clone(),
                asset: asset.clone(),
                kind: LineKind::Missing,
                quantity: *quantity,
            })
    }))
}

fn task_error(error: impl ToString) -> TaskExit {
    TaskExit::Error(McpError::internal_error(error.to_string(), None))
}

async fn study_problem(
    context: TaskContext,
    archive: Archive,
    request: StudyRequest,
) -> Result<CallToolResult, TaskExit> {
    let control = Arc::new(StudyControl::default());
    let worker_control = Arc::clone(&control);
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<ServiceProgress>();
    let mut worker = tokio::task::spawn_blocking(move || {
        studies::compute(&request, &worker_control, Some(progress_tx))
    });

    let mut reporting = true;
    let (study, trajectories) = loop {
        tokio::select! {
            progress = progress_rx.recv(), if reporting => match progress {
                Some(progress) => context.set_status_message(format!("{}: {} ({}/{})", progress.phase, progress.message, progress.completed, progress.total)),
                // The worker has returned; its result is ready.
                None => reporting = false,
            },
            result = &mut worker => {
                break result.map_err(task_error)?.map_err(task_error)?;
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
        "recording {} documents as economies",
        study.documents.len()
    ));
    let (study_id, record) = archive
        .keep_study(study, trajectories)
        .await
        .map_err(task_error)?;
    let summary = StudySummary {
        study_id,
        key: record.key,
        selected_document: record.selected_document,
        documents: record.documents,
    };
    context.set_status_message(format!(
        "completed: {} documents, selected {}",
        summary.documents.len(),
        summary.selected_document
    ));
    let mut links = vec![
        Target::Study {
            study_id: summary.study_id.clone(),
        }
        .link("study", "The study's key and documents, each with its two economies"),
        Target::Frames {
            study_id: summary.study_id.clone(),
            document: summary.selected_document.clone(),
        }
        .link("frames", "The selected document's frames, paged; frame i's exchange applies to step i of its final economy"),
    ];
    if let Some(selected) = summary
        .documents
        .iter()
        .find(|document| document.id == summary.selected_document)
    {
        links.push(
            Target::Economy {
                economy_id: selected.final_economy_id.clone(),
            }
            .link(
                "economy",
                "The economy the selected document reaches; step into it with axionomy_trace_step",
            ),
        );
    }
    Linked::new(summary, links)
        .into_result()
        .map_err(TaskExit::Error)
}

async fn run_search(
    context: TaskContext,
    archive: Archive,
    source: Arc<Loaded>,
    request: SearchRequest,
    goal: paths::Goal,
) -> Result<CallToolResult, TaskExit> {
    let SearchRequest {
        economy_id,
        candidates,
        max_expansions,
        chunk_size,
        ..
    } = request;
    let max_expansions = max_expansions.get();
    let mut session = BfsSession::new(&source.state.economy, goal, move |_| candidates.clone());
    let mut remaining = max_expansions;

    let outcome = loop {
        match session.status() {
            SearchStatus::Solved => break SearchOutcome::Solved,
            SearchStatus::Exhausted => break SearchOutcome::Exhausted,
            SearchStatus::Running | SearchStatus::Interrupted => {}
        }
        if context.is_cancel_requested() {
            return Err(TaskExit::Cancelled);
        }
        if remaining == 0 {
            break SearchOutcome::ExpansionLimit;
        }
        let report = session.advance(WorkBudget::new(remaining.min(chunk_size)), &mut Continue);
        remaining = remaining.saturating_sub(report.work_completed());
        context.set_status_message(progress_message(
            *report.progress(),
            max_expansions,
            remaining,
        ));
        tokio::task::yield_now().await;
    };

    let progress = session.progress();
    context.set_status_message(format!("finished: {}", describe(progress)));
    let mut links = Vec::new();
    let solution = match session.into_solution() {
        Some(solution) => {
            let exchanges = solution.trace().exchanges().to_vec();
            let mut state = source.state.clone();
            let mut steps = Vec::with_capacity(exchanges.len());
            for (index, exchange) in exchanges.iter().enumerate() {
                let receipt = state.apply(exchange.clone()).map_err(task_error)?;
                let step = source.record.point.step + index as u64 + 1;
                steps.push((receipt, checkpoint(step).then(|| state.tree.clone())));
            }
            let stored = store_exchanged(
                &archive,
                economy_id.clone(),
                &source,
                &exchanges,
                steps,
                state,
            )
            .await
            .map_err(|error| task_error(error.failure.message))?
            .value;
            links = exchanged_links(&stored.economy_id);
            Some(SearchSolved {
                economy_id: stored.economy_id,
                exchanges: stored.exchanges,
                cost: solution.cost(),
            })
        }
        None => None,
    };
    Linked::new(
        SearchResponse {
            economy_id,
            outcome,
            progress,
            solution,
        },
        links,
    )
    .into_result()
    .map_err(TaskExit::Error)
}

fn describe(progress: GraphSearchProgress) -> String {
    format!(
        "expanded={} generated={} frontier={} visited={}",
        progress.expanded(),
        progress.generated(),
        progress.frontier(),
        progress.visited()
    )
}

fn progress_message(
    progress: GraphSearchProgress,
    max_expansions: usize,
    remaining: usize,
) -> String {
    format!(
        "{} of at most {max_expansions} expansions, remaining={remaining}",
        describe(progress)
    )
}
