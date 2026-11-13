"""Builds every sample package for the session and serves the connect-four
one with `axionomy studio` on a free port, its store in memory."""

import os
import socket
import subprocess
import time
from collections.abc import Iterator
from pathlib import Path

import httpx
import pytest

REPOSITORY = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("AXIONOMY_BIN", REPOSITORY / "target" / "debug" / "axionomy"))


def binary() -> Path:
    if not BINARY.exists():
        pytest.fail(f"{BINARY} is missing; build it with `cargo build -p axionomy-cli --bin axionomy`")
    return BINARY


@pytest.fixture(scope="session")
def samples(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Every sample's package, zipped, as `axionomy package samples` writes
    them: built for the session, never committed."""
    directory = tmp_path_factory.mktemp("packages") / "samples"
    subprocess.run([binary(), "package", "samples", directory], check=True, capture_output=True)
    return directory


@pytest.fixture(scope="session")
def studio(samples: Path, tmp_path_factory: pytest.TempPathFactory) -> Iterator[str]:
    """The Studio server's base URL, with connect-four's package loaded into
    a store in memory, until the session ends."""
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    environment = {**os.environ, "RUST_LOG": "warn"}
    # In memory, whatever the caller's environment names.
    environment.pop("AXIONOMY_STUDIO_STORE", None)
    arguments = [
        binary(),
        "studio",
        "--bind",
        f"127.0.0.1:{port}",
        "--package",
        samples / "connect-four.zip",
        # The API alone: no built frontend to serve.
        "--frontend",
        tmp_path_factory.mktemp("no-frontend"),
    ]
    server = subprocess.Popen(arguments, env=environment, stdout=subprocess.DEVNULL)
    base_url = f"http://127.0.0.1:{port}"
    try:
        deadline = time.monotonic() + 30
        while True:
            try:
                if httpx.get(f"{base_url}/api/health").status_code == 200:
                    break
            except httpx.TransportError:
                pass
            if server.poll() is not None or time.monotonic() > deadline:
                pytest.fail("axionomy studio did not start")
            time.sleep(0.05)
        yield base_url
    finally:
        server.terminate()
        server.wait(timeout=10)
