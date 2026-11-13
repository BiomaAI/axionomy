use crate::{
    authoring::{self, AuthoringError},
    handles::{AssessmentId, EconomyId, Handle, RefId},
    issues,
    messages::{
        Amounts, ArmResults, AssessResponse, EconomyCounts, EconomyCreateRequest, EconomyEntries,
        EconomyExtendRequest, EconomyStored, ExchangeRequest, ExchangedResponse,
        ExperimentObjective, ExperimentRequest, ExperimentResults, FailureCode, FrameReceipt,
        Issue, LineKind, PackageListRequest, PackageListResponse, PackageRef, PackageSummary,
        PackageVersion, ProblemCatalogRequest, ProblemCatalogResponse, QuantityLine,
        RefAdvanceRequest, RefCreateRequest, RefDeleteRequest, RefFrame, RefFramesRequest,
        RefFramesResponse, RefPosition, RefReadRequest, ReplayRequest, SearchOutcome,
        SearchRequest, SearchResponse, SearchSolved, StateRoot, StudySummary, ToolFailure,
        TraceFindRequest, TraceFindResponse, TraceStepRequest, TraceStepResponse, TrialResult,
    },
    resources::{self, Address, IMMUTABLE_TTL_MS, LIST_TTL_MS, Target},
    store::{self, Archive, Assessed, Loaded, Origin, content_name},
    studies,
};
use axionomy::{
    AccountPath, AssessmentStatus, PathKind, Quantity, QuantityComparison, path::Path, paths,
};
use axionomy_engine::{
    CandidateSet, Effort, Engine, EngineError, Experiment, ExperimentPath, Outcome, Results,
    Strategy,
};
use axionomy_search::GraphSearchProgress;
use axionomy_store::{DefinitionPath, StoreError};
use axionomy_study::{Progress, Registry, StudyControl, StudyError, StudyRequest};
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
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    ops::ControlFlow,
    sync::Arc,
    time::Duration,
};

#[cfg(test)]
mod tests;

const TASK_POLL_INTERVAL_MS: u64 = 100;
/// Exchanges one replay or search may carry.
pub(crate) const MAX_EXCHANGES: usize = 1_000;
/// Lines a tool result shows before pointing at their resource.
const PREVIEW_LINES: usize = 10;
/// The most state expansions a search runs between cancellation checks.
const MAX_CHUNK_SIZE: usize = 10_000;
/// Frames one call to `axionomy_ref_frames` returns at most.
const MAX_FRAMES: usize = 100;
/// The longest `axionomy_ref_frames` waits for a ref to move.
const MAX_WAIT_SECONDS: u64 = 30;
/// Trials over all its arms an experiment may run, or report, in one call.
const MAX_TRIALS: u64 = 1_000;
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
1. axionomy_problem_catalog lists the problems this server studies with their instance and strategy keys.
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

Packages loaded when the server started carry economies, strategies and experiments. \
axionomy_package_list lists them: each package's refs, each with the economy_id of the point it \
names, and its experiments and strategies by path. axionomy_experiment_run runs an experiment, \
such as `maze/micro`, and returns every arm's trials, each with its seed, how it ended, its \
objective values and the economy_id it reached: step through that economy's trace with \
axionomy_trace_step and branch from any step, as from a study's final economy. \
axionomy_experiment_results gives the results an experiment last recorded.

