//! Reads `axionomy://` resources and completes their template arguments.

use crate::{
    handles::{AssessmentId, EconomyId, InvariantPath},
    messages::{Direction, DocumentSummary, EconomyCounts},
    resources::{Address, PAGE_FRAMES, PAGE_ROWS, Page, Target, page},
    server::{AxionomyMcp, StoredAssessment, assessment_lines, counts, receipt_lines},
    store::{Origin, Snapshot, SnapshotStore},
};
use axionomy::{AccountPath, AssessmentStatus, AssetPath, Quantity, RatePath, RolePath, paths};
use axionomy_service::{RunArtifact, RunRequest};
use rmcp::{
    ErrorData as McpError,
    model::{CompleteRequestParams, CompleteResult, CompletionInfo},
};
use serde::Serialize;
use std::sync::Arc;

impl<S: SnapshotStore> AxionomyMcp<S> {
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
                let stored = self.assessment(assessment_id).ok_or_else(not_found)?;
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
                let stored = self.assessment(assessment_id).ok_or_else(not_found)?;
                json(&page(
                    address,
                    assessment_lines(&stored.assessment).enumerate(),
                    PAGE_ROWS,
                )?)
            }
            Target::Artifact { artifact_id } => {
                let artifact = self.artifact(artifact_id).ok_or_else(not_found)?;
                json(&ArtifactView {
                    artifact_id,
                    problem: &artifact.problem.key,
                    instance: &artifact.instance.key,
                    request: &artifact.request,
                    selected_document_id: &artifact.selected_document_id,
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
                document_id,
            } => {
                let artifact = self.artifact(artifact_id).ok_or_else(not_found)?;
                let document = artifact
                    .documents
                    .iter()
                    .find(|document| &document.id == document_id)
                    .ok_or_else(not_found)?;
                let frames = document.frames.len();
                let mut document = document.clone();
                document.frames = Vec::new();
                json(&DocumentView { frames, document })
            }
            Target::Frames {
                artifact_id,
                document_id,
            } => {
                let artifact = self.artifact(artifact_id).ok_or_else(not_found)?;
                let document = artifact
                    .documents
                    .iter()
                    .find(|document| &document.id == document_id)
                    .ok_or_else(not_found)?;
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
                let snapshot = self.stored(economy_id).await?.ok_or_else(not_found)?;
                read_snapshot(address, economy_id, &snapshot).ok_or_else(not_found)?
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
        let candidates: Vec<String> = match argument {
            "account" | "rate" | "invariant" | "asset" | "prefix" => {
                let snapshot = self
                    .stored(
                        &needed("economy_id")?
                            .parse::<EconomyId>()
                            .map_err(|error| {
                                McpError::invalid_params(format!("`economy_id`: {error}"), None)
                            })?,
                    )
                    .await?
                    .ok_or_else(not_found)?;
                let economy = &snapshot.economy;
                match argument {
                    "account" => economy.accounts().map(|(id, _)| id.to_string()).collect(),
                    "prefix" if accounts_template => {
                        economy.accounts().map(|(id, _)| id.to_string()).collect()
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
                        let account: AccountPath = needed("account")?.parse().map_err(|error| {
                            McpError::invalid_params(
                                format!("`account` is not a path: {error}"),
                                None,
                            )
                        })?;
                        economy
                            .account(&account)
                            .ok_or_else(not_found)?
                            .balances()
                            .iter()
                            .map(|(asset, _)| asset.to_string())
                            .collect()
                    }
                }
            }
            "document_id" => self
                .artifact(needed("artifact_id")?)
                .ok_or_else(not_found)?
                .documents
                .iter()
                .map(|document| document.id.clone())
                .collect(),
            _ => Vec::new(),
        };
        let prefix = request.argument.value.as_str();
        let matches: Vec<String> = candidates
            .into_iter()
            .filter(|candidate| candidate.starts_with(prefix))
            .collect();
        let total = matches.len();
        let values: Vec<String> = matches
            .into_iter()
            .take(CompletionInfo::MAX_VALUES)
            .collect();
        let has_more = total > values.len();
        let completion =
            CompletionInfo::with_pagination(values, u32::try_from(total).ok(), has_more)
                .map_err(|message| McpError::internal_error(message, None))?;
        Ok(CompleteResult::new(completion))
    }

    async fn stored(&self, economy_id: &EconomyId) -> Result<Option<Arc<Snapshot>>, McpError> {
        match self.snapshot(economy_id).await {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(error) if error.is_not_found() => Ok(None),
            Err(error) => Err(McpError::internal_error(error.message().to_owned(), None)),
        }
    }

    fn assessment(&self, assessment_id: &AssessmentId) -> Option<Arc<StoredAssessment>> {
        self.assessments.read().ok()?.get(assessment_id).cloned()
    }

    pub(crate) fn artifact(&self, artifact_id: &str) -> Option<Arc<RunArtifact>> {
        self.artifacts.read().ok()?.get(artifact_id).cloned()
    }
}

/// One snapshot resource, or `None` when it names an account, rate or
/// invariant the snapshot does not have.
fn read_snapshot(
    address: &Address,
    economy_id: &EconomyId,
    snapshot: &Snapshot,
) -> Option<Result<String, McpError>> {
    let economy = &snapshot.economy;
    let receipts = match &snapshot.origin {
        Origin::Exchanged { receipts } => &receipts[..],
        Origin::Authored { .. } => &[],
    };
    let content = match &address.target {
        Target::Economy { .. } => json(&EconomyView {
            economy_id,
            parent_id: snapshot.parent_id.as_ref(),
            origin: match &snapshot.origin {
                Origin::Authored { entries } => OriginView::Authored { entries: *entries },
                Origin::Exchanged { receipts } => OriginView::Exchanged {
                    exchanges: receipts.len(),
                },
            },
            counts: counts(economy),
        }),
        Target::Accounts { prefix, .. } => page(
            address,
            economy
                .accounts()
                .enumerate()
                .filter(|(_, (account, _))| {
                    prefix
                        .as_ref()
                        .is_none_or(|prefix| account.as_str().starts_with(prefix.as_str()))
                })
                .map(|(position, (account, holder))| {
                    (
                        position,
                        AccountRow {
                            account,
                            assets: holder.balances().len(),
                        },
                    )
                }),
            PAGE_ROWS,
        )
        .and_then(|page| json(&page)),
        Target::Balances {
            account, prefix, ..
        } => {
            let balances = economy.account(account)?.balances();
            page(
                address,
                balances
                    .iter()
                    .enumerate()
                    .filter(|(_, (asset, _))| {
                        prefix
                            .as_ref()
                            .is_none_or(|prefix| asset.as_str().starts_with(prefix.as_str()))
                    })
                    .map(|(position, (asset, quantity))| {
                        (
                            position,
                            BalanceRow {
                                asset,
                                quantity: *quantity,
                            },
                        )
                    }),
                PAGE_ROWS,
            )
            .and_then(|page| json(&page))
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
        Target::Assessment { .. }
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
    counts: EconomyCounts,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OriginView {
    Authored { entries: usize },
    Exchanged { exchanges: usize },
}

#[derive(Serialize)]
struct AccountRow<'a> {
    account: &'a AccountPath,
    assets: usize,
}

#[derive(Serialize)]
struct BalanceRow<'a> {
    asset: &'a AssetPath,
    quantity: Quantity,
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
    artifact_id: &'a str,
    problem: &'a str,
    instance: &'a str,
    request: &'a RunRequest,
    selected_document_id: &'a str,
    documents: Vec<DocumentSummary>,
}

#[derive(Serialize)]
struct DocumentView<T> {
    /// Frames are paged through the document's `frames` resource.
    frames: usize,
    document: T,
}
