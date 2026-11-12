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
from conftest import running
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
        # A first-time client learns where to start, what an exchange looks
        # like, and which tools need the tasks extension.
        for words in ("Start here", "axionomy_problem_catalog", "axionomy_search", "io.modelcontextprotocol/tasks", '"bindings"'):
            assert words in client.instructions, words
        capabilities = client.server_capabilities
        assert capabilities.tools and capabilities.resources and capabilities.completions is not None

        tools = (await client.list_tools()).tools
        assert len(tools) == 10
        for tool in tools:
            assert tool.output_schema is not None, tool.name
            assert tool.annotations is not None and tool.annotations.open_world_hint is False, tool.name
        templates = (await client.list_resource_templates()).resource_templates
        assert len(templates) == 17
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
        assert failed.structured_content["message"] == "the exchange is infeasible: `source` lacks 4 `token`; nothing was stored"
        assessment = await page(client, links(failed)[0])
        assert assessment["status"] == "infeasible"


async def test_a_client_steps_through_a_replay_and_diffs_its_states(url: str) -> None:
    async with Client(url) as client:
        parent_id = await create(client, ECONOMY)
        replayed = await client.call_tool(
            "axionomy_trace_replay",
            {"economy_id": parent_id, "exchanges": [transfer(1), transfer(2), transfer(1)]},
        )
        replayed_id = replayed.structured_content["economy_id"]
        steps = []
        for step in range(4):
            stepped = await client.call_tool("axionomy_trace_step", {"economy_id": replayed_id, "step": step})
            assert not stepped.is_error, stepped.structured_content
            steps.append(stepped.structured_content)
            assert stepped.structured_content["step"] == step
            # Step zero is the trace's source; the economy passed in is its last step.
            assert stepped.structured_content["source_economy_id"] == parent_id
            assert stepped.structured_content["traced_economy_id"] == replayed_id
        assert steps[3]["root"] == replayed.structured_content["root"]
        parent = await page(client, f"axionomy://economy/{parent_id}")
        assert steps[0]["root"] == parent["root"]

        # A change line names the exchange that moved it by its index, from zero.
        changes = await page(client, f"axionomy://economy/{replayed_id}/changes")
        assert [line["exchange_index"] for line in changes["items"]] == [0, 0, 1, 1, 2, 2]

        diff = await page(client, f"axionomy://economy/{steps[0]['economy_id']}/diff/{steps[2]['economy_id']}")
        assert diff["items"] == [
            {"kind": "balance", "account": "sink", "asset": "token", "before": "0", "after": "3"},
            {"kind": "balance", "account": "source", "asset": "token", "before": "5", "after": "2"},
        ]
        beyond = await client.call_tool("axionomy_trace_step", {"economy_id": replayed_id, "step": 4})
        assert beyond.is_error and beyond.structured_content["error"] == "invalid_request"
        assert beyond.structured_content["details"] == {"pointer": "/step"}

        def condition(account: str, comparison: str, quantity: str) -> dict[str, Any]:
            return {"economy_id": replayed_id, "account": account, "asset": "token", "comparison": comparison, "quantity": quantity}

        found = await client.call_tool("axionomy_trace_find", condition("sink", "greater_than_or_equal", "3"))
        assert found.structured_content["step"] == 2
        assert found.structured_content["economy"]["root"] == steps[2]["root"]
        assert any(uri.endswith(steps[2]["economy_id"]) for uri in links(found))


async def test_trace_find_says_when_nothing_matches_and_refuses_unknown_accounts(url: str) -> None:
    async with Client(url) as client:
        parent_id = await create(client, ECONOMY)
        replayed = await client.call_tool("axionomy_trace_replay", {"economy_id": parent_id, "exchanges": [transfer(1), transfer(2)]})
        replayed_id = replayed.structured_content["economy_id"]

        def condition(account: str) -> dict[str, Any]:
            return {"economy_id": replayed_id, "account": account, "asset": "token", "comparison": "equal", "quantity": "9"}

        never = await client.call_tool("axionomy_trace_find", condition("sink"))
        assert not never.is_error
        assert never.structured_content["step"] is None
        assert "economy" not in never.structured_content
        assert never.structured_content["message"].startswith("`sink`'s balance of `token` is equal to 9 at none of the 3 steps")

        ghost = await client.call_tool("axionomy_trace_find", condition("ghost"))
        assert ghost.is_error
        assert ghost.structured_content["error"] == "not_found"
        assert ghost.structured_content["details"] == {"pointer": "/account"}
        assert "has an account `ghost`" in ghost.structured_content["message"]


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


