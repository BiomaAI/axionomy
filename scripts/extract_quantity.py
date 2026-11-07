#!/usr/bin/env python3
"""Regenerate the Lean model from the production Quantity<u64> call paths.

Default: compare with committed generated files. --write: replace them for review.
Handwritten proofs and intrinsic models are never generated or overwritten here.
"""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from setup_aeneas import PIN, ROOT, verify_tools

LEAN = ROOT / "verification/lean"
WORK = ROOT / "verification/.work"
GENERATED = {
    "Types.lean": "QuantityExtraction/Types.lean",
    "Funs.lean": "QuantityExtraction/Funs.lean",
    "QuantityExtraction.lean": "QuantityExtraction.lean",
}
TEMPLATE = "FunsExternal_Template.lean"
TEMPLATE_SHA256 = "59e80d3854107fc68ac37bfebed0feeeed218d23d64a4d96bc60c42a63e5b54d"


def validate_generated(directory):
    names = {p.name for p in directory.iterdir()}
    if names != set(GENERATED) | {TEMPLATE}:
        raise ValueError(f"unexpected extraction files: {sorted(names)}")
    template = (directory / TEMPLATE).read_bytes()
    if hashlib.sha256(template).hexdigest() != TEMPLATE_SHA256:
        raise ValueError("external intrinsic boundary changed; review the generated template and its models")
    functions = (directory / "Funs.lean").read_text()
    # The hand-written intrinsic adapters support only u64. Reject every other
    # instantiation, even when a new proof might happen not to exercise it.
    for op in ("add", "mul"):
        calls = re.findall(r"core\.intrinsics\." + op + r"_with_overflow\s+(\S+)", functions)
        if calls != ["core.marker.CopyU64"]:
            raise ValueError(f"unsupported {op}_with_overflow instantiation: {calls}")
    for name in GENERATED:
        if re.search(r"\b(sorry|admit|axiom)\b", (directory / name).read_text()):
            raise ValueError(f"unfinished or axiomatic generated code: {name}")
    for op in ("add", "sub", "mul", "div"):
        if not re.search(r"^def checked_" + op + r"\b", functions, re.MULTILINE):
            raise ValueError(f"missing extracted entry point: checked_{op}")


def extract(destination):
    aeneas, charon = verify_tools()
    WORK.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(WORK / "cargo-target")
    environment["CARGO_INCREMENTAL"] = "0"
    rustc = subprocess.check_output(["rustc", "+" + PIN["charon"]["rust_toolchain"], "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    # Force the root library and entry points to rebuild. Dependency artifacts
    # stay cached, but a successful cargo invocation cannot reuse stale LLBC.
    subprocess.run(["cargo", "+" + PIN["charon"]["rust_toolchain"], "clean", "--target", host, "-p", "axionomy"],
                   cwd=ROOT, env=environment, check=True)
    with tempfile.TemporaryDirectory(prefix="llbc-", dir=WORK) as scratch:
        llbc = Path(scratch) / "quantity_extraction.llbc"
        command = [str(charon), "cargo", "--preset", "aeneas",
                   "--start-from-if-exists", "quantity_extraction",
                   "--mir", "optimized", "--sysroot", "default",
                   "--rustc-arg=-Zinline-mir=yes",
                   "--rustc-arg=-Zinline-mir-threshold=1000",
                   "--rustc-arg=-Zinline-mir-hint-threshold=1000",
                   "--rustc-arg=-Zinline-mir-forwarder-threshold=1000",
                   "--include", "axionomy::quantity", "--include", "num_traits",
                   "--dest-file", str(llbc), "--", "--locked", "-p", "axionomy",
                   "--example", "quantity_extraction", "--no-default-features"]
        print("Extracting production Quantity<u64> with pinned Charon...", flush=True)
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
        if not llbc.is_file():
            raise ValueError("Charon did not emit a fresh LLBC file")
        subprocess.run([str(aeneas), "-backend", "lean", "-namespace", "Axionomy.Extracted",
                        "-dest", str(destination), "-use-lean-modules", "false", "-split-files",
                        "-gen-lib-entry", "-warnings-as-errors", "-no-progress-bar", str(llbc)],
                       cwd=ROOT, check=True)
    validate_generated(destination)


def check_extraction(write=False):
    WORK.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="generated-", dir=WORK) as scratch:
        directory = Path(scratch)
        extract(directory)
        compare_generated(directory, LEAN, write=write)
    print("Production Rust extraction matches the committed Lean model." if not write else
          "Generated Lean model updated; recheck proofs and review the diff.", flush=True)


def compare_generated(directory, lean_root, write=False):
    for name, relative in GENERATED.items():
        actual = (directory / name).read_bytes()
        target = lean_root / relative
        if write:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(actual)
        elif not target.is_file() or actual != target.read_bytes():
            raise ValueError(f"stale generated model: {relative}; run python3 scripts/extract_quantity.py --write and review/reprove the change")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    try:
        check_extraction(write=args.write)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Aeneas extraction failed: {error}", file=sys.stderr)
        sys.exit(1)
