//! Reads `axionomy://` resources and completes their template arguments.

use crate::{
    handles::{ArtifactId, AssessmentId, EconomyId, InvariantPath},
    messages::{Direction, DocumentSummary, EconomyCounts, StateRoot},
    resources::{Address, PAGE_FRAMES, PAGE_ROWS, Page, Target, page, span_page},
    server::{AxionomyMcp, assessment_lines, counts, receipt_lines},
    store::{Loaded, Origin},
};
use axionomy::{
    AccountPath, AssessmentStatus, AssetPath, Quantity, RatePath, RolePath,
    paths::{self, Paths},
};
use axionomy_service::{ArtifactKey, RunArtifact};
use axionomy_store::state;
use axionomy_view::DocumentPath;
use rmcp::{
    ErrorData as McpError,
    model::{CompleteRequestParams, CompleteResult, CompletionInfo},
};
use serde::Serialize;
use std::{ops::Range, sync::Arc};

impl AxionomyMcp {
    /// The JSON content of one resource page.
    pub(crate) async fn read(&self, address: &Address) -> Result<String, McpError> {
        let not_found = || {
            McpError::resource_not_found(
                "Resource not found",
                Some(serde_json::json!({ "uri": address.target.uri() })),
            )
        };
        match &address.target {
            Target::Assessment { assessment_id } => {
                let stored = self
                    .assessment(assessment_id)
                    .await?
                    .ok_or_else(not_found)?;
                let assessment = &stored.assessment;
                json(&AssessmentView {
                    assessment_id,
                    economy_id: &stored.economy_id,
                    after_exchanges: stored.after_exchanges,
                    exchange: &stored.exchange,
                    status: assessment.status(),
                    issues: assessment.issues(),
                    lines: assessment_lines(assessment).count(),
                })
            }
            Target::AssessmentLines { assessment_id } => {
                let stored = self
                    .assessment(assessment_id)
                    .await?
                    .ok_or_else(not_found)?;
                json(&page(
                    address,
                    assessment_lines(&stored.assessment).enumerate(),
                    PAGE_ROWS,
                )?)
            }
            Target::Artifact { artifact_id } => {
                let artifact = self.artifact(artifact_id).await?.ok_or_else(not_found)?;
                json(&ArtifactView {
                    artifact_id,
                    key: &artifact.key,
                    selected_document: &artifact.selected_document,
                    documents: artifact
                        .documents
                        .iter()
                        .map(|document| DocumentSummary {
                            id: document.id.clone(),
                            title: document.title.clone(),
                            frames: document.frames.len(),
                        })
                        .collect(),
                })
            }
            Target::Document {
                artifact_id,
                document,
            } => {
                let artifact = self.artifact(artifact_id).await?.ok_or_else(not_found)?;
                let document = artifact.document(document).ok_or_else(not_found)?;
                let frames = document.frames.len();
                let mut document = document.clone();
                document.frames = Vec::new();
                json(&DocumentView { frames, document })
            }
            Target::Frames {
                artifact_id,
                document,
            } => {
                let artifact = self.artifact(artifact_id).await?.ok_or_else(not_found)?;
                let document = artifact.document(document).ok_or_else(not_found)?;
                json(&page(
                    address,
                    document.frames.iter().enumerate(),
                    PAGE_FRAMES,
                )?)
            }
            Target::Economy { economy_id }
            | Target::Accounts { economy_id, .. }
            | Target::Balances { economy_id, .. }
            | Target::Rates { economy_id, .. }
            | Target::Rate { economy_id, .. }
            | Target::RateAmounts { economy_id, .. }
            | Target::RateConditions { economy_id, .. }
            | Target::Invariants { economy_id }
            | Target::InvariantWeights { economy_id, .. }
            | Target::Changes { economy_id }
            | Target::Exchanges { economy_id } => {
                let loaded = self.stored(economy_id).await?.ok_or_else(not_found)?;
                read_snapshot(address, economy_id, &loaded).ok_or_else(not_found)?
            }
            Target::Diff {
                economy_id,
                other_id,
            } => {
                let before = self.stored(economy_id).await?.ok_or_else(not_found)?;
                let after = self.stored(other_id).await?.ok_or_else(not_found)?;
                let changes = state::changes::<Paths>(&before.state.tree, &after.state.tree)
                    .map_err(internal)?
                    .into_iter()
                    .map(DiffRow::from)
                    .enumerate();
                json(&page(address, changes, PAGE_ROWS)?)
            }
        }
    }