An application that moves through economies, such as a game or a simulation, keeps a ref on the \
economy it is at. axionomy_ref_create makes one and axionomy_ref_read says where it is. \
axionomy_ref_advance moves it forward from the economy the mover saw it at to one made from \
there, such as axionomy_exchange_apply's result; when another move came first it fails with \
`moved` and details.current_economy_id, where the ref is, so look again from there. \
axionomy_ref_frames gives a follower the steps the ref was moved over after the last economy it \
handled, waiting up to wait_seconds for the next: an exchange that was only tried out is never \
delivered. axionomy_ref_delete deletes a ref from the economy the deleter saw it at, failing with \
`moved` as an advance does; the economies it named stay. A package's refs are the package's and \
never move: make a ref of your own at the economy one names. Every economy a ref or a frame names \
is an economy_id like any other.

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
    /// The problems `axionomy_problem_study` studies, until packages replace
    /// them.
    pub(crate) registry: Arc<Registry>,
    tasks: TaskManager,
    task_options: TaskOptions,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AxionomyMcp {
    /// A server over `engine`, keeping its economies, and the handles that
    /// name them, in the engine's store, and studying the problems of
    /// `registry`. A store on disk keeps them, and the secret the handles are
    /// keyed with, across restarts.
    pub fn new(engine: Engine, registry: Registry) -> Result<Self, StoreError> {
        Ok(Self {
            archive: Archive::open(engine)?,
            registry: Arc::new(registry),
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
        let assessment = self
            .archive
            .assess(&source, &request.exchange)
            .await
            .map_err(ToolError::internal)?;
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
                .receipts()
                .iter()
                .flat_map(|receipt| receipt.opened())
                .any(|account| account == &request.account)
        };
        if source.state().economy.account(&request.account).is_none() && !opened() {
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
            .state()
            .economy
            .balance(&request.account, &request.asset);
        let mut found = holds(balance).then_some(0);
        for (index, receipt) in loaded.receipts().iter().enumerate() {
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
                        loaded.receipts().len() + 1,
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
        description = "List the problems this server studies: each problem's `key`, the `key`s of its instances and strategies, which axionomy_problem_study takes, and its defaults. Each problem declares its own instances, family and capabilities as a `key` with a `label` to show people.",
        output_schema = schema_for_output::<ProblemCatalogResponse>(),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn problem_catalog(
        &self,
        Parameters(_request): Parameters<ProblemCatalogRequest>,
    ) -> Result<Linked<ProblemCatalogResponse>, ToolError> {
        Ok(Linked::new(
            ProblemCatalogResponse {
                problems: self.registry.catalog(),
            },
            Vec::new(),
        ))
    }

    #[tool(
        name = "axionomy_problem_study",
        title = "Study a problem",
        description = "Study one problem from axionomy_problem_catalog and return a summary of its documents: the run of the requested `strategy`, which is the `selected_document`, beside the problem's other outcomes, usually the runs of its other strategies and cases such as a deadlock. Take the `problem`, `instance` and `strategy` keys from axionomy_problem_catalog; the instance is one of the problem's own, its default when omitted. Each document is two stored economies: initial_economy_id, where its trajectory starts, and final_economy_id, where its exchanges reach. Step into the final one with axionomy_trace_step and branch from any step; frame i's exchange applies to step i. The study's documents and frames are linked resources. Runs only as an MCP task: a client that does not declare the io.modelcontextprotocol/tasks extension gets `tasks_required`.",
        output_schema = schema_for_output::<StudySummary>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn problem_study_requires_tasks(
        &self,
        Parameters(_request): Parameters<StudyRequest>,
    ) -> Result<Linked<StudySummary>, ToolError> {
        Err(tasks_required(PROBLEM_STUDY))
    }

    #[tool(
        name = "axionomy_package_list",
        title = "List packages",
        description = "List the packages loaded when the server started: each one's name, version, store format and script host API, its refs, each with the economy_id of the point it names (such as an instance an experiment starts from), and the experiments and strategies it carries, by path. axionomy_experiment_run runs an experiment. A ref's economy is an economy like any other: read, assess, apply and branch from it.",
        output_schema = schema_for_output::<PackageListResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn package_list(
        &self,
        Parameters(_request): Parameters<PackageListRequest>,
    ) -> Result<Linked<PackageListResponse>, ToolError> {
        let engine = self.archive.engine();
        let manifests = engine.manifests().map_err(ToolError::internal)?;
        // The refs that name a point, and the points they name.
        let mut named = Vec::with_capacity(manifests.len());
        let mut points = Vec::new();
        for manifest in &manifests {
            let mut refs = Vec::with_capacity(manifest.refs.len());
            for reference in &manifest.refs {
                match engine.referred(reference) {
                    Ok(point) => {
                        refs.push(reference.clone());
                        points.push(point);
                    }
                    Err(EngineError::UnknownRef(_)) => {}
                    Err(error) => return Err(ToolError::internal(error)),
                }
            }
            named.push(refs);
        }
        let mut economies = self
            .archive
            .names(points)
            .await
            .map_err(ToolError::internal)?
            .into_iter();
        let mut links = Vec::new();
        let packages = manifests
            .into_iter()
            .zip(named)
            .map(|(manifest, refs)| {
                let refs: Vec<PackageRef> = refs
                    .into_iter()
                    .zip(economies.by_ref())
                    .map(|(reference, economy_id)| PackageRef {
                        reference,
                        economy_id,
                    })
                    .collect();
                links.extend(refs.iter().map(|named| {
                    Target::Economy {
                        economy_id: named.economy_id.clone(),
                    }
                    .link(
                        "economy",
                        format!("The economy ref `{}` names", named.reference),
                    )
                }));
                PackageSummary {
                    experiments: carried(&manifest.definitions),
                    strategies: carried(&manifest.definitions),
                    name: manifest.name,
                    version: PackageVersion(manifest.version),
                    format: manifest.format,
                    host_api: manifest.host_api,
                    refs,
                }
            })
            .collect();
        Ok(Linked::new(PackageListResponse { packages }, links))
    }

    #[tool(
        name = "axionomy_experiment_run",
        title = "Run an experiment",
        description = "Run an experiment of a loaded package, such as `maze/micro` (axionomy_package_list lists them): every arm's strategy from its start toward the experiment's goal, once per trial, trial i seeded from the experiment's seed and i so every arm meets the same chances, measured on the experiment's objectives; and record the results, which axionomy_experiment_results gives again. Returns the goal, each arm's strategy and start economy, and every trial's seed, how it ended, its objective values, exact, and the economy_id it reached: step through its trace with axionomy_trace_step, branch from any step, diff, assess and apply as with any economy. An experiment runs at most 1,000 trials over all its arms. Every call records runs of its own, so the economies it names get handles of their own; equal roots say they are equal.",
        output_schema = schema_for_output::<ExperimentResults>(),
        annotations(destructive_hint = false, idempotent_hint = false, open_world_hint = false)
    )]
    async fn experiment_run(
        &self,
        Parameters(request): Parameters<ExperimentRequest>,
    ) -> Result<Linked<ExperimentResults>, ToolError> {
        let experiment = self.experiment(&request.experiment)?;
        let trials = experiment
            .trials
            .saturating_mul(experiment.arms.len() as u64);
        within_trials(&request.experiment, trials)?;
        let results = self
            .archive
            .run_experiment(request.experiment.clone())
            .await
            .map_err(|error| {
                ToolError::internal(format!(
                    "experiment `{}` failed: {error}",
                    request.experiment
                ))
            })?;
        self.reported(request.experiment, &experiment, results)
            .await
    }

    #[tool(
        name = "axionomy_experiment_results",
        title = "Read an experiment's results",
        description = "Give the results an experiment of a loaded package last recorded, by axionomy_experiment_run or in its package, in the shape axionomy_experiment_run returns them: the goal, each arm's strategy and start economy, and every trial's seed, how it ended, its objective values and the economy_id it reached. An experiment that has recorded none is `not_found`; run it first.",
        output_schema = schema_for_output::<ExperimentResults>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn experiment_results(
        &self,
        Parameters(request): Parameters<ExperimentRequest>,
    ) -> Result<Linked<ExperimentResults>, ToolError> {
        let experiment = self.experiment(&request.experiment)?;
        let results = match self.archive.engine().results(&request.experiment) {
            Ok(results) => results,
            Err(EngineError::Undefined(_)) => {
                return Err(ToolError::new(
                    FailureCode::NotFound,
                    format!(
                        "experiment `{}` has recorded no results; run it with axionomy_experiment_run",
                        request.experiment
                    ),
                )
                .at("/experiment"));
            }
            Err(error) => return Err(ToolError::internal(error)),
        };
        let trials = results.arms.iter().map(|arm| arm.trials.len() as u64).sum();
        within_trials(&request.experiment, trials)?;
        self.reported(request.experiment, &experiment, results)
            .await
    }

    #[tool(
        name = "axionomy_ref_create",
        title = "Create a ref",
        description = "Make a new ref naming an economy: the economy an application, such as a game or a simulation, is at. Moving the ref forward with axionomy_ref_advance is what makes a step count, and axionomy_ref_frames delivers the steps it is moved over to whoever follows it. Every call makes a ref of its own, named by the `ref_id` it returns.",
        output_schema = schema_for_output::<RefPosition>(),
        annotations(destructive_hint = false, idempotent_hint = false, open_world_hint = false)
    )]
    async fn ref_create(
        &self,
        Parameters(request): Parameters<RefCreateRequest>,
    ) -> Result<Linked<RefPosition>, ToolError> {
        let economy = self.economy(&request.economy_id).await?;
        let (ref_id, economy_id) = self
            .archive
            .refer(&request.economy_id, &economy)
            .await
            .map_err(ToolError::internal)?;
        Ok(positioned(ref_id, economy_id))
    }

    #[tool(
        name = "axionomy_ref_read",
        title = "Read a ref",
        description = "Name the economy a ref is at. Apply exchanges to it and move the ref to the result with axionomy_ref_advance, or follow the steps the ref is moved over from it with axionomy_ref_frames.",
        output_schema = schema_for_output::<RefPosition>(),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn ref_read(
        &self,
        Parameters(request): Parameters<RefReadRequest>,
    ) -> Result<Linked<RefPosition>, ToolError> {
        let economy_id = self.referred(&request.ref_id)?;
        Ok(positioned(request.ref_id, economy_id))
    }

    #[tool(
        name = "axionomy_ref_advance",
        title = "Advance a ref",
        description = "Move a ref forward from `expected_economy_id`, where the mover saw it, to `economy_id`, an economy made by exchanges from it, such as axionomy_exchange_apply's or axionomy_trace_replay's result: the steps between are what followers receive. When another move came first, the call fails with `moved` and `details.current_economy_id`, where the ref is; look again from there. A ref never moves back, or to an economy that was not made from where it is, so asking again for a move already made succeeds.",
        output_schema = schema_for_output::<RefPosition>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn ref_advance(
        &self,
        Parameters(request): Parameters<RefAdvanceRequest>,
    ) -> Result<Linked<RefPosition>, ToolError> {
        let RefAdvanceRequest {
            ref_id,
            expected_economy_id,
            economy_id,
        } = request;
        let expected = self.economy(&expected_economy_id).await?;
        let to = self.economy(&economy_id).await?;
        let advanced = self
            .archive
            .advance(&ref_id, expected.record.point, &economy_id, to.record.point)
            .await;
        match advanced {
            Ok(named) => Ok(positioned(ref_id, named)),
            // A ref that names nothing is one this store does not hold.
            Err(EngineError::Moved { current: None, .. }) => Err(ToolError::new(
                FailureCode::NotFound,
                self.not_stored(&ref_id),
            )),
            Err(EngineError::Moved {
                current: Some(current),
                ..
            }) => {
                let current = self
                    .archive
                    .handle_of(current)
                    .map_err(ToolError::internal)?;
                Err(moved(&ref_id, &expected_economy_id, &current))
            }
            Err(EngineError::Backward { .. }) => Err(ToolError::argument(
                "/economy_id",
                format!(
                    "`{economy_id}` was not made by exchanges from `{expected_economy_id}`, so ref `{ref_id}` cannot move there: a ref moves only forward, to an economy made from where it is"
                ),
            )),
            Err(error) => Err(self.ref_failure(&ref_id, error)),
        }
    }

    #[tool(
        name = "axionomy_ref_delete",
        title = "Delete a ref",
        description = "Delete a ref from `expected_economy_id`, where the deleter saw it, and return where it was. When it moved since, the call fails with `moved` and `details.current_economy_id`, where the ref is; look again from there. Only the ref goes: every economy it named stays, and a follower asking for its frames is told it is not found.",
        output_schema = schema_for_output::<RefPosition>(),
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn ref_delete(
        &self,
        Parameters(request): Parameters<RefDeleteRequest>,
    ) -> Result<Linked<RefPosition>, ToolError> {
        let RefDeleteRequest {
            ref_id,
            expected_economy_id,
        } = request;
        let expected = self.economy(&expected_economy_id).await?;
        match self.archive.delete(&ref_id, expected.record.point).await {
            Ok(()) => {
                let named = self
                    .archive
                    .handle_of(expected.record.point)
                    .map_err(ToolError::internal)?;
                Ok(positioned(ref_id, named))
            }
            // A ref that names nothing is one this store does not hold.
            Err(EngineError::Moved { current: None, .. }) => Err(ToolError::new(
                FailureCode::NotFound,
                self.not_stored(&ref_id),
            )),
            Err(EngineError::Moved {
                current: Some(current),
                ..
            }) => {
                let current = self
                    .archive
                    .handle_of(current)
                    .map_err(ToolError::internal)?;
                Err(moved(&ref_id, &expected_economy_id, &current))
            }
            Err(error) => Err(self.ref_failure(&ref_id, error)),
        }
    }

    #[tool(
        name = "axionomy_ref_frames",
        title = "Follow a ref",
        description = "Deliver the steps a ref was moved over after `after_economy_id`, the last economy the follower handled, earliest first: each frame is a step's exchange, its receipt and the `economy_id` it made, which every other tool takes. Only the steps a ref is moved over are delivered, never an exchange that was only tried out. At most `limit` frames, up to 100, per call; while the ref's `economy_id` is past the last frame, call again from that frame. With `wait_seconds`, up to 30, the call waits for the ref to move when it is still at `after_economy_id`. With a `goal`, as axionomy_search takes it, each frame says whether its economy `holds` it.",
        output_schema = schema_for_output::<RefFramesResponse>(),
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn ref_frames(
        &self,
        Parameters(request): Parameters<RefFramesRequest>,
    ) -> Result<Linked<RefFramesResponse>, ToolError> {
        if !(1..=MAX_FRAMES).contains(&request.limit) {
            return Err(ToolError::argument(
                "/limit",
                format!(
                    "limit must be from 1 to {MAX_FRAMES}; follow from the last frame for more"
                ),
            ));
        }
        if request.wait_seconds > MAX_WAIT_SECONDS {
            return Err(ToolError::argument(
                "/wait_seconds",
                format!(
                    "wait_seconds must be at most {MAX_WAIT_SECONDS}; call again to wait longer"
                ),
            ));
        }
        let goal = request.goal.as_ref().map(goal_of).transpose()?;
        let after = self.economy(&request.after_economy_id).await?;
        let followed = self
            .archive
            .frames(
                &request.ref_id,
                &request.after_economy_id,
                after.record.point,
                goal,
                request.limit,
                Duration::from_secs(request.wait_seconds),
            )
            .await;
        let (frames, economy_id) = match followed {
            Ok(followed) => followed,
            Err(EngineError::NotOnHistory { .. }) => {
                let current = self.referred(&request.ref_id)?;
                return Err(ToolError::argument(
                    "/after_economy_id",
                    format!(
                        "`{}` is not an economy ref `{}` was at or moved over, so it has no steps after it; follow from where the ref is, `{current}`, or from a frame it delivered",
                        request.after_economy_id, request.ref_id
                    ),
                ));
            }
            Err(error) => return Err(self.ref_failure(&request.ref_id, error)),
        };
        let link = Target::Economy {
            economy_id: economy_id.clone(),
        }
        .link("economy", "The economy the ref is at");
        Ok(Linked::new(
            RefFramesResponse {
                ref_id: request.ref_id,
                economy_id,
                frames: frames
                    .into_iter()
                    .map(|(economy_id, frame)| RefFrame {
                        economy_id,
                        receipt: FrameReceipt {
                            changes: delta_lines(None, frame.receipt.deltas()).collect(),
                            opened: frame.receipt.opened().to_vec(),
                            closed: frame.receipt.closed().to_vec(),
                        },
                        exchange: frame.exchange,
                        holds: frame.holds,
                    })
                    .collect(),
            },
            vec![link],
        ))
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

    /// The experiment at `path`, which a loaded package carries.
    fn experiment(&self, path: &ExperimentPath) -> Result<Experiment, ToolError> {
        match self.archive.engine().experiment(path) {
            Ok(experiment) => Ok(experiment),
            Err(EngineError::Undefined(_)) => Err(ToolError::new(
                FailureCode::NotFound,
                format!(
                    "no loaded package carries an experiment `{path}`; axionomy_package_list lists the experiments of every package this server loaded"
                ),
            )
            .at("/experiment")),
            Err(error) => Err(ToolError::internal(error)),
        }
    }

    /// An experiment's `results`, every point they name written as the
    /// handle of its economy.
    async fn reported(
        &self,
        path: ExperimentPath,
        experiment: &Experiment,
        results: Results,
    ) -> Result<Linked<ExperimentResults>, ToolError> {
        let engine = self.archive.engine();
        let objectives = experiment
            .objectives
            .iter()
            .map(|objective| {
                engine
                    .objective(objective)
                    .map(|defined| ExperimentObjective {
                        objective: objective.clone(),
                        direction: defined.direction,
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ToolError::internal)?;
        // Each arm's start, then its trials' ends, in order.
        let points = results
            .arms
            .iter()
            .flat_map(|arm| {
                std::iter::once(arm.start).chain(arm.trials.iter().map(|trial| trial.end))
            })
            .collect();
        let mut names = self
            .archive
            .names(points)
            .await
            .map_err(ToolError::internal)?
            .into_iter();
        let mut next = || {
            names
                .next()
                .ok_or_else(|| ToolError::internal("a point the results name was not named"))
        };
        let mut links = Vec::new();
        let mut linked = BTreeSet::new();
        let mut arms = Vec::with_capacity(results.arms.len());
        for arm in results.arms {
            let start_economy_id = next()?;
            if linked.insert(start_economy_id.clone()) {
                links.push(
                    Target::Economy {
                        economy_id: start_economy_id.clone(),
                    }
                    .link("economy", "An economy the experiment's arms start from"),
                );
            }
            let mut trials = Vec::with_capacity(arm.trials.len());
            for trial in arm.trials {
                if trial.values.len() != experiment.objectives.len() {
                    return Err(ToolError::internal(format!(
                        "experiment `{path}` recorded {} objective values for its {} objectives",
                        trial.values.len(),
                        experiment.objectives.len()
                    )));
                }
                trials.push(TrialResult {
                    seed: trial.seed,
                    ended: trial.ended.into(),
                    values: experiment
                        .objectives
                        .iter()
                        .cloned()
                        .zip(trial.values)
                        .collect(),
                    economy_id: next()?,
                    exchanges: trial.end.step.saturating_sub(arm.start.step),
                });
            }
            arms.push(ArmResults {
                strategy: arm.arm.strategy,
                start: arm.arm.start.unwrap_or_else(|| experiment.start.clone()),
                start_economy_id,
                trials,
            });
        }
        Ok(Linked::new(
            ExperimentResults {
                experiment: path,
                goal: experiment.goal.clone(),
                objectives,
                arms,
            },
            links,
        ))
    }

    /// The economy a ref is at.
    fn referred(&self, ref_id: &RefId) -> Result<EconomyId, ToolError> {
        self.archive
            .referred(ref_id)
            .map_err(ToolError::internal)?
            .ok_or_else(|| ToolError::new(FailureCode::NotFound, self.not_stored(ref_id)))
    }

    /// A ref tool's failure: a ref the store lacks, or an internal one.
    fn ref_failure(&self, ref_id: &RefId, error: EngineError) -> ToolError {
        match error {
            EngineError::UnknownRef(_) => {
                ToolError::new(FailureCode::NotFound, self.not_stored(ref_id))
            }
            error => ToolError::internal(error),
        }
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
        let economy = authoring::extend(
            base.as_deref().map(|loaded| &loaded.state().economy),
            entries,
        )
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
            .author(parent_id.clone(), &operation, size, economy)
            .await
            .map_err(ToolError::internal)?;
        let root = self.economy(&stored.economy_id).await?.state().root();
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

    /// Applies `exchanges` in order to an economy and stores the economy they
    /// make, or explains the first that fails and stores nothing.
    async fn exchange(
        &self,
        source_economy_id: EconomyId,
        exchanges: Vec<paths::Exchange>,
    ) -> Result<Linked<ExchangedResponse>, ToolError> {
        let source = self.economy(&source_economy_id).await?;
        store_exchanged(&self.archive, source_economy_id, &source, exchanges).await
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
        let goal = goal_of(&request.goal)?;
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
        self.registry.resolve(&request).map_err(|error| {
            let refused = ToolError::new(
                FailureCode::InvalidRequest,
                format!(
                    "{error}; axionomy_problem_catalog lists every problem with its instances and strategies"
                ),
            );
            match error {
                StudyError::UnknownProblem(_) => refused.at("/problem"),
                StudyError::UnknownInstance { .. } => refused.at("/instance"),
                StudyError::UnknownStrategy { .. } => refused.at("/strategy"),
                _ => refused,
            }
        })?;
        let archive = self.archive.clone();
        let registry = Arc::clone(&self.registry);
        let task = self.tasks.spawn(self.task_options.clone(), move |context| {
            Box::pin(study_problem(context, archive, registry, request))
        });
        Ok(CreateTaskResult::new(task).into())
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

/// A ref and the economy it is at, linked.
fn positioned(ref_id: RefId, economy_id: EconomyId) -> Linked<RefPosition> {
    let link = Target::Economy {
        economy_id: economy_id.clone(),
    }
    .link("economy", "The economy the ref is at");
    Linked::new(RefPosition { ref_id, economy_id }, vec![link])
}

/// A move refused because the ref is no longer at `expected`: it says where
/// the ref is, so the mover looks again from there.
fn moved(ref_id: &RefId, expected: &EconomyId, current: &EconomyId) -> ToolError {
    ToolError::new(
        FailureCode::Moved,
        format!(
            "ref `{ref_id}` is no longer at `{expected}`: another move came first, and it is at `{current}`; look again from there"
        ),
    )
    .with_details(MovedDetails {
        current_economy_id: current,
    })
}

/// The details of a `moved` failure: where the ref is.
#[derive(Serialize)]
struct MovedDetails<'a> {
    current_economy_id: &'a EconomyId,
}

/// The goal a request names: balances each account must reach. Amounts that
/// are not a basket are refused at the argument at fault.
fn goal_of(goal: &BTreeMap<AccountPath, Amounts>) -> Result<paths::Goal, ToolError> {
    goal.iter()
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
            Ok(goal.require(account.clone(), basket))
        })
}

/// The definitions of one kind among a package's, by their paths: the
/// definition `experiment/maze/micro` is the experiment `maze/micro`.
fn carried<K: PathKind>(definitions: &[DefinitionPath]) -> Vec<Path<K>> {
    let kind = K::SCHEMA
        .strip_suffix("Path")
        .unwrap_or(K::SCHEMA)
        .to_ascii_lowercase();
    definitions
        .iter()
        .filter_map(|definition| {
            let (category, path) = definition.as_str().split_once('/')?;
            if category == kind {
                path.parse().ok()
            } else {
                None
            }
        })
        .collect()
}

/// Refuses an experiment of more than [`MAX_TRIALS`] trials over all its
/// arms, which one call neither runs nor reports.
fn within_trials(experiment: &ExperimentPath, trials: u64) -> Result<(), ToolError> {
    if trials <= MAX_TRIALS {
        return Ok(());
    }
    Err(ToolError::new(
        FailureCode::TooLarge,
        format!(
            "experiment `{experiment}` runs {trials} trials over all its arms; one call runs and reports at most {MAX_TRIALS}"
        ),
    )
    .at("/experiment"))
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
        "axionomy_package_list" => check::<PackageListRequest>(arguments),
        "axionomy_experiment_run" | "axionomy_experiment_results" => {
            check::<ExperimentRequest>(arguments)
        }
        "axionomy_ref_create" => check::<RefCreateRequest>(arguments),
        "axionomy_ref_read" => check::<RefReadRequest>(arguments),
        "axionomy_ref_advance" => check::<RefAdvanceRequest>(arguments),
        "axionomy_ref_delete" => check::<RefDeleteRequest>(arguments),
        "axionomy_ref_frames" => check::<RefFramesRequest>(arguments),
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

/// Stores the economy a call's exchanges reach from `source` and describes
/// it, or explains the first that fails, assessed against the economy it met,
/// and stores nothing.
async fn store_exchanged(
    archive: &Archive,
    source_economy_id: EconomyId,
    source: &Loaded,
    exchanges: Vec<paths::Exchange>,
) -> Result<Linked<ExchangedResponse>, ToolError> {
    let count = exchanges.len();
    let stored = match archive
        .exchanged(&source_economy_id, source, exchanges.clone())
        .await
    {
        Ok(stored) => stored,
        Err(EngineError::Refused { index, assessment }) => {
            let (assessment_id, stored) = archive
                .keep_assessment(
                    &source_economy_id,
                    &exchanges[..index],
                    &exchanges[index],
                    *assessment,
                )
                .await
                .map_err(ToolError::internal)?;
            return Err(failed_exchange(index, count, &assessment_id, &stored));
        }
        Err(error) => return Err(ToolError::internal(error)),
    };
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
    let receipts = loaded.receipts();
    let links = exchanged_links(&economy_id);
    Linked::new(
        ExchangedResponse {
            source_economy_id,
            economy_id,
            deduplicated,
            root: StateRoot(loaded.state().root()),
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
    registry: Arc<Registry>,
    request: StudyRequest,
) -> Result<CallToolResult, TaskExit> {
    let control = Arc::new(StudyControl::default());
    let worker_control = Arc::clone(&control);
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<Progress>();
    let mut worker = tokio::task::spawn_blocking(move || {
        studies::compute(&registry, &request, &worker_control, Some(progress_tx))
    });

    let mut reporting = true;
    let (study, trajectories) = loop {
        tokio::select! {
            progress = progress_rx.recv(), if reporting => match progress {
                Some(progress) => context.set_status_message(format!("{}: {} ({}/{})", progress.phase.label, progress.message, progress.completed, progress.total)),
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
    let start = source.record.point;
    let engine = archive.engine().clone();
    let watching = context.clone();
    // The engine's breadth-first strategy over the goal and the candidates,
    // kept as definitions named by their content.
    let ran = tokio::task::spawn_blocking(move || {
        let goal_path = content_name(&goal);
        let candidates_path = content_name(&candidates);
        engine.keep_goal(&goal_path, &goal)?;
        engine.keep_candidates(&candidates_path, &CandidateSet::Listed(candidates))?;
        let strategy = Strategy::BreadthFirst {
            candidates: candidates_path,
        };
        // A search spends its expansions and plays no turns.
        let effort = Effort {
            seed: 0,
            budget: max_expansions as u64,
            turns: 0,
        };
        engine.run_strategy(start, &strategy, &goal_path, effort, &mut |progress| {
            if watching.is_cancel_requested() {
                return ControlFlow::Break(());
            }
            let expanded = progress.expanded();
            if expanded > 0 && expanded.is_multiple_of(chunk_size) {
                watching.set_status_message(progress_message(
                    *progress,
                    max_expansions,
                    max_expansions.saturating_sub(expanded),
                ));
            }
            ControlFlow::Continue(())
        })
    })
    .await
    .map_err(task_error)?
    .map_err(task_error)?;
    let progress = ran.thinking.expect("a search reports what it explored");
    context.set_status_message(format!("finished: {}", describe(progress)));
    let outcome = match ran.outcome {
        Outcome::Reached => SearchOutcome::Solved,
        Outcome::Exhausted => SearchOutcome::Exhausted,
        Outcome::Spent => SearchOutcome::ExpansionLimit,
        Outcome::Interrupted => return Err(TaskExit::Cancelled),
        other => {
            return Err(task_error(format!(
                "a search ended as a policy does: {other:?}"
            )));
        }
    };
    let mut links = Vec::new();
    let solution = match outcome {
        SearchOutcome::Solved => {
            let stored = archive
                .reached(&economy_id, ran.end)
                .await
                .map_err(task_error)?;
            let loaded = archive
                .economy(&stored.economy_id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error("a stored economy cannot be read back"))?;
            links = exchanged_links(&stored.economy_id);
            Some(SearchSolved {
                economy_id: stored.economy_id,
                exchanges: loaded.receipts().len(),
                cost: ran.cost.unwrap_or_default(),
            })
        }
        SearchOutcome::Exhausted | SearchOutcome::ExpansionLimit => None,
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
