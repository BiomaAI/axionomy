"""Negative controls for the proof gate: stale or fabricated evidence must fail."""
import copy
import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path

from check_proofs import ROOT, audit_source, validate_coverage


class CoverageGateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-gate-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name, content in {
            "Cargo.toml": "[package]\n",
            "Cargo.lock": "version = 4\n",
            "verification/extract.rs": "// extraction entry points\n",
            "verification/extract_basket.rs": "// shared basket extraction entry point\n",
            "verification/extract_account.rs": "// shared account extraction entry points\n",
            "verification/patches/aeneas-namespaced-discriminant.patch": "// pinned backend name fix\n",
            "verification/aeneas-toolchain.json": "{}\n",
            "src/lib.rs": "pub struct TypedCore;\n",
            "tests/contract.rs": "// regression evidence\n",
            "verification/lean/Axionomy.lean": "import Axionomy.Core\n",
            "verification/lean/Axionomy/Core.lean": "namespace Axionomy\nend Axionomy\n",
        }.items():
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content)
        sources = ["Cargo.toml", "Cargo.lock", "src/lib.rs", "verification/extract.rs",
                   "verification/extract_basket.rs", "verification/extract_account.rs",
                   "verification/patches/aeneas-namespaced-discriminant.patch", "verification/aeneas-toolchain.json"]
        self.coverage = {
            "version": 1,
            "sources": {name: hashlib.sha256((self.root / name).read_bytes()).hexdigest() for name in sources},
            "features": [{"id": "core", "sources": sources,
                          "status": "design_proved_rust_tested",
                          "theorems": ["Axionomy.Quantity.add_exact"],
                          "rust_tests": ["tests/contract.rs"],
                          "limitations": ["Rust refinement remains unproved"]}],
        }

    def test_valid_review_passes(self):
        self.assertEqual(validate_coverage(self.root, self.coverage), ["Axionomy.Quantity.add_exact"])

    def test_changed_source_invalidates_review(self):
        (self.root / "src/lib.rs").write_text("pub struct ChangedCore;\n")
        with self.assertRaisesRegex(ValueError, "stale"):
            validate_coverage(self.root, self.coverage)

    def test_new_core_file_requires_coverage(self):
        (self.root / "src/new_feature.rs").write_text("pub struct NewFeature;\n")
        with self.assertRaisesRegex(ValueError, "source coverage differs"):
            validate_coverage(self.root, self.coverage)

    def test_missing_feature_contract_is_rejected(self):
        self.coverage["features"][0]["sources"].remove("src/lib.rs")
        with self.assertRaisesRegex(ValueError, "without a proof contract"):
            validate_coverage(self.root, self.coverage)

    def test_empty_proof_evidence_is_rejected(self):
        self.coverage["features"][0]["theorems"] = []
        with self.assertRaisesRegex(ValueError, "incomplete proof contract"):
            validate_coverage(self.root, self.coverage)

    def test_unbuilt_proof_file_is_rejected(self):
        (self.root / "verification/lean/Axionomy/Unchecked.lean").write_text("")
        with self.assertRaisesRegex(ValueError, "unbuilt or missing"):
            validate_coverage(self.root, self.coverage)

    def test_duplicate_feature_is_rejected(self):
        self.coverage["features"].append(copy.deepcopy(self.coverage["features"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate feature"):
            validate_coverage(self.root, self.coverage)

    def test_missing_rust_evidence_is_rejected(self):
        self.coverage["features"][0]["rust_tests"] = ["tests/missing.rs"]
        with self.assertRaisesRegex(ValueError, "missing Rust evidence"):
            validate_coverage(self.root, self.coverage)

    def test_unsupported_refinement_claim_is_rejected(self):
        self.coverage["features"][0]["status"] = "aeneas_refinement_proved"
        with self.assertRaisesRegex(ValueError, "no supported extraction pipeline"):
            validate_coverage(self.root, self.coverage)

    def test_incomplete_basket_refinement_is_rejected(self):
        feature = self.coverage["features"][0]
        feature.update(id="basket_construction", status="aeneas_refinement_proved",
                       extracted_module="BasketExtraction.Funs")
        with self.assertRaisesRegex(ValueError, "incomplete implementation refinement contract"):
            validate_coverage(self.root, self.coverage)

    def test_incomplete_preparation_refinement_is_rejected(self):
        feature = self.coverage["features"][0]
        feature.update(id="account_exchange_preparation", status="aeneas_refinement_proved",
                       extracted_module="AccountExtraction.Funs")
        with self.assertRaisesRegex(ValueError, "incomplete implementation refinement contract"):
            validate_coverage(self.root, self.coverage)

    def test_incomplete_rate_refinement_is_rejected(self):
        feature = self.coverage["features"][0]
        feature.update(id="static_rate_construction", status="aeneas_refinement_proved",
                       extracted_module="AccountExtraction.Funs")
        with self.assertRaisesRegex(ValueError, "incomplete implementation refinement contract"):
            validate_coverage(self.root, self.coverage)

    def test_incomplete_conservation_refinement_is_rejected(self):
        feature = self.coverage["features"][0]
        feature.update(id="runtime_linear_conservation", status="aeneas_refinement_proved",
                       extracted_module="AccountExtraction.Funs")
        with self.assertRaisesRegex(ValueError, "incomplete implementation refinement contract"):
            validate_coverage(self.root, self.coverage)

    def test_incomplete_basket_operations_refinement_is_rejected(self):
        feature = self.coverage["features"][0]
        feature.update(id="basket_operations", status="aeneas_refinement_proved",
                       extracted_module="AccountExtraction.Funs")
        with self.assertRaisesRegex(ValueError, "incomplete implementation refinement contract"):
            validate_coverage(self.root, self.coverage)


class KernelAuditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(["lake", "build", "Audit"], cwd=ROOT / "verification/lean", check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

    def audit(self, text):
        with tempfile.TemporaryDirectory(prefix="axionomy-kernel-test-") as scratch:
            target = Path(scratch) / "Probe.lean"
            target.write_text(text)
            return subprocess.run(["lake", "env", "lean", str(target)], cwd=ROOT / "verification/lean",
                                  stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

    def test_finished_proof_passes(self):
        result = self.audit(audit_source(["Axionomy.Quantity.add_exact"]))
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_unfinished_proof_is_rejected(self):
        result = self.audit("import Audit\nnamespace Axionomy\ntheorem unfinished : False := by sorry\nend Axionomy\n#audit_axionomy\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("forbidden axiom sorryAx", result.stdout)

    def test_axiom_hidden_in_external_helper_is_rejected(self):
        result = self.audit("import Audit\naxiom externalAssumption : False\nnamespace Axionomy\ntheorem hidden : False := externalAssumption\nend Axionomy\n#audit_axionomy\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("forbidden axiom externalAssumption", result.stdout)

    def test_missing_theorem_is_rejected(self):
        result = self.audit(audit_source(["Axionomy.Quantity.does_not_exist"]))
        self.assertNotEqual(result.returncode, 0)

    def test_definition_cannot_masquerade_as_proof(self):
        result = self.audit(audit_source(["Axionomy.Quantity.checked"]))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("is not a theorem", result.stdout)


if __name__ == "__main__":
    unittest.main()
