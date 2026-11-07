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
                "static_rate_construction": ("AccountExtraction.Funs", {
                    f"Axionomy.RateConstructionRefinement.{kind}_{contract}"
                    for kind in ("consume", "produce", "preserve")
                    for contract in ("refines", "registers_and_frames")}),
                "runtime_linear_conservation": ("AccountExtraction.Funs", {
                    "Axionomy.ConservationRefinement.weighted_exact",
                    "Axionomy.ConservationRefinement.signed_add_exact",
                    "Axionomy.ConservationRefinement.conserves_refines_integers",
                    "Axionomy.ConservationRefinement.accepted_conserves"}),
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
    run("cargo", "test", "-p", "axionomy", "--all-features", "--test", "lean_conformance")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--lib",
        "runtime_signed_conservation_matches_lean")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--doc")
    print(f"Verified {len(theorems)} registered theorems and their axiom dependencies, "
          "fresh Aeneas extraction of u64 arithmetic, basket construction/operations, account updates/staging, static rate builders and signed conservation, "
          "and Rust conformance/type checks. Collection storage uses an explicit trusted model. "
          "Other core implementation refinement remains unproved; see verification/README.md.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"proof check failed: {error}", file=sys.stderr)
        sys.exit(1)
