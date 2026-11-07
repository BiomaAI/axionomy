#!/usr/bin/env python3
"""Check coverage, fresh Rust extraction, kernel proofs and Rust conformance.

Source hashes record review freshness. They do NOT establish Rust refinement.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from extract_quantity import check_extraction
from extract_basket import check_extraction as check_basket_extraction
from extract_account import check_extraction as check_account_extraction

ROOT = Path(__file__).resolve().parents[1]


def validate_coverage(root, coverage):
    if coverage.get("version") != 1:
        raise ValueError("unsupported proof coverage version")
    sources = {"Cargo.toml", "Cargo.lock", "verification/extract.rs", "verification/extract_basket.rs",
               "verification/extract_account.rs",
               "verification/patches/aeneas-namespaced-discriminant.patch",
               "verification/aeneas-toolchain.json"} | {
                   str(p.relative_to(root)) for p in (root / "src").rglob("*.rs")}
    recorded = coverage["sources"]
    if sources != set(recorded):
        raise ValueError(f"core source coverage differs: {sorted(sources ^ set(recorded))}")
    for source, expected in recorded.items():
        actual = hashlib.sha256((root / source).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"{source}: proof coverage review is stale (current SHA-256: {actual}). "
                             "Review the specification, proofs, Rust bridge and limitations before updating coverage.json.")
    features = coverage["features"]
    if not features:
        raise ValueError("no proof-bearing core features registered")
    covered = set()
    ids = set()
    theorems = set()
    for feature in features:
        if feature["id"] in ids:
            raise ValueError(f"duplicate feature: {feature['id']}")
        ids.add(feature["id"])
        if feature["status"] not in {"design_proved_rust_tested", "aeneas_refinement_proved"}:
            raise ValueError(f"unsupported verification status: {feature['status']}")
        if feature["status"] == "aeneas_refinement_proved":
            # These are the implemented extraction pipelines. Future
            # claims need their own regeneration and trust-boundary checks.
            pipelines = {
                "observations_and_goals": ("AccountExtraction.Funs", {
                    "Axionomy.GoalRefinement.new_goal", "Axionomy.GoalRefinement.default_goal",
                    "Axionomy.GoalRefinement.require_goal", "Axionomy.GoalRefinement.requirements",
                    "Axionomy.GoalRefinement.asset_keys", "Axionomy.GoalRefinement.matches_goal",
                    "Axionomy.ViewConstructionRefinement.model_view", "Axionomy.ObservationRefinement.view_account",
                    "Axionomy.ObservationRefinement.view_balance", "Axionomy.Observations.satisfies_iff",
                    "Axionomy.Observations.goal_monotone", "Axionomy.Observations.noninterference"}),
                "canonical_keys_and_fingerprints": ("AccountExtraction.Funs", {
                    "Axionomy.KeyRefinement.state_key", "Axionomy.KeyRefinement.state_key_contract",
                    "Axionomy.KeyRefinement.observation_key", "Axionomy.KeyRefinement.view_key",
                    "Axionomy.KeyRefinement.view_key_sound", "Axionomy.KeyRefinement.key_visible",
                    "Axionomy.KeyRefinement.key_existing", "Axionomy.KeyRefinement.key_balances",
                    "Axionomy.CanonicalKeys.noninterference", "Axionomy.CanonicalKeys.missing_distinguished",
                    "Axionomy.CanonicalKeys.queries_equal", "Axionomy.CanonicalKeys.reordered",
                    "Axionomy.CanonicalSort.sort_canonical", "Axionomy.CanonicalSort.slice_sort",
                    "Axionomy.KeyOrdering.triples_laws", "Axionomy.FingerprintRefinement.fingerprint_refines",
                    "Axionomy.FingerprintRefinement.run_value", "Axionomy.FingerprintRefinement.number_hash",
                    "Axionomy.FingerprintRefinement.fingerprint_get"}),
                "receipts_and_replay": ("AccountExtraction.Funs", {
                    "Axionomy.ExecutionRefinement.fork_refines",
                    "Axionomy.ExecutionRefinement.simulate_refines",
                    "Axionomy.ExecutionRefinement.replay_refines",
                    "Axionomy.ExecutionRefinement.replayed_refines",
                    "Axionomy.ExecutionRefinement.accepted_replay_contract",
                    "Axionomy.ExecutionRefinement.rejected_replay_prefix",
                    "Axionomy.ExecutionRefinement.replay_loop_refines",
                    "Axionomy.ExecutionRefinement.economy_clone",
                    "Axionomy.ExecutionRefinement.proposal_clone",
                    "Axionomy.TransactionExecution.replay_error_prefix",
                    "Axionomy.TransactionExecution.replay_receipt_order",
                    "Axionomy.TransactionExecution.accepted_changes",
                    "Axionomy.TransactionExecution.apply_conserves",
                    "Axionomy.TransactionExecution.replay_conserves"}),
                "complete_transactions": ("AccountExtraction.Funs", {
                    "Axionomy.AnalysisRefinement.analyze_refines",
                    "Axionomy.AnalysisRefinement.assess_refines",
                    "Axionomy.ApplyRefinement.apply_refines",
                    "Axionomy.ApplyRefinement.accepted_receipt_refines",
                    "Axionomy.Transaction.apply_receipt_changes",
                    "Axionomy.FilteringRefinement.is_applicable_refines",
                    "Axionomy.FilteringRefinement.applicable_refines",
                    "Axionomy.AggregationContract.validated_aggregate",
                    "Axionomy.AggregationContract.ready_preflight",
                    "Axionomy.AggregationContract.ready_prepare",
                    "Axionomy.Transaction.analyze_ready",
                    "Axionomy.OrderedEffects.aggregate_contract",
                    "Axionomy.RateWellFormed.new_registered",
                    "Axionomy.RateWellFormed.consume_registered",
                    "Axionomy.RateWellFormed.produce_registered",
                    "Axionomy.RateWellFormed.preserve_registered",
                    "Axionomy.RateWellFormed.static_consume_registered",
                    "Axionomy.RateWellFormed.static_produce_registered",
                    "Axionomy.RateWellFormed.static_preserve_registered",
                    "Axionomy.RateWellFormed.condition_registered",
                    "Axionomy.RateWellFormed.distinct_registered"}),
                'structural_validation': ("AccountExtraction.Funs", {
                    'Axionomy.ValidationCompleteRefinement.validate_refines',
                    'Axionomy.ValidationCompleteRefinement.accepted_has_accounts',
                    'Axionomy.ValidationCompleteRefinement.accepted_parameters_and_distinct',
                    'Axionomy.ValidationCompleteRefinement.rejected_has_issue'}),
                'model_construction': ("AccountExtraction.Funs", {
                    'Axionomy.ModelConstructionRefinement.new_refines',
                    'Axionomy.ModelConstructionRefinement.default_refines',
                    'Axionomy.ModelConstructionRefinement.account_refines',
                    'Axionomy.ModelConstructionRefinement.rate_refines',
                    'Axionomy.ModelConstructionRefinement.invariant_refines',
                    'Axionomy.ModelConstructionRefinement.new_establishes_valid',
                    'Axionomy.ModelConstructionRefinement.account_preserves_valid',
                    'Axionomy.ModelConstructionRefinement.rate_preserves_valid',
                    'Axionomy.ModelConstructionRefinement.invariant_preserves_valid',
                    'Axionomy.ModelConstructionRefinement.build_refines'}),
                'computed_rate_construction': ("AccountExtraction.Funs", {
                    'Axionomy.RateMetadataRefinement.new_refines',
                    'Axionomy.RateMetadataRefinement.default_refines',
                    'Axionomy.RateMetadataRefinement.consume_refines',
                    'Axionomy.RateMetadataRefinement.produce_refines',
                    'Axionomy.RateMetadataRefinement.preserve_refines',
                    'Axionomy.RateMetadataRefinement.condition_refines',
                    'Axionomy.RateMetadataRefinement.distinct_refines',
                    'Axionomy.RateMetadataRefinement.u64_less_equal_lawful'}),
                'rate_discovery': ("AccountExtraction.Funs", {
                    'Axionomy.RateDiscoveryRefinement.parameters_refines',
                    'Axionomy.RateDiscoveryRefinement.parameters_complete',
                    'Axionomy.RateDiscoveryRefinement.assets_refines'}),
                "snapshot_evaluation": ("AccountExtraction.Funs", {
                    "Axionomy.EvaluationRefinement.evaluate_refines_naturals",
                    "Axionomy.EvaluationRefinement.condition_refines_naturals",
                    "Axionomy.DiagnosticRefinement.message_refines"}),
                "bounded_effect_helpers": ("AccountExtraction.Funs", {
                    "Axionomy.EffectsRefinement.scaled_refines_naturals",
                    "Axionomy.EffectsRefinement.amount_refines_naturals"}),
                "static_rate_construction": ("AccountExtraction.Funs", {
                    f"Axionomy.RateConstructionRefinement.{kind}_{contract}"
                    for kind in ("consume", "produce", "preserve")
                    for contract in ("refines", "registers_and_frames")}),
                "runtime_linear_conservation": ("AccountExtraction.Funs", {
                    "Axionomy.ConservationRefinement.weighted_exact",
                    "Axionomy.ConservationRefinement.signed_add_exact",
                    "Axionomy.ConservationRefinement.conserves_refines_integers",
                    "Axionomy.ConservationRefinement.accepted_conserves",
                    "Axionomy.ConservationRefinement.measure_refines_integers"}),
                "u64_checked_arithmetic": ("QuantityExtraction.Funs", {
                    f"Axionomy.QuantityRefinement.checked_{op}_refines"
                    for op in ("add", "sub", "mul", "div")}),
                "basket_construction": ("BasketExtraction.Funs", {
                    "Axionomy.BasketRefinement.construct_refines",
                    "Axionomy.BasketRefinement.duplicate_rejected",
                    "Axionomy.BasketRefinement.exactEquality_lawful"}),
                "account_updates": ("AccountExtraction.Funs", {
                    "Axionomy.AccountRefinement.deposit_refines_naturals",
                    "Axionomy.AccountRefinement.withdraw_refines_naturals",
                    "Axionomy.AccountRefinement.shortfall_refines",
                    "Axionomy.AccountRefinement.exactClone_lawful"}),
                "account_exchange_preparation": ("AccountExtraction.Funs", {
                    "Axionomy.AccountRefinement.prepare_refines_naturals",
                    "Axionomy.Account.prepare_success_iff",
                    "Axionomy.Account.prepare_insufficient",
                    "Axionomy.Account.prepare_late_overflow"}),
                "basket_operations": ("AccountExtraction.Funs", {
                    "Axionomy.BasketOperationsRefinement.add_refines_naturals",
                    "Axionomy.BasketOperationsRefinement.scale_refines_naturals",
                    "Axionomy.BasketOperationsRefinement.contains_refines",
                    "Axionomy.BasketOperationsRefinement.equal_refines",
                    "Axionomy.BasketOperationsRefinement.equal_iff_permutation"}),
            }
            pipeline = pipelines.get(feature["id"])
            if pipeline is None or feature.get("extracted_module") != pipeline[0]:
                raise ValueError("refinement claim has no supported extraction pipeline")
            required = pipeline[1]
            if not required <= set(feature["theorems"]):
                raise ValueError(f"incomplete implementation refinement contract: {feature['id']}")
        if not feature["sources"] or not set(feature["sources"]) <= sources:
            raise ValueError(f"invalid source mapping: {feature['id']}")
        covered.update(feature["sources"])
        if not feature["theorems"] or not feature["rust_tests"] or not feature["limitations"]:
            raise ValueError(f"incomplete proof contract: {feature['id']}")
        for theorem in feature["theorems"]:
            if not re.fullmatch(r"Axionomy\.[A-Za-z0-9_.]+", theorem):
                raise ValueError(f"invalid theorem name: {theorem}")
            theorems.add(theorem)
        for test in feature["rust_tests"]:
            test_file = (root / test).resolve()
            if not test_file.is_relative_to(root.resolve()) or not test_file.is_file():
                raise ValueError(f"missing Rust evidence: {test}")
    if covered != sources:
        raise ValueError(f"core sources without a proof contract: {sorted(sources - covered)}")
    # Every spec module must be imported, so adding an unbuilt file cannot
    # silently evade the kernel audit. Keep the umbrella imports explicit.
    modules = {"Axionomy." + ".".join(p.relative_to(root / "verification/lean/Axionomy").with_suffix("").parts)
               for p in (root / "verification/lean/Axionomy").rglob("*.lean")}
    umbrella = (root / "verification/lean/Axionomy.lean").read_text()
    imports = set(re.findall(r"^import (Axionomy\.[\w.]+)$", umbrella, re.MULTILINE))
    if imports != modules:
        raise ValueError(f"unbuilt or missing proof modules: {sorted(imports ^ modules)}")
    return sorted(theorems)


def run(*command, cwd=ROOT, capture=False):
    print("+ " + " ".join(map(str, command)), flush=True)
    return subprocess.run(command, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def audit_source(theorems):
    return "import Audit\n#audit_axionomy\n" + "".join(
        f"#require_theorem {theorem}\n" for theorem in theorems)


def main():
    coverage = json.loads((ROOT / "verification/coverage.json").read_text())
    theorems = validate_coverage(ROOT, coverage)
    check_extraction()
    check_basket_extraction()
    check_account_extraction()
    lean_root = ROOT / "verification/lean"
    run("lake", "build", cwd=lean_root)
    run("lake", "env", "leanchecker", "Axionomy", cwd=lean_root)
    with tempfile.TemporaryDirectory(prefix="axionomy-proof-audit-") as scratch:
        audit = Path(scratch) / "CoverageAudit.lean"
        audit.write_text(audit_source(theorems))
        run("lake", "env", "lean", str(audit), cwd=lean_root)
    generated = json.loads(run(str(lean_root / ".lake/build/bin/conformance"), capture=True))
    committed = json.loads((ROOT / "verification/conformance.json").read_text())
    if generated != committed:
        raise ValueError("Lean conformance cases are stale; regenerate them from Conformance.lean and review the diff")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--test", "lean_conformance",
        "--test", "transaction_conformance", "--test", "observation_conformance")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--lib",
        "runtime_")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--doc")
    print(f"Verified {len(theorems)} registered theorems and their axiom dependencies, "
          "fresh Aeneas extraction of u64 arithmetic, basket construction/operations, account updates/staging, model/rate construction and metadata discovery, complete transaction assessment/application and vector filtering, fork/simulation/ordered replay, goals/views/canonical keys and scoped fingerprints, snapshot evaluation/conditions, signed conservation and measurement, "
          "and Rust conformance/type checks. Collection storage uses an explicit trusted model. "
          "Other core implementation refinement remains unproved; see verification/README.md.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"proof check failed: {error}", file=sys.stderr)
        sys.exit(1)
