"""Negative controls for the account's shared-source and storage boundaries."""
import hashlib
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

from extract_account import (GENERATED, LEAN, ROOT, TEMPLATES, compare_generated,
                            validate_bridge, validate_generated)


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

    def test_production_sources_pass(self):
        validate_bridge(self.root)

    def test_copied_source_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("../src/account.rs", "proof_only.rs"))
        with self.assertRaisesRegex(ValueError, "exact production source files"):
            validate_bridge(self.root)

    def test_changed_entry_point_is_rejected(self):
        self.target.write_text(self.target.read_text().replace("state.deposit(assets)", "state.withdraw(assets)"))
        with self.assertRaisesRegex(ValueError, "not a forwarding call: deposit"):
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

    def test_bypassed_validation_handoff_is_rejected(self):
        path = self.root / "src/economy.rs"
        path.write_text(path.read_text().replace("self.validate_structure(exchange)", "Ok(rule)"))
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

    def test_new_external_interface_is_rejected(self):
        (self.directory / "FunsExternal_Template.lean").write_text("new dependency")
        with self.assertRaisesRegex(ValueError, "external model boundary changed"):
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
