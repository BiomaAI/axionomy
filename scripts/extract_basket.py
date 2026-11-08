#!/usr/bin/env python3
"""Extract the shared production basket constructor and check its storage boundary."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from setup_aeneas import PIN, ROOT, verify_tools
from extract_quantity import LEAN, WORK

GENERATED = {
    "Types.lean": "BasketExtraction/Types.lean",
    "Funs.lean": "BasketExtraction/Funs.lean",
    "BasketExtraction.lean": "BasketExtraction.lean",
}
TEMPLATES = {
    "TypesExternal_Template.lean": "2fc137d5d7bfb8cda00f0f65fff6fa45ee367f748f7b3e532260144ff5a2a913",
    "FunsExternal_Template.lean": "82bba47ea1496ba291364c0ef34da072247499c6cc51bab9cd1a5aa88fe2bac4",
}


def validate_bridge(root=ROOT):
    """Fail if the public constructor or modeled adapter stops being a forwarding call."""
    source = (root / "src/basket.rs").read_text()
    for function, expected in {
        "try_from_entries": "construct_entries(entries)",
        "retain_quantities": "quantities.retain(|_,quantity|keep(quantity));",
    }.items():
        matches = re.findall(r"\bfn " + function + r"\b[^{}]*\{([^{}]*)\}", source)
        if len(matches) != 1 or re.sub(r"\s+", "", matches[0]) != expected:
            raise ValueError(f"basket source bridge changed: {function}; review its proof boundary")
    harness = (root / "verification/extract_basket.rs").read_text()
    paths = re.findall(r'#\[path\s*=\s*"([^"]+)"\]', harness)
    if sorted(paths) != ["../src/amount.rs", "../src/basket.rs", "../src/quantity.rs"]:
        raise ValueError("basket extraction must include the exact production source files")
    matches = re.findall(r"\bfn construct\b[^{}]*\{([^{}]*)\}", harness)
    if len(matches) != 1 or re.sub(r"\s+", "", matches[0]) != "basket::construct_entries(entries)":
        raise ValueError("basket extraction entry point is not a forwarding call")


def validate_generated(directory):
    names = {p.name for p in directory.iterdir()}
    if names != set(GENERATED) | set(TEMPLATES):
        raise ValueError(f"unexpected basket extraction files: {sorted(names)}")
    for name, expected in TEMPLATES.items():
        if hashlib.sha256((directory / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"basket external model boundary changed: {name}; review its semantics")
    for name in GENERATED:
        if re.search(r"\b(sorry|admit|axiom)\b", (directory / name).read_text()):
            raise ValueError(f"unfinished or axiomatic basket extraction: {name}")
    functions = (directory / "Funs.lean").read_text()
    for entry in ("construct", "basket.construct_entries", "basket.construct_entries_loop.body",
                  "quantity.Quantity.scalar_is_zero"):
        if not re.search(r"^def " + re.escape(entry) + r"\s", functions, re.MULTILINE):
            raise ValueError(f"missing basket production definition: {entry}")


def compare_generated(directory, lean=LEAN, write=False):
    for name, relative in GENERATED.items():
        actual = (directory / name).read_bytes()
        target = lean / relative
        if write:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(actual)
        elif not target.is_file() or actual != target.read_bytes():
            raise ValueError(f"stale basket model: {relative}; run python3 scripts/extract_basket.py --write and review/reprove")


def check_extraction(write=False):
    validate_bridge()
    aeneas, charon = verify_tools()
    WORK.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(WORK / "cargo-target")
    environment["CARGO_INCREMENTAL"] = "0"
    toolchain = "+" + PIN["charon"]["rust_toolchain"]
    rustc = subprocess.check_output(["rustc", toolchain, "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    subprocess.run(["cargo", toolchain, "clean", "--target", host, "-p", "axionomy"],
                   cwd=ROOT, env=environment, check=True)
    with tempfile.TemporaryDirectory(prefix="basket-extraction-", dir=WORK) as scratch:
        scratch = Path(scratch)
        llbc = scratch / "basket_extraction.llbc"
        generated = scratch / "generated"
        generated.mkdir()
        command = [str(charon), "cargo", "--preset", "aeneas", "--monomorphize-mut",
                   "--resugar-drops", "--rustc-arg=-Zinline-mir=no",
                   "--start-from-if-exists", "basket_extraction::construct",
                   "--mir", "promoted", "--sysroot", "default",
                   "--include", "basket_extraction::basket", "--include", "basket_extraction::quantity",
                   "--include", "num_traits", "--opaque", "indexmap",
                   "--opaque", "basket_extraction::basket::retain_quantities",
                   "--dest-file", str(llbc), "--", "--locked", "-p", "axionomy",
                   "--example", "basket_extraction", "--no-default-features"]
        print("Extracting the shared production basket constructor...", flush=True)
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
        if not llbc.is_file():
            raise ValueError("Charon did not emit fresh basket LLBC")
        subprocess.run([str(aeneas), "-backend", "lean", "-namespace", "Axionomy.BasketExtracted",
                        "-dest", str(generated), "-use-lean-modules", "false", "-split-files",
                        "-gen-lib-entry", "-warnings-as-errors", "-no-progress-bar", str(llbc)],
                       cwd=ROOT, check=True)
        validate_generated(generated)
        compare_generated(generated, write=write)
    print("Basket extraction updated; review and recheck proofs." if write else
          "Production basket extraction matches the committed model.", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    try:
        check_extraction(write=args.write)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Basket extraction failed: {error}", file=sys.stderr)
        sys.exit(1)
