#!/usr/bin/env python3
"""Check proof coverage, kernel dependencies, generated cases and Rust conformance.

Source hashes record review freshness. They do NOT establish Rust refinement.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def validate_coverage(root, coverage):
    if coverage.get("version") != 1:
        raise ValueError("unsupported proof coverage version")
    sources = {"Cargo.toml", "Cargo.lock"} | {str(p.relative_to(root)) for p in (root / "src").rglob("*.rs")}
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
        # The initial proof layer is explicitly design-level. Introducing a
        # stronger status requires changing this checker and its trust contract.
        if feature["status"] != "design_proved_rust_tested":
            raise ValueError(f"unsupported verification status: {feature['status']}")
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
    run("cargo", "test", "-p", "axionomy", "--all-features", "--test", "lean_conformance")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--doc")
    print(f"Verified {len(theorems)} registered design theorems, their axiom dependencies, "
          "and the Rust conformance/type checks. Rust implementation refinement remains unproved.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"proof check failed: {error}", file=sys.stderr)
        sys.exit(1)
