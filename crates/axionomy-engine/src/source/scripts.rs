//! A source script's literal host calls are checked against the vocabulary
//! of each ontology it advertises. Computed identifiers are resolved when
//! the engine runs the script.
use super::*;
use rhai::{ASTNode, Expr, Position};
#[derive(Clone, Copy)]
enum Kind {
    Account,
    Asset,
    Rate,
    Role,
    Parameter,
}
impl Kind {
    fn check(self, vocab: &Vocabulary, text: &str) -> Result<(), String> {
        let bad = |e: axionomy::PathError| e.to_string();
        match self {
            Self::Account => vocab.account(&text.parse().map_err(bad)?),
            Self::Asset => vocab.asset(&text.parse().map_err(bad)?),
            Self::Rate => vocab.rate(text),
            Self::Role => declared(&vocab.roles, "role", &text.parse().map_err(bad)?),
            Self::Parameter => {
                declared(&vocab.parameters, "parameter", &text.parse().map_err(bad)?)
            }
        }
    }
}

impl Vocabulary {
    fn rate(&self, text: &str) -> Result<(), String> {
        let id = text.parse::<RatePath>().map_err(|e| e.to_string())?;
        let parts = id.segments().collect::<Vec<_>>();
        if self.rates.iter().any(|p| p.read(&parts).is_some()) {
            Ok(())
        } else {
            Err(format!("undeclared rate `{id}`"))
        }
    }
    pub(super) fn exchange(&self, e: &paths::Exchange) -> Result<(), String> {
        self.rate(e.rate().as_str())?;
        for (role, account) in e.bindings() {
            declared(&self.roles, "role", role)?;
            self.account(account)?;
        }
        for param in e.parameters().keys() {
            declared(&self.parameters, "parameter", param)?;
        }
        Ok(())
    }

    pub(super) fn script(&self, file: &SourceFile, ast: &rhai::AST) -> Result<(), Diagnostic> {
        let mut error = None;
        ast.walk(&mut |nodes| {
            let (call, method) = match nodes.last() {
                Some(ASTNode::Expr(Expr::MethodCall(call, _))) => (call, true),
                Some(ASTNode::Expr(Expr::FnCall(call, _)))
                | Some(ASTNode::Stmt(rhai::Stmt::FnCall(call, _)))
                    if call.namespace.is_empty() =>
                {
                    (call, false)
                }
                _ => return true,
            };
            let constructor = !method
                && matches!(
                    call.name.as_str(),
                    "asset" | "account" | "rate" | "role" | "exchange"
                );
            let offset = usize::from(!method && !constructor);
            let args = call.args.get(offset..).unwrap_or_default();
            use Kind::*;
            let kinds: &[Kind] = match (call.name.as_str(), args.len()) {
                ("balance", 2) => &[Account, Asset],
                ("has_account" | "balances", 1) => &[Account],
                ("consumed" | "produced" | "preserved", 2) => &[Rate, Role],
                ("exchange_of" | "values" | "roles", 1) | ("exchange", 3) => &[Rate],
                ("account", 1) if constructor => &[Account],
                ("asset", 1) if constructor => &[Asset],
                ("rate", 1) if constructor => &[Rate],
                ("role", 1) if constructor => &[Role],
                ("account", 1) => &[Role],
                ("parameter", 2) => &[Parameter],
                _ => return true,
            };
            let mut check = |kind: Kind, text: &str, pos: Position| {
                if let Err(reason) = kind.check(self, text) {
                    let mut d = Diagnostic::new(file, reason);
                    d.line = pos.line();
                    d.column = pos.position();
                    d.field = format!("{} at {}", call.name, pos);
                    error = Some(d);
                    return false;
                }
                true
            };
            for (arg, &kind) in args.iter().zip(kinds) {
                if let Expr::StringConstant(text, pos) = arg
                    && !check(kind, text, *pos)
                {
                    return false;
                }
            }
            if call.name == "exchange" {
                let bindings = &args[1];
                // Rhai folds an entirely literal map to a constant. A mixed
                // map retains its individual key and expression locations.
                match bindings {
                    Expr::Map(map, _) => {
                        for (key, value) in &map.0 {
                            if !check(Role, &key.name, key.pos) {
                                return false;
                            }
                            if let Expr::StringConstant(text, pos) = value
                                && !check(Account, text, *pos)
                            {
                                return false;
                            }
                        }
                    }
                    Expr::DynamicConstant(value, pos) => {
                        if let Some(map) = value.clone().try_cast::<rhai::Map>() {
                            for (role, value) in map {
                                if !check(Role, role.as_str(), *pos) {
                                    return false;
                                }
                                if let Some(text) = value.try_cast::<rhai::ImmutableString>()
                                    && !check(Account, &text, *pos)
                                {
                                    return false;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            true
        });
        error.map_or(Ok(()), Err)
    }
}

/// Recover the location from Rhai itself on the error path. Successful
/// scripts use the engine's cached compilation.
pub(super) fn diagnose(
    engine: &Engine,
    file: &SourceFile,
    source: &str,
    error: EngineError,
) -> Diagnostic {
    let mut d = Diagnostic::new(file, &error);
    let pos = match engine.host.compile(source) {
        Err(parse) => Some(parse.position()),
        Ok(ast) => {
            let mut at = None;
            ast.walk(&mut |nodes| {
                if let Some(ASTNode::Stmt(rhai::Stmt::Import(import, pos))) = nodes.last() {
                    let matches = match (&error, &import.0) {
                        (
                            EngineError::UnknownImport {
                                import: missing, ..
                            },
                            Expr::StringConstant(text, _),
                        ) => text.as_str() == missing.as_str(),
                        (EngineError::Script { .. }, Expr::StringConstant(text, _)) => {
                            text.parse::<ScriptPath>().is_err()
                        }
                        (EngineError::Script { .. }, _) => true,
                        _ => false,
                    };
                    if matches {
                        at = Some(*pos);
                        return false;
                    }
                }
                true
            });
            at
        }
    };
    if let Some(pos) = pos {
        d.line = pos.line();
        d.column = pos.position();
    }
    d
}
