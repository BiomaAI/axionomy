//! Reads `axionomy://` resources and completes their template arguments.

use axionomy::{InvariantPath, ParameterPath};

use crate::{
    handles::{AssessmentId, EconomyId, StudyId},
    messages::{Direction, DocumentSummary, EconomyCounts, StateRoot, StudySummary},
    resources::{Address, PAGE_FRAMES, PAGE_ROWS, Page, Target, page, span_page},
    server::{AxionomyMcp, assessment_lines, counts, receipt_lines},
    store::{Loaded, Origin, StudyRecord},
    studies,
};
use axionomy::{
    AccountPath, AssessmentStatus, AssetPath, Quantity, RatePath, RolePath,
    paths::{self, Paths},
};
use axionomy_service::{Study, StudyControl};
use axionomy_store::state;
use axionomy_view::{DocumentPath, ExchangeFrame};
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
            Target::Study { study_id } => {
                let record = self.study_record(study_id)?.ok_or_else(not_found)?;
                json(&StudySummary {
                    study_id: study_id.clone(),
                    key: record.key,
                    selected_document: record.selected_document,
                    documents: record.documents,
                })
            }
            Target::Document { study_id, document } => {
                let study = self.study(study_id).await?.ok_or_else(not_found)?;
                let view = study.document(document).ok_or_else(not_found)?;
                let economies = self.document_economies(study_id, document)?;
                let mut view = view.clone();
                let frames = std::mem::take(&mut view.frames).len();
                json(&DocumentView {
                    initial_economy_id: &economies.initial_economy_id,
                    final_economy_id: &economies.final_economy_id,
                    frames,
                    document: view,
                })
            }
            Target::Frames { study_id, document } => {
                let study = self.study(study_id).await?.ok_or_else(not_found)?;
                let view = study.document(document).ok_or_else(not_found)?;
                let economies = self.document_economies(study_id, document)?;
                // The final economy's receipts are the document's trace, one
                // exchange per frame, written as its economies write them.
                let last = self
                    .stored(&economies.final_economy_id)
                    .await?
                    .ok_or_else(not_found)?;
                if last.receipts.len() != view.frames.len() {
                    return Err(internal(format!(
                        "document `{document}` shows {} frames, but its final economy was made by {} exchanges",
                        view.frames.len(),
                        last.receipts.len()
                    )));
                }
                let frames = view.frames.iter().zip(&last.receipts).enumerate().map(
                    |(index, (frame, receipt))| {
                        let item = FrameItem {
                            step: index + 1,
                            exchange: receipt.exchange(),
                            view: frame,
                        };
                        (index, item)
                    },
                );
                json(&page(address, frames, PAGE_FRAMES)?)
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
    /// or study holds, by prefix.
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
        let candidates: Vec<String> = match argument {
            "account" | "rate" | "invariant" | "asset" | "prefix" => {
                let loaded = self
                    .stored(
                        &needed("economy_id")?
                            .parse::<EconomyId>()
                            .map_err(|error| {
                                McpError::invalid_params(format!("`economy_id`: {error}"), None)
                            })?,
                    )
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
                        .map(|invariant| invariant.name().to_string())
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
            "document" => self
                .study_record(&needed("study_id")?.parse::<StudyId>().map_err(|error| {
                    McpError::invalid_params(format!("`study_id`: {error}"), None)
                })?)?
                .ok_or_else(not_found)?
                .documents
                .iter()
                .map(|document| document.id.to_string())
                .collect(),
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

    fn study_record(&self, study_id: &StudyId) -> Result<Option<StudyRecord>, McpError> {
        self.archive.study_record(study_id).map_err(internal)
    }

    /// The economies a study recorded for one of its documents.
    fn document_economies(
        &self,
        study_id: &StudyId,
        document: &DocumentPath,
    ) -> Result<DocumentSummary, McpError> {
        self.study_record(study_id)?
            .and_then(|record| {
                record
                    .documents
                    .into_iter()
                    .find(|summary| &summary.id == document)
            })
            .ok_or_else(|| {
                internal(format!(
                    "study `{study_id}` serves document `{document}` but recorded no economies for it"
                ))
            })
    }

    /// A study's views: from memory, or computed again from its key and
    /// served only if they replay the very trajectories the study recorded.
    pub(crate) async fn study(&self, study_id: &StudyId) -> Result<Option<Arc<Study>>, McpError> {
        if let Some(study) = self.archive.cached_study(study_id) {
            return Ok(Some(study));
        }
        let Some(record) = self.study_record(study_id)? else {
            return Ok(None);
        };
        let request = studies::request(&record.key);
        let (study, trajectories) = tokio::task::spawn_blocking(move || {
            studies::compute(&request, &StudyControl::default(), None)
        })
        .await
        .map_err(internal)?
        .map_err(internal)?;
        if !self.archive.recorded(&record, &trajectories) {
            return Err(McpError::resource_not_found(
                format!(
                    "study `{study_id}` was recorded by another version of axionomy-mcp: computed again from its key, its documents no longer replay the trajectories it recorded. Its economies remain; study the problem again to read its documents"
                ),
                Some(
                    serde_json::json!({ "uri": Target::Study { study_id: study_id.clone() }.uri() }),
                ),
            ));
        }
        Ok(Some(self.archive.cache_study(study_id.clone(), study)))
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
                    let row = InvariantRow {
                        invariant: invariant.name().clone(),
                        weights: invariant.weights().count(),
                    };
                    (position, row)
                }),
            PAGE_ROWS,
        )
        .and_then(|page| json(&page)),
        Target::InvariantWeights { invariant, .. } => {
            let invariant = economy
                .invariants()
                .find(|candidate| candidate.name() == invariant)?;
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
        | Target::Study { .. }
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
pub(crate) fn amounts(rate: &paths::Rate) -> impl Iterator<Item = AmountRow<'_>> {
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
    parameters: Vec<&'a ParameterPath>,
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
pub(crate) struct AmountRow<'a> {
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

/// A study document without its frames, and the economies it runs between.
#[derive(Serialize)]
struct DocumentView<'a, T> {
    /// The economy the document starts from.
    initial_economy_id: &'a EconomyId,
    /// The economy its exchanges reach: step N of its trace is the economy
    /// after the first N, the one frame N − 1 shows.
    final_economy_id: &'a EconomyId,
    /// Frames are paged through the document's `frames` resource.
    frames: usize,
    document: T,
}

/// One frame of a study document: its exchange as the document's economies
/// take it, and the frame as Studio shows it.
#[derive(Serialize)]
struct FrameItem<'a> {
    /// The step of the document's trace this frame shows: `axionomy_trace_step`
    /// with this step on the document's `final_economy_id` names the economy
    /// `view.after` shows. One more than `view.index`.
    step: usize,
    /// The frame's exchange in the economy's own paths, as
    /// `axionomy_exchange_apply` takes it: applied to step `step − 1` of the
    /// final economy's trace, it makes step `step`.
    exchange: &'a paths::Exchange,
    /// The frame as Studio shows it, keyed for display under the problem's
    /// name; act with `exchange` and the economies' resources instead.
    view: &'a ExchangeFrame,
}
