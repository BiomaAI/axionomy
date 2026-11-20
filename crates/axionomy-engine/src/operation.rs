//! The Rhai input accepted by interactive operations. A stored function and
//! an inline expression run against the same host and return native values.
use crate::{Engine, EngineError, ScriptCall, ScriptState};
use axionomy::{AccountPath, AssetPath, RatePath, RolePath, paths};
use rhai::{Dynamic, Scope};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RhaiInput {
    /// Rhai source. When the operation supplies a state, it is named `state`.
    Source(String),
    /// A function kept by the engine. Its single argument is the supplied
    /// state. Operations without a state invoke a zero-argument function.
    Function(ScriptCall),
}

/// A runnable Rhai expression for a concrete exchange offered by the engine.
/// Identifiers contain only canonical path characters and quantities only digits.
pub fn exchange_source(exchange: &paths::Exchange) -> String {
    use std::fmt::Write;
    let mut source = format!(
        r#"exchange(rate("{}"), quantity("{}"))"#,
        exchange.rate(),
        exchange.units()
    );
    for (role, account) in exchange.bindings() {
        write!(
            &mut source,
            r#".bind(role("{role}"), account("{account}"))"#
        )
        .expect("write to string");
    }
    for (parameter, quantity) in exchange.parameters() {
        write!(
            &mut source,
            r#".parameter("{parameter}", quantity("{quantity}"))"#
        )
        .expect("write to string");
    }
    source
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RhaiApi {
    pub authoring: Vec<String>,
    pub runtime: Vec<String>,
}

impl Engine {
    /// The host functions actually registered in this build.
    pub fn rhai_api(&self) -> RhaiApi {
        let mut runtime = self.host.gen_fn_signatures(false);
        runtime.sort();
        RhaiApi {
            runtime,
            authoring: axionomy_authoring::program::Program::new().functions(),
        }
    }

    pub fn evaluate(
        &self,
        program: &RhaiInput,
        state: Option<ScriptState>,
    ) -> Result<Dynamic, EngineError> {
        match program {
            RhaiInput::Function(call) => match state {
                Some(state) => self.call(&call.script, call.function.as_str(), (state,)),
                None => self.call(&call.script, call.function.as_str(), ()),
            },
            RhaiInput::Source(source) => {
                let mut scope = Scope::new();
                if let Some(state) = state {
                    scope.push("state", state);
                }
                self.host
                    .eval_with_scope(&mut scope, source)
                    .map_err(|e| EngineError::Operation(e.to_string()))
            }
        }
    }

    pub fn evaluate_model(
        &self,
        input: &RhaiInput,
        state: Option<ScriptState>,
    ) -> Result<axionomy_authoring::model::Model, EngineError> {
        returned(self.evaluate(input, state)?, "Model")
    }

    pub fn evaluate_balance_condition(
        &self,
        input: &RhaiInput,
        state: ScriptState,
    ) -> Result<axionomy_authoring::script::BalanceCondition, EngineError> {
        returned(self.evaluate(input, Some(state))?, "BalanceCondition")
    }

    pub fn evaluate_exchange(
        &self,
        input: &RhaiInput,
        state: ScriptState,
    ) -> Result<paths::Exchange, EngineError> {
        returned(self.evaluate(input, Some(state))?, "Exchange")
    }

    pub fn evaluate_exchanges(
        &self,
        input: &RhaiInput,
        state: ScriptState,
    ) -> Result<Vec<paths::Exchange>, EngineError> {
        let array = returned(self.evaluate(input, Some(state))?, "array of Exchange")?;
        crate::exchanges_of(array)
    }

    pub fn evaluate_goal(
        &self,
        input: &RhaiInput,
        state: ScriptState,
    ) -> Result<paths::Goal, EngineError> {
        returned(self.evaluate(input, Some(state))?, "Goal")
    }

    /// Project an inspection result to protocol data. Exact quantities are
    /// decimal strings, including inside maps and arrays. Unknown objects are
    /// rejected rather than leaking implementation details of the host.
    pub fn query(
        &self,
        input: &RhaiInput,
        state: ScriptState,
    ) -> Result<serde_json::Value, EngineError> {
        export(self.evaluate(input, Some(state))?)
    }
}

fn returned<T: Clone + Send + Sync + 'static>(
    value: Dynamic,
    expected: &'static str,
) -> Result<T, EngineError> {
    value
        .try_cast_result()
        .map_err(|found| EngineError::Returned {
            expected,
            found: found.type_name().to_owned(),
        })
}

fn export(value: Dynamic) -> Result<serde_json::Value, EngineError> {
    use serde_json::Value;
    if value.is::<rhai::Array>() {
        return value
            .cast::<rhai::Array>()
            .into_iter()
            .map(export)
            .collect::<Result<_, _>>()
            .map(Value::Array);
    }
    if value.is::<rhai::Map>() {
        return value
            .cast::<rhai::Map>()
            .into_iter()
            .map(|(key, v)| Ok((key.into(), export(v)?)))
            .collect::<Result<_, _>>()
            .map(Value::Object);
    }
    if value.is::<AccountPath>() {
        return Ok(value.cast::<AccountPath>().to_string().into());
    }
    if value.is::<AssetPath>() {
        return Ok(value.cast::<AssetPath>().to_string().into());
    }
    if value.is::<RatePath>() {
        return Ok(value.cast::<RatePath>().to_string().into());
    }
    if value.is::<RolePath>() {
        return Ok(value.cast::<RolePath>().to_string().into());
    }
    let value = axionomy_authoring::program::normalize(value)
        .map_err(|e| EngineError::Operation(e.to_string()))?;
    axionomy_authoring::program::decode(&value).map_err(|e| EngineError::Operation(e.to_string()))
}
