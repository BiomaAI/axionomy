"""An application loop over the Studio server's HTTP API (P13, D-046).

Connect Four on the standard board, from the connect-four package's
`connect-four/showcase` point. Two players move the ref `connect-four/live`
forward, each from where it saw the ref: a player drops a piece when the
moves at the ref's point are its drops, and both adjudicate every drop, so
each adjudication is proposed twice from one point and one of the two is
refused with where the ref went. A thinker tries every move out at the ref's
point without ever moving the ref, and a learner follows the ref through its
frames, waiting for each with a long poll. Only the steps the ref is moved
over reach the learner, each named as the point its player advanced to."""

import random
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from decimal import Decimal
from typing import Any, Self
from urllib.parse import quote

import httpx

PACKAGE = "connect-four"
# The standard 7×6 board, where every drop waits on its adjudication.
START = "connect-four/showcase"
LIVE = "connect-four/live"
MOVES = "connect-four/moves"
# Each player's value where the game ends, in tenths: ten for a win, five
# for a draw, none for a loss; none before it ends.
OBJECTIVES = ("connect-four/red", "connect-four/yellow")
# Each player drops into one of its legal columns at random, from its seed.
SEEDS = {"red": 13, "yellow": 31}
# The longest a long poll waits before its caller looks again.
WAIT_SECONDS = 10
# The longest the whole game may take.
GAME_SECONDS = 120

Exchange = dict[str, Any]
Frame = dict[str, Any]


def step(point: str) -> int:
    """A point's step: a `PointId` is its run, `:` and its step."""
    return int(point.rsplit(":", 1)[1])


def drop_by(exchange: Exchange) -> str | None:
    """The player whose drop `exchange` is, or nothing for an adjudication.
    A drop's rate is `standard-move/<player>/<column>/<row>`; every other
    move settles the drop before it: a line claimed, the draw on a full
    board, or the turn passing on."""
    kind, *rest = exchange["rate"].split("/")
    return rest[0] if kind == "standard-move" else None


def decided(frame: Frame) -> bool:
    """Whether the game is over at the frame's point: a player's value is
    worth something only once it is."""
    return any(Decimal(value) for value in frame["values"])


class Studio:
    """What an application asks of the Studio server, over a connection of
    its own."""

    def __init__(self, base_url: str) -> None:
        self.http = httpx.Client(base_url=base_url, timeout=WAIT_SECONDS + 20)
        self.ref = f"/api/refs/{quote(LIVE, safe='')}"

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *_: object) -> None:
        self.http.close()

    @staticmethod
    def ok(response: httpx.Response) -> Any:
        assert response.status_code == 200, f"{response.request.url}: {response.status_code} {response.text}"
        return response.json()

    def package(self) -> dict[str, Any]:
        (package,) = [p for p in self.ok(self.http.get("/api/packages"))["packages"] if p["name"] == PACKAGE]
        return package

    def refer(self, point: str) -> None:
        created = self.ok(self.http.post("/api/refs", json={"name": LIVE, "point": point}))
        assert created == {"name": LIVE, "point": point}

    def live(self) -> str:
        return self.ok(self.http.get(self.ref))["point"]

    def candidates(self, point: str) -> list[Exchange]:
        offered = self.ok(self.http.get(f"/api/points/{point}/candidates", params={"set": MOVES}))
        assert offered["point"] == point
        # Each is assessed where it is offered, and the set offers only
        # moves that apply.
        assert all(c["proposal"]["assessment"]["status"] == "applicable" for c in offered["candidates"])
        return [c["exchange"] for c in offered["candidates"]]

    def apply(self, point: str, exchanges: list[Exchange]) -> str:
        applied = self.ok(self.http.post(f"/api/points/{point}/apply", json={"exchanges": exchanges}))
        assert applied["from"] == point
        assert step(applied["point"]) == step(point) + len(exchanges)
        return applied["point"]

    def advance(self, expected: str, to: str) -> httpx.Response:
        return self.http.post(f"{self.ref}/advance", json={"expected": expected, "to": to})

    def frames(self, after: str, **query: Any) -> dict[str, Any]:
        return self.ok(self.http.get(f"{self.ref}/frames", params={"after": after, **query}))

    def moved_from(self, point: str) -> str:
        """Where the ref is once it has moved from `point`, waiting for it
        with a long poll; `point` again if it has not moved by then."""
        return self.frames(point, limit=1, wait_seconds=WAIT_SECONDS)["point"]

    def ancestor(self, point: str, at: int) -> str:
        return self.ok(self.http.get(f"/api/points/{point}/ancestor", params={"step": at}))["point"]