async def test_exchanges_open_and_close_accounts(url: str) -> None:
    spawn_and_retire = {
        "accounts": {"treasury": {"cash": "10"}},
        "rates": {
            "agents/spawn": {
                "consume": {"treasury": {"cash": "3"}},
                "produce": {"agent": {"cash": "3"}},
                "opens": ["agent"],
            },
            "agents/retire": {
                "consume": {"agent": {"cash": "3"}},
                "produce": {"treasury": {"cash": "3"}},
                "closes": ["agent"],
            },
        },
    }

    def exchange(rate: str) -> dict[str, Any]:
        return {"rate": rate, "bindings": {"treasury": "treasury", "agent": "agents/7/wallet"}, "units": "1"}

    async with Client(url) as client:
        economy_id = await create(client, spawn_and_retire)
        spawned = await client.call_tool("axionomy_exchange_apply", {"economy_id": economy_id, "exchange": exchange("agents/spawn")})
        assert spawned.structured_content["opened"] == ["agents/7/wallet"]
        spawned_id = spawned.structured_content["economy_id"]
        wallet = await page(client, f"axionomy://economy/{spawned_id}/accounts/agents/7/wallet")
        assert wallet["items"] == [{"asset": "cash", "quantity": "3"}]
        rate = await page(client, f"axionomy://economy/{spawned_id}/rates/agents/spawn")
        assert rate["opens"] == ["agent"]

        retired = await client.call_tool("axionomy_exchange_apply", {"economy_id": spawned_id, "exchange": exchange("agents/retire")})
        assert retired.structured_content["closed"] == ["agents/7/wallet"]
        accounts = await page(client, f"axionomy://economy/{retired.structured_content['economy_id']}/accounts")
        assert [item["account"] for item in accounts["items"]] == ["treasury"]


# Work done outside the economy (a robot, a model, a person) is modelled, not
# orchestrated: asking produces a `requested` token, the outside party's result
# is an exchange that consumes it, and a deadline is a condition on a clock.
OUTSIDE_WORK = {
    "accounts": {"agent": {"budget": "1"}, "clock": {}},
    "rates": {
        "request": {"consume": {"asker": {"budget": "1"}}, "produce": {"asker": {"requested": "1"}}},
        "report": {"consume": {"asker": {"requested": "1"}}, "produce": {"asker": {"report": "1"}}},
        "tick": {"produce": {"clock": {"tick": "1"}}},
        "lapse": {
            "consume": {"asker": {"requested": "1"}},
            "produce": {"asker": {"lapsed": "1"}},
            "preserve": {"clock": {"tick": "1"}},
            "conditions": [
                {
                    "name": "deadline-passed",
                    "left": {"kind": "balance", "role": "clock", "asset": "tick"},
                    "comparison": "greater_than_or_equal",
                    "right": {"kind": "constant", "value": "3"},
                }
            ],
        },
    },
}


def outside(rate: str) -> dict[str, Any]:
    bindings = {"tick": {"clock": "clock"}, "lapse": {"asker": "agent", "clock": "clock"}}
    return {"rate": rate, "bindings": bindings.get(rate, {"asker": "agent"}), "units": "1"}


async def apply(client: Client, economy_id: str, rate: str) -> CallToolResult:
    return await client.call_tool("axionomy_exchange_apply", {"economy_id": economy_id, "exchange": outside(rate)})


