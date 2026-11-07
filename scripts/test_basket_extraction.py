"""Negative controls for the basket's shared-source and storage boundaries."""
import hashlib
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

from extract_basket import (GENERATED, LEAN, ROOT, TEMPLATES, compare_generated,
                            validate_bridge, validate_generated)


class BasketBridgeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-basket-bridge-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("src/basket.rs", "verification/extract_basket.rs"):
            target = self.root / name
            target.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / name, target)

    def replace(self, file, before, after):
        target = self.root / file
        source = target.read_text()
        self.assertEqual(source.count(before), 1)
        target.write_text(source.replace(before, after))

    def test_current_production_bridge_passes(self):
        validate_bridge(self.root)

    def test_public_constructor_cannot_bypass_extracted_algorithm(self):
        self.replace("src/basket.rs", "        construct_entries(entries)", "        Ok(Self::new())")
        with self.assertRaisesRegex(ValueError, "source bridge changed: try_from_entries"):
            validate_bridge(self.root)

    def test_storage_adapter_change_requires_review(self):
        self.replace("src/basket.rs", "keep(quantity)", "!keep(quantity)")
        with self.assertRaisesRegex(ValueError, "source bridge changed: retain_quantities"):
            validate_bridge(self.root)

    def test_copied_proof_only_source_is_rejected(self):
        self.replace("verification/extract_basket.rs", "../src/basket.rs", "proof_only_basket.rs")
        with self.assertRaisesRegex(ValueError, "exact production source files"):
            validate_bridge(self.root)

    def test_harness_cannot_filter_inputs(self):
        self.replace("verification/extract_basket.rs", "basket::construct_entries(entries)",
                     "basket::construct_entries(entries.into_iter().filter(|_| false))")
        with self.assertRaisesRegex(ValueError, "not a forwarding call"):
            validate_bridge(self.root)


class BasketExtractionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-basket-extraction-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        for generated, relative in GENERATED.items():
            shutil.copyfile(LEAN / relative, self.directory / generated)
        templates = {}
        for name in TEMPLATES:
            content = ("controlled interface " + name).encode()
            (self.directory / name).write_bytes(content)
            templates[name] = hashlib.sha256(content).hexdigest()
        patched = patch("extract_basket.TEMPLATES", templates)
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
        with self.assertRaisesRegex(ValueError, "unexpected basket extraction files"):
            validate_generated(self.directory)

    def test_missing_extracted_constructor_is_rejected(self):
        path = self.directory / "Funs.lean"
        path.write_text(path.read_text().replace("def basket.construct_entries\n", "def bypass\n"))
        with self.assertRaisesRegex(ValueError, "missing basket production definition"):
            validate_generated(self.directory)

    def test_axiomatic_generated_code_is_rejected(self):
        path = self.directory / "Types.lean"
        path.write_text(path.read_text() + "\naxiom fabricated : False\n")
        with self.assertRaisesRegex(ValueError, "axiomatic basket extraction"):
            validate_generated(self.directory)

    def test_stale_model_is_rejected(self):
        model = self.directory / "committed"
        compare_generated(self.directory, model, write=True)
        (model / "BasketExtraction/Funs.lean").write_text("-- stale\n")
        with self.assertRaisesRegex(ValueError, "stale basket model"):
            compare_generated(self.directory, model)


if __name__ == "__main__":
    unittest.main()
