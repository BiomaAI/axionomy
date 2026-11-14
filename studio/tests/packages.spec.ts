import { expect, test, type Page } from "@playwright/test";

/**
 * Studio on packages, run against the native server and against the engine
 * in the browser's Worker alike: both engines offer the same operations, so
 * the same steps give the same pages. The history tree is how a history is
 * opened; what to do from a point is beside the picture. A package's refs
 * are fixed, so a test that moves a ref names one of its own at a package's
 * ref's point, a name no other run gives, since the native server's store
 * outlives a test.
 */

/** Opens the sample `name` by its link: loaded, or found current in the store. */
async function openSample(page: Page, name: string) {
  await page.goto(`/?package=packages/${name}.zip`);
  await expect(page.getByRole("combobox", { name: "Package", exact: true })).toHaveValue(name, { timeout: 30_000 });
  await expect(page.getByRole("group", { name: "History tree" })).toBeVisible();
}

const trace = (page: Page) => page.getByRole("slider", { name: "Trace position" });
const fromHere = (page: Page) => page.locator(".action-panel");
const tree = (page: Page) => page.getByRole("group", { name: "History tree" });

/** How many steps the history on screen has, once it is drawn. */
async function steps(page: Page): Promise<number> {
  await expect(page.locator(".point-cockpit")).toHaveAttribute("aria-busy", "false");
  return Number(await trace(page).getAttribute("max"));
}

/** Opens a ref from its label in the tree, and gives how many steps its history has. */
async function openRef(page: Page, name: string): Promise<number> {
  await tree(page).getByRole("button", { name: `Open ${name}`, exact: true }).click();
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText(`of ${name}`);
  return await steps(page);
}

/** A ref name under `pkg` no other run gives. */
function ownRef(pkg: string): string {
  return `${pkg}/play/${Date.now().toString(36)}${Math.floor(Math.random() * 1e6).toString(36)}`;
}

/**
 * Opens the package's ref `shipped`, which is fixed, and names a ref of
 * one's own at its point; gives how many steps its history has.
 */
async function playFrom(page: Page, shipped: string, name: string): Promise<number> {
  const start = await openRef(page, shipped);
  await expect(fromHere(page)).toContainText(`${shipped} is fixed`);
  await fromHere(page).getByRole("textbox", { name: "Ref name" }).fill(name);
  await fromHere(page).getByRole("button", { name: "Name it" }).click();
  await expect(page.getByRole("status").filter({ hasText: `${name} names step ${start} now` })).toBeVisible();
  await expect(fromHere(page)).toContainText(`What you do here moves ${name} forward`);
  expect(await steps(page)).toBe(start);
  return start;
}

/** The first move offered here that applies, applied. */
async function applyFirst(page: Page) {
  await fromHere(page).locator(".move-applicable").first().getByRole("button", { name: "Apply" }).click();
}

test("a sample opens from the package picker, and its tree opens its refs", async ({ page }) => {
  await page.goto("/");
  const picker = page.getByRole("combobox", { name: "Package", exact: true });
  await expect(picker).toBeEnabled({ timeout: 30_000 });
  // A sample not in the store yet loads; one in it opens.
  const option = picker.locator("option", { hasText: /^sokoban/ }).first();
  await picker.selectOption(await option.getAttribute("value") ?? "");
  await expect(picker).toHaveValue("sokoban", { timeout: 30_000 });
  await expect(tree(page).getByRole("button", { name: "Open sokoban/micro/deadlock" })).toBeVisible();
  expect(await openRef(page, "sokoban/micro/deadlock")).toBe(3);
  // A ref opens where it is: the end of its history.
  await expect(trace(page)).toHaveValue("3");
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await expect(trace(page)).toHaveValue("0");
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page.locator(".ticker-panel .panel-heading small")).toHaveText("1 / 3");
  await expect(page.locator(".transition-panel .assessment-applicable")).toContainText("applicable");
  // Picking a step of the history on screen in the tree moves the cursor there.
  await tree(page).getByRole("button", { name: /^Step 2 of / }).first().click();
  await expect(trace(page)).toHaveValue("2");
  await expect(page.locator(".stage .world-panel")).toBeVisible();
  await expect(page.locator(".stage .accounts-panel")).toBeVisible();
});

