//! Source-level economic assertions. Each test starts from its own copy
//! of a named economy and uses the kernel's assessment, apply and replay.
use super::*;
use axionomy::{AssessmentStatus, Quantity};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EconomicTest {
    start: RefPath,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    exchange: paths::Exchange,
    status: AssessmentStatus,
    #[serde(default)]
    shortfalls: Option<BTreeMap<AccountPath, paths::Basket>>,
    #[serde(default)]
    receipt: Option<paths::Receipt>,
    #[serde(default)]
    balances: Vec<(AccountPath, AssetPath, Quantity)>,
    #[serde(default)]
    exists: BTreeMap<AccountPath, bool>,
    #[serde(default)]
    supply: BTreeMap<AssetPath, Quantity>,
}

impl EconomicTest {
    pub(super) fn run(
        &self,
        engine: &Engine,
        vocabs: &BTreeMap<&OntologyPath, Vocabulary>,
    ) -> Result<(), String> {
        let tip = engine.referred(&self.start).map_err(|e| e.to_string())?;
        let ontology = engine.ontology(tip).map_err(|e| e.to_string())?;
        let vocabulary = &vocabs[&ontology];
        let initial = engine.reach(tip).map_err(|e| e.to_string())?;
        let mut state = initial.state.economy.clone();
        let mut trace = paths::Trace::new();
        if self.steps.is_empty() {
            return Err("an economic test needs at least one step".into());
        }
        for (i, step) in self.steps.iter().enumerate() {
            step.check(vocabulary, &mut state, &mut trace)
                .map_err(|e| format!("step {}: {e}", i + 1))?;
        }
        let replay = initial
            .state
            .economy
            .replayed(&trace)
            .map_err(|e| e.to_string())?;
        if state.state_key() != replay.state_key() {
            return Err("replay reached a different state".into());
        }
        Ok(())
    }
}

impl Step {
    fn check(
        &self,
        vocabulary: &Vocabulary,
        state: &mut paths::Economy,
        trace: &mut paths::Trace,
    ) -> Result<(), String> {
        vocabulary.exchange(&self.exchange)?;
        for (account, asset, _) in &self.balances {
            vocabulary.account(account)?;
            vocabulary.asset(asset)?;
        }
        for account in self.exists.keys() {
            vocabulary.account(account)?;
        }
        for asset in self.supply.keys() {
            vocabulary.asset(asset)?;
        }
        let assessment = state.assess(&self.exchange);
        if assessment.status() != self.status {
            return Err(format!(
                "expected {:?}, got {:?}: {:?}",
                self.status,
                assessment.status(),
                assessment.issues()
            ));
        }
        if let Some(expected) = &self.shortfalls {
            let found: BTreeMap<_, _> = assessment
                .shortfalls()
                .iter()
                .map(|s| (s.account().clone(), s.missing().clone()))
                .collect();
            if &found != expected {
                return Err(format!("expected shortfalls {expected:?}, got {found:?}"));
            }
        }
        let before = state.state_key();
        match state.apply(self.exchange.clone()) {
            Ok(receipt) => {
                if self.status != AssessmentStatus::Applicable {
                    return Err("refused assessment applied".into());
                }
                if let Some(expected) = &self.receipt
                    && postcard::to_allocvec(expected).map_err(|e| e.to_string())?
                        != postcard::to_allocvec(&receipt).map_err(|e| e.to_string())?
                {
                    return Err(format!("receipt differs: {receipt:?}"));
                }
                trace.push(self.exchange.clone());
            }
            Err(error) => {
                if self.status == AssessmentStatus::Applicable || self.receipt.is_some() {
                    return Err(error.to_string());
                }
                if state.state_key() != before {
                    return Err("refused exchange changed state".into());
                }
            }
        }
        for (account, asset, amount) in &self.balances {
            let held = state
                .account(account)
                .ok_or_else(|| format!("account {account} is absent"))?
                .balance(asset);
            if held != *amount {
                return Err(format!(
                    "{account} balance of {asset}: expected {amount}, got {held}"
                ));
            }
        }
        for (account, exists) in &self.exists {
            if state.account(account).is_some() != *exists {
                return Err(format!("expected existence of {account} to be {exists}"));
            }
        }
        for (asset, expected) in &self.supply {
            let total = state
                .accounts()
                .try_fold(Quantity::ZERO, |n, (_, a)| n.checked_add(a.balance(asset)))
                .ok_or_else(|| {
                    format!("supply of {asset} exceeds the assertion's quantity range")
                })?;
            if total != *expected {
                return Err(format!(
                    "supply of {asset}: expected {expected}, got {total}"
                ));
            }
        }
        Ok(())
    }
}