async def test_outside_work_is_a_request_the_economy_holds(url: str) -> None:
    async with Client(url) as client:
        asked = await apply(client, await create(client, OUTSIDE_WORK), "request")
        assert not asked.is_error, asked.structured_content
        asked_id = asked.structured_content["economy_id"]

        # The outside party reports once; a duplicate report has nothing left
        # to consume, so the kernel refuses it.
        reported = await apply(client, asked_id, "report")
        assert not reported.is_error, reported.structured_content
        duplicate = await apply(client, reported.structured_content["economy_id"], "report")
        assert duplicate.is_error and duplicate.structured_content["error"] == "infeasible"

        # The request lapses only once its deadline has passed.
        early = await apply(client, asked_id, "lapse")
        assert early.is_error and early.structured_content["error"] == "invalid"
        (issue,) = early.structured_content["details"]["issues"]
        assert {key: issue[key] for key in ("kind", "rate", "condition")} == {
            "kind": "rate_condition_failed",
            "rate": "lapse",
            "condition": "deadline-passed",
        }
        assert issue["message"].endswith(f"axionomy://economy/{asked_id}/rate-conditions/lapse")
        waited = await client.call_tool(
            "axionomy_trace_replay",
            {"economy_id": asked_id, "exchanges": [outside("tick")] * 3},
        )
        assert not waited.is_error, waited.structured_content
        lapsed = await apply(client, waited.structured_content["economy_id"], "lapse")
        assert not lapsed.is_error, lapsed.structured_content

        # A report that arrives after the lapse is refused: the outcome stays
        # unknown, and nothing is recorded twice.
        late = await apply(client, lapsed.structured_content["economy_id"], "report")
        assert late.is_error and late.structured_content["error"] == "infeasible"


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


async def test_a_problem_study_pages_its_frames(url: str) -> None:
    async with Client(url, extensions=[Tasks()]) as client:
        result = await client.call_tool(
            "axionomy_problem_study",
            {"problem": "maze", "instance": "micro", "strategy": "a-star"},
        )
        summary = result.structured_content
        assert summary["study_id"].startswith("stu-")
        assert summary["key"] == {"problem": "maze", "instance": "micro", "strategy": "a-star", "seed": 17, "budget": 128}
        assert f"axionomy://study/{summary['study_id']}" in links(result)
        frames_uri = next(uri for uri in links(result) if uri.endswith("/frames"))
        frames = await page(client, frames_uri)
        selected = next(doc for doc in summary["documents"] if doc["id"] == summary["selected_document"])
        assert 0 < len(frames["items"]) <= selected["frames"]

        # Every document is an economy from its initial state to the state
        # its exchanges reach, one exchange per frame.
        for document in summary["documents"]:
            assert document["initial_economy_id"].startswith("eco-")
            final = await page(client, f"axionomy://economy/{document['final_economy_id']}")
            assert final["parent_id"] == document["initial_economy_id"]
            assert final["origin"] == {"kind": "exchanged", "exchanges": document["frames"]}
        assert f"axionomy://economy/{selected['final_economy_id']}" in links(result)
        study = await page(client, f"axionomy://study/{summary['study_id']}")
        assert study["documents"] == summary["documents"]

        refused = await client.call_tool(
            "axionomy_problem_study",
            {"problem": "maze", "strategy": "a_star"},
        )
        assert refused.is_error


async def test_the_catalog_carries_each_problems_own_words(url: str) -> None:
    async with Client(url, extensions=[Tasks()]) as client:
        listed = await client.call_tool("axionomy_problem_catalog", {})
        assert not listed.is_error, listed.structured_content
        problems = {problem["key"]: problem for problem in listed.structured_content["problems"]}
        maze = problems["maze"]
        # A problem declares its family, capabilities and instances: each a
        # key to send back and a label to show people.
        assert maze["family"] == {"key": "pathfinding", "label": "Pathfinding"}
        assert {"key": "exact-pareto", "label": "Exact Pareto frontier"} in maze["capabilities"]
        assert [instance["key"] for instance in maze["instances"]] == ["micro", "showcase", "stress"]
        assert [instance["label"] for instance in maze["instances"]] == ["Micro", "Showcase", "Stress"]
        assert maze["default_instance"] == "showcase"

        # The study takes the instance as a path, not a closed list of sizes.
        tools = {tool.name: tool for tool in (await client.list_tools()).tools}
        schema = tools["axionomy_problem_study"].input_schema

        def resolved(node: dict[str, Any]) -> dict[str, Any]:
            while "$ref" in node:
                node = schema["$defs"][node["$ref"].rsplit("/", 1)[-1]]
            return node

        instance = schema["properties"]["instance"]
        written = [resolved(option) for option in instance.get("anyOf", [instance]) if option.get("type") != "null"]
        assert written and all(option["type"] == "string" and "pattern" in option and "enum" not in option for option in written)

        # An instance the problem does not declare is refused where it is read.
        refused = await client.call_tool("axionomy_problem_study", {"problem": "maze", "instance": "huge"})
        assert refused.is_error
        assert refused.structured_content["details"] == {"pointer": "/instance"}
        assert "has no instance `huge`" in refused.structured_content["message"]


