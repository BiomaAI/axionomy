import { expect, test } from "@playwright/test";

/** A page of Studio's origin that runs no Studio, so no Worker of Studio's holds the store a test's own Workers open. */
const BLANK = "/tests/worker.html";

test("a Worker loads a package from a URL, runs its experiment, and finds both again after a reload", async ({ page }) => {
  const served: number[] = [];
  page.on("response", (response) => {
    if (new URL(response.url()).pathname === "/packages/maze.zip") served.push(response.status());
  });
  await page.goto(BLANK);
  // The client module as Vite serves it, which Studio's packages workspace uses.
  const client = "/src/browserPackages.ts";
  const first = await page.evaluate(async (client) => {
    const { BrowserPackages } = await import(client);
    const packages = new BrowserPackages();
    const opened = await packages.open();
    const fetched = await packages.loadUrl("/packages/maze.zip");
    const experiments = await packages.experiments();
    let progressed = 0;
    const results = await packages.runExperiment("maze/micro", () => { progressed += 1; }).result;
    const document = await packages.draw(results.arms[0].trials[0].end);
    return { opened, fetched, experiments, results, progressed, frames: document.frames.length };
  }, client);
  expect(first.opened).toEqual({ persistent: true });
  expect(first.fetched.got).toBe("loaded");
  expect(first.fetched.loaded.packages).toEqual(["maze"]);
  expect(first.experiments).toContain("maze/micro");
  expect(first.results.arms.length).toBeGreaterThan(1);
  expect(first.progressed).toBeGreaterThan(0);
  expect(first.frames).toBeGreaterThan(0);

  // The page's Workers end with it; the next one opens the same store.
  await page.reload();
  const again = await page.evaluate(async (client) => {
    const { BrowserPackages } = await import(client);
    const packages = new BrowserPackages();
    const opened = await packages.open();
    const held = await packages.packages();
    const results = await packages.results("maze/micro");
    const fetched = await packages.loadUrl("/packages/maze.zip");
    return { opened, held: held.packages.map((listed: { name: string }) => listed.name), results, fetched };
  }, client);
  expect(again.opened).toEqual({ persistent: true });
  expect(again.held).toEqual(["maze"]);
  expect(again.results).toEqual(first.results);
  // Asked only whether it changed: the URL answers 304 and nothing is downloaded.
  expect(again.fetched).toEqual({ url: first.fetched.url, got: "current", status: 304, packages: ["maze"] });
  expect(served).toEqual([200, 304]);
});

test("a follower waits in the Worker for its ref to move, asking nothing in the meantime", async ({ page }) => {
  await page.goto(BLANK);
  const followed = await page.evaluate(async (client) => {
    // Every request the page posts to a Worker, by kind.
    const posted: string[] = [];
    const post = Worker.prototype.postMessage;
    Worker.prototype.postMessage = function (this: Worker, message: { kind?: string; request?: { kind: string } }, ...rest: never[]) {
      if (message?.kind === "store" && message.request) posted.push(message.request.kind);
      return (post as (message: unknown, ...rest: never[]) => void).call(this, message, ...rest);
    } as typeof Worker.prototype.postMessage;
    const { BrowserPackages } = await import(client);
    const packages = new BrowserPackages();
    await packages.open();
    await packages.loadUrl("/packages/maze.zip");
    const listed = await packages.packages();
    const micro: string = listed.packages[0].refs.find((ref: { name: string }) => ref.name === "maze/micro").point;
    // The package's ref is fixed: the page follows and moves one of its own there.
    await packages.createRef("maze/followed", micro);
    const offered = await packages.candidates(micro, "maze/moves");
    const exchange = offered.candidates.find((candidate: { proposal: { assessment: { status: string } } }) => candidate.proposal.assessment.status === "applicable").exchange;

    // The ref does not move for a while; then another request moves it.
    const asked = posted.length;
    const waited = performance.now();
    const waiting = packages.frames("maze/followed", { after: micro, wait_seconds: 20 });
    await new Promise((resolve) => setTimeout(resolve, 1_000));
    const applied = await packages.apply(micro, [exchange]);
    await packages.advanceRef("maze/followed", micro, applied.point);
    const moved = await waiting;
    const movedAfter = performance.now() - waited;
    const requests = posted.slice(asked);

    // Nothing moves it: the wait ends with its time, with no frames.
    const idleFrom = performance.now();
    const idle = await packages.frames("maze/followed", { after: applied.point, wait_seconds: 1 });
    const idleFor = performance.now() - idleFrom;
    packages.terminate();
    return { moved, applied: applied.point, movedAfter, requests, idle, idleFor };
  }, "/src/browserPackages.ts");
  expect(followed.moved.point).toBe(followed.applied);
  expect(followed.moved.frames.map((frame: { point: string }) => frame.point)).toEqual([followed.applied]);
  expect(followed.movedAfter).toBeGreaterThanOrEqual(1_000);
  expect(followed.movedAfter).toBeLessThan(10_000);
  // The follower asked once and waited: the move answered it.
  expect(followed.requests).toEqual(["frames", "apply", "advance_ref"]);
  expect(followed.idle.point).toBe(followed.applied);
  expect(followed.idle.frames).toEqual([]);
  expect(followed.idleFor).toBeGreaterThanOrEqual(1_000);
});

