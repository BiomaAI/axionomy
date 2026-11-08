"""Negative controls for the account's shared-source and storage boundaries."""
import hashlib
import re
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

from extract_account import (GENERATED, LEAN, ROOT, TEMPLATES, compare_generated,
                            validate_bridge, validate_generated, validate_fingerprint_target,
                            FILTERED_TRAIT_WARNING, FILTERED_HASH_WARNING,
                            INSPECTION_DEFINITIONS, CONVENIENCE_DEFINITIONS,
                            DERIVED_DEFINITIONS, INVARIANT_INPUT_DEFINITIONS, ENTRY_CALLS,
                            validate_extraction_diagnostics)


class ExtractionDiagnosticsTests(unittest.TestCase):
    def test_only_reviewed_filtered_dictionary_warnings_pass(self):
        for warnings in ((FILTERED_TRAIT_WARNING, FILTERED_HASH_WARNING),
                         (FILTERED_HASH_WARNING, FILTERED_TRAIT_WARNING)):
            validate_extraction_diagnostics("".join("[\x1b[93mWarn\x1b[39m ] " + warning + "\n" for warning in warnings))

    def test_other_diagnostics_and_missing_expected_warning_fail(self):
        iterator = "[Warn ] " + FILTERED_TRAIT_WARNING + "\n"
        hashing = "[Warn ] " + FILTERED_HASH_WARNING + "\n"
        allowed = iterator + hashing
        for output in ("", allowed + "[Error] translation failed\n",
                       allowed + "[Warn ] a different unsupported operation\n",
                       allowed.replace("size_hint", "next"), allowed + allowed,
                       iterator, hashing, allowed + hashing,
                       allowed.replace("hash_slice", "hash")):
            with self.subTest(output=output), self.assertRaisesRegex(ValueError, "unexpected account extraction diagnostics"):
                validate_extraction_diagnostics(output)


class FingerprintTargetTests(unittest.TestCase):
    def test_reviewed_target_passes(self):
        validate_fingerprint_target('target_pointer_width="64"\ntarget_endian="little"\n')

    def test_other_integer_hash_formats_are_rejected(self):
        for cfg in ['target_pointer_width="32"\ntarget_endian="little"',
                    'target_pointer_width="64"\ntarget_endian="big"', '']:
            with self.subTest(cfg=cfg), self.assertRaisesRegex(ValueError, "64-bit little-endian"):
                validate_fingerprint_target(cfg)


class AccountBridgeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-account-bridge-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = self.root / "verification/extract_account.rs"
        self.target.parent.mkdir(parents=True)
        shutil.copyfile(ROOT / "verification/extract_account.rs", self.target)
        (self.root / "src").mkdir()
        shutil.copyfile(ROOT / "src/economy.rs", self.root / "src/economy.rs")
        shutil.copyfile(ROOT / "src/basket.rs", self.root / "src/basket.rs")
        shutil.copyfile(ROOT / "src/rate.rs", self.root / "src/rate.rs")

    def test_invariant_input_handoff_cannot_be_bypassed(self):
        path = self.root / "src/rate.rs"
        path.write_text(path.read_text().replace(
            "Self::from_serialized_weights(data.name, data.weights)", "Ok(Self::new(data.name))"))
        with self.assertRaisesRegex(ValueError, "invariant deserialization handoff changed"):
            validate_bridge(self.root)

    def test_invariant_input_forwarder_cannot_discard_entries(self):
        self.target.write_text(self.target.read_text().replace(
            "LinearInvariant::from_serialized_weights(name, entries)",
            "LinearInvariant::from_serialized_weights(name, Vec::new())"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: invariant_from_serialized_weights"):
            validate_bridge(self.root)

    def test_changed_retention_adapter_is_rejected(self):
        source = self.root / "src/basket.rs"
        source.write_text(source.read_text().replace("quantities.retain(|_, quantity| keep(quantity));", "quantities.clear();"))
        with self.assertRaisesRegex(ValueError, "basket retention adapter changed"):
            validate_bridge(self.root)

    def test_changed_basket_constructor_entries_are_rejected(self):
        original = self.target.read_text()
        for name, call in (("basket_try_entries", "Basket::try_from_entries(entries)"),
                           ("basket_try_amounts", "Basket::try_from_amounts(amounts)"),
                           ("basket_collect", "Basket::from_iter(entries)"),
                           ("basket_from_array", "Basket::from(entries)")):
            with self.subTest(entry=name):
                self.target.write_text(original.replace(call, "bypassed()"))
                with self.assertRaisesRegex(ValueError, "not a forwarding call: " + name):
                    validate_bridge(self.root)

    def test_changed_sorted_basket_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("state.iter_sorted()", "state.iter()"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: basket_sorted_entries"):
            validate_bridge(self.root)

    def test_production_sources_pass(self):
        validate_bridge(self.root)

    def test_changed_inherited_forwarders_are_rejected(self):
        original = self.target.read_text()
        entries = [name for name in ENTRY_CALLS if name.startswith("inherited_")]
        self.assertEqual(len(entries), 60)
        for name in entries:
            with self.subTest(entry=name):
                pattern = r"(pub fn " + re.escape(name) + r"\b.*?\{)(.*?)(\n\})"
                changed, count = re.subn(pattern, r"\1 bypassed()\3", original, flags=re.DOTALL)
                self.assertEqual(count, 1)
                self.target.write_text(changed)
                with self.assertRaisesRegex(ValueError, "not a forwarding call: " + name):
                    validate_bridge(self.root)

    def test_changed_derived_forwarders_are_rejected(self):
        original = self.target.read_text()
        entries = {name: call for name, call in ENTRY_CALLS.items()
                   if name.startswith("derived_") or name == "clone_value"}
        self.assertEqual(len(entries), 65)
        for name, call in entries.items():
            with self.subTest(entry=name):
                pattern = r"(pub fn " + re.escape(name) + r"\b.*?\{)(.*?)(\n\})"
                match = re.search(pattern, original, flags=re.DOTALL)
                self.assertIsNotNone(match)
                self.assertIn(call, re.sub(r"\s+", "", match[2]))
                changed = original[:match.start(2)] + "\n    bypassed()" + original[match.end(2):]
                self.target.write_text(changed)
                with self.assertRaisesRegex(ValueError, "not a forwarding call: " + name):
                    validate_bridge(self.root)
        self.target.write_text(original)

    def test_copied_source_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("../src/account.rs", "proof_only.rs"))
        with self.assertRaisesRegex(ValueError, "exact production source files"):
            validate_bridge(self.root)

    def test_changed_entry_point_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("state.deposit(assets)", "state.withdraw(assets)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: deposit"):
            validate_bridge(self.root)

    def test_changed_trace_extension_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("trace.extend(exchanges)", "trace.push(exchanges)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: trace_extend"):
            validate_bridge(self.root)

    def test_changed_preparation_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace(
            "state.prepare_exchange(consumed, produced)", "state.prepare_exchange(produced, consumed)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: prepare_exchange"):
            validate_bridge(self.root)

    def test_bypassed_economy_handoff_is_rejected(self):
        path = self.root / "src/economy.rs"
        path.write_text(path.read_text().replace(
            ".prepare_exchange(&effect.consume, &effect.produce)", ".clone()"))
        with self.assertRaisesRegex(ValueError, "economy preparation handoff changed"):
            validate_bridge(self.root)

    def test_changed_basket_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("state.checked_scale(units)", "state.clone()"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: basket_scale"):
            validate_bridge(self.root)

    def test_changed_conservation_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("invariant.conserves(deltas)", "Some(true)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: invariant_conserves"):
            validate_bridge(self.root)

    def test_changed_evaluator_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace(
            "economy::evaluate_expression(expression, exchange, accounts)", "Ok(Quantity::new(0))"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: evaluate"):
            validate_bridge(self.root)

    def test_changed_model_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("builder.build()", "Ok(builder)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: model_build"):
            validate_bridge(self.root)

    def test_changed_validation_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("model.validate_structure(proposal)", "Ok(rule)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: validate_model_proposal"):
            validate_bridge(self.root)

    def test_changed_transaction_entries_are_rejected(self):
        original = self.target.read_text()
        for name, call in (("assess_model_proposal", "model.assess(proposal)"),
                           ("apply_model_proposal", "model.apply(proposal)"),
                           ("model_is_applicable", "model.is_applicable(proposal)"),
                           ("model_applicable", "model.applicable(candidates)")):
            with self.subTest(entry=name):
                self.target.write_text(original.replace(call, "bypassed()"))
                with self.assertRaisesRegex(ValueError, "not a forwarding call: " + name):
                    validate_bridge(self.root)

    def test_bypassed_validation_handoff_is_rejected(self):
        path = self.root / "src/economy.rs"
        path.write_text(path.read_text().replace("self.validate_structure(proposal)", "Ok(rule)"))
        with self.assertRaisesRegex(ValueError, "economy validation handoff changed"):
            validate_bridge(self.root)

    def test_changed_discovery_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("rule.parameter_names()", "Default::default()"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: rate_parameters"):
            validate_bridge(self.root)

    def test_changed_rate_entry_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("rule.try_consume(role, assets)", "rule.try_produce(role, assets)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: rate_consume"):
            validate_bridge(self.root)


class AccountExtractionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-account-extraction-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        for generated, relative in GENERATED.items():
            shutil.copyfile(LEAN / relative, self.directory / generated)
        templates = {}
        for name in TEMPLATES:
            content = ("controlled interface " + name).encode()
            (self.directory / name).write_bytes(content)
            templates[name] = hashlib.sha256(content).hexdigest()
        patched = patch("extract_account.TEMPLATES", templates)
        patched.start()
        self.addCleanup(patched.stop)

    def test_current_generated_code_passes(self):
        validate_generated(self.directory)

    def test_missing_inherited_roots_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        entries = [name for name in ENTRY_CALLS if name.startswith("inherited_")]
        self.assertEqual(len(entries), 60)
        for name in entries:
            with self.subTest(name=name):
                changed, count = re.subn(r"(^def\s+)" + re.escape(name) + r"(?=\s)",
                                        r"\1bypassed_inherited", original, flags=re.MULTILINE)
                self.assertEqual(count, 1)
                path.write_text(changed)
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)

    def test_missing_derived_methods_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        self.assertEqual(len(DERIVED_DEFINITIONS), 56)
        for name in DERIVED_DEFINITIONS:
            with self.subTest(name=name):
                changed, count = re.subn(r"(^def\s+)" + re.escape(name) + r"(?=\s)",
                                        r"\1bypassed_derived", original, flags=re.MULTILINE)
                self.assertEqual(count, 1)
                path.write_text(changed)
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)
        path.write_text(original)

    def test_missing_inspection_methods_or_loops_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for name in INSPECTION_DEFINITIONS:
            with self.subTest(name=name):
                # Names may follow a newline after `def` in generated Lean.
                changed, count = re.subn(r"(^def\s+)" + re.escape(name) + r"(?=\s)",
                                        r"\1bypassed_inspection", original, flags=re.MULTILINE)
                self.assertEqual(count, 1)
                path.write_text(changed)
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)
        path.write_text(original)

    def test_missing_convenience_methods_or_conversion_closure_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for name in CONVENIENCE_DEFINITIONS:
            with self.subTest(name=name):
                changed, count = re.subn(r"(^def\s+)" + re.escape(name) + r"(?=\s)",
                                        r"\1bypassed_convenience", original, flags=re.MULTILINE)
                self.assertEqual(count, 1)
                path.write_text(changed)
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)
        path.write_text(original)

    def test_missing_invariant_validation_or_normalization_loop_is_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for name in INVARIANT_INPUT_DEFINITIONS:
            with self.subTest(name=name):
                changed, count = re.subn(r"(^def\s+)" + re.escape(name) + r"(?=\s)",
                                        r"\1bypassed_invariant_input", original, flags=re.MULTILINE)
                self.assertEqual(count, 1)
                path.write_text(changed)
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)
        path.write_text(original)

    def test_missing_key_and_goal_definitions_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for name in ["economy.Goal.asset_keys", "economy.Economy.matches_loop.body",
                     "economy.Economy.observation_key_for", "economy.Economy.state_key",
                     "economy.FingerprintHasher.Insts.CoreHashHasher.write_loop.body"]:
            with self.subTest(name=name):
                path.write_text(original.replace("def " + name + "\n", "def bypass\n"))
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)
        path.write_text(original)

    def test_missing_public_data_definitions_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for definition in ("amount.AssetAmount.new", "exchange.Exchange.bind",
                           "economy.ExchangeAssessment.status", "rate.QuantityExpression.mul_div"):
            with self.subTest(definition=definition):
                needle = "def " + definition + "\n"
                self.assertEqual(original.count(needle), 1)
                path.write_text(original.replace(needle, "def bypass\n"))
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)

    def test_missing_iterator_consumers_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for definition in ("exchange.Trace.extend", "exchange.Trace.extend_loop.body",
                           "economy.Economy.view_loop.body", "economy.Economy.applicable_loop.body"):
            with self.subTest(definition=definition):
                needle = "def " + definition + "\n"
                self.assertEqual(original.count(needle), 1)
                path.write_text(original.replace(needle, "def bypass\n"))
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)

    def test_missing_basket_collection_definitions_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for definition in ("basket.Basket.try_from_entries", "basket.Basket.try_from_amounts",
                           "basket.Basket.Insts.CoreConvertFromArrayPairAQuantityLEN.from",
                           "basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next"):
            with self.subTest(definition=definition):
                needle = "def " + definition + "\n"
                self.assertEqual(original.count(needle), 1)
                path.write_text(original.replace(needle, "def bypass\n"))
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)

    def test_new_external_interface_is_rejected(self):
        (self.directory / "FunsExternal_Template.lean").write_text("new dependency")
        with self.assertRaisesRegex(ValueError, "external model boundary changed"):
            validate_generated(self.directory)

    def test_missing_sorted_basket_definitions_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for definition in ("basket.sorted_entries", "basket.Basket.iter_sorted"):
            with self.subTest(definition=definition):
                needle = "def " + definition + "\n"
                self.assertEqual(original.count(needle), 1)
                path.write_text(original.replace(needle, "def bypass\n"))
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)

    def test_unexpected_generated_file_is_rejected(self):
        (self.directory / "Unexpected.lean").write_text("")
        with self.assertRaisesRegex(ValueError, "unexpected account extraction files"):
            validate_generated(self.directory)

    def test_missing_extracted_model_build_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def economy.EconomyBuilder.build\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_validation_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def economy.Economy.validate_structure\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_transaction_stages_are_rejected(self):
        path = self.directory / "Funs.lean"
        original = path.read_text()
        for definition in ("analyze", "assess", "apply", "applicable", "is_applicable",
                           "aggregate_effects_loop.body", "merge_computed_effects_loop.body",
                           "check_conditions_loop.body", "preflight_effects_loop.body",
                           "prepare_effects_loop.body", "check_invariants_loop.body",
                           "accounts_with_prepared_loop.body", "apply_loop.body", "applicable_loop.body"):
            with self.subTest(definition=definition):
                needle = "def economy.Economy." + definition + "\n"
                self.assertEqual(original.count(needle), 1)
                path.write_text(original.replace(needle, "def bypass\n"))
                with self.assertRaisesRegex(ValueError, "missing account production definition"):
                    validate_generated(self.directory)

    def test_missing_validation_loop_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def economy.Economy.validate_structure_loop5.body\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_discovery_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def rate.Rate.parameter_names\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_withdrawal_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def account.Account.withdraw\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_preparation_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def account.Account.prepare_exchange\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_basket_addition_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def basket.Basket.checked_add\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_conservation_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def rate.LinearInvariant.conserves\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_evaluation_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def economy.evaluate_expression\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_measurement_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def rate.LinearInvariant.measure_loop.body\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_missing_extracted_rate_builder_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def rate.Rate.try_consume\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing account production definition"):
            validate_generated(self.directory)

    def test_axiomatic_generated_code_is_rejected(self):
        path = self.directory / "Types.lean"
        path.write_text(path.read_text() + "\naxiom fabricated : False\n")
        with self.assertRaisesRegex(ValueError, "axiomatic account extraction"):
            validate_generated(self.directory)

    def test_stale_model_is_rejected(self):
        model = self.directory / "committed"
        compare_generated(self.directory, model, write=True)
        (model / "AccountExtraction/Funs.lean").write_text("-- stale\n")
        with self.assertRaisesRegex(ValueError, "stale account model"):
            compare_generated(self.directory, model)


if __name__ == "__main__":
    unittest.main()
