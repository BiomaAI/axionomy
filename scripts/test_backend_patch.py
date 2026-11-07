"""The reviewed Lean support patch must apply exactly and fail closed on drift."""
import hashlib
from pathlib import Path
import tempfile
import unittest

from setup_aeneas import check_backend_patch


class BackendPatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="axionomy-backend-patch-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.destination = self.root / "backend"
        self.destination.mkdir()
        self.source = self.destination / "source.lean"
        self.source.write_text("before\n")
        self.artifact = self.root / "fix.patch"
        self.artifact.write_text("--- a/source.lean\n+++ b/source.lean\n@@ -1 +1 @@\n-before\n+after\n")
        self.pin = {"lean_backend_patch": {
            "path": "fix.patch", "source": "source.lean",
            "sha256": hashlib.sha256(self.artifact.read_bytes()).hexdigest(),
            "before_sha256": hashlib.sha256(b"before\n").hexdigest(),
            "after_sha256": hashlib.sha256(b"after\n").hexdigest(),
        }}

    def check(self, write=False):
        check_backend_patch(self.destination, self.root, self.pin, write=write)

    def test_patch_is_required(self):
        with self.assertRaisesRegex(ValueError, "patch missing"):
            self.check()

    def test_reviewed_patch_applies_and_is_idempotent(self):
        self.check(write=True)
        self.check(write=True)
        self.check()
        self.assertEqual(self.source.read_text(), "after\n")

    def test_unknown_source_is_rejected(self):
        self.source.write_text("unreviewed change\n")
        with self.assertRaisesRegex(ValueError, "differs from both reviewed versions"):
            self.check(write=True)

    def test_changed_patch_is_rejected(self):
        self.artifact.write_text("altered patch\n")
        with self.assertRaisesRegex(ValueError, "patch checksum mismatch"):
            self.check(write=True)


if __name__ == "__main__":
    unittest.main()