async def exchanges(client: Client, economy_id: str) -> list[dict[str, Any]]:
    return (await page(client, f"axionomy://economy/{economy_id}/exchanges"))["items"]


async def step(client: Client, economy_id: str, step: int) -> dict[str, Any]:
    stepped = await client.call_tool("axionomy_trace_step", {"economy_id": economy_id, "step": step})
    assert not stepped.is_error, stepped.structured_content
    return stepped.structured_content


async def test_a_client_steps_into_a_study_and_tries_another_exchange(url: str) -> None:
    async with Client(url, extensions=[Tasks()]) as client:
        result = await client.call_tool(
            "axionomy_problem_study",
            {"problem": "logistics", "instance": "micro", "strategy": "reliable"},
        )
        documents = {document["id"]: document for document in result.structured_content["documents"]}
        studied = documents["reliable"]["final_economy_id"]
        other = documents["direct"]["final_economy_id"]

        # The two strategies agree until step N, where they choose differently.
        taken, alternative = await exchanges(client, studied), await exchanges(client, other)
        n = next(index for index, (left, right) in enumerate(zip(taken, alternative)) if left != right)
        assert n > 0
        state = await step(client, studied, n)
        assert state["root"] == (await step(client, other, n))["root"]

        # From the study's state at step N, take the other strategy's exchange
        # instead of the study's, and compare with the study's own step N + 1.
        tried = await client.call_tool("axionomy_exchange_apply", {"economy_id": state["economy_id"], "exchange": alternative[n]})
        assert not tried.is_error, tried.structured_content
        assert tried.structured_content["source_economy_id"] == state["economy_id"]
        followed = await step(client, studied, n + 1)
        diff = await page(client, f"axionomy://economy/{tried.structured_content['economy_id']}/diff/{followed['economy_id']}")
        # The vehicle left on the direct road instead of the safe one.
        assert (taken[n]["rate"], alternative[n]["rate"]) == ("depart/safe-out-first", "depart/direct-out")
        assert diff["items"] == [
            {"kind": "balance", "account": "vehicle", "asset": "traveling/direct-out", "before": "1", "after": "0"},
            {"kind": "balance", "account": "vehicle", "asset": "traveling/safe-out-first", "before": "0", "after": "1"},
        ]


async def test_a_frames_exchange_is_tried_in_the_studys_economy(url: str) -> None:
    async with Client(url, extensions=[Tasks()]) as client:
        result = await client.call_tool(
            "axionomy_problem_study",
            {"problem": "maze", "instance": "micro", "strategy": "a-star"},
        )
        summary = result.structured_content
        selected = next(doc for doc in summary["documents"] if doc["id"] == summary["selected_document"])
        final_id = selected["final_economy_id"]
        document = await page(client, f"axionomy://study/{summary['study_id']}/documents/{selected['id']}")
        assert document["final_economy_id"] == final_id
        frames = (await page(client, f"axionomy://study/{summary['study_id']}/documents/{selected['id']}/frames"))["items"]
        assert frames

        # Frame i shows step i + 1 of the final economy, and its exchange is
        # written as that economy writes it: applied to step i, it makes the
        # economy of step i + 1.
        for i, frame in enumerate(frames):
            assert (frame["step"], frame["view"]["index"]) == (i + 1, i)
            assert not frame["exchange"]["rate"].startswith("maze/")
            before = await step(client, final_id, i)
            tried = await client.call_tool("axionomy_exchange_apply", {"economy_id": before["economy_id"], "exchange": frame["exchange"]})
            assert not tried.is_error, tried.structured_content
            after = await step(client, final_id, i + 1)
            diff = await page(client, f"axionomy://economy/{tried.structured_content['economy_id']}/diff/{after['economy_id']}")
            assert diff["items"] == []
            assert tried.structured_content["root"] == after["root"]


