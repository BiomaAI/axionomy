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
    def test_reversing_rust_subtraction_breaks_the_refinement_proof(self):
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

            quantity = root / "src/quantity.rs"
            source = quantity.read_text()
            original = "Self::from_checked(self.0.checked_sub(&rhs.0))"
            self.assertEqual(source.count(original), 1)
            quantity.write_text(source.replace(original, "Self::from_checked(rhs.0.checked_sub(&self.0))"))
            environment = os.environ.copy()
            environment["LEAN_NUM_THREADS"] = "2"

            def run(*command, cwd=root):
                return subprocess.run(command, cwd=cwd, env=environment, text=True,
                                      stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300)

            stale = run(sys.executable, "scripts/extract_quantity.py")
            self.assertNotEqual(stale.returncode, 0, stale.stdout)
            self.assertIn("stale generated model", stale.stdout)
            regenerate = run(sys.executable, "scripts/extract_quantity.py", "--write")
            self.assertEqual(regenerate.returncode, 0, regenerate.stdout)
            proof = run("lake", "build", "Axionomy.QuantityRefinement", cwd=root / "verification/lean")
            self.assertNotEqual(proof.returncode, 0, proof.stdout)
            self.assertIn("QuantityRefinement.lean", proof.stdout)
            self.assertIn("unsolved goals", proof.stdout)


if __name__ == "__main__":
    unittest.main()
