"""The Axionomy MCP server, driven by the official MCP Python SDK over
Streamable HTTP: an independent client checking the whole protocol surface a
real caller uses. Economic rules are tested in Rust; this checks that a
standard client can reach them."""

import json
from collections.abc import Sequence
from typing import Any, Literal

import anyio
import httpx
import pytest
from mcp import Client
from mcp.client.extension import ClaimContext, ClientExtension, ResultClaim
from mcp.shared.exceptions import MCPError
from mcp_types import (
    CallToolResult,
    Request,
    RequestParams,
    ResourceLink,
    ResourceTemplateReference,
    Result,
    TextContent,
)

pytestmark = pytest.mark.anyio

BALANCES = "axionomy://economy/{economy_id}/accounts/{+account}{?prefix,cursor}"

ECONOMY = {
    "accounts": {"source": {"token": "5"}, "sink": {}},
    "rates": {
        "transfer": {
            "consume": {"giver": {"token": "1"}},
            "produce": {"receiver": {"token": "1"}},
            "distinct": [["giver", "receiver"]],
        }
    },
    "invariants": {"supply": {"token": 1}},
}


def transfer(units: int) -> dict[str, Any]:
    return {"rate": "transfer", "bindings": {"giver": "source", "receiver": "sink"}, "units": str(units)}


class TaskCreated(Result):
    result_type: Literal["task"]
    task_id: str


class TaskState(Result):
    status: str
    result: dict[str, Any] | None = None


class TaskParams(RequestParams):
    task_id: str


class GetTask(Request[TaskParams, Literal["tasks/get"]]):
    method: Literal["tasks/get"] = "tasks/get"
    params: TaskParams
    name_param = "taskId"


class CancelTask(Request[TaskParams, Literal["tasks/cancel"]]):
    method: Literal["tasks/cancel"] = "tasks/cancel"
    params: TaskParams
    name_param = "taskId"


class Tasks(ClientExtension):
    """The MCP tasks extension (SEP-2663), which the SDK does not ship: a tool
    call the server runs as a task resolves to the task's terminal result by
    polling `tasks/get`, optionally after asking the server to cancel it."""

    identifier = "io.modelcontextprotocol/tasks"

    def __init__(self, *, cancel: bool = False) -> None:
        self.cancel = cancel
        self.status: str | None = None

    def claims(self) -> Sequence[ResultClaim[Any]]:
        return [ResultClaim(result_type="task", model=TaskCreated, resolve=self.finish)]

    async def finish(self, created: TaskCreated, context: ClaimContext) -> CallToolResult:
        params = TaskParams(task_id=created.task_id)
        if self.cancel:
            await context.session.send_request(CancelTask(params=params), Result)
        while True:
            state = await context.session.send_request(GetTask(params=params), TaskState)
            if state.status != "working":
                break
            await anyio.sleep(0.01)
        self.status = state.status
        if state.status == "completed" and state.result is not None:
            return CallToolResult.model_validate(state.result)
        return CallToolResult(content=[TextContent(type="text", text=state.status)], is_error=True)


def links(result: CallToolResult) -> list[str]:
    return [str(block.uri) for block in result.content if isinstance(block, ResourceLink)]


async def page(client: Client, uri: str) -> Any:
    result = await client.read_resource(uri)
    (contents,) = result.contents
    return json.loads(contents.text)


async def create(client: Client, entries: dict[str, Any]) -> str:
    result = await client.call_tool("axionomy_economy_create", {"entries": entries})
    assert not result.is_error, result.structured_content
    return result.structured_content["economy_id"]


async def test_the_server_describes_itself_to_a_standard_client(url: str) -> None:
    async with Client(url) as client:
        assert client.protocol_version == "2026-07-28"
        assert client.server_info is not None and client.server_info.name == "axionomy-mcp"
        assert client.instructions and "axionomy_economy_create" in client.instructions
        capabilities = client.server_capabilities
        assert capabilities.tools and capabilities.resources and capabilities.completions is not None

        tools = (await client.list_tools()).tools
        assert len(tools) == 8
        for tool in tools:
            assert tool.output_schema is not None, tool.name
            assert tool.annotations is not None and tool.annotations.open_world_hint is False, tool.name
        templates = (await client.list_resource_templates()).resource_templates
        assert len(templates) == 16
        assert BALANCES in {template.uri_template for template in templates}
        assert (await client.list_resources()).resources == []


async def test_a_client_authors_applies_and_reads_back_through_links(url: str) -> None:
    async with Client(url) as client:
        created = await client.call_tool("axionomy_economy_create", {"entries": ECONOMY})
        economy_id = created.structured_content["economy_id"]
        (summary_uri,) = links(created)
        summary = await page(client, summary_uri)
        assert summary["counts"] == {"accounts": 2, "assets": 1, "rates": 1, "invariants": 1}

        extended = await client.call_tool(
            "axionomy_economy_extend",
            {"economy_id": economy_id, "entries": {"accounts": {"source": {"gem": "1"}}}},
        )
        assert extended.structured_content["parent_id"] == economy_id

        applied = await client.call_tool("axionomy_exchange_apply", {"economy_id": economy_id, "exchange": transfer(2)})
        assert applied.structured_content["changes"] == 2
        changes_uri = next(uri for uri in links(applied) if uri.endswith("/changes"))
        changes = await page(client, changes_uri)
        assert {(line["account"], line["kind"], line["quantity"]) for line in changes["items"]} == {
            ("source", "consumed", "2"),
            ("sink", "produced", "2"),
        }

        failed = await client.call_tool("axionomy_exchange_apply", {"economy_id": economy_id, "exchange": transfer(9)})
        assert failed.is_error
        assert failed.structured_content["error"] == "infeasible"
        assessment = await page(client, links(failed)[0])
        assert assessment["status"] == "Infeasible"


