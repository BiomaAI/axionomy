//! Compare two independently built models with identical opening holdings,
//! actor bindings, observation history, trial seeds and search limits.
use super::*;
use crate::{Effort, Ended};
use axionomy::Quantity;
use axionomy_search::sampling::sample_seed;
use axionomy_store::Point;
use std::ops::ControlFlow;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BuildComparison {
    pub experiment: ExperimentPath,
    pub equal: bool,
    pub trials: Vec<ComparedTrial>,
}
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ComparedTrial {
    pub strategy: StrategyPath,
    pub start: RefPath,
    #[serde(serialize_with = "seed_digits")]
    #[schemars(with = "String")]
    pub seed: u64,
    pub left: TrialOutcome,
    pub right: TrialOutcome,
    pub first_difference: Option<FirstDifference>,
}
#[derive(Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct TrialOutcome {
    pub ended: Ended,
    pub steps: usize,
    pub objectives: BTreeMap<ObjectivePath, Quantity>,
}
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FirstDifference {
    pub step: usize,
    pub left: Option<paths::Exchange>,
    pub right: Option<paths::Exchange>,
    pub balances: Vec<(AccountPath, AssetPath, Quantity, Quantity)>,
    pub accounts: Vec<(AccountPath, bool, bool)>,
}

pub fn compare(
    left: &Engine,
    right: &Engine,
    id: &ExperimentPath,
    seed: Option<u64>,
) -> Result<BuildComparison, Diagnostic> {
    let error = |e: EngineError| Diagnostic::new(SourceFile::Definitions, e);
    let a = left.experiment(id).map_err(error)?;
    let b = right.experiment(id).map_err(error)?;
    let seed = seed.unwrap_or(a.effort.seed);
    let effort = Effort { seed, ..a.effort };
    if a.start != b.start
        || a.goal != b.goal
        || a.arms != b.arms
        || a.objectives != b.objectives
        || a.trials != b.trials
        || a.effort.budget != b.effort.budget
        || a.effort.turns != b.effort.turns
    {
        return Err(Diagnostic::new(
            SourceFile::Definitions,
            format!("{id} differs in starts, goal, arms, objectives or effort"),
        ));
    }
    let mut trials = Vec::new();
    for arm in &a.arms {
        let start = arm.start.as_ref().unwrap_or(&a.start);
        let p = left.referred(start).map_err(error)?;
        let q = right.referred(start).map_err(error)?;
        let initial = |engine: &Engine, at: Point| -> Result<_, EngineError> {
            let (original, trace) = engine.history(at)?;
            Ok((
                engine.ontology(at)?,
                original.state_key(),
                trace,
                engine.reach(at)?.state.economy.state_key(),
            ))
        };
        if initial(left, p).map_err(error)? != initial(right, q).map_err(error)? {
            return Err(Diagnostic::new(
                SourceFile::Instance(start.clone()),
                "opening ontology, holdings or observed history differs",
            ));
        }
        for i in 0..a.trials {
            let seed = sample_seed(
                seed,
                usize::try_from(i).map_err(|e| Diagnostic::new(SourceFile::Definitions, e))?,
            );
            let run = |engine: &Engine, at: Point| -> Result<_, EngineError> {
                let ran = engine.run_strategy(
                    at,
                    &engine.strategy(&arm.strategy)?,
                    &a.goal,
                    Effort { seed, ..effort },
                    &mut |_| ControlFlow::Continue(()),
                )?;
                let objectives = a
                    .objectives
                    .iter()
                    .map(|id| Ok((id.clone(), engine.measure(ran.end, id)?)))
                    .collect::<Result<_, EngineError>>()?;
                let (_, history) = engine.history(ran.end)?;
                let played = history
                    .into_iter()
                    .skip(at.step.try_into().unwrap_or(usize::MAX))
                    .collect::<Vec<_>>();
                Ok((
                    TrialOutcome {
                        ended: Ended::from(&ran.outcome),
                        steps: played.len(),
                        objectives,
                    },
                    played,
                ))
            };
            let (l, ltrace) = run(left, p).map_err(error)?;
            let (r, rtrace) = run(right, q).map_err(error)?;
            let mut states = (
                left.reach(p).map_err(error)?.state.economy.clone(),
                right.reach(q).map_err(error)?.state.economy.clone(),
            );
            let mut first_difference = None;
            for index in 0..ltrace.len().max(rtrace.len()) {
                let actions = (ltrace.get(index), rtrace.get(index));
                for (state, action) in [(&mut states.0, actions.0), (&mut states.1, actions.1)] {
                    if let Some(action) = action {
                        state
                            .apply(action.clone())
                            .map_err(|e| Diagnostic::new(SourceFile::Definitions, e))?;
                    }
                }
                if actions.0 != actions.1 || states.0.state_key() != states.1.state_key() {
                    let mut balances = Vec::new();
                    let mut accounts = Vec::new();
                    for account in states
                        .0
                        .accounts()
                        .chain(states.1.accounts())
                        .map(|(id, _)| id)
                        .collect::<BTreeSet<_>>()
                    {
                        let a = states.0.account(account);
                        let b = states.1.account(account);
                        if a.is_some() != b.is_some() {
                            accounts.push((account.clone(), a.is_some(), b.is_some()));
                        }
                        let assets = a
                            .into_iter()
                            .chain(b)
                            .flat_map(|v| v.balances().iter().map(|(id, _)| id))
                            .collect::<BTreeSet<_>>();
                        for asset in assets {
                            let x = states.0.balance(account, asset);
                            let y = states.1.balance(account, asset);
                            if x != y {
                                balances.push((account.clone(), asset.clone(), x, y));
                            }
                        }
                    }
                    first_difference = Some(FirstDifference {
                        step: index + 1,
                        left: actions.0.cloned(),
                        right: actions.1.cloned(),
                        balances,
                        accounts,
                    });
                    break;
                }
            }
            trials.push(ComparedTrial {
                strategy: arm.strategy.clone(),
                start: start.clone(),
                seed,
                left: l,
                right: r,
                first_difference,
            });
        }
    }
    Ok(BuildComparison {
        experiment: id.clone(),
        equal: trials
            .iter()
            .all(|t| t.first_difference.is_none() && t.left == t.right),
        trials,
    })
}

fn seed_digits<S: serde::Serializer>(n: &u64, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(n)
}
