"""Starts `axionomy mcp` on a free port for the whole test session, and once
more with each of the maze's and the logistics packages, which the session
builds, loaded."""

import os
import socket
import subprocess
import time
from collections.abc import Iterator, Sequence
from contextlib import contextmanager
from pathlib import Path

import httpx
import pytest

REPOSITORY = Path(__file__).resolve().parents[3]
BINARY = Path(os.environ.get("AXIONOMY_BIN", REPOSITORY / "target" / "debug" / "axionomy"))


@pytest.fixture
def anyio_backend() -> str:
    return "asyncio"


def binary() -> Path:
    if not BINARY.exists():
        pytest.fail(f"{BINARY} is missing; build it with `cargo build -p axionomy-cli --bin axionomy`")
    return BINARY


@contextmanager
def running(store: Path | None = None, packages: Sequence[Path] = ()) -> Iterator[str]:
    """Runs the server until the block ends, keeping snapshots in `store` if
    given and loading `packages` first, and yields its base URL."""
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    environment = {**os.environ, "AXIONOMY_MCP_BIND": f"127.0.0.1:{port}", "RUST_LOG": "warn"}
    environment.pop("AXIONOMY_MCP_PACKAGES", None)
    if store is not None:
        environment["AXIONOMY_MCP_STORE"] = str(store)
    arguments = [binary(), "mcp"]
    for package in packages:
        arguments += ["--package", str(package)]
    server = subprocess.Popen(arguments, env=environment)
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
                pytest.fail("axionomy mcp did not start")
            time.sleep(0.05)
        yield base_url
    finally:
        server.terminate()
        server.wait(timeout=10)


@pytest.fixture(scope="session")
def base_url() -> Iterator[str]:
    with running() as base_url:
        yield base_url


@pytest.fixture
def url(base_url: str) -> str:
    return f"{base_url}/mcp"


@pytest.fixture(scope="session")
def samples(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Every sample's package, zipped, as `axionomy package samples` writes
    them: built for the session, never committed."""
    directory = tmp_path_factory.mktemp("packages") / "samples"
    subprocess.run([binary(), "package", "samples", directory], check=True, capture_output=True)
    return directory


@pytest.fixture(scope="session")
def maze_url(samples: Path) -> Iterator[str]:
    """A server that loaded the maze's package when it started."""
    with running(packages=[samples / "maze.zip"]) as base_url:
        yield f"{base_url}/mcp"


@pytest.fixture(scope="session")
def logistics_url(samples: Path) -> Iterator[str]:
    """A server that loaded the logistics package when it started."""
    with running(packages=[samples / "logistics.zip"]) as base_url:
        yield f"{base_url}/mcp"