async def test_a_large_economy_is_built_in_chunks_and_read_in_pages(url: str) -> None:
    assets, chunk = 25_000, 9_999
    async with Client(url) as client:
        economy_id = await create(client, {"accounts": {"store": {}}})
        for start in range(0, assets, chunk):
            balances = {f"sku/{index:06}": "3" for index in range(start, min(start + chunk, assets))}
            result = await client.call_tool(
                "axionomy_economy_extend",
                {"economy_id": economy_id, "entries": {"accounts": {"warehouse": balances}}},
            )
            assert len(json.dumps(result.structured_content)) < 512
            economy_id = result.structured_content["economy_id"]

        first = f"axionomy://economy/{economy_id}/accounts/warehouse"
        seen: list[str] = []
        uri, pages = first, 0
        while True:
            listing = await page(client, uri)
            seen.extend(item["asset"] for item in listing["items"])
            pages += 1
            cursor = listing.get("nextCursor")
            if cursor is None:
                break
            uri = f"{first}?cursor={cursor}"
        assert len(seen) == len(set(seen)) == assets
        assert pages > 1

        completion = await client.complete(
            ResourceTemplateReference(type="ref/resource", uri=BALANCES),
            {"name": "prefix", "value": "sku/00"},
            {"economy_id": economy_id, "account": "warehouse"},
        )
        assert len(completion.completion.values) == 100
        assert completion.completion.total == 10_000
        assert completion.completion.has_more is True


async def test_account_paths_compose_into_resource_uris(url: str) -> None:
    async with Client(url) as client:
        economy_id = await create(client, {"accounts": {"agents/7/wallet": {"cash": "5"}, "agents/8/wallet": {}}})
        listing = await page(client, f"axionomy://economy/{economy_id}/accounts?prefix=agents/7/")
        assert [item["account"] for item in listing["items"]] == ["agents/7/wallet"]
        wallet = await page(client, f"axionomy://economy/{economy_id}/accounts/agents/7/wallet")
        assert wallet["items"] == [{"asset": "cash", "quantity": "5"}]
        refused = await client.call_tool("axionomy_economy_create", {"entries": {"accounts": {"Agents_7": {}}}})
        assert refused.is_error


async def test_a_search_runs_as_a_task_and_its_solution_is_a_snapshot(url: str) -> None:
    async with Client(url) as plain:
        economy_id = await create(plain, ECONOMY)
        search = {
            "economy_id": economy_id,
            "goal": {"sink": {"token": "1"}},
            "candidates": [transfer(1)],
            "max_expansions": 8,
        }
        refused = await plain.call_tool("axionomy_search", search)
        assert refused.is_error and refused.structured_content["error"] == "tasks_required"

    async with Client(url, extensions=[Tasks()]) as client:
        result = await client.call_tool("axionomy_search", search)
        assert result.structured_content["outcome"] == "solved"
        solution = result.structured_content["solution"]["economy_id"]
        trace = await page(client, f"axionomy://economy/{solution}/exchanges")
        assert len(trace["items"]) == 1


async def test_a_task_can_be_cancelled(url: str) -> None:
    async with Client(url) as plain:
        economy_id = await create(plain, ECONOMY)
    tasks = Tasks(cancel=True)
    async with Client(url, extensions=[tasks]) as client:
        search = {
            "economy_id": economy_id,
            "goal": {"sink": {"token": "100"}},
            "candidates": [transfer(1)],
            "max_expansions": 100_000,
            "chunk_size": 1,
        }
        await client.call_tool("axionomy_search", search)
    assert tasks.status in {"cancelled", "completed"}


async def test_a_problem_run_pages_its_frames(url: str) -> None:
    async with Client(url, extensions=[Tasks()]) as client:
        result = await client.call_tool(
            "axionomy_problem_run",
            {"problem": "maze", "instance": "micro", "strategy": "a_star"},
        )
        summary = result.structured_content
        frames_uri = next(uri for uri in links(result) if uri.endswith("/frames"))
        frames = await page(client, frames_uri)
        selected = next(doc for doc in summary["documents"] if doc["id"] == summary["selected_document_id"])
        assert 0 < len(frames["items"]) <= selected["frames"]


async def test_bad_resources_are_invalid_params(url: str) -> None:
    async with Client(url) as client:
        economy_id = await create(client, ECONOMY)
        for uri in (
            "axionomy://economy/eco_missing",
            f"axionomy://economy/{economy_id}/accounts?cursor=1.00000000",
            f"axionomy://economy/{economy_id}/nothing",
        ):
            with pytest.raises(MCPError) as raised:
                await client.read_resource(uri)
            assert raised.value.code == -32602, uri


def test_requests_without_routing_headers_are_refused(base_url: str) -> None:
    response = httpx.post(
        f"{base_url}/mcp",
        headers={"Accept": "application/json, text/event-stream"},
        json={"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}},
    )
    assert response.status_code == 400