    /// Suggests values for a template argument from what the named snapshot
    /// or run holds, by prefix.
    pub(crate) async fn completion(
        &self,
        request: CompleteRequestParams,
    ) -> Result<CompleteResult, McpError> {
        let Some(template) = request.r#ref.as_resource_uri() else {
            return Ok(CompleteResult::default());
        };
        // `prefix` filters account paths on the accounts listing and asset
        // paths on an account's balances.
        let accounts_template = template.contains("/accounts{");
        let arguments = request
            .context
            .and_then(|context| context.arguments)
            .unwrap_or_default();
        let argument = request.argument.name.as_str();
        let needed = |name: &str| {
            arguments.get(name).ok_or_else(|| {
                McpError::invalid_params(format!("completing `{argument}` needs `{name}`"), None)
            })
        };
        let not_found = || McpError::resource_not_found("Resource not found", None);
        let prefix = request.argument.value.as_str();
        let candidates: Vec<String> =
            match argument {
                "account" | "rate" | "invariant" | "asset" | "prefix" => {
                    let loaded =
                        self.stored(&needed("economy_id")?.parse::<EconomyId>().map_err(
                            |error| {
                                McpError::invalid_params(format!("`economy_id`: {error}"), None)
                            },
                        )?)
                        .await?
                        .ok_or_else(not_found)?;
                    let (economy, tree) = (&loaded.state.economy, &loaded.state.tree);
                    match argument {
                        "account" | "prefix" if argument == "account" || accounts_template => {
                            // Accounts are read by prefix from the state's tree,
                            // however many there are.
                            let span = state::accounts(tree, prefix);
                            let total = span.end - span.start;
                            let shown = span.start..span.end.min(span.start + MAX_COMPLETIONS);
                            let values = state::account_rows(tree, shown)
                                .map(|row| row.map(|(_, account, _)| account.to_string()))
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(internal)?;
                            return completed(values, total);
                        }
                        "rate" => economy.rate_ids().map(ToString::to_string).collect(),
                        "invariant" => economy
                            .invariants()
                            .map(|invariant| invariant.name().to_owned())
                            .collect(),
                        "asset" => economy
                            .asset_keys()
                            .into_iter()
                            .map(ToString::to_string)
                            .collect(),
                        _ => {
                            let account: AccountPath =
                                needed("account")?.parse().map_err(|error| {
                                    McpError::invalid_params(
                                        format!("`account` is not a path: {error}"),
                                        None,
                                    )
                                })?;
                            economy.account(&account).ok_or_else(not_found)?;
                            let span = state::balances(tree, &account, prefix);
                            let total = span.end - span.start;
                            let shown = span.start..span.end.min(span.start + MAX_COMPLETIONS);
                            let values = state::balance_rows(tree, &account, shown)
                                .map(|row| row.map(|(_, asset, _)| asset.to_string()))
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(internal)?;
                            return completed(values, total);
                        }
                    }
                }
                "document" => {
                    self.artifact(&needed("artifact_id")?.parse::<ArtifactId>().map_err(
                        |error| McpError::invalid_params(format!("`artifact_id`: {error}"), None),
                    )?)
                    .await?
                    .ok_or_else(not_found)?
                    .documents
                    .iter()
                    .map(|document| document.id.to_string())
                    .collect()
                }
                _ => Vec::new(),
            };
        let matches: Vec<String> = candidates
            .into_iter()
            .filter(|candidate| candidate.starts_with(prefix))
            .collect();
        let total = matches.len() as u64;
        completed(
            matches.into_iter().take(MAX_COMPLETIONS as usize).collect(),
            total,
        )
    }

    async fn stored(&self, economy_id: &EconomyId) -> Result<Option<Arc<Loaded>>, McpError> {
        self.archive.economy(economy_id).await.map_err(internal)
    }

    async fn assessment(
        &self,
        assessment_id: &AssessmentId,
    ) -> Result<Option<Arc<crate::store::Assessed>>, McpError> {
        self.archive
            .assessment(assessment_id)
            .await
            .map_err(internal)
    }

    pub(crate) async fn artifact(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<Option<Arc<RunArtifact>>, McpError> {
        self.archive.artifact(artifact_id).await.map_err(internal)
    }
}