test("each change to the store reaches the page as it is made, a run's branches before the run ends", async ({ page }) => {
  await page.goto(BLANK);
  const heard = await page.evaluate(async (client) => {
    const { BrowserPackages } = await import(client);
    const packages = new BrowserPackages();
    await packages.open();
    const told: { kind: string; at: number }[] = [];
    packages.onChange((change: { kind: string }) => told.push({ kind: change.kind, at: performance.now() }));
    await packages.loadUrl("/packages/maze.zip");
    const loaded = told.length;
    const before = await packages.histories("maze");
    const results = await packages.runExperiment("maze/micro", () => undefined).result;
    const ended = performance.now();
    const after = await packages.histories("maze");
    packages.terminate();
    const ran = told.slice(loaded);
    return {
      loaded: told.slice(0, loaded).map((change) => change.kind),
      runs: ran.filter((change) => change.kind === "run").map((change) => change.at),
      recorded: ran.filter((change) => change.kind === "definitions").length,
      ended,
      trials: results.arms.length,
      grew: after.segments.length - before.segments.length,
    };
  }, "/src/browserPackages.ts");
  expect(heard.loaded).toEqual(["definitions"]);
  // Each trial's branch is told on its own, as it is recorded, before the run gives its results.
  expect(heard.runs).toHaveLength(heard.trials);
  for (const at of heard.runs) expect(at).toBeLessThanOrEqual(heard.ended);
  expect(heard.recorded).toBeGreaterThan(0);
  expect(heard.grew).toBe(heard.trials);
});

test("a second Worker keeps its store in memory while another holds the saved one", async ({ page }) => {
  await page.goto(BLANK);
  const opened = await page.evaluate(async (client) => {
    const { BrowserPackages } = await import(client);
    const holder = new BrowserPackages();
    const first = await holder.open();
    const second = await new BrowserPackages().open();
    return { first, second };
  }, "/src/browserPackages.ts");
  expect(opened.first).toEqual({ persistent: true });
  expect(opened.second).toEqual({ persistent: false, reason: "another tab holds the saved store" });
});

test("cancelling a run terminates its Worker mid-run, and the next one opens the store with all committed before", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto(BLANK);
  const client = "/src/browserPackages.ts";
  const first = await page.evaluate(async (client) => {
    let terminated = 0;
    const terminate = Worker.prototype.terminate;
    Worker.prototype.terminate = function (this: Worker) {
      terminated += 1;
      return terminate.call(this);
    };
    const { BrowserPackages } = await import(client);
    const packages = new BrowserPackages();
    await packages.open();
    await packages.loadUrl("/packages/connect-four.zip");
    const listed = await packages.packages();
    const refs = listed.packages.find((listed: { name: string }) => listed.name === "connect-four").refs;
    const start: string = refs.find((ref: { name: string }) => ref.name === "connect-four/stress").point;
    const move = async (at: string) => {
      const offered = await packages.candidates(at, "connect-four/moves");
      const exchange = offered.candidates.find((candidate: { proposal: { assessment: { status: string } } }) => candidate.proposal.assessment.status === "applicable").exchange;
      return (await packages.apply(at, [exchange])).point as string;
    };
    // Committed before the cancel: a drop at the point of a ref the package
    // ships, which is fixed, a ref of the page's own named there and moved
    // over it, and another naming it.
    await packages.createRef("connect-four/live", start);
    const dropped = await move(start);
    await packages.advanceRef("connect-four/live", start, dropped);
    await packages.createRef("connect-four/kept", dropped);
    // A follower waiting for the ref across the cancel.
    const following = packages.frames("connect-four/live", { after: dropped, wait_seconds: 30 });

    // A game, cancelled once it has reported its progress a few times.
    let reports = 0;
    const run = packages.runStrategy(dropped, { strategy: "connect-four/mcts-32/self-play", goal: "connect-four/red-wins", seed: "17", budget: "1", turns: "1000" }, () => { reports += 1; });
    while (reports < 3) await new Promise((resolve) => setTimeout(resolve, 10));
    // Asked while the game runs, so answered by the next Worker.
    const reading = packages.readRef("connect-four/kept");
    run.cancel();
    const cancelled = await run.result.then(() => "it ended", (cause: Error) => cause.message);
    // Mid-game: the Worker running it was terminated.
    const terminatedMidRun = terminated;
    const kept = await reading;
    const opened = await packages.open();
    // The follower goes on following: the next move reaches it.
    const adjudicated = await move(dropped);
    await packages.advanceRef("connect-four/live", dropped, adjudicated);
    const followed = await following;
    packages.terminate();
    return { start, dropped, adjudicated, cancelled, terminatedMidRun, kept, opened, followed };
  }, client);
  expect(first.cancelled).toBe("cancelled by terminating the Worker");
  expect(first.terminatedMidRun).toBe(1);
  expect(first.kept).toEqual({ name: "connect-four/kept", point: first.dropped });
  expect(first.opened).toEqual({ persistent: true });
  expect(first.followed.point).toBe(first.adjudicated);
  expect(first.followed.frames.map((frame: { point: string }) => frame.point)).toEqual([first.adjudicated]);

  // A page loaded again opens the same store: everything committed is there.
  await page.reload();
  const again = await page.evaluate(async (client) => {
    const { BrowserPackages } = await import(client);
    const packages = new BrowserPackages();
    const opened = await packages.open();
    const shipped = await packages.readRef("connect-four/stress");
    const live = await packages.readRef("connect-four/live");
    const kept = await packages.readRef("connect-four/kept");
    const drawn = await packages.draw(live.point);
    return { opened, shipped: shipped.point, live: live.point, kept: kept.point, steps: drawn.frames.length };
  }, client);
  expect(again.opened).toEqual({ persistent: true });
  expect(again.shipped).toBe(first.start);
  expect(again.live).toBe(first.adjudicated);
  expect(again.kept).toBe(first.dropped);
  expect(again.steps).toBeGreaterThanOrEqual(2);
});
