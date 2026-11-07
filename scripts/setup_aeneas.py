#!/usr/bin/env python3
"""Install the pinned extraction tools locally (Python 3.12+, rustup, git, elan).

Aeneas includes its matching Lean backend. Build Charon at Aeneas's exact
charon-pin; the independently published nightly binary has a different commit.
No global Rust default or repository toolchain override is changed.
"""
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "verification/.tools"
PIN = json.loads((ROOT / "verification/aeneas-toolchain.json").read_text())


def check_backend_patch(destination, root=ROOT, pin=PIN, write=False):
    """Pin the instance-name fix as well as the unmodified release binary."""
    patch = pin["lean_backend_patch"]
    artifact = root / patch["path"]
    if hashlib.sha256(artifact.read_bytes()).hexdigest() != patch["sha256"]:
        raise ValueError("Lean backend patch checksum mismatch")
    source = destination / patch["source"]
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    if digest == patch["after_sha256"]:
        return
    if digest != patch["before_sha256"]:
        raise ValueError("Lean backend source differs from both reviewed versions")
    if not write:
        raise ValueError("Lean backend patch missing; run python3 scripts/setup_aeneas.py")
    run("git", "apply", "--check", str(artifact), cwd=destination)
    run("git", "apply", str(artifact), cwd=destination)
    if hashlib.sha256(source.read_bytes()).hexdigest() != patch["after_sha256"]:
        raise ValueError("Lean backend patch produced an unexpected source hash")


def run(*args, cwd=ROOT, capture=False):
    print("+ " + " ".join(map(str, args)), flush=True)
    return subprocess.run(args, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def verify_tools():
    aeneas = TOOLS / "aeneas/aeneas"
    charon = TOOLS / "charon-source/charon/target/release/charon"
    if not aeneas.is_file() or not charon.is_file():
        raise ValueError("missing extraction tools; run python3 scripts/setup_aeneas.py")
    if run(aeneas, "-version", capture=True).strip() != "aeneas " + PIN["aeneas"]["release"]:
        raise ValueError("Aeneas version does not match the pin")
    expected = f"{PIN['charon']['version']} ({PIN['charon']['revision']})"
    if run(charon, "version", capture=True).strip() != expected:
        raise ValueError("Charon version does not match Aeneas's pinned revision")
    lean = ROOT / "verification/lean/lean-toolchain"
    backend_lean = TOOLS / "aeneas/backends/lean/lean-toolchain"
    if lean.read_text().strip() != PIN["lean"] or backend_lean.read_text().strip() != PIN["lean"]:
        raise ValueError("Lean versions do not match the extraction toolchain")
    check_backend_patch(TOOLS / "aeneas")
    return aeneas, charon


def main():
    TOOLS.mkdir(parents=True, exist_ok=True)
    key = platform.system() + "-" + platform.machine()
    archive = PIN["aeneas"]["archives"].get(key)
    if archive is None:
        raise ValueError(f"no pinned Aeneas binary for {key}; use a supported Linux or macOS host")
    destination = TOOLS / "aeneas"
    marker = destination / ".archive-sha256"
    if not marker.exists() or marker.read_text().strip() != archive["sha256"]:
        url = ("https://github.com/AeneasVerif/aeneas/releases/download/"
               + PIN["aeneas"]["release"] + "/" + archive["name"])
        print(f"Downloading {url}", flush=True)
        with tempfile.TemporaryDirectory(dir=TOOLS) as scratch:
            path = Path(scratch) / archive["name"]
            urllib.request.urlretrieve(url, path)
            with path.open("rb") as downloaded:
                digest = hashlib.file_digest(downloaded, "sha256").hexdigest()
            if digest != archive["sha256"]:
                raise ValueError("Aeneas release archive checksum mismatch")
            destination.mkdir(exist_ok=True)
            with tarfile.open(path) as bundle:
                bundle.extractall(destination, filter="data")
        marker.write_text(archive["sha256"] + "\n")

    check_backend_patch(destination, write=True)
    toolchain = PIN["charon"]["rust_toolchain"]
    run("rustup", "toolchain", "install", toolchain, "--profile", "minimal",
        "--component", "rustc-dev", "--component", "llvm-tools", "--component", "rust-src")
    source = TOOLS / "charon-source"
    if not (source / ".git").exists():
        run("git", "init", str(source))
        run("git", "remote", "add", "origin", "https://github.com/AeneasVerif/charon.git", cwd=source)
        run("git", "fetch", "--depth", "1", "origin", PIN["charon"]["revision"], cwd=source)
        run("git", "checkout", "--detach", "FETCH_HEAD", cwd=source)
    revision = run("git", "rev-parse", "HEAD", cwd=source, capture=True).strip()
    if revision != PIN["charon"]["revision"]:
        raise ValueError("cached Charon source has another revision; remove verification/.tools/charon-source and retry")
    if run("git", "status", "--porcelain", cwd=source, capture=True).strip():
        raise ValueError("cached Charon source is modified")
    run("cargo", "+" + toolchain, "build", "--release", "--locked", "--no-default-features",
        "--features", "rustc", "--bin", "charon", "--bin", "charon-driver", cwd=source / "charon")
    verify_tools()
    print("Pinned Aeneas and Charon are ready; Lean versions match.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Aeneas setup failed: {error}", file=sys.stderr)
        sys.exit(1)