const MAX_COMPLETIONS: u64 = CompletionInfo::MAX_VALUES as u64;

fn completed(values: Vec<String>, total: u64) -> Result<CompleteResult, McpError> {
    let has_more = total > values.len() as u64;
    let completion = CompletionInfo::with_pagination(values, u32::try_from(total).ok(), has_more)
        .map_err(|message| McpError::internal_error(message, None))?;
    Ok(CompleteResult::new(completion))
}

fn internal(error: impl ToString) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

/// A page of rows read from positions of a state's tree.
fn tree_page<T: Serialize>(
    address: &Address,
    span: Range<u64>,
    rows: impl FnOnce(Range<u64>) -> Result<Vec<T>, state::StateError>,
) -> Result<String, McpError> {
    let span = usize::try_from(span.start).map_err(internal)?
        ..usize::try_from(span.end).map_err(internal)?;
    let (positions, next_cursor) = span_page(address, span, PAGE_ROWS)?;
    let items = rows(positions.start as u64..positions.end as u64).map_err(internal)?;
    json(&Page { items, next_cursor })
}

/// One snapshot resource, or `None` when it names an account, rate or
/// invariant the snapshot does not have.
fn read_snapshot(
    address: &Address,
    economy_id: &EconomyId,
    loaded: &Loaded,
) -> Option<Result<String, McpError>> {
    let (economy, tree) = (&loaded.state.economy, &loaded.state.tree);
    let receipts = &loaded.receipts[..];
    let content = match &address.target {
        Target::Economy { .. } => json(&EconomyView {
            economy_id,
            parent_id: loaded.record.parent.as_ref(),
            origin: match loaded.record.origin {
                Origin::Authored { entries } => OriginView::Authored { entries },
                Origin::Exchanged { exchanges } => OriginView::Exchanged { exchanges },
            },
            root: StateRoot(loaded.state.root()),
            counts: counts(economy),
        }),
        Target::Accounts { prefix, .. } => tree_page(
            address,
            state::accounts(tree, prefix.as_deref().unwrap_or_default()),
            |positions| {
                state::account_rows(tree, positions)
                    .map(|row| row.map(|(_, account, assets)| AccountRow { account, assets }))
                    .collect()
            },
        ),
        Target::Balances {
            account, prefix, ..
        } => {
            economy.account(account)?;
            tree_page(
                address,
                state::balances(tree, account, prefix.as_deref().unwrap_or_default()),
                |positions| {
                    state::balance_rows(tree, account, positions)
                        .map(|row| row.map(|(_, asset, quantity)| BalanceRow { asset, quantity }))
                        .collect()
                },
            )
        }
        Target::Rates { asset, .. } => page(
            address,
            economy
                .rate_ids()
                .enumerate()
                .filter_map(|(position, id)| Some((position, id, economy.rate(id)?)))
                .filter(|(_, _, rate)| asset.as_ref().is_none_or(|asset| moves(rate, asset)))
                .map(|(position, id, rate)| {
                    (
                        position,
                        RateRow {
                            rate: id,
                            roles: rate.roles().collect(),
                        },
                    )
                }),
            PAGE_ROWS,
        )
        .and_then(|page| json(&page)),
        Target::Rate { rate: id, .. } => {
            let rate = economy.rate(id)?;
            json(&RateView {
                economy_id,
                rate: id,
                roles: rate.roles().collect(),
                distinct: rate
                    .distinct_roles()
                    .map(|(left, right)| (left, right))
                    .collect(),
                parameters: rate.parameter_names().into_iter().collect(),
                opens: rate.opened_roles().collect(),
                closes: rate.closed_roles().collect(),
                amounts: amounts(rate).count(),
                conditions: rate.conditions().len(),
            })
        }
        Target::RateAmounts { rate, .. } => {
            let rate = economy.rate(rate)?;
            page(address, amounts(rate).enumerate(), PAGE_ROWS).and_then(|page| json(&page))
        }
        Target::RateConditions { rate, .. } => {
            let rate = economy.rate(rate)?;
            page(address, rate.conditions().iter().enumerate(), PAGE_ROWS)
                .and_then(|page| json(&page))
        }
        Target::Invariants { .. } => page(
            address,
            economy
                .invariants()
                .enumerate()
                .map(|(position, invariant)| {
                    let row = invariant
                        .name()
                        .parse()
                        .map(|path| InvariantRow {
                            invariant: path,
                            weights: invariant.weights().count(),
                        })
                        .map_err(|error| McpError::internal_error(error.to_string(), None));
                    (position, row)
                }),
            PAGE_ROWS,
        )
        .and_then(|page| {
            let items = page.items.into_iter().collect::<Result<Vec<_>, _>>()?;
            json(&Page {
                items,
                next_cursor: page.next_cursor,
            })
        }),
        Target::InvariantWeights { invariant, .. } => {
            let invariant = economy
                .invariants()
                .find(|candidate| candidate.name() == invariant.as_str())?;
            page(
                address,
                invariant
                    .weights()
                    .enumerate()
                    .map(|(position, (asset, weight))| (position, WeightRow { asset, weight })),
                PAGE_ROWS,
            )
            .and_then(|page| json(&page))
        }
        Target::Changes { .. } => page(address, receipt_lines(receipts).enumerate(), PAGE_ROWS)
            .and_then(|page| json(&page)),
        Target::Exchanges { .. } => page(
            address,
            receipts
                .iter()
                .map(|receipt| receipt.exchange())
                .enumerate(),
            PAGE_ROWS,
        )
        .and_then(|page| json(&page)),
        Target::Diff { .. }
        | Target::Assessment { .. }
        | Target::AssessmentLines { .. }
        | Target::Artifact { .. }
        | Target::Document { .. }
        | Target::Frames { .. } => return None,
    };
    Some(content)
}