@dataclass(frozen=True)
class Advanced:
    player: str
    expected: str
    to: str
    exchange: Exchange


@dataclass(frozen=True)
class Refused:
    player: str
    expected: str
    tried: str
    # Where the 409 said the ref was.
    current: str


class Table:
    """What the players did to the ref, as each reports it."""

    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.advanced: list[Advanced] = []
        self.refused: list[Refused] = []
        # Advances asked for again once they counted, and told they had.
        self.repeated: list[Advanced] = []
        # Both players adjudicate each drop and meet first, so both propose
        # from the same point before either moves the ref.
        self.meet = threading.Barrier(2, timeout=GAME_SECONDS)


def play(base_url: str, player: str, table: Table) -> None:
    """Drops when the moves at the ref's point are this player's, and
    adjudicates every drop; looks again from where the ref is when the other
    player's move came first. Stops when no move is left."""
    chooser = random.Random(SEEDS[player])
    with Studio(base_url) as studio:
        point = studio.live()
        while offered := studio.candidates(point):
            dropping = drop_by(offered[0])
            if dropping not in (None, player):
                point = studio.moved_from(point)
                continue
            exchange = chooser.choice(offered) if dropping else offered[0]
            made = studio.apply(point, [exchange])
            if dropping is None:
                table.meet.wait()
            answer = studio.advance(point, made)
            if answer.status_code == 200:
                assert answer.json() == {"name": LIVE, "point": made}
                advanced = Advanced(player, point, made, exchange)
                if dropping:
                    # The ref stays at a drop until both players meet to
                    # adjudicate it, so asking again for the move made is
                    # told it was made.
                    again = studio.advance(point, made)
                    assert again.status_code == 200, f"advance again: {again.status_code} {again.text}"
                    assert again.json() == answer.json()
                with table.lock:
                    table.advanced.append(advanced)
                    if dropping:
                        table.repeated.append(advanced)
                point = made
            else:
                assert answer.status_code == 409, f"advance: {answer.status_code} {answer.text}"
                moved = answer.json()
                assert moved["name"] == LIVE
                with table.lock:
                    table.refused.append(Refused(player, point, made, moved["current"]))
                point = moved["current"]


def think(base_url: str) -> list[str]:
    """Tries every move out at each point it finds the ref on, never moving
    the ref: every point it made."""
    thought: list[str] = []
    with Studio(base_url) as studio:
        point = studio.live()
        while offered := studio.candidates(point):
            thought += [studio.apply(point, [exchange]) for exchange in offered]
            point = studio.moved_from(point)
    return thought


def learn(base_url: str, start: str) -> list[Frame]:
    """Every step the ref is moved over after `start`, each measuring both
    players' values, until one says the game is over."""
    frames: list[Frame] = []
    with Studio(base_url) as studio:
        after = start
        while not frames or not decided(frames[-1]):
            delivered = studio.frames(after, objectives=",".join(OBJECTIVES), wait_seconds=WAIT_SECONDS)["frames"]
            frames += delivered
            if delivered:
                after = delivered[-1]["point"]
    return frames


class Role(threading.Thread):
    """Runs `work` in a thread of its own, keeping what it returns or
    raises."""

    def __init__(self, name: str, work: Callable[[], Any], table: Table) -> None:
        super().__init__(name=name, daemon=True)
        self.work = work
        self.table = table
        self.result: Any = None
        self.error: BaseException | None = None

    def run(self) -> None:
        try:
            self.result = self.work()
        except BaseException as error:
            self.error = error
            # No player waits to meet one that has stopped.
            self.table.meet.abort()


def outcomes(roles: list[Role]) -> list[Any]:
    """What each role returned, once all have finished. If any failed, the
    first failure that is not a meeting broken off, which only follows
    another."""
    deadline = time.monotonic() + GAME_SECONDS
    for role in roles:
        role.join(max(0.0, deadline - time.monotonic()))
    unfinished = [role.name for role in roles if role.is_alive()]
    assert not unfinished, f"{unfinished} did not finish within {GAME_SECONDS} seconds"
    errors = [role.error for role in roles if role.error is not None]
    if errors:
        raise next((e for e in errors if not isinstance(e, threading.BrokenBarrierError)), errors[0])
    return [role.result for role in roles]


