#!/usr/bin/env python3
"""Extract the production account updates and check its storage boundary."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from setup_aeneas import PIN, ROOT, verify_tools
from extract_quantity import LEAN, WORK

GENERATED = {
    "Types.lean": "AccountExtraction/Types.lean",
    "Funs.lean": "AccountExtraction/Funs.lean",
    "AccountExtraction.lean": "AccountExtraction.lean",
}
TEMPLATES = {
    "TypesExternal_Template.lean": "5d799b1ee543006cebe6325df3382239259a305cd75139d39e816fb0122ccd6b",
    "FunsExternal_Template.lean": "171a72da913bb2fdf3c2d1249a607c22b8bcb7883e4cd9cc93c431cc67fd2877",
}

# Keep critical handoffs explicit alongside the complete caller extraction.
# These syntactic guards supplement, rather than replace, the Lean refinements.
PREPARATION_CALL = """let holder = match self
                .accounts
                .get(&account_id)
                .expect("bound accounts were checked")
                .prepare_exchange(&effect.consume, &effect.produce)
            {
                Ok(holder) => holder,
                Err(error) => {
                    return Err(map_account_error(account_id.clone(), error));
                }
            };"""

VALIDATION_CALL = """let rule = match self.validate_structure(proposal) {
            Ok(rule) => rule,
            Err(issues) => return invalid_analysis(issues),
        };"""

ENTRY_CALLS = {
    "amount_new": "AssetAmount::new(asset,value)",
    "amount_asset": "value.asset()",
    "amount_quantity": "value.quantity()",
    "amount_parts": "value.into_parts()",
    "amount_from_pair": "AssetAmount::from(parts)",
    "amount_to_pair": "value.into()",
    "quantity_new": "Quantity::new(value)",
    "quantity_get": "value.get()",
    "quantity_from_u64": "Quantity::from(value)",
    "quantity_from_u32": "Quantity::from(value)",
    "quantity_validate": "Quantity::try_from_scalar(value)",
    "quantity_borrow": "value.as_scalar()",
    "quantity_consume": "value.into_scalar()",
    "account_new": "Account::new(assets)",
    "account_default": "Account::default()",
    "account_from_basket": "Account::from(assets)",
    "account_into_basket": "holder.into_balances()",
    "account_borrow_basket": "holder.balances()",
    "account_equal": "left==right",
    "proposal_new": "Exchange::new(rate_id,units)",
    "proposal_bind": "proposal.bind(role,holder)",
    "proposal_parameter": "proposal.parameter(name,value)",
    "proposal_rate": "proposal.rate()",
    "proposal_bindings": "proposal.bindings()",
    "proposal_units": "proposal.units()",
    "proposal_parameters": "proposal.parameters()",
    "delta_account": "value.account()",
    "delta_consumed": "value.consumed()",
    "delta_produced": "value.produced()",
    "delta_preserved": "value.preserved()",
    "receipt_exchange": "value.exchange()",
    "receipt_deltas": "value.deltas()",
    "account_assessment_account": "value.account()",
    "account_assessment_available": "value.available()",
    "account_assessment_required": "value.required()",
    "account_assessment_consumed": "value.consumed()",
    "account_assessment_produced": "value.produced()",
    "account_assessment_preserved": "value.preserved()",
    "account_shortfall_account": "value.account()",
    "account_shortfall_missing": "value.missing()",
    "assessment_status": "value.status()",
    "assessment_accounts": "value.accounts()",
    "assessment_shortfalls": "value.shortfalls()",
    "assessment_projected_deltas": "value.projected_deltas()",
    "assessment_issues": "value.issues()",
    "computed_role": "value.role()",
    "computed_asset": "value.asset()",
    "computed_quantity": "value.quantity()",
    "condition_name": "value.name()",
    "condition_left": "value.left()",
    "condition_comparison": "value.comparison()",
    "condition_right": "value.right()",
    "model_build_error_issues": "value.issues()",
    "expression_constant": "QuantityExpression::constant(value)",
    "expression_units": "QuantityExpression::units()",
    "expression_balance": "QuantityExpression::balance(role,asset)",
    "expression_parameter": "QuantityExpression::parameter(name)",
    "expression_plus": "QuantityExpression::plus(left,right)",
    "expression_subtract": "QuantityExpression::subtract(left,right)",
    "expression_multiply": "QuantityExpression::multiply(left,right)",
    "expression_divide_floor": "QuantityExpression::divide_floor(left,right)",
    "expression_minimum": "QuantityExpression::minimum(left,right)",
    "expression_maximum": "QuantityExpression::maximum(left,right)",
    "expression_add": "left+right",
    "basket_insert_amount": "state.insert_amount(value)",

    "goal_new": "economy::Goal::new()",
    "goal_default": "economy::Goal::default()",
    "goal_require": "goal.require(id,assets)",
    "goal_requirements": "goal.requirements()",
    "goal_assets": "goal.asset_keys()",
    "inspect_model_account": "model.account(id)",
    "model_balance": "model.balance(id,asset)",
    "model_matches": "model.matches(goal)",
    "model_view": "model.view(visible)",
    "model_state_key": "model.state_key()",
    "model_fingerprint": "model.state_fingerprint()",
    "view_account": "view.account(id)",
    "view_balance": "view.balance(id,asset)",
    "view_key": "view.observation_key()",
    "key_visible": "key.visible_accounts()",
    "key_existing": "key.existing_accounts()",
    "key_balances": "key.balances()",
    "fingerprint_get": "value.get()",
    "model_fork": "model.fork()",
    "model_simulate": "model.simulate(proposal)",
    "model_replay": "model.replay(trace)",
    "model_replayed": "model.replayed(trace)",
    "trace_new": "Trace::new()",
    "trace_default": "Trace::default()",
    "trace_push": "trace.push(proposal)",
    "trace_exchanges": "trace.exchanges()",
    "trace_into_exchanges": "trace.into_exchanges()",
    "model_is_applicable": "model.is_applicable(proposal)",
    "model_applicable": "model.applicable(candidates)",
    "assess_model_proposal": "model.assess(proposal)",
    "apply_model_proposal": "model.apply(proposal)",
    "validate_model_proposal": "model.validate_structure(proposal)",
    "model_new": "economy::EconomyBuilder::new()",
    "model_default": "economy::EconomyBuilder::default()",
    "model_account": "builder.account(id,state)",
    "model_rate": "builder.rate(id,rule)",
    "model_invariant": "builder.invariant(invariant)",
    "model_build": "builder.build()",
    "rate_new": "Rate::new()",
    "rate_default": "Rate::default()",
    "rate_distinct": "rule.distinct(left,right)",
    "rate_computed_consume": "rule.consume_computed(role,asset,expression)",
    "rate_computed_produce": "rule.produce_computed(role,asset,expression)",
    "rate_computed_preserve": "rule.preserve_computed(role,asset,expression)",
    "rate_condition": "rule.condition(label,left,comparison,right)",
    "rate_parameters": "rule.parameter_names()",
    "rate_assets": "rule.computed_asset_keys()",
    "deposit": "state.deposit(assets)",
    "withdraw": "state.withdraw(assets)",
    "prepare_exchange": "state.prepare_exchange(consumed,produced)",
    "basket_add": "state.checked_add(assets)",
    "basket_scale": "state.checked_scale(units)",
    "basket_contains": "state.contains(required)",
    "basket_equal": "left==right",
    "weighted": "value.checked_weighted(coefficient)",
    "expression_message": "error.into_message()",
    "evaluate": "economy::evaluate_expression(expression,exchange,accounts)",
    "evaluate_condition": "economy::evaluate_condition(condition,exchange,accounts)",
    "merge_effects": "economy::merge_scaled(target,source,units)",
    "merge_amount": "economy::merge_quantity(target,asset,amount)",
    "invariant_measure": "invariant.measure(accounts)",
    "invariant_conserves": "invariant.conserves(deltas)",
    "rate_consume": "rule.try_consume(role,assets)",
    "rate_produce": "rule.try_produce(role,assets)",
    "rate_preserve": "rule.try_preserve(role,assets)",
}


PUBLIC_DEFINITIONS = (
    "amount.AssetAmount.new", "amount.AssetAmount.impl.asset", "amount.AssetAmount.impl.quantity",
    "amount.AssetAmount.into_parts", "amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from",
    "PairAQuantity.Insts.CoreConvertFromAssetAmount.from", "basket.Basket.insert_amount",
    "quantity.QuantityU64.new", "quantity.QuantityU64.get", "quantity.Quantity.into_scalar",
    "quantity.QuantityU64.Insts.CoreConvertFromU32.from", "quantity.QuantityU64.Insts.CoreConvertFromU64.from",
    "account.Account.new", "account.Account.into_balances", "account.Account.Insts.CoreDefaultDefault.default",
    "account.Account.Insts.CoreConvertFromBasket.from", "account.Account.Insts.CoreCmpPartialEqAccount.eq",
    "exchange.Exchange.new", "exchange.Exchange.bind", "exchange.Exchange.parameter",
    "exchange.Receipt.exchange", "exchange.Receipt.impl.deltas", "exchange.AccountDelta.impl.account",
    "exchange.AccountDelta.impl.preserved", "economy.ModelBuildError.impl.issues",
    *[f"economy.AccountAssessment.impl.{field}" for field in
      ("account", "available", "required", "consumed", "produced", "preserved")],
    "economy.AccountShortfall.impl.account", "economy.AccountShortfall.impl.missing",
    *[f"economy.ExchangeAssessment.{method}" for method in
      ("status", "accounts", "shortfalls", "projected_deltas", "issues")],
    "rate.QuantityExpressionRoleAU64.constant",
    *[f"rate.QuantityExpression.{method}" for method in
      ("units", "balance", "parameter", "plus", "subtract", "multiply", "divide_floor", "minimum", "maximum")],
    "rate.QuantityExpression.Insts.CoreOpsArithAddQuantityExpressionQuantityExpression.add",
    *[f"rate.ComputedAmount.impl.{field}" for field in ("role", "asset", "quantity")],
    *[f"rate.RateCondition.impl.{field}" for field in ("name", "left", "comparison", "right")],
)


def validate_bridge(root=ROOT):
    """Require the harness to forward to the unchanged production methods."""
    harness = (root / "verification/extract_account.rs").read_text()
    paths = re.findall(r'#\[path\s*=\s*"([^"]+)"\]', harness)
    if sorted(paths) != ["../src/account.rs", "../src/amount.rs", "../src/basket.rs", "../src/economy.rs", "../src/exchange.rs",
                         "../src/quantity.rs", "../src/rate.rs"]:
        raise ValueError("account extraction must include the exact production source files")
    for operation, call in ENTRY_CALLS.items():
        matches = re.findall(r"\bfn " + operation + r"\b[^{}]*\{([^{}]*)\}", harness)
        if len(matches) != 1 or re.sub(r"\s+", "", matches[0]) != call:
            raise ValueError(f"account extraction entry point is not a forwarding call: {operation}")
    economy = (root / "src/economy.rs").read_text()
    if re.sub(r"\s+", "", economy).count(re.sub(r"\s+", "", PREPARATION_CALL)) != 1:
        raise ValueError("economy preparation handoff changed; review the production connection")
    if re.sub(r"\s+", "", economy).count(re.sub(r"\s+", "", VALIDATION_CALL)) != 1:
        raise ValueError("economy validation handoff changed; review the production connection")


def validate_fingerprint_target(cfg):
    if not {'target_pointer_width="64"', 'target_endian="little"'} <= set(cfg.splitlines()):
        raise ValueError("fingerprint proof adapter requires a 64-bit little-endian extraction target")


def validate_generated(directory):
    names = {p.name for p in directory.iterdir()}
    if names != set(GENERATED) | set(TEMPLATES):
        raise ValueError(f"unexpected account extraction files: {sorted(names)}")
    for name, expected in TEMPLATES.items():
        if hashlib.sha256((directory / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"account external model boundary changed: {name}; review its semantics")
    for name in GENERATED:
        if re.search(r"\b(sorry|admit|axiom)\b", (directory / name).read_text()):
            raise ValueError(f"unfinished or axiomatic account extraction: {name}")
    functions = (directory / "Funs.lean").read_text()
    for entry in (*ENTRY_CALLS, *PUBLIC_DEFINITIONS, "account.Account.prepare_exchange",
                  "account.Account.Insts.CoreCloneClone.clone",
                  "account.Account.deposit", "account.Account.withdraw",
                  "account.Account.deposit_loop.body", "account.Account.withdraw_loop.body",
                  "basket.Basket.shortfall", "basket.Basket.shortfall_loop.body",
                  "basket.Basket.insert", "basket.Basket.checked_add", "basket.Basket.checked_add_loop.body",
                  "basket.Basket.checked_scale", "basket.Basket.checked_scale_loop.body", "basket.Basket.contains",
                  "basket.Basket.Insts.CoreCmpPartialEqBasket.eq", "basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body",
                  "quantity.Quantity.checked_add", "quantity.Quantity.checked_sub", "quantity.Quantity.checked_mul",
                  "economy.EconomyBuilder.new", "economy.EconomyBuilder.account", "economy.EconomyBuilder.rate",
                  "economy.EconomyBuilder.invariant", "economy.EconomyBuilder.build", "economy.EconomyBuilder.build_loop.body",
                  "economy.Economy.validate_structure",
                  *[f"economy.Economy.validate_structure_loop{i}.body" for i in range(7)],
                  "economy.Economy.analyze", "economy.Economy.assess", "economy.Economy.apply",
                  "economy.Economy.fork", "economy.Economy.simulate", "economy.Economy.replay",
                  "economy.Economy.replay_loop.body", "economy.Economy.replayed",
                  "economy.Economy.Insts.CoreCloneClone.clone", "exchange.Exchange.Insts.CoreCloneClone.clone",
                  "exchange.Trace.new", "exchange.Trace.push", "exchange.Trace.impl.exchanges", "exchange.Trace.into_exchanges",
                  "economy.Goal.new", "economy.Goal.require", "economy.Goal.requirements",
                  "economy.Goal.collect_assets_loop.body", "economy.Goal.asset_keys_loop.body", "economy.Goal.asset_keys",
                  "economy.Economy.account", "economy.Economy.balance", "economy.Economy.matches", "economy.Economy.matches_loop.body",
                  "economy.Economy.view", "economy.Economy.view_loop.body", "economy.Economy.state_key",
                  "economy.Economy.state_key_loop0.body", "economy.Economy.state_key_loop0_loop0.body",
                  "economy.Economy.observation_key_for", "economy.Economy.observation_key_for_loop0.body",
                  "economy.Economy.observation_key_for_loop0_loop0.body", "economy.EconomicView.account",
                  "economy.EconomicView.balance", "economy.EconomicView.observation_key",
                  "economy.ObservationKey.impl.visible_accounts", "economy.ObservationKey.impl.existing_accounts",
                  "economy.ObservationKey.impl.balances", "economy.Economy.state_fingerprint", "economy.StateFingerprint.get",
                  "economy.FingerprintHasher.new", "economy.FingerprintHasher.Insts.CoreHashHasher.write",
                  "economy.FingerprintHasher.Insts.CoreHashHasher.write_loop.body", "economy.FingerprintHasher.Insts.CoreHashHasher.finish",
                  "economy.Economy.is_applicable", "economy.Economy.applicable", "economy.ExchangeAssessment.is_applicable",
                  "exchange.Receipt.new", "economy.Effect.merge_computed", "economy.relevant_balances",
                  *[f"economy.Economy.{stage}{suffix}" for stage in (
                      "merge_computed_effects", "aggregate_effects", "check_conditions", "preflight_effects",
                      "prepare_effects", "accounts_with_prepared", "check_invariants", "apply", "applicable")
                    for suffix in ("_loop", "_loop.body")],
                  *[f"economy.Economy.{stage}" for stage in (
                      "merge_computed_effects", "aggregate_effects", "check_conditions", "preflight_effects",
                      "prepare_effects", "accounts_with_prepared", "check_invariants")],
                  "rate.Rate.has_role", "rate.Rate.has_role_loop.body",
                  "rate.Rate.new", "rate.Rate.impl.distinct", "rate.Rate.register_expression_roles_loop.body",
                  "rate.Rate.consume_computed", "rate.Rate.produce_computed", "rate.Rate.preserve_computed",
                  "rate.Rate.condition", "rate.Rate.condition_loop.body",
                  "rate.QuantityExpression.collect_roles", "rate.QuantityExpression.collect_parameters", "rate.QuantityExpression.collect_assets",
                  "rate.ComputedAmount.collect_parameters_loop.body", "rate.ComputedAmount.collect_assets_loop.body",
                  "rate.Rate.parameter_names", "rate.Rate.parameter_names_loop.body",
                  "rate.Rate.computed_asset_keys", "rate.Rate.computed_asset_keys_loop.body",
                  "rate.merge", "rate.Rate.try_consume", "rate.Rate.try_produce", "rate.Rate.try_preserve",
                  "rate.LinearInvariant.checked_add_basket_measure_loop.body",
                  "rate.LinearInvariant.conserves_loop.body", "rate.LinearInvariant.conserves",
                  "rate.LinearInvariant.measure_loop.body", "rate.LinearInvariant.measure",
                  "economy.evaluate_expression", "economy.evaluate_binary", "economy.evaluate_condition",
                  "economy.merge_scaled", "economy.merge_quantity", "economy.ExpressionError.into_message",
                  "U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_weighted"):
        if not re.search(r"^def\s+" + re.escape(entry) + r"\s", functions, re.MULTILINE):
            raise ValueError(f"missing account production definition: {entry}")


def compare_generated(directory, lean=LEAN, write=False):
    for name, relative in GENERATED.items():
        actual = (directory / name).read_bytes()
        target = lean / relative
        if write:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(actual)
        elif not target.is_file() or actual != target.read_bytes():
            raise ValueError(f"stale account model: {relative}; run python3 scripts/extract_account.py --write and review/reprove")


def check_extraction(write=False):
    validate_bridge()
    aeneas, charon = verify_tools()
    WORK.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(WORK / "cargo-target")
    environment["CARGO_INCREMENTAL"] = "0"
    toolchain = "+" + PIN["charon"]["rust_toolchain"]
    rustc = subprocess.check_output(["rustc", toolchain, "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    # The reviewed integer/length-prefix Hash adapters model native 64-bit
    # little-endian writes. Make the extraction target explicit and fail closed.
    environment["CARGO_BUILD_TARGET"] = host
    cfg = subprocess.check_output(["rustc", toolchain, "--print", "cfg", "--target", host], text=True)
    validate_fingerprint_target(cfg)
    subprocess.run(["cargo", toolchain, "clean", "--target", host, "-p", "axionomy"],
                   cwd=ROOT, env=environment, check=True)
    with tempfile.TemporaryDirectory(prefix="account-extraction-", dir=WORK) as scratch:
        scratch = Path(scratch)
        llbc = scratch / "account_extraction.llbc"
        generated = scratch / "generated"
        generated.mkdir()
        command = [str(charon), "cargo", "--preset", "aeneas", "--monomorphize-mut",
                   "--resugar-drops", "--rustc-arg=-Zinline-mir=no",
                   *[arg for entry in ENTRY_CALLS for arg in
                     ("--start-from-if-exists", "account_extraction::" + entry)],
                   "--mir", "promoted", "--sysroot", "default",
                   "--include", "account_extraction::account", "--include", "account_extraction::basket",
                   "--include", "account_extraction::quantity", "--include", "account_extraction::amount",
                   "--include", "account_extraction::rate", "--include", "account_extraction::exchange",
                   "--include", "account_extraction::economy",
                   "--include", "num_traits", "--opaque", "indexmap",
                   "--dest-file", str(llbc), "--", "--locked", "-p", "axionomy",
                   "--example", "account_extraction", "--no-default-features"]
        print("Extracting the production account updates...", flush=True)
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
        if not llbc.is_file():
            raise ValueError("Charon did not emit fresh account LLBC")
        subprocess.run([str(aeneas), "-backend", "lean", "-namespace", "Axionomy.AccountExtracted",
                        "-dest", str(generated), "-use-lean-modules", "false", "-split-files",
                        "-gen-lib-entry", "-warnings-as-errors", "-no-progress-bar", str(llbc)],
                       cwd=ROOT, check=True)
        validate_generated(generated)
        compare_generated(generated, write=write)
    print("Account extraction updated; review and recheck proofs." if write else
          "Production account extraction matches the committed model.", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    try:
        check_extraction(write=args.write)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Account extraction failed: {error}", file=sys.stderr)
        sys.exit(1)