test("opens a package and plays it: Play finds the moves where nothing has happened yet", async ({ page }) => {
  await openSample(page, "sokoban");
  expect(await openRef(page, "sokoban/micro")).toBe(0);
  const branches = await tree(page).locator(".tree-branch").count();
  await page.getByRole("button", { name: "Play", exact: true }).click();
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible({ timeout: 45_000 });
  await expect.poll(async () => Number(await trace(page).getAttribute("max"))).toBeGreaterThan(0);
  await expect.poll(async () => Number(await trace(page).inputValue()), { timeout: 15_000 }).toBeGreaterThan(0);
  // The moves it found are a branch of the tree.
  await expect(tree(page).locator(".tree-branch")).toHaveCount(branches + 1);
  // Space pauses it, as the transport says.
  await page.locator("body").press(" ");
  await expect(page.getByRole("button", { name: "Play", exact: true })).toBeVisible();
});

test("applies a move at a ref of one's own, moving it, and anywhere else makes a branch", async ({ page }) => {
  await openSample(page, "maze");
  // The package's ref is fixed: at its point, applying makes a branch.
  await openRef(page, "maze/showcase");
  await expect(fromHere(page)).toContainText("maze/showcase is fixed: what you do here makes a branch");
  const mine = ownRef("maze");
  const start = await playFrom(page, "maze/showcase", mine);
  // Each move is shown checked before anything is applied.
  await fromHere(page).locator(".move-applicable .move-name").first().click();
  await expect(fromHere(page).locator(".move-detail .assessment-applicable")).toBeVisible();
  await applyFirst(page);
  await expect(page.getByRole("status").filter({ hasText: `${mine} moved to step ${start + 1}` })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));

  // Back at step 0 the point is not where the ref is: applying there branches.
  const branches = await tree(page).locator(".tree-branch").count();
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await expect(fromHere(page)).toContainText("What you do here makes a branch from step 0");
  await fromHere(page).locator(".move-applicable").last().getByRole("button", { name: "Apply" }).click();
  await expect(page.getByRole("status").filter({ hasText: "a new branch from step 0" })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", "1");
  await expect(tree(page).locator(".tree-branch")).toHaveCount(branches + 1);
  // The ref stayed where the move took it, and the package's where it shipped.
  expect(await openRef(page, mine)).toBe(start + 1);
  expect(await openRef(page, "maze/showcase")).toBe(start);
  expect(await openRef(page, mine)).toBe(start + 1);

  // A move written by hand is checked before it can be applied.
  await fromHere(page).locator(".exchange-editor summary").click();
  await fromHere(page).getByRole("textbox", { name: "Exchange" }).fill('{"rate": "nowhere", "bindings": {}, "units": "1"}');
  const apply = fromHere(page).locator(".exchange-editor").getByRole("button", { name: "Apply" });
  await expect(apply).toBeDisabled();
  await fromHere(page).getByRole("button", { name: "Check it" }).click();
  await expect(fromHere(page).locator(".exchange-editor .assessment-invalid")).toBeVisible();
  await expect(apply).toBeDisabled();

  // One's own ref is deleted; its history stays on screen and in the tree.
  await fromHere(page).getByRole("button", { name: "Delete it" }).click();
  await expect(page.getByRole("status").filter({ hasText: `${mine} is deleted` })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await expect(tree(page).getByRole("button", { name: `Open ${mine}` })).toHaveCount(0);
  // A package's ref offers no deleting.
  await openRef(page, "maze/showcase");
  await expect(fromHere(page).getByRole("button", { name: "Delete it" })).toHaveCount(0);
});

test("lets a strategy play from here: its branch opens and plays", async ({ page }) => {
  await openSample(page, "maze");
  const start = await openRef(page, "maze/showcase");
  const play = fromHere(page).getByRole("region", { name: "Let a strategy play" });
  await play.getByRole("combobox", { name: "Strategy" }).selectOption("maze/a-star");
  // The goal and the effort are those of the experiment that runs the strategy.
  await expect(play.locator("summary")).toContainText("Toward solved, seed 17");
  await play.getByRole("button", { name: "Play it" }).click();
  await expect(page.getByRole("status").filter({ hasText: /a-star reached the goal in \d+ steps, after looking at/ })).toBeVisible({ timeout: 30_000 });
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText(`a-star, from step ${start}`);
  expect(await steps(page)).toBeGreaterThan(start);
  // The package's ref stays where it shipped.
  expect(await openRef(page, "maze/showcase")).toBe(start);
});

test("follows a ref and receives the steps it is moved over", async ({ page }) => {
  await openSample(page, "maze");
  const mine = ownRef("maze");
  const start = await playFrom(page, "maze/stress", mine);
  await fromHere(page).getByRole("button", { name: "Follow it" }).click();
  // Following goes on while this page acts at the ref's point.
  await applyFirst(page);
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await expect(fromHere(page).getByRole("list", { name: "Delivered steps" })).toContainText(`step ${start + 1}`);
  await expect(fromHere(page).getByRole("button", { name: "Stop following" })).toBeVisible();
});

test("stops a game mid-play, and nothing else notices", async ({ page }) => {
  test.setTimeout(120_000);
  await openSample(page, "connect-four");
  // The package's ref is fixed: the game is followed and moved on one of its own.
  const live = ownRef("connect-four");
  const start = await playFrom(page, "connect-four/stress", live);
  await fromHere(page).getByRole("button", { name: "Follow it" }).click();
  const play = fromHere(page).getByRole("region", { name: "Let a strategy play" });
  await play.getByRole("combobox", { name: "Strategy" }).selectOption("connect-four/mcts-32/self-play");
  await play.getByRole("button", { name: "Play it" }).click();
  await expect(page.locator(".scrubber-label")).toContainText("Finding the moves to play");
  await page.waitForTimeout(1_500);
  await play.getByRole("button", { name: "Stop" }).click();
  await expect(play.getByRole("button", { name: "Play it" })).toBeEnabled({ timeout: 30_000 });
  await expect(page.getByRole("alert")).toHaveCount(0);
  // The follower never noticed: it is still waiting, and hears the next move.
  await expect(fromHere(page).getByRole("button", { name: "Stop following" })).toBeVisible();
  await applyFirst(page);
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await expect(fromHere(page).getByRole("list", { name: "Delivered steps" })).toContainText(`step ${start + 1}`);
});

test("plays an experiment from where it starts, and each trial is a branch to watch", async ({ page }) => {
  test.setTimeout(90_000);
  await openSample(page, "maze");
  await openRef(page, "maze/micro");
  const branches = await tree(page).locator(".tree-branch").count();
  const here = fromHere(page).getByRole("region", { name: "Experiments from here" });
  await here.getByRole("button", { name: /^Play (each arm|again)$/ }).first().click();
  await expect(here.getByRole("status")).toContainText(/Played/, { timeout: 60_000 });
  // Five strategies, each a trial that reached the goal, each a new branch.
  await expect(here.locator(".trial-list li")).toHaveCount(5);
  await expect(here.locator(".trial-list")).toContainText("a-star");
  await expect(here.locator(".trial-list")).toContainText("reached the goal");
  await expect(tree(page).locator(".tree-branch")).toHaveCount(branches + 5);
  await expect(tree(page).locator(".tone-trial").first()).toBeVisible();
  // Played again with the same seeds, it says the results are the same.
  await here.getByRole("button", { name: "Play again" }).first().click();
  await expect(here.getByRole("status")).toContainText("the same seeds gave the same results", { timeout: 60_000 });
  // A trial is watched: its branch opens where it started and plays.
  await here.locator(".trial-list li", { hasText: "a-star" }).getByRole("button", { name: "Watch" }).click();
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("a-star");
  expect(await steps(page)).toBeGreaterThan(0);
  // A branch is set against the history it left: the same up to where it split, then step by step.
  const comparison = page.getByRole("region", { name: "Comparison" });
  await expect(comparison).toContainText("The same up to step 0, then they part.");
  await expect(comparison.locator("tbody tr").first()).toContainText("Walk Start");
  // Any branch of the same tree is compared from its card in the tree.
  await tree(page).getByRole("button", { name: /^Step 3 of breadth-first/ }).last().hover();
  await page.getByRole("button", { name: "Compare with the history on screen" }).click();
  await expect(comparison.locator(".panel-heading h2")).toContainText("breadth-first");
  await expect(comparison.locator(".compare-ends")).toContainText("energy");
});

test("a link names a package by its URL and opens it", async ({ page }) => {
  await page.goto("/?package=/packages/sokoban.zip");
  await expect(page.getByRole("combobox", { name: "Package", exact: true })).toHaveValue("sokoban", { timeout: 30_000 });
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("of sokoban/micro");
  // The link keeps naming the URL, so reloading opens it again.
  await expect(page).toHaveURL(/package=%2Fpackages%2Fsokoban\.zip/);
  await openRef(page, "sokoban/micro/deadlock");
  await expect(page).toHaveURL(/ref=sokoban%2Fmicro%2Fdeadlock/);
  await page.reload();
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("of sokoban/micro/deadlock", { timeout: 30_000 });
  await expect(trace(page)).toHaveAttribute("max", "3");
});

test("keeps Studio within a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?package=/packages/sokoban.zip&name=sokoban&ref=sokoban%2Fmicro%2Fdeadlock");
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("of sokoban/micro/deadlock", { timeout: 30_000 });
  expect(await steps(page)).toBe(3);
  await expect(fromHere(page).locator(".move").first()).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});
