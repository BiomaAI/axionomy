//! Evaluates package source in memory. Imports resolve exclusively to modules
//! supplied by the package, on native targets and in the browser alike.
use crate::script;
use axionomy::{AccountPath, PathKind, path::Path, paths};
use rhai::{
    ASTNode, Array, Dynamic, Engine, EvalAltResult, Expr, Map, Module, Position, Scope, Stmt,
    module_resolvers::StaticModuleResolver,
};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub use rhai::Dynamic as Value;

pub enum Library {}
impl PathKind for Library {
    const SCHEMA: &'static str = "ScriptPath";
    const EXAMPLE: &'static str = "maze/heuristic";
}
pub type LibraryPath = Path<Library>;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct Error {
    pub field: String,
    pub position: Position,
    pub message: String,
}
impl Error {
    fn new(position: Position, message: impl ToString) -> Self {
        Self {
            field: String::new(),
            position,
            message: message.to_string(),
        }
    }
}
impl From<Box<EvalAltResult>> for Error {
    fn from(error: Box<EvalAltResult>) -> Self {
        Self::new(error.position(), error)
    }
}

/// A package's finite set of authoring helpers and the common economic API.
pub struct Program(Engine);

impl Program {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine
            .set_module_resolver(StaticModuleResolver::new())
            .set_max_operations(5_000_000)
            .set_max_expr_depths(128, 128)
            .set_max_call_levels(64)
            .set_max_array_size(250_000)
            .set_max_map_size(100_000)
            .set_max_string_size(4 * 1024 * 1024)
            .set_fail_on_invalid_map_property(true)
            .on_print(|_| {})
            .on_debug(|_, _, _| {});
        script::register(&mut engine);
        crate::vocabulary::register(&mut engine);
        engine.register_fn(
            "unsigned",
            |decimal: &str| -> Result<u64, Box<EvalAltResult>> {
                decimal
                    .parse()
                    .map_err(|_| format!("invalid unsigned integer `{decimal}`").into())
            },
        );
        engine.register_fn(
            "opening",
            |account: AccountPath, basket: paths::Basket| -> Result<Array, Box<EvalAltResult>> {
                let mut holder = Map::new();
                holder.insert("balances".into(), Dynamic::from(basket));
                Ok(vec![
                    Dynamic::from(account.to_string()),
                    Dynamic::from(holder),
                ])
            },
        );
        Self(engine)
    }

    pub fn with_modules<'a>(
        sources: impl IntoIterator<Item = (&'a LibraryPath, &'a str)>,
    ) -> Result<Self, (LibraryPath, Error)> {
        let mut program = Self::new();
        let mut resolver = StaticModuleResolver::new();
        let mut pending = BTreeMap::new();
        for (id, source) in sources {
            let mut ast = program
                .0
                .compile(source)
                .map_err(|e| (id.clone(), Error::new(e.position(), e)))?;
            ast.set_source(id.as_str());
            let mut dependencies = BTreeSet::new();
            let mut invalid = None;
            ast.walk(&mut |nodes| {
                if let Some(ASTNode::Stmt(Stmt::Import(import, pos))) = nodes.last() {
                    if let Expr::StringConstant(name, _) = &import.0
                        && let Ok(id) = name.parse::<LibraryPath>()
                    {
                        dependencies.insert(id);
                    } else {
                        invalid = Some(Error::new(
                            *pos,
                            "an import requires a literal package module path",
                        ));
                        return false;
                    }
                }
                true
            });
            if let Some(e) = invalid {
                return Err((id.clone(), e));
            }
            pending.insert(id, (ast, dependencies));
        }
        let mut installed = BTreeSet::new();
        while !pending.is_empty() {
            let Some(id) = pending
                .iter()
                .find(|(_, (_, deps))| deps.is_subset(&installed))
                .map(|(id, _)| *id)
            else {
                let (id, (_, deps)) = pending.first_key_value().expect("nonempty");
                let missing = deps
                    .difference(&installed)
                    .map(Path::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err((
                    (*id).clone(),
                    Error::new(
                        Position::NONE,
                        format!("unresolved or cyclic module imports in {id}: {missing}"),
                    ),
                ));
            };
            let (ast, _) = pending.remove(id).expect("pending module");
            let module = Module::eval_ast_as_new(Scope::new(), &ast, &program.0)
                .map_err(|e| (id.clone(), e.into()))?;
            resolver.insert(id.as_str(), module);
            program.0.set_module_resolver(resolver.clone());
            installed.insert(id.clone());
        }
        Ok(program)
    }

    pub fn evaluate(&self, source: &str) -> Result<Dynamic, Error> {
        let value = self.0.eval::<Dynamic>(source)?;
        normalize(value).map_err(Into::into)
    }

    pub fn read<T: DeserializeOwned>(&self, source: &str) -> Result<T, Error> {
        decode(&self.evaluate(source)?)
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

// Completed economic values retain their Rust type throughout Rhai execution.
// Only at the return boundary do we project them into the receiving ontology.
pub fn normalize(value: Dynamic) -> Result<Dynamic, Box<EvalAltResult>> {
    if value.is::<Array>() {
        let mut items = value.cast::<Array>();
        for item in &mut items {
            *item = normalize(std::mem::take(item))?;
        }
        return Ok(Dynamic::from(items));
    } else if value.is::<Map>() {
        let mut items = value.cast::<Map>();
        for item in items.values_mut() {
            *item = normalize(std::mem::take(item))?;
        }
        return Ok(Dynamic::from(items));
    } else if value.is::<crate::vocabulary::Vocabulary>() {
        return rhai::serde::to_dynamic(value.cast::<crate::vocabulary::Vocabulary>());
    } else if value.is::<script::Invariant>() {
        return rhai::serde::to_dynamic(value.cast::<script::Invariant>().finish());
    } else if value.is::<paths::LinearInvariant>() {
        return rhai::serde::to_dynamic(value.cast::<paths::LinearInvariant>());
    } else if value.is::<paths::Basket>() {
        return rhai::serde::to_dynamic(value.cast::<paths::Basket>());
    } else if value.is::<paths::Exchange>() {
        return rhai::serde::to_dynamic(value.cast::<paths::Exchange>());
    } else if value.is::<paths::Goal>() {
        return rhai::serde::to_dynamic(value.cast::<paths::Goal>());
    } else if value.is::<axionomy::Quantity>() {
        return Ok(Dynamic::from(
            value.cast::<axionomy::Quantity>().to_string(),
        ));
    }
    Ok(value)
}

pub fn decode<T: DeserializeOwned>(value: &Dynamic) -> Result<T, Error> {
    let mut unknown = None;
    let mut ignored = |path: serde_ignored::Path<'_>| {
        unknown.get_or_insert_with(|| path.to_string());
    };
    let strict = serde_ignored::Deserializer::new(
        rhai::serde::DynamicDeserializer::new(value),
        &mut ignored,
    );
    let result = serde_path_to_error::deserialize(strict).map_err(|e| Error {
        field: e.path().to_string(),
        position: e.inner().position(),
        message: e.inner().to_string(),
    })?;
    if let Some(field) = unknown {
        return Err(Error {
            message: format!("unknown field {field}"),
            position: Position::NONE,
            field,
        });
    }
    Ok(result)
}
