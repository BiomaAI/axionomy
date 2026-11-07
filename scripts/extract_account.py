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
    "TypesExternal_Template.lean": "6c1bb6c6058d25191551c6400ddf411958e9dd7963a89e708c520fc427c0f6dc",
    "FunsExternal_Template.lean": "db33da6731c63ac3fd32590429be62ceeb06dead894b270a574cac2a2a95482f",
}

# The caller remains outside the extraction. Keep its exact staging handoff
# reviewable and reject bypasses; this is not a proof of Economy::analyze.
PREPARATION_CALL = """let account = match self
                .accounts
                .get(&account_id)
                .expect("bound accounts were checked")
                .prepare_exchange(&effect.consume, &effect.produce)
            {
                Ok(account) => account,
                Err(error) => {
                    return invalid_analysis(vec![map_account_error(account_id.clone(), error)]);
                }
            };"""

VALIDATION_CALL = """let rate = match self.validate_structure(exchange) {
            Ok(rate) => rate,
            Err(issues) => return invalid_analysis(issues),
        };"""

ENTRY_CALLS = {
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
    for entry in (*ENTRY_CALLS, "account.Account.prepare_exchange",
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
        if not re.search(r"^def " + re.escape(entry) + r"\s", functions, re.MULTILINE):
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
                   "--include", "account_extraction::quantity",
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
