//! A source script's literal host calls are checked against the vocabulary
//! of each ontology it advertises. Computed identifiers are resolved when
//! the engine runs the script.
use super::*;
use rhai::{ASTNode, Expr};

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
            let (call, offset) = match nodes.last() {
                Some(ASTNode::Expr(Expr::MethodCall(call, _))) => (call, 0),
                Some(ASTNode::Expr(Expr::FnCall(call, _))) if call.namespace.is_empty() => {
                    (call, usize::from(call.name != "exchange"))
                }
                _ => return true,
            };
            let args = call.args.get(offset..).unwrap_or_default();
            let kinds: &[&str] = match (call.name.as_str(), args.len()) {
                ("balance", 2) => &["account", "asset"],
                ("has_account" | "balances", 1) => &["account"],
                ("consumed" | "produced" | "preserved", 2) => &["rate", "role"],
                ("exchange_of" | "values" | "roles", 1) => &["rate"],
                ("exchange", 3 | 4) => &["rate"],
                _ => return true,
            };
            for (arg, kind) in args.iter().zip(kinds) {
                let Expr::StringConstant(text, pos) = arg else {
                    continue;
                };
                let known = match *kind {
                    "account" => text
                        .parse()
                        .map_err(|e: axionomy::PathError| e.to_string())
                        .and_then(|id| self.account(&id)),
                    "asset" => text
                        .parse()
                        .map_err(|e: axionomy::PathError| e.to_string())
                        .and_then(|id| self.asset(&id)),
                    "role" => text
                        .parse()
                        .map_err(|e: axionomy::PathError| e.to_string())
                        .and_then(|id| declared(&self.roles, kind, &id)),
                    _ => self.rate(text),
                };
                if let Err(reason) = known {
                    let mut d = Diagnostic::new(file, reason);
                    d.line = pos.line();
                    d.column = pos.position();
                    d.field = format!("{} at {}", call.name, pos);
                    error = Some(d);
                    return false;
                }
            }
            true
        });
        error.map_or(Ok(()), Err)
    }
}
