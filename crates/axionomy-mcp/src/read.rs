//! Reads `axionomy://` resources and completes their template arguments.

use axionomy::{InvariantPath, ParameterPath};

use crate::{
    handles::{AssessmentId, EconomyId},
    issues,
    messages::{AssessmentStatus, Direction, EconomyCounts, Issue, StateRoot},
    resources::{Address, PAGE_ROWS, Page, Target, page, span_page},
    server::{AxionomyMcp, assessment_lines, counts, receipt_lines},
    store::{Loaded, Origin},
};
use axionomy::{
    AccountPath, AssetPath, Quantity, RatePath, RolePath,
    paths::{self, Paths},
};
use axionomy_store::state;
use rmcp::{
    ErrorData as McpError,
    model::{CompleteRequestParams, CompleteResult, CompletionInfo},
};
use serde::Serialize;
use std::{ops::Range, sync::Arc};

impl AxionomyMcp {
    /// The JSON content of one resource page.
    pub(crate) async fn read(&self, address: &Address) -> Result<String, McpError> {
        let missing = |message: String| absent(address, message);
        match &address.target {
            Target::Assessment { assessment_id } => {
                let stored = self
                    .assessment(assessment_id)
                    .await?
                    .ok_or_else(|| missing(self.not_stored(assessment_id)))?;
                let assessment = &stored.assessment;
                json(&AssessmentView {
                    assessment_id,
                    economy_id: &stored.economy_id,
                    after_exchanges: stored.after_exchanges,
                    exchange: &stored.exchange,
                    status: assessment.status().into(),
                    issues: issues::of(assessment, &stored.exchange, &stored.economy_id),
                    lines: assessment_lines(assessment).count(),
                })
            }
            Target::AssessmentLines { assessment_id } => {
                let stored = self
                    .assessment(assessment_id)
                    .await?
                    .ok_or_else(|| missing(self.not_stored(assessment_id)))?;
                json(&page(
                    address,
                    assessment_lines(&stored.assessment).enumerate(),
                    PAGE_ROWS,
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
                let loaded = self.loaded(address, economy_id).await?;
                read_economy(address, economy_id, &loaded)
            }
            Target::Diff {
                economy_id,
                other_id,
            } => {
                let before = self.loaded(address, economy_id).await?;
                let after = self.loaded(address, other_id).await?;
                let changes = state::changes::<Paths>(&before.state().tree, &after.state().tree)
                    .map_err(internal)?
                    .into_iter()
                    .map(DiffRow::from)
                    .enumerate();
                json(&page(address, changes, PAGE_ROWS)?)
            }
        }
    }

    /// Suggests values for a template argument from what the named economy
    /// holds, by prefix.
    pub(crate) async fn completion(
        &self,
        request: CompleteRequestParams,
    ) -> Result<CompleteResult, McpError> {
        let Some(template) = request.r#ref.as_resource_uri() else {
            return Ok(CompleteResult::default());
        };
        let Some(listing) = crate::resources::templates()
            .into_iter()
            .find(|known| known.uri_template == template)
        else {
            return Err(McpError::invalid_params(
                format!("`{template}` is not one of this server's resource templates"),
                None,
            ));
        };
        // `prefix` filters account paths on the accounts listing and asset
        // paths on an account's balances.
        let accounts_template = listing.name == "accounts";
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
        let not_found = |message: String| McpError::resource_not_found(message, None);
        let prefix = request.argument.value.as_str();
        let candidates: Vec<String> = match argument {
            "account" | "rate" | "invariant" | "asset" | "prefix" => {
                let economy_id = needed("economy_id")?
                    .parse::<EconomyId>()
                    .map_err(|error| {
                        McpError::invalid_params(format!("`economy_id`: {error}"), None)
                    })?;
                let loaded = self
                    .stored(&economy_id)
                    .await?
                    .ok_or_else(|| not_found(self.not_stored(&economy_id)))?;
                let (economy, tree) = (&loaded.state().economy, &loaded.state().tree);
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
                        economy.account(&account).ok_or_else(|| {
                            not_found(format!("economy `{economy_id}` has no account `{account}`"))
                        })?;
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

    /// The economy a resource reads, or why it is not there.
    async fn loaded(
        &self,
        address: &Address,
        economy_id: &EconomyId,
    ) -> Result<Arc<Loaded>, McpError> {
        self.stored(economy_id)
            .await?
            .ok_or_else(|| absent(address, self.not_stored(economy_id)))
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

/// The error for a resource that names something not there, saying what.
fn absent(address: &Address, message: String) -> McpError {
    McpError::resource_not_found(
        message,
        Some(serde_json::json!({ "uri": address.target.uri() })),
    )
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

/// One resource of an economy, or why the account, rate or invariant it names
/// is not there.
fn read_economy(
    address: &Address,
    economy_id: &EconomyId,
    loaded: &Loaded,
) -> Result<String, McpError> {
    let (economy, tree) = (&loaded.state().economy, &loaded.state().tree);
    let receipts = loaded.receipts();
    let economy_id = economy_id.clone();
    // "economy `…` has no rate `swap`; its rates are listed at …"
    let lacks = |kind: &str, path: &dyn std::fmt::Display, listing: Target| {
        absent(
            address,
            format!(
                "economy `{economy_id}` has no {kind} `{path}`; its {kind}s are listed at {}",
                listing.uri()
            ),
        )
    };
    let no_rate = |rate: &RatePath| {
        lacks(
            "rate",
            rate,
            Target::Rates {
                economy_id: economy_id.clone(),
                asset: None,
            },
        )
    };
    let economy_id = &economy_id;
    match &address.target {
        Target::Economy { .. } => json(&EconomyView {
            economy_id,
            parent_id: loaded.record.parent.as_ref(),
            origin: match loaded.record.origin {
                Origin::Authored { entries } => OriginView::Authored { entries },
                Origin::Exchanged { exchanges } => OriginView::Exchanged { exchanges },
            },
            root: StateRoot(loaded.state().root()),
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
            economy.account(account).ok_or_else(|| {
                lacks(
                    "account",
                    account,
                    Target::Accounts {
                        economy_id: economy_id.clone(),
                        prefix: None,
                    },
                )
            })?;
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
            let rate = economy.rate(id).ok_or_else(|| no_rate(id))?;
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
        Target::RateAmounts { rate: id, .. } => {
            let rate = economy.rate(id).ok_or_else(|| no_rate(id))?;
            page(address, amounts(rate).enumerate(), PAGE_ROWS).and_then(|page| json(&page))
        }
        Target::RateConditions { rate: id, .. } => {
            let rate = economy.rate(id).ok_or_else(|| no_rate(id))?;
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
        Target::InvariantWeights {
            invariant: path, ..
        } => {
            let invariant = economy
                .invariants()
                .find(|candidate| candidate.name() == path)
                .ok_or_else(|| {
                    lacks(
                        "invariant",
                        path,
                        Target::Invariants {
                            economy_id: economy_id.clone(),
                        },
                    )
                })?;
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
        Target::Diff { .. } | Target::Assessment { .. } | Target::AssessmentLines { .. } => {
            Err(internal(format!(
                "`{}` is not one of an economy's own resources",
                address.target.uri()
            )))
        }
    }
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

/// One difference between two economies.
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
    /// How many exchanges of the failing call were applied to `economy_id`
    /// before this one: the failure's `exchange_index`. Zero for an exchange
    /// assessed against the economy itself.
    after_exchanges: usize,
    exchange: &'a paths::Exchange,
    status: AssessmentStatus,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    issues: Vec<Issue>,
    lines: usize,
}
