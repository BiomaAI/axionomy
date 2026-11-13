import { expect, test } from "@playwright/test";

test("studies a problem in a Rust Worker and loads the verified study", async ({ page }) => {
  await page.goto("/?studies");
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("logistics");
  await page.getByLabel("Budget").fill("8");
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(page.getByText("Live thinking")).toBeVisible();
  await expect(page.getByText("New study computed and loaded")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByText("Saved thinking")).toBeVisible();
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await expect(page.getByText("Cold van", { exact: true }).first()).toBeVisible();
  await expect(page.locator(".graph-scene")).toBeVisible();
  await page.locator(".react-flow__node.occupant-overlay").filter({ hasText: "Cold van" }).click();
  await expect(page.locator(".account-card.focused")).toContainText("Cold van");
});

test("cancels a browser study by terminating its isolated Worker", async ({ page }) => {
  await page.goto("/?studies");
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("logistics");
  await page.getByRole("combobox", { name: "Instance (size)", exact: true }).selectOption("stress");
  await page.getByLabel("Budget").fill("2048");
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Study", exact: true })).toBeEnabled();
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible();
});

test("browser Worker streams the same verified Work League frames", async ({ page }) => {
  await page.goto("/?problem=work-league&instance=micro&strategy=mixed-field&view=thinking&step=0&seed=17&budget=32");
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await page.getByRole("button", { name: "Study", exact: true }).click();
  await expect(page.locator(".live-standings")).toContainText(/(?:Live|Last) verified frame/, { timeout: 30_000 });
  await expect(page.getByText("New study computed and loaded")).toBeVisible({ timeout: 60_000 });
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await expect(page.getByText("Who is winning depends on what you value")).toBeVisible();
  await expect(page.getByLabel("Study on screen")).toContainText("Micro");
  expect(Number(await page.getByRole("slider", { name: "Trace position" }).getAttribute("max"))).toBeLessThan(100);
});

test("a Worker loads a package from a URL, runs its experiment, and finds both again after a reload", async ({ page }) => {
  const served: number[] = [];
  page.on("response", (response) => {
    if (new URL(response.url()).pathname === "/packages/maze.zip") served.push(response.status());
  });
  // The study workspace, whose Worker leaves the store alone for this test's.
  await page.goto("/?studies");
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
  await page.goto("/?studies");
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
    const offered = await packages.candidates(micro, "maze/moves");
    const exchange = offered.candidates.find((candidate: { proposal: { assessment: { status: string } } }) => candidate.proposal.assessment.status === "applicable").exchange;

    // The ref does not move for a while; then another request moves it.
    const asked = posted.length;
    const waited = performance.now();
    const waiting = packages.frames("maze/micro", { after: micro, wait_seconds: 20 });
    await new Promise((resolve) => setTimeout(resolve, 1_000));
    const applied = await packages.apply(micro, [exchange]);
    await packages.advanceRef("maze/micro", micro, applied.point);
    const moved = await waiting;
    const movedAfter = performance.now() - waited;
    const requests = posted.slice(asked);

    // Nothing moves it: the wait ends with its time, with no frames.
    const idleFrom = performance.now();
    const idle = await packages.frames("maze/micro", { after: applied.point, wait_seconds: 1 });
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

test("a second Worker keeps its store in memory while another holds the saved one", async ({ page }) => {
  await page.goto("/?studies");
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