/// Whether one unit of `rate` moves or requires `asset`.
fn moves(rate: &paths::Rate, asset: &AssetPath) -> bool {
    amounts(rate).any(|amount| amount.asset == asset)
}

/// What one unit of a rate consumes, produces and preserves: fixed amounts by
/// direction and role, then computed ones.
fn amounts(rate: &paths::Rate) -> impl Iterator<Item = AmountRow<'_>> {
    let fixed = [Direction::Consume, Direction::Produce, Direction::Preserve]
        .into_iter()
        .flat_map(move |direction| {
            rate.roles().flat_map(move |role| {
                let basket = match direction {
                    Direction::Consume => rate.consumed(role),
                    Direction::Produce => rate.produced(role),
                    Direction::Preserve => rate.preserved(role),
                };
                basket
                    .into_iter()
                    .flat_map(|basket| basket.iter())
                    .map(move |(asset, quantity)| AmountRow {
                        direction,
                        role,
                        asset,
                        quantity: Some(*quantity),
                        computed: None,
                    })
            })
        });
    let computed = [
        (Direction::Consume, rate.computed_consumed()),
        (Direction::Produce, rate.computed_produced()),
        (Direction::Preserve, rate.computed_preserved()),
    ]
    .into_iter()
    .flat_map(|(direction, amounts)| {
        amounts.iter().map(move |amount| AmountRow {
            direction,
            role: amount.role(),
            asset: amount.asset(),
            quantity: None,
            computed: Some(amount.quantity()),
        })
    });
    fixed.chain(computed)
}

