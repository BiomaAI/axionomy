"""Negative controls for the production-source-to-Lean connection."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from extract_quantity import GENERATED, LEAN, ROOT, TEMPLATE, WORK, compare_generated, validate_generated


class ExtractionBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-extraction-test-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        for generated, relative in GENERATED.items():
            shutil.copyfile(LEAN / relative, self.directory / generated)
        # A controlled schema isolates the boundary checker from downloads.
        template = b"test intrinsic interface\n"
        (self.directory / TEMPLATE).write_bytes(template)
        patched = patch("extract_quantity.TEMPLATE_SHA256", hashlib.sha256(template).hexdigest())
        patched.start()
        self.addCleanup(patched.stop)

    def test_current_generated_code_passes(self):
        validate_generated(self.directory)

    def test_external_interface_drift_is_rejected(self):
        (self.directory / TEMPLATE).write_text("another external function")
        with self.assertRaisesRegex(ValueError, "external intrinsic boundary changed"):
            validate_generated(self.directory)

    def test_unsupported_intrinsic_type_is_rejected(self):
        functions = self.directory / "Funs.lean"
        functions.write_text(functions.read_text().replace("core.marker.CopyU64", "core.marker.CopyU32", 1))
        with self.assertRaisesRegex(ValueError, "unsupported add_with_overflow"):
            validate_generated(self.directory)

    def test_axiomatic_generated_code_is_rejected(self):
        types = self.directory / "Types.lean"
        types.write_text(types.read_text() + "\naxiom fabricated : False\n")
        with self.assertRaisesRegex(ValueError, "axiomatic generated code"):
            validate_generated(self.directory)

    def test_stale_generated_model_is_rejected(self):
        model = self.directory / "committed"
        compare_generated(self.directory, model, write=True)
        (model / "QuantityExtraction/Funs.lean").write_text("-- stale model\n")
        with self.assertRaisesRegex(ValueError, "stale generated model"):
            compare_generated(self.directory, model)


class ProductionMutationTests(unittest.TestCase):
    def test_discarding_proposal_bindings_breaks_public_data(self):
        self.check_mutation(
            "src/exchange.rs", "self.bindings.insert(role, account);",
            "self.bindings.insert(role, account); self.bindings = BTreeMap::new();",
            "extract_account.py", "stale account model", "TypedDataRefinement", "AccountExtraction")

    def test_wrong_delta_accessor_breaks_public_data(self):
        self.check_mutation(
            "src/exchange.rs", "pub fn preserved(&self) -> &Basket<A, N> {\n        &self.preserved",
            "pub fn preserved(&self) -> &Basket<A, N> {\n        &self.produced",
            "extract_account.py", "stale account model", "TypedDataRefinement", "AccountExtraction",
            proof_failure="Application type mismatch")

    def test_swapped_division_operands_break_expression_construction(self):
        self.check_mutation(
            "src/rate.rs", "numerator: Box::new(numerator),\n            denominator: Box::new(denominator)",
            "numerator: Box::new(denominator),\n            denominator: Box::new(numerator)",
            "extract_account.py", "stale account model", "ExpressionConstructionRefinement", "AccountExtraction")

    def test_missing_goal_account_acceptance_breaks_matching(self):
        self.check_mutation(
            "src/economy.rs", "let Some(holder) = self.accounts.get(account_id) else {\n                return false;",
            "let Some(holder) = self.accounts.get(account_id) else {\n                return true;",
            "extract_account.py", "stale account model", "GoalRefinement", "AccountExtraction")

    def test_reversed_visibility_breaks_observations(self):
        self.check_mutation(
            "src/economy.rs", "if self.visible.contains(id) {", "if !self.visible.contains(id) {",
            "extract_account.py", "stale account model", "ObservationRefinement", "AccountExtraction")

    def test_omitting_empty_accounts_breaks_observation_keys(self):
        self.check_mutation(
            "src/economy.rs", "existing_accounts.push(account_id.clone());",
            "if !holder.balances().is_empty() { existing_accounts.push(account_id.clone()); }",
            "extract_account.py", "stale account model", "KeyRefinement", "AccountExtraction")

    def test_skipping_state_sort_breaks_canonical_keys(self):
        self.check_mutation(
            "src/economy.rs", "key.sort();", "if key.is_empty() { key.sort(); }",
            "extract_account.py", "stale account model", "KeyRefinement", "AccountExtraction")

    def test_wrong_fnv_prime_breaks_fingerprints(self):
        self.check_mutation(
            "src/economy.rs", "wrapping_mul(0x0000_0100_0000_01b3)", "wrapping_mul(0x0000_0100_0000_01b5)",
            "extract_account.py", "stale account model", "FingerprintRefinement", "AccountExtraction")

    def test_replay_ignoring_error_breaks_execution(self):
        self.check_mutation(
            "src/economy.rs", "receipts.push(self.apply(proposal.clone())?);",
            "if let Ok(receipt) = self.apply(proposal.clone()) { receipts.push(receipt); }",
            "extract_account.py", "stale account model", "ExecutionRefinement", "AccountExtraction")

    def test_replay_discarding_receipts_breaks_execution(self):
        self.check_mutation(
            "src/economy.rs", "receipts.push(self.apply(proposal.clone())?);",
            "receipts.push(self.apply(proposal.clone())?); receipts = Vec::new();",
            "extract_account.py", "stale account model", "ExecutionRefinement", "AccountExtraction",
            proof_failure="Type mismatch")

    def test_simulation_applying_twice_breaks_execution(self):
        self.check_mutation(
            "src/economy.rs", "let receipt = fork.apply(proposal)?;",
            "fork.apply(proposal.clone())?; let receipt = fork.apply(proposal)?;",
            "extract_account.py", "stale account model", "ExecutionRefinement", "AccountExtraction")

    def test_reversed_filter_breaks_transaction_filtering(self):
        self.check_mutation(
            "src/economy.rs", "if self.is_applicable(&proposal)", "if !self.is_applicable(&proposal)",
            "extract_account.py", "stale account model", "FilteringRefinement", "AccountExtraction",
            proof_failure="Type mismatch")

    def test_wrong_computed_channel_breaks_aggregation(self):
        self.check_mutation(
            "src/economy.rs", "ComputedDirection::Produce => merge_quantity(&mut self.produce, asset, value)",
            "ComputedDirection::Produce => merge_quantity(&mut self.consume, asset, value)",
            "extract_account.py", "stale account model", "ComputedEffectRefinement", "AccountExtraction")

    def test_wrong_preservation_requirement_breaks_preflight(self):
        self.check_mutation(
            "src/economy.rs", "required.checked_add(&effect.preserve)", "required.checked_add(&effect.produce)",
            "extract_account.py", "stale account model", "AvailabilityRefinement", "AccountExtraction")

    def test_reversed_condition_breaks_transaction_conditions(self):
        self.check_mutation(
            "src/economy.rs", "if !satisfied {", "if satisfied {",
            "extract_account.py", "stale account model", "ConditionRefinement", "AccountExtraction")

    def test_reversed_invariant_diagnostic_breaks_invariant_caller(self):
        self.check_mutation(
            "src/economy.rs", "                    before,\n                    after,",
            "                    before: after,\n                    after: before,",
            "extract_account.py", "stale account model", "InvariantRefinement", "AccountExtraction")

    def test_empty_receipt_breaks_complete_application(self):
        self.check_mutation(
            "src/economy.rs", "Receipt::new(proposal, projected_deltas)", "Receipt::new(proposal, Vec::new())",
            "extract_account.py", "stale account model", "ApplyRefinement", "AccountExtraction")

    def test_wrong_committed_balance_breaks_atomic_commit(self):
        # Replacing the prepared account with an empty one breaks the exact
        # backward update at the proof's typed state-equivalence step.
        self.check_mutation(
            "src/economy.rs", "self.accounts[&account_id] = holder;",
            "self.accounts[&account_id] = Arc::new(Account::new(Basket::new()));",
            "extract_account.py", "stale account model", "CommitRefinement", "AccountExtraction",
            proof_failure="'change' tactic failed")

    def test_reversed_zero_units_breaks_structural_validation(self):
        self.check_mutation(
            "src/economy.rs", "if proposal.units().is_zero()", "if !proposal.units().is_zero()",
            "extract_account.py", "stale account model", "ValidationCompleteRefinement", "AccountExtraction")

    def test_reversed_missing_binding_breaks_structural_validation(self):
        self.check_mutation(
            "src/economy.rs", "if !proposal.bindings().contains_key(role)", "if proposal.bindings().contains_key(role)",
            "extract_account.py", "stale account model", "ValidationRefinement", "AccountExtraction")

    def test_reversed_missing_parameter_breaks_structural_validation(self):
        self.check_mutation(
            "src/economy.rs", "if !proposal.parameters().contains_key(name)", "if proposal.parameters().contains_key(name)",
            "extract_account.py", "stale account model", "ValidationParametersRefinement", "AccountExtraction")

    def test_aliased_distinct_roles_break_structural_validation(self):
        self.check_mutation(
            "src/economy.rs", "&& left_account == right_account", "&& !(left_account == right_account)",
            "extract_account.py", "stale account model", "ValidationRefinement", "AccountExtraction")

    def test_reversed_missing_account_breaks_structural_validation(self):
        self.check_mutation(
            "src/economy.rs", "&& !self.accounts.contains_key(holder)", "&& self.accounts.contains_key(holder)",
            "extract_account.py", "stale account model", "ValidationAccountsRefinement", "AccountExtraction")

    def test_reversed_duplicate_check_breaks_model_construction(self):
        self.check_mutation(
            "src/economy.rs", "if self.accounts.contains_key(&id)", "if !self.accounts.contains_key(&id)",
            "extract_account.py", "stale account model", "ModelConstructionRefinement", "AccountExtraction")

    def test_accepting_duplicate_models_breaks_model_build(self):
        self.check_mutation(
            "src/economy.rs", "if !self.issues.is_empty()", "if self.issues.is_empty()",
            "extract_account.py", "stale account model", "ModelConstructionRefinement", "AccountExtraction")

    def test_missing_computed_roles_breaks_metadata_construction(self):
        original = "self.register_expression_roles(&role, &quantity);\n        self.computed_consume.push"
        replacement = "self.roles.insert(role.clone());\n        self.computed_consume.push"
        self.check_mutation(
            "src/rate.rs", original, replacement,
            "extract_account.py", "stale account model", "RateMetadataRefinement", "AccountExtraction")

    def test_reversed_distinct_pair_breaks_metadata_construction(self):
        self.check_mutation(
            "src/rate.rs", "if left <= right", "if left >= right",
            "extract_account.py", "stale account model", "RateMetadataRefinement", "AccountExtraction",
            # The selected ge method cannot use the proved le law; simplification
            # stops at the semantic branch comparison after generated code compiles.
            proof_failure="`simp` made no progress")

    def test_missing_preserve_parameters_breaks_rate_discovery(self):
        self.check_mutation(
            "src/rate.rs", "ComputedAmount::collect_parameters(&self.computed_preserve, &mut parameters);",
            "ComputedAmount::collect_parameters(&self.computed_consume, &mut parameters);",
            "extract_account.py", "stale account model", "RateDiscoveryRefinement", "AccountExtraction")

    def test_reversed_condition_asset_order_breaks_rate_discovery(self):
        original = "condition.left.collect_assets(&mut assets);\n            condition.right.collect_assets(&mut assets);"
        replacement = "condition.right.collect_assets(&mut assets);\n            condition.left.collect_assets(&mut assets);"
        self.check_mutation(
            "src/rate.rs", original, replacement,
            "extract_account.py", "stale account model", "RateDiscoveryRefinement", "AccountExtraction")

    def test_reversed_expression_error_order_breaks_the_evaluator_proof(self):
        original = """let left = evaluate_expression(left, proposal, accounts)?;
    let right = evaluate_expression(right, proposal, accounts)?;
    operation(&left, &right).ok_or(error)"""
        replacement = """let right = evaluate_expression(right, proposal, accounts)?;
    let left = evaluate_expression(left, proposal, accounts)?;
    operation(&left, &right).ok_or(error)"""
        self.check_mutation(
            "src/economy.rs", original, replacement,
            "extract_account.py", "stale account model", "EvaluationRefinement", "AccountExtraction",
            proof_failure="⊢ encodeError")

    def test_strict_condition_breaks_the_condition_proof(self):
        self.check_mutation(
            "src/economy.rs", "QuantityComparison::LessThanOrEqual => left <= right",
            "QuantityComparison::LessThanOrEqual => left < right",
            "extract_account.py", "stale account model", "EvaluationRefinement", "AccountExtraction",
            proof_failure="omega could not prove the goal")

    def test_reset_measurement_total_breaks_the_measurement_proof(self):
        self.check_mutation(
            "src/rate.rs", "self.checked_add_basket_measure(total, holder.as_ref().balances())?",
            "self.checked_add_basket_measure(N::SignedMeasure::zero(), holder.as_ref().balances())?",
            "extract_account.py", "stale account model", "ConservationRefinement", "AccountExtraction")

    def test_changed_error_text_breaks_the_diagnostic_proof(self):
        self.check_mutation(
            "src/economy.rs", 'String::from("missing parameter `")', 'String::from("unknown parameter `")',
            "extract_account.py", "stale account model", "DiagnosticRefinement", "AccountExtraction")

    def test_unscaled_effect_merge_breaks_the_effects_proof(self):
        self.check_mutation(
            "src/economy.rs", "target.checked_add(&scaled)", "target.checked_add(source)",
            "extract_account.py", "stale account model", "EffectsRefinement", "AccountExtraction")

    def test_reversed_conservation_breaks_the_runtime_proof(self):
        self.check_mutation(
            "src/rate.rs", "Some(consumed == produced)", "Some(!(consumed == produced))",
            "extract_account.py", "stale account model", "ConservationRefinement", "AccountExtraction")

    def test_ignored_rate_overflow_breaks_the_builder_proof(self):
        original = "target.entry(role).or_default().checked_add(&assets)"
        self.check_mutation(
            "src/rate.rs", original, "let _ = " + original + ";\n    Ok(())",
            "extract_account.py", "stale account model", "RateConstructionRefinement", "AccountExtraction")

    def test_partial_basket_addition_breaks_the_basket_operations_proof(self):
        original = """let Some(amount) = updated.quantity(asset).checked_add(amount) else {
                return Err(asset.clone());"""
        self.check_mutation(
            "src/basket.rs", original, original.replace("return Err", "*self = updated;\n                return Err"),
            "extract_account.py", "stale account model", "BasketOperationsRefinement", "AccountExtraction")

    def test_ignoring_scale_units_breaks_the_basket_operations_proof(self):
        self.check_mutation(
            "src/basket.rs", "amount.checked_mul(units)", "amount.checked_mul(amount)",
            "extract_account.py", "stale account model", "BasketOperationsRefinement", "AccountExtraction")

    def test_producing_before_consuming_breaks_the_preparation_proof(self):
        self.check_mutation(
            "src/account.rs", "updated.withdraw(consumed)?;\n        updated.deposit(produced)?;",
            "updated.deposit(produced)?;\n        updated.withdraw(consumed)?;",
            "extract_account.py", "stale account model", "AccountRefinement")

    def test_partial_deposit_commit_breaks_the_account_proof(self):
        original = "return Err(AccountError::Overflow {"
        self.check_mutation(
            "src/account.rs", original, "self.balances = updated;\n                " + original,
            "extract_account.py", "stale account model", "AccountRefinement")

    def test_reversed_withdrawal_check_breaks_the_account_proof(self):
        self.check_mutation(
            "src/account.rs", "if !shortfall.is_empty() {", "if shortfall.is_empty() {",
            "extract_account.py", "stale account model", "AccountRefinement")

    def test_reversing_rust_subtraction_breaks_the_refinement_proof(self):
        self.check_mutation(
            "src/quantity.rs", "Self::from_checked(self.0.checked_sub(&rhs.0))",
            "Self::from_checked(rhs.0.checked_sub(&self.0))", "extract_quantity.py",
            "stale generated model", "QuantityRefinement")

    def test_zero_filtering_before_validation_breaks_the_basket_proof(self):
        original = """for (asset, quantity) in entries {
        if quantities.contains_key(&asset) {
            return Err(BasketError::DuplicateAsset { asset });
        }
        quantities.insert(asset, quantity);
    }"""
        # Keep the new local call from shadowing the extracted `quantity`
        # module; this mutation must reach the proof, not a name-resolution error.
        replacement = original.replace("quantity", "value").replace(
            "in entries {", "in entries {\n        if value.scalar_is_zero() { continue; }")
        self.check_mutation(
            "src/basket.rs", original, replacement,
            "extract_basket.py", "stale basket model", "BasketRefinement")

    def check_mutation(self, source_path, original, replacement, script, stale_message, module, extraction=None,
                       proof_failure="unsolved goals"):
        # Copy only reviewed/unignored project files into an isolated checkout.
        # Never edit the user's working tree. Tools and dependency caches are
        # shared; proof artifacts and the mutated Rust source remain isolated.
        WORK.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="mutation-", dir=WORK) as scratch:
            root = Path(scratch)
            paths = subprocess.check_output(
                ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
            ).decode().split("\0")
            for relative in filter(None, paths):
                source = ROOT / relative
                if source.is_file():
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, target)
            (root / "verification/.tools").symlink_to(ROOT / "verification/.tools", target_is_directory=True)
            # Keep a separate work directory while reusing only Cargo dependencies.
            (root / "verification/.work").mkdir()
            (root / "verification/.work/cargo-target").symlink_to(WORK / "cargo-target", target_is_directory=True)
            lake = root / "verification/lean/.lake"
            lake.mkdir()
            (lake / "packages").symlink_to(LEAN / ".lake/packages", target_is_directory=True)

            target = root / source_path
            source = target.read_text()
            self.assertEqual(source.count(original), 1)
            target.write_text(source.replace(original, replacement))
            environment = os.environ.copy()
            environment["LEAN_NUM_THREADS"] = "2"

            def run(*command, cwd=root):
                return subprocess.run(command, cwd=cwd, env=environment, text=True,
                                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300)

            stale = run(sys.executable, "scripts/" + script)
            self.assertNotEqual(stale.returncode, 0, stale.stdout)
            self.assertIn(stale_message, stale.stdout)
            regenerate = run(sys.executable, "scripts/" + script, "--write")
            self.assertEqual(regenerate.returncode, 0, regenerate.stdout)
            generated = run("lake", "build", extraction or module.replace("Refinement", "Extraction"),
                            cwd=root / "verification/lean")
            self.assertEqual(generated.returncode, 0, generated.stdout)
            proof = run("lake", "build", "Axionomy." + module, cwd=root / "verification/lean")
            self.assertNotEqual(proof.returncode, 0, proof.stdout)
            self.assertIn(module + ".lean", proof.stdout)
            # Require the reviewed semantic proof failure, after the regenerated
            # implementation itself has compiled. Unknown declarations or model
            # compilation failures do not satisfy this control.
            self.assertIn(proof_failure, proof.stdout)


if __name__ == "__main__":
    unittest.main()