def alone(studio: Studio, start: str) -> list[Exchange]:
    """The same players' game from `start` with nobody else at the table,
    applied a move at a time, with no ref."""
    choosers = {player: random.Random(seed) for player, seed in SEEDS.items()}
    played: list[Exchange] = []
    point = start
    while offered := studio.candidates(point):
        dropping = drop_by(offered[0])
        exchange = choosers[dropping].choice(offered) if dropping else offered[0]
        point = studio.apply(point, [exchange])
        played.append(exchange)
    return played


def test_two_players_a_thinker_and_a_learner_share_a_live_ref(studio: str) -> None:
    with Studio(studio) as client:
        package = client.package()
        start = {ref["name"]: ref["point"] for ref in package["refs"]}[START]
        assert MOVES in package["candidates"]
        assert {o["objective"] for o in package["objectives"]} >= set(OBJECTIVES)
        client.refer(start)

        table = Table()
        roles = [
            Role("red", lambda: play(studio, "red", table), table),
            Role("yellow", lambda: play(studio, "yellow", table), table),
            Role("thinker", lambda: think(studio), table),
            Role("learner", lambda: learn(studio, start), table),
        ]
        for role in roles:
            role.start()
        _, _, thought, frames = outcomes(roles)
        final = client.live()

        # The ref moved only from where its movers saw it: of the advances
        # from one point at most one counted, and those that counted lead
        # from the start to where the ref is.
        counted: dict[str, Advanced] = {}
        for advanced in table.advanced:
            assert advanced.expected not in counted, f"two advances counted from {advanced.expected}"
            counted[advanced.expected] = advanced
        game: list[Advanced] = []
        point = start
        while point in counted:
            game.append(counted[point])
            point = counted[point].to
        assert point == final
        assert len(game) == len(table.advanced)

        # Every advance that did not count was refused with where the ref
        # was: where the other player's advance from the same point had
        # moved it. Only adjudications are proposed twice, and each was
        # refused once.
        for refused in table.refused:
            first = counted[refused.expected]
            assert refused.current == first.to
            assert refused.player != first.player
        adjudications = [advanced.expected for advanced in game if drop_by(advanced.exchange) is None]
        assert adjudications
        assert sorted(refused.expected for refused in table.refused) == sorted(adjudications)

        # Every drop's advance was asked for again once it counted, and told
        # it was made; the move was made once, so the frames below hold each
        # step once.
        assert sorted(again.to for again in table.repeated) == sorted(
            advanced.to for advanced in game if drop_by(advanced.exchange) is not None
        )

        # The learner received exactly the steps the players advanced over,
        # in order, each the step after the one before, ending where the ref
        # is. A point has one name, so each frame names the point its player
        # advanced to, however late it was delivered, as the history of
        # where the ref is names it too.
        assert [frame["exchange"] for frame in frames] == [advanced.exchange for advanced in game]
        for index, (frame, advanced) in enumerate(zip(frames, game, strict=True), start=1):
            assert step(frame["point"]) == step(start) + index
            assert frame["point"] == advanced.to
            assert client.ancestor(final, step(start) + index) == advanced.to
            assert frame["receipt"]["exchange"] == advanced.exchange
        assert frames[-1]["point"] == final

        # A step only tried out is never delivered: neither one the thinker
        # made nor one an advance refused would have moved the ref to.
        assert thought
        tried = set(thought) | {refused.tried for refused in table.refused}
        assert tried.isdisjoint(frame["point"] for frame in frames)

        # The game is over where the ref is: no move is left, and it was won
        # or drawn there and not before.
        assert client.candidates(final) == []
        assert not any(decided(frame) for frame in frames[:-1])
        assert sorted(Decimal(value) for value in frames[-1]["values"]) in ([0, 10], [5, 5])

        # None of it changed the game: the players alone, from the same
        # start, make the same moves.
        assert alone(client, start) == [advanced.exchange for advanced in game]

        # Studio lists the ref with the package, where the game ended.
        assert {"name": LIVE, "point": final} in client.package()["refs"]