async def test_malformed_arguments_are_structured_errors_naming_the_argument(url: str) -> None:
    async with Client(url) as client:
        economy_id = await create(client, ECONOMY)
        missing_units = {"rate": "transfer", "bindings": {"giver": "source", "receiver": "sink"}}
        for arguments, pointer in (
            ({"economy_id": economy_id, "exchange": missing_units}, "/exchange/units"),
            ({"economy_id": economy_id, "exchange": {**transfer(1), "rate": "Transfer"}}, "/exchange/rate"),
            ({"economy_id": economy_id, "exchange": {**transfer(1), "units": "-1"}}, "/exchange/units"),
            ({"economy_id": "eco-1", "exchange": transfer(1)}, "/economy_id"),
        ):
            refused = await client.call_tool("axionomy_exchange_apply", arguments)
            assert refused.is_error
            assert refused.structured_content["error"] == "invalid_request", refused.structured_content
            assert refused.structured_content["details"] == {"pointer": pointer}
            assert refused.structured_content["message"].startswith(f"argument `{pointer}` is invalid: ")

        # A path that is a key is named by its JSON pointer, `/` escaped as `~1`.
        refused = await client.call_tool("axionomy_economy_create", {"entries": {"accounts": {"agents/7/Wallet": {}}}})
        assert refused.structured_content["details"] == {"pointer": "/entries/accounts/agents~17~1Wallet"}

        # The schema states the bounds the server keeps.
        tools = {tool.name: tool for tool in (await client.list_tools()).tools}
        search = tools["axionomy_search"].input_schema["properties"]
        assert search["max_expansions"]["minimum"] == 1
        assert (search["chunk_size"]["minimum"], search["chunk_size"]["maximum"]) == (1, 10_000)
        assert tools["axionomy_problem_study"].input_schema["properties"]["budget"]["minimum"] == 1


async def test_bad_resources_are_invalid_params(url: str) -> None:
    async with Client(url) as client:
        economy_id = await create(client, ECONOMY)
        for uri in (
            "axionomy://economy/eco_missing",
            f"axionomy://economy/{economy_id}/accounts?cursor=1.00000000",
            f"axionomy://economy/{economy_id}/nothing",
            f"axionomy://economy/{economy_id}/accounts/Bad_Path",
        ):
            with pytest.raises(MCPError) as raised:
                await client.read_resource(uri)
            assert raised.value.code == -32602, uri
        with pytest.raises(MCPError) as raised:
            await client.read_resource(f"axionomy://economy/{economy_id}/accounts/Bad_Path")
        assert "`Bad_Path` is not an account path" in raised.value.message

        # What a resource names but the economy lacks is named in the error.
        with pytest.raises(MCPError) as raised:
            await client.read_resource(f"axionomy://economy/{economy_id}/accounts/ghost")
        assert raised.value.message.startswith(f"economy `{economy_id}` has no account `ghost`")


def test_requests_without_routing_headers_are_refused(base_url: str) -> None:
    response = httpx.post(
        f"{base_url}/mcp",
        headers={"Accept": "application/json, text/event-stream"},
        json={"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}},
    )
    assert response.status_code == 400


async def test_snapshots_on_disk_outlive_the_server(tmp_path) -> None:
    store = tmp_path / "snapshots.redb"
    with running(store) as base_url:
        async with Client(f"{base_url}/mcp", extensions=[Tasks()]) as client:
            economy_id = await create(client, ECONOMY)
            applied = await client.call_tool("axionomy_exchange_apply", {"economy_id": economy_id, "exchange": transfer(2)})
            applied_id = applied.structured_content["economy_id"]
            studied = await client.call_tool("axionomy_problem_study", {"problem": "maze", "instance": "micro"})
            frames_uri = next(uri for uri in links(studied) if uri.endswith("/frames"))
            frames = await page(client, frames_uri)
    with running(store) as base_url:
        async with Client(f"{base_url}/mcp") as client:
            summary = await page(client, f"axionomy://economy/{applied_id}")
            assert summary["parent_id"] == economy_id
            assert summary["root"] == applied.structured_content["root"]
            again = await client.call_tool("axionomy_exchange_apply", {"economy_id": economy_id, "exchange": transfer(2)})
            assert again.structured_content["economy_id"] == applied_id
            assert again.structured_content["deduplicated"] is True
            # The store keeps a study's key and economies; its frames are
            # computed again, the same as they were.
            assert await page(client, frames_uri) == frames
            for document in studied.structured_content["documents"]:
                kept = await page(client, f"axionomy://economy/{document['final_economy_id']}")
                assert kept["parent_id"] == document["initial_economy_id"]
