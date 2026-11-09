"""Starts the axionomy-mcp binary on a free port for the whole test session."""

import os
import socket
import subprocess
import time
from collections.abc import Iterator
from pathlib import Path

import httpx
import pytest

REPOSITORY = Path(__file__).resolve().parents[3]
BINARY = Path(os.environ.get("AXIONOMY_MCP_BIN", REPOSITORY / "target" / "debug" / "axionomy-mcp"))


@pytest.fixture
def anyio_backend() -> str:
    return "asyncio"


@pytest.fixture(scope="session")
def base_url() -> Iterator[str]:
    if not BINARY.exists():
        pytest.fail(f"{BINARY} is missing; build it with `cargo build -p axionomy-mcp --bin axionomy-mcp`")
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    environment = {**os.environ, "AXIONOMY_MCP_BIND": f"127.0.0.1:{port}", "RUST_LOG": "warn"}
    server = subprocess.Popen([BINARY], env=environment)
    base_url = f"http://127.0.0.1:{port}"
    try:
        deadline = time.monotonic() + 30
        while True:
            try:
                if httpx.get(f"{base_url}/health").status_code == 200:
                    break
            except httpx.TransportError:
                pass
            if server.poll() is not None or time.monotonic() > deadline:
                pytest.fail("axionomy-mcp did not start")
            time.sleep(0.05)
        yield base_url
    finally:
        server.terminate()
        server.wait(timeout=10)


@pytest.fixture
def url(base_url: str) -> str:
    return f"{base_url}/mcp"