fn json(value: &impl Serialize) -> Result<String, McpError> {
    serde_json::to_string(value).map_err(|error| McpError::internal_error(error.to_string(), None))
}

#[derive(Serialize)]
struct EconomyView<'a> {
    economy_id: &'a EconomyId,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<&'a EconomyId>,
    origin: OriginView,
    /// Equal roots mean equal states, however they were made.
    root: StateRoot,
    counts: EconomyCounts,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OriginView {
    Authored { entries: usize },
    Exchanged { exchanges: usize },
}

#[derive(Serialize)]
struct AccountRow {
    account: AccountPath,
    assets: u64,
}

#[derive(Serialize)]
struct BalanceRow {
    asset: AssetPath,
    quantity: Quantity,
}

/// One difference between two snapshots.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DiffRow {
    Opened {
        account: AccountPath,
    },
    Closed {
        account: AccountPath,
    },
    Balance {
        account: AccountPath,
        asset: AssetPath,
        before: Quantity,
        after: Quantity,
    },
    /// A rate added, removed or redefined.
    Rate {
        rate: RatePath,
    },
    Invariants,
}

impl From<state::ChangeOf<Paths>> for DiffRow {
    fn from(change: state::ChangeOf<Paths>) -> Self {
        match change {
            state::Change::Opened(account) => Self::Opened { account },
            state::Change::Closed(account) => Self::Closed { account },
            state::Change::Balance {
                account,
                asset,
                before,
                after,
            } => Self::Balance {
                account,
                asset,
                before,
                after,
            },
            state::Change::Rate(rate) => Self::Rate { rate },
            state::Change::Invariants => Self::Invariants,
        }
    }
}

#[derive(Serialize)]
struct RateRow<'a> {
    rate: &'a RatePath,
    roles: Vec<&'a RolePath>,
}

#[derive(Serialize)]
struct RateView<'a> {
    economy_id: &'a EconomyId,
    rate: &'a RatePath,
    roles: Vec<&'a RolePath>,
    distinct: Vec<(&'a RolePath, &'a RolePath)>,
    parameters: Vec<&'a str>,
    /// Roles whose bound account an exchange creates.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    opens: Vec<&'a RolePath>,
    /// Roles whose bound account an exchange removes once its effects leave it empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    closes: Vec<&'a RolePath>,
    amounts: usize,
    conditions: usize,
}

#[derive(Serialize)]
struct AmountRow<'a> {
    direction: Direction,
    role: &'a RolePath,
    asset: &'a AssetPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantity: Option<Quantity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    computed: Option<&'a paths::QuantityExpression>,
}

#[derive(Serialize)]
struct InvariantRow {
    invariant: InvariantPath,
    weights: usize,
}

#[derive(Serialize)]
struct WeightRow<'a> {
    asset: &'a AssetPath,
    weight: i64,
}

#[derive(Serialize)]
struct AssessmentView<'a> {
    assessment_id: &'a AssessmentId,
    economy_id: &'a EconomyId,
    /// Exchanges of the failing replay applied before this one; zero for a
    /// snapshot's own state.
    after_exchanges: usize,
    exchange: &'a paths::Exchange,
    status: AssessmentStatus,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    issues: &'a [paths::ApplyError],
    lines: usize,
}

#[derive(Serialize)]
struct ArtifactView<'a> {
    artifact_id: &'a ArtifactId,
    key: &'a ArtifactKey,
    selected_document: &'a DocumentPath,
    documents: Vec<DocumentSummary>,
}

#[derive(Serialize)]
struct DocumentView<T> {
    /// Frames are paged through the document's `frames` resource.
    frames: usize,
    document: T,
}
