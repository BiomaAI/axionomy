//! Why exchanges fail, in words a client can act on: each issue names what
//! the exchange named and the resource that lists what it could name instead.

use crate::{
    handles::EconomyId,
    messages::{Amounts, Issue, IssueKind, QuantityLine},
    resources::Target,
};
use axionomy::{RatePath, paths};
use std::fmt::Write as _;

/// Shortfalls a failure message names before counting the rest.
const NAMED_SHORTFALLS: usize = 3;

/// Why an assessed exchange cannot apply to the economy `economy_id`, each
/// issue in words.
pub(crate) fn of(
    assessment: &paths::Assessment,
    exchange: &paths::Exchange,
    economy_id: &EconomyId,
) -> Vec<Issue> {
    assessment
        .issues()
        .iter()
        .map(|issue| Issue::new(issue, exchange, economy_id))
        .collect()
}

impl Issue {
    /// Explains why `exchange` cannot apply to the economy `economy_id`.
    pub(crate) fn new(
        error: &paths::ApplyError,
        exchange: &paths::Exchange,
        economy_id: &EconomyId,
    ) -> Self {
        use paths::ApplyError as Error;
        let rate = exchange.rate();
        let rate_uri = |rate: &RatePath| {
            Target::Rate {
                economy_id: economy_id.clone(),
                rate: rate.clone(),
            }
            .uri()
        };
        let (message, kind) = match error.clone() {
            Error::MissingRate { rate } => (
                format!(
                    "the economy has no rate `{rate}`; its rates are listed at {}",
                    Target::Rates {
                        economy_id: economy_id.clone(),
                        asset: None,
                    }
                    .uri()
                ),
                IssueKind::MissingRate { rate },
            ),
            Error::MissingBinding { role } => (
                format!(
                    "the exchange binds no account to role `{role}`; bind every role of rate `{rate}`, listed at {}",
                    rate_uri(rate)
                ),
                IssueKind::MissingBinding { role },
            ),
            Error::UnknownBinding { role } => (
                format!(
                    "rate `{rate}` has no role `{role}`; its roles are listed at {}",
                    rate_uri(rate)
                ),
                IssueKind::UnknownBinding { role },
            ),
            Error::RolesMustDiffer { left, right } => (
                format!(
                    "rate `{rate}` needs roles `{left}` and `{right}` bound to different accounts"
                ),
                IssueKind::RolesMustDiffer { left, right },
            ),
            Error::MissingAccount { account } => (
                format!(
                    "the economy has no account `{account}`; bind one it has, listed at {}, or name a new account only for a role the rate opens",
                    Target::Accounts {
                        economy_id: economy_id.clone(),
                        prefix: None,
                    }
                    .uri()
                ),
                IssueKind::MissingAccount { account },
            ),
            Error::ZeroUnits => (
                "`units` must be greater than zero".to_owned(),
                IssueKind::ZeroUnits,
            ),
            Error::MissingParameter { name } => (
                format!(
                    "rate `{rate}` needs the parameter `{name}`; give it in the exchange's `parameters`"
                ),
                IssueKind::MissingParameter { name },
            ),
            Error::UnknownParameter { name } => (
                format!(
                    "rate `{rate}` takes no parameter `{name}`; its parameters are listed at {}",
                    rate_uri(rate)
                ),
                IssueKind::UnknownParameter { name },
            ),
            Error::RateOverflow { rate, asset } => (
                format!("`units` times rate `{rate}`'s amount of `{asset}` exceeds 2²⁵⁶ − 1"),
                IssueKind::RateOverflow { rate, asset },
            ),
            Error::RateLawEvaluation { rate, message } => (
                format!("a computed amount of rate `{rate}` cannot be evaluated: {message}"),
                IssueKind::RateLawEvaluation {
                    rate,
                    reason: message,
                },
            ),
            Error::RateConditionFailed { rate, condition } => (
                format!(
                    "condition `{condition}` of rate `{rate}` does not hold in this economy; the rate's conditions are listed at {}",
                    Target::RateConditions {
                        economy_id: economy_id.clone(),
                        rate: rate.clone(),
                    }
                    .uri()
                ),
                IssueKind::RateConditionFailed { rate, condition },
            ),
            Error::Infeasible { .. } => (
                "accounts lack assets for the exchange; the assessment's lines say how much"
                    .to_owned(),
                IssueKind::Infeasible,
            ),
            Error::BalanceOverflow { account, asset } => (
                format!("the exchange would take `{account}`'s balance of `{asset}` past 2²⁵⁶ − 1"),
                IssueKind::BalanceOverflow { account, asset },
            ),
            Error::InvariantViolation {
                invariant,
                before,
                after,
            } => (
                format!(
                    "the exchange would change the conservation law `{invariant}` from {before} to {after}; every exchange must keep it"
                ),
                IssueKind::InvariantViolation {
                    invariant,
                    before: *before,
                    after: *after,
                },
            ),
            Error::AccountExists { account } => (
                format!(
                    "account `{account}` already exists; a role the rate opens must name a new account path"
                ),
                IssueKind::AccountExists { account },
            ),
            Error::OpeningShared { role } => (
                format!(
                    "another role binds the account role `{role}` opens; an opened account is bound by its opening role alone"
                ),
                IssueKind::OpeningShared { role },
            ),
            Error::ClosedNotEmpty { account, remaining } => {
                let remaining: Amounts = remaining
                    .iter()
                    .map(|(asset, quantity)| (asset.clone(), *quantity))
                    .collect();
                let left = words(
                    remaining
                        .iter()
                        .map(|(asset, quantity)| format!("{quantity} `{asset}`")),
                    remaining.len(),
                );
                (
                    format!(
                        "the exchange closes account `{account}` but would leave {left} in it; an account closes only once empty"
                    ),
                    IssueKind::ClosedNotEmpty { account, remaining },
                )
            }
        };
        Self { message, kind }
    }
}

/// What accounts lack, in words: "`source` lacks 5 `token`", naming the first
/// few shortfalls and counting the rest.
pub(crate) fn shortfalls(lines: &[QuantityLine]) -> String {
    words(
        lines.iter().take(NAMED_SHORTFALLS).map(|line| {
            format!(
                "`{}` lacks {} `{}`",
                line.account, line.quantity, line.asset
            )
        }),
        lines.len(),
    )
}

/// Phrases joined as a list, "a, b and c", ending with how many of `total`
/// were left unnamed.
fn words(phrases: impl Iterator<Item = String>, total: usize) -> String {
    let phrases: Vec<String> = phrases.collect();
    let unnamed = total - phrases.len();
    let mut text = String::new();
    for (index, phrase) in phrases.iter().enumerate() {
        if index > 0 {
            text.push_str(if index + 1 == phrases.len() && unnamed == 0 {
                " and "
            } else {
                ", "
            });
        }
        text.push_str(phrase);
    }
    if unnamed > 0 {
        let _ = write!(text, " and {unnamed} more");
    }
    text
}
