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
/** The rail with its tab `name` open: Step, Moves, Strategy, Experiments or Ref. */
async function rail(page: Page, name: string) {
  await fromHere(page).getByRole("tab", { name, exact: true }).click();
  return fromHere(page);
}
/** The drawer with its tab `name` open, whichever was open before. */
async function drawer(page: Page, name: string) {
  const tab = page.getByRole("tablist", { name: "Drawer" }).getByRole("tab", { name, exact: true });
  if (await tab.getAttribute("aria-selected") !== "true") await tab.click();
}
const stepNow = (page: Page) => page.getByRole("region", { name: "This step" });
const tree = (page: Page) => page.getByRole("group", { name: "History tree" });
const activity = (page: Page) => page.getByRole("region", { name: "Activity" });
const inspectors = (page: Page) => page.getByRole("complementary", { name: "Inspectors" });

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
  await expect(fromHere(page).locator(".panel-heading small")).toHaveText("fixed");
  await rail(page, "Ref");
  await fromHere(page).getByRole("textbox", { name: "Ref name" }).fill(name);
  await fromHere(page).getByRole("button", { name: "Create" }).click();
  await expect(page.getByRole("status").filter({ hasText: `${name} names step ${start}.` })).toBeVisible();
  await expect(fromHere(page).locator(".panel-heading small")).toHaveText("your ref");
  expect(await steps(page)).toBe(start);
  return start;
}

/** The first move offered here that applies, applied. */
async function applyFirst(page: Page) {
  await rail(page, "Moves");
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
  // This step is the exchange applied, with what each account gave, kept and
  // got; its full check is the step's inspector.
  await expect(stepNow(page).locator(".exchange-rate")).toHaveText("Move from cell 8 to cell 9");
  await expect(stepNow(page).locator(".effect").first()).toBeVisible();
  await stepNow(page).getByRole("button", { name: "1 / 3" }).click();
  await expect(inspectors(page).getByRole("region", { name: "Step 1" }).locator(".assessment-applicable")).toContainText("applicable");
  // Picking a step of the history on screen in the tree moves the cursor there.
  await tree(page).getByRole("button", { name: /^Step 2 of / }).first().click();
  await expect(trace(page)).toHaveValue("2");
  await expect(page.locator(".picture-area .world-panel")).toBeVisible();
  await expect(page.getByRole("region", { name: "State" })).toBeVisible();
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

test("offers the moves of the first candidate set with any here, and those of the set picked", async ({ page }) => {
  await openSample(page, "marketplace");
  expect(await openRef(page, "marketplace/micro")).toBe(0);
  // The package's first set fulfills the larger markets' contracts; Micro's
  // orders settle through the second.
  await rail(page, "Moves");
  const candidates = fromHere(page).getByRole("combobox", { name: "Candidates" });
  await expect(candidates).toHaveValue("marketplace/settlements");
  await expect(fromHere(page).locator(".move-applicable").first()).toBeVisible();
  await candidates.selectOption("marketplace/fulfillment");
  await expect(fromHere(page).getByText("No moves", { exact: true })).toBeVisible();
});

test("applies a move at a ref of one's own, moving it, and anywhere else makes a branch", async ({ page }) => {
  await openSample(page, "maze");
  // The package's ref is fixed: at its point, applying makes a branch.
  await openRef(page, "maze/showcase");
  await expect(fromHere(page).locator(".panel-heading small")).toHaveText("fixed");
  const mine = ownRef("maze");
  const start = await playFrom(page, "maze/showcase", mine);
  // Each move is shown checked before anything is applied.
  await rail(page, "Moves");
  await fromHere(page).locator(".move-applicable").first().getByRole("button", { name: "Details" }).click();
  await expect(fromHere(page).locator(".move-detail .assessment-applicable")).toBeVisible();
  await applyFirst(page);
  await expect(page.getByRole("status").filter({ hasText: `${mine} moved to step ${start + 1}` })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));

  // Back at step 0 the point is not where the ref is: applying there branches.
  const branches = await tree(page).locator(".tree-branch").count();
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await expect(fromHere(page).locator(".panel-heading h2")).toHaveText(`Step 0 of ${mine}`);
  await expect(fromHere(page).locator(".panel-heading small")).toHaveCount(0);
  await fromHere(page).locator(".move-applicable").last().getByRole("button", { name: "Apply" }).click();
  await expect(page.getByRole("status").filter({ hasText: "a branch from step 0" })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", "1");
  await expect(tree(page).locator(".tree-branch")).toHaveCount(branches + 1);
  // The ref stayed where the move took it, and the package's where it shipped.
  expect(await openRef(page, mine)).toBe(start + 1);
  expect(await openRef(page, "maze/showcase")).toBe(start);
  expect(await openRef(page, mine)).toBe(start + 1);

  // An exchange written from a rate is checked before it can be applied; the
  // accounts the rate fixes are filled in.
  await rail(page, "Moves");
  const editor = fromHere(page).getByRole("region", { name: "Exchange", exact: true });
  const apply = editor.getByRole("button", { name: "Apply" });
  const check = editor.getByRole("button", { name: "Check" });
  const rate = editor.getByRole("combobox", { name: "Rate" });
  const offered = (await fromHere(page).locator(".move-applicable .exchange-rate").allTextContents()).map((text) => text.trim());
  const refused = (await rate.locator("option").allTextContents()).find((label) => !offered.includes(label.trim()));
  // A rate whose move does not apply here is refused, and never applied.
  await rate.selectOption({ label: refused ?? "" });
  await expect(editor.locator("output").first()).toBeVisible();
  await expect(apply).toBeDisabled();
  await check.click();
  await expect(editor.locator(".assessment-infeasible, .assessment-invalid")).toBeVisible();
  await expect(apply).toBeDisabled();
  await rate.selectOption({ label: offered[0] });
  await expect(apply).toBeDisabled();
  await check.click();
  await expect(editor.locator(".assessment-applicable")).toBeVisible();
  await apply.click();
  await expect(page.getByRole("status").filter({ hasText: `${mine} moved to step ${start + 2}` })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", String(start + 2));

  // One's own ref is deleted; its history stays on screen and in the tree.
  await rail(page, "Ref");
  await fromHere(page).getByRole("button", { name: "Delete it" }).click();
  await expect(page.getByRole("status").filter({ hasText: `Deleted ${mine}.` })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", String(start + 2));
  await expect(tree(page).getByRole("button", { name: `Open ${mine}` })).toHaveCount(0);
  // A package's ref offers no deleting.
  await openRef(page, "maze/showcase");
  await expect(fromHere(page).getByRole("button", { name: "Delete it" })).toHaveCount(0);
});

test("lets a strategy play from here: its branch opens and plays", async ({ page }) => {
  await openSample(page, "maze");
  const start = await openRef(page, "maze/showcase");
  const play = (await rail(page, "Strategy")).getByRole("region", { name: "Strategy", exact: true });
  await play.getByRole("combobox", { name: "Strategy" }).selectOption("maze/a-star");
  // The goal and the effort are those of the experiment that runs the strategy.
  await expect(play.getByRole("combobox", { name: "Goal" })).toHaveValue("maze/solved");
  await expect(play.getByRole("textbox", { name: "Seed" })).toHaveValue("17");
  await play.getByRole("button", { name: "Run" }).click();
  await expect(page.getByRole("status").filter({ hasText: /maze\/a-star: reached the goal in \d+ steps, [\d,]+ states looked at/ })).toBeVisible({ timeout: 30_000 });
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
  await rail(page, "Ref");
  await expect(fromHere(page).getByRole("list", { name: "Delivered steps" })).toContainText(`step ${start + 1}`);
  await expect(fromHere(page).getByRole("button", { name: "Stop following" })).toBeVisible();
  // What runs is said at the top of the page, and stopped from there.
  await expect(activity(page)).toContainText(`Following ${mine} · 1 step arrived`);
  await activity(page).getByRole("button", { name: "Stop following" }).click();
  await expect(fromHere(page).getByRole("button", { name: "Follow it" })).toBeVisible();
  await expect(activity(page)).toContainText(/Saved|Not saved/);
});

test("stops a game mid-play, and nothing else notices", async ({ page }) => {
  test.setTimeout(120_000);
  await openSample(page, "connect-four");
  // The package's ref is fixed: the game is followed and moved on one of its own.
  const live = ownRef("connect-four");
  const start = await playFrom(page, "connect-four/stress", live);
  await fromHere(page).getByRole("button", { name: "Follow it" }).click();
  const play = (await rail(page, "Strategy")).getByRole("region", { name: "Strategy", exact: true });
  // Against the minimax opponent the game takes long enough to be stopped.
  await play.getByRole("combobox", { name: "Strategy" }).selectOption("connect-four/mcts-32/search-opponent");
  await play.getByRole("button", { name: "Run" }).click();
  await expect(page.locator(".scrubber-label")).toContainText("Finding moves");
  await expect(activity(page)).toContainText(`Running connect-four/mcts-32/search-opponent from step ${start}`);
  await expect(activity(page)).toContainText(`Following ${live}`);
  await activity(page).getByRole("button", { name: "Stop", exact: true }).click();
  await expect(play.getByRole("button", { name: "Run" })).toBeEnabled({ timeout: 30_000 });
  await expect(page.getByRole("alert")).toHaveCount(0);
  // The follower never noticed: it is still waiting, and hears the next move.
  await rail(page, "Ref");
  await expect(fromHere(page).getByRole("button", { name: "Stop following" })).toBeVisible();
  await applyFirst(page);
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await rail(page, "Ref");
  await expect(fromHere(page).getByRole("list", { name: "Delivered steps" })).toContainText(`step ${start + 1}`);
});

test("plays an experiment from where it starts, and each trial is a branch to watch", async ({ page }) => {
  test.setTimeout(90_000);
  await openSample(page, "maze");
  await openRef(page, "maze/micro");
  const branches = await tree(page).locator(".tree-branch").count();
  // Every experiment of the package is listed, one line each, those that
  // start in this tree first; a line opens on what it compares.
  const experiments = (await rail(page, "Experiments")).getByRole("region", { name: "Experiments", exact: true });
  await expect(experiments.locator(".experiment-item").first()).toContainText("maze/micro");
  await expect(experiments).toContainText("Other experiments");
  const here = experiments.locator(".experiment-item").filter({ has: page.locator(".experiment-name .id-link", { hasText: /^maze\/micro$/ }) });
  await here.getByRole("button", { name: "What maze/micro compares" }).click();
  await expect(here.locator(".facts")).toContainText("maze/micro");
  await here.getByRole("button", { name: /^Run( again)?$/ }).click();
  await expect(activity(page).getByRole("status")).toContainText(/^Ran maze\/micro( again: same results)?\.$/, { timeout: 60_000 });
  // Its results open in the drawer: five strategies, each a trial that
  // reached the goal, each a new branch.
  const results = page.getByRole("region", { name: "Results of maze/micro" });
  await expect(results.locator(".results-table tbody tr")).toHaveCount(5);
  await expect(results.locator(".results-table")).toContainText("a-star");
  await expect(results.locator(".results-table")).toContainText("reached the goal");
  await expect(tree(page).locator(".tree-branch")).toHaveCount(branches + 5);
  await expect(tree(page).locator(".tone-trial").first()).toBeVisible();
  // Played again with the same seeds, it says the results are the same.
  await here.getByRole("button", { name: "Run again" }).click();
  await expect(activity(page).getByRole("status")).toContainText("again: same results", { timeout: 60_000 });
  // A trial is watched: its branch opens where it started and plays.
  await results.locator(".results-table tbody tr", { hasText: "a-star" }).getByRole("button", { name: "Watch" }).click();
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("a-star");
  expect(await steps(page)).toBeGreaterThan(0);
  // A branch is set against the history it left: the same up to where it split, then step by step.
  await drawer(page, "Compare");
  const comparison = page.getByRole("region", { name: "Comparison" });
  await expect(comparison.locator(".compare-note")).toContainText("Split at step 0");
  await expect(comparison.locator("tbody tr").first()).toContainText("Walk Start");
  // Any branch of the same tree is compared from its card in the tree.
  const compareWith = page.getByRole("button", { name: "Compare with the history on screen" });
  await expect(async () => {
    await tree(page).getByRole("button", { name: /^Step 3 of breadth-first/ }).last().hover();
    await expect(compareWith).toBeVisible({ timeout: 1_000 });
  }).toPass();
  await compareWith.click();
  await expect(comparison.locator(".panel-heading h2")).toContainText("breadth-first");
  await expect(comparison.locator(".compare-ends")).toContainText("energy");
});

test("AMM holdings and lending balances show their economic denominations", async ({ page }) => {
  await openSample(page, "amm");
  await openRef(page, "amm/micro");
  await expect(page.locator(".scene-metric", { hasText: "Discovered price" }).locator("strong")).toHaveText("10.000 credits / energy lot");
  await page.locator(".state-list").getByRole("button", { name: "Energy / credit AMM", exact: true }).click();
  const pool = inspectors(page).getByRole("region", { name: "Account Energy / credit AMM" });
  await pool.getByRole("button", { name: "Usable energy" }).click();
  const energy = inspectors(page).getByRole("region", { name: "Asset Usable energy" });
  await expect(energy.locator(".balance", { hasText: "Solar generator" })).toContainText("3000 energy lots");
  await expect(energy.locator(".balance", { hasText: "Energy / credit AMM" })).toContainText("4000 energy lots");

  await openSample(page, "lending");
  await openRef(page, "lending/micro");
  await expect(page.locator(".scene-metric", { hasText: "Collateral price" }).locator("strong")).toHaveText("100 cash / collateral unit");
  await page.locator(".state-list").getByRole("button", { name: "Lending market", exact: true }).click();
  const market = inspectors(page).getByRole("region", { name: "Account Lending market" });
  for (const [label, balance] of [
    ["Borrow index", "1 ×"], ["Cash", "7000 cash"], ["Collateral price", "100 cash / collateral unit"],
    ["Locked genesis share", "1000 vault shares"], ["Vault share supply", "7000 vault shares"],
  ]) {
    await expect(market.locator(".balance").filter({ has: page.getByRole("button", { name: label, exact: true }) }).locator("strong")).toHaveText(balance);
  }
});

test("names open inspectors: an account pinned follows the cursor, and the rules list every rate", async ({ page }) => {
  await openSample(page, "sokoban");
  expect(await openRef(page, "sokoban/micro/deadlock")).toBe(3);
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await page.getByRole("button", { name: "Next exchange" }).click();
  const picture = async () => (await page.locator(".world-panel").boundingBox())!;
  const unpinned = await picture();
  // The step changed two balances of the cell the player left, listed under it.
  const left = page.locator(".state-list li", { hasText: /^Cell 8/ });
  await expect(left).toContainText("2 changed");
  await expect(left.locator(".state-changes .balance.changed")).toHaveCount(2);
  // A hover previews it; a click pins it.
  await left.getByRole("button", { name: "Cell 8" }).hover();
  await expect(page.getByRole("tooltip").locator(".balance.changed")).toHaveCount(2);
  await left.getByRole("button", { name: "Cell 8" }).click();
  const pinned = inspectors(page).getByRole("region", { name: "Account Cell 8" });
  await expect(pinned.locator(".balance.changed")).toHaveCount(2);
  await expect(pinned.locator(".balance.changed").first()).toContainText("→");
  // It follows the cursor: before the step, nothing of it has changed.
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await expect(pinned.locator(".balance").first()).toBeVisible();
  await expect(pinned.locator(".balance.changed")).toHaveCount(0);
  // The rate of a step pins beside it; Esc closes the latest, × the other.
  await page.getByRole("button", { name: "Next exchange" }).click();
  await stepNow(page).locator(".exchange-rate").click();
  await expect(inspectors(page).getByRole("region", { name: /^Rate / })).toContainText("Player's cell");
  // Pinned inspectors float over the picture's edge: nothing moves to make
  // room for them, and they cover neither the rail, the transport nor the tree.
  expect(await picture()).toEqual(unpinned);
  expect(await page.evaluate(() => {
    const dock = document.querySelector(".inspector-dock")!.getBoundingClientRect();
    return [".playback", ".tree-panel", ".action-panel"].filter((panel) => {
      const box = document.querySelector(panel)!.getBoundingClientRect();
      return !(dock.bottom <= box.top || dock.top >= box.bottom || dock.right <= box.left || dock.left >= box.right);
    });
  })).toEqual([]);
  // The page is one screen: it does not scroll.
  expect(await page.evaluate(() => document.documentElement.scrollHeight - window.innerHeight)).toBeLessThanOrEqual(1);
  await page.keyboard.press("Escape");
  await expect(inspectors(page).getByRole("region")).toHaveCount(1);
  await pinned.getByRole("button", { name: "Close Cell 8" }).click();
  await expect(inspectors(page)).toHaveCount(0);
  // Rules, in the drawer, lists the model's rates as written, each opening on its name.
  await page.getByRole("button", { name: "Rules" }).click();
  const rules = page.getByRole("region", { name: "Rules" });
  await expect(rules.locator("tbody tr")).toHaveCount(3);
  await rules.getByRole("searchbox", { name: "Filter rates" }).fill("finish");
  await rules.locator("tbody tr").getByRole("button", { name: "Finish" }).click();
  await expect(inspectors(page).getByRole("region", { name: "Rate Finish" })).toContainText("Puzzle");
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

test("a link to what the store does not hold opens the first package, and the brand goes home", async ({ page }) => {
  await openSample(page, "sokoban");
  await page.goto("/?name=no-such-package&ref=no%2Fsuch%2Fref");
  await expect(activity(page)).toContainText("No package no-such-package is stored here", { timeout: 30_000 });
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText(" of ");
  await page.goto("/?name=sokoban&ref=sokoban%2Fno-such-ref");
  await expect(activity(page)).toContainText("sokoban has no ref sokoban/no-such-ref: opened sokoban.", { timeout: 30_000 });
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("of sokoban/micro");
  // Home is Studio with nothing linked: the store's first package, at its first ref.
  await page.getByRole("link", { name: /Axionomy Studio home/ }).click();
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText(" of ", { timeout: 30_000 });
  await expect(page).not.toHaveURL(/no-such/);
  await expect(activity(page)).not.toContainText("no-such");
});

test("keeps Studio within a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?package=/packages/sokoban.zip&name=sokoban&ref=sokoban%2Fmicro%2Fdeadlock");
  await expect(fromHere(page).locator(".panel-heading h2")).toContainText("of sokoban/micro/deadlock", { timeout: 30_000 });
  expect(await steps(page)).toBe(3);
  await rail(page, "Moves");
  await expect(fromHere(page).locator(".move").first()).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});

test("a release exposes its source and compares an uploaded build on matching seeds", async ({ page }) => {
  await openSample(page, "maze");
  await page.getByRole("button", { name: "Source", exact: true }).click();
  const source = page.getByRole("region", { name: "Package source" });
  const editor = source.getByRole("textbox", { name: "Rhai source" });
  await expect(editor.locator(".cm-line").first()).toHaveText("#{");
  await expect(editor.locator(".cm-line").filter({ hasText: /^\s*version: #\{$/ })).toBeVisible();
  await expect(source.getByRole("button", { name: "Discard edits" })).toHaveCount(0);
  await expect(editor.locator(".tok-number").first()).toBeVisible();
  await source.getByLabel("Source file").selectOption("scripts/maze.rhai");
  await expect(editor.locator(".tok-keyword").filter({ hasText: /^fn$/ }).first()).toBeVisible();
  await editor.fill("fn invalid(");
  await source.getByRole("button", { name: "Build package" }).click();
  await expect(source.getByRole("alert")).toContainText("scripts/maze.rhai");
  await source.getByRole("button", { name: "Discard edits" }).click();
  await source.getByLabel("Source file").selectOption("scripts/maze.rhai");
  await editor.click();
  await editor.press("ControlOrMeta+End");
  await editor.press("Enter");
  await page.keyboard.insertText("// An edited release.");
  await source.getByRole("button", { name: "Build package" }).click();
  const download = page.waitForEvent("download");
  await source.getByRole("link", { name: "Download package" }).click();
  const archive = await (await download).path();
  expect(archive).toBeTruthy();
  await source.getByLabel("Experiment", { exact: true }).selectOption("maze/micro");
  await source.getByLabel("Compare build").setInputFiles(archive!);
  await expect(source.getByRole("heading", { name: "Matching trials" })).toBeVisible({ timeout: 30_000 });
});

test("Rhai at a point reads a balance and applies the assessed move", async ({ page }) => {
  await openSample(page, "maze");
  await openRef(page, "maze/micro");
  await drawer(page, "Console");
  const console = page.getByRole("region", { name: "Rhai console" });
  const program = console.getByLabel("Program");
  await program.fill('let wallet=#{owner:account("agent"),unit:asset("energy")};state.balance(wallet.owner,wallet.unit)');
  await console.getByRole("button", { name: "Format", exact: true }).click();
  await expect(program.locator(".cm-line")).toHaveCount(5);
  await expect(program.locator(".tok-keyword")).toHaveText("let");
  await program.press("ControlOrMeta+z");
  await expect(program.locator(".cm-line")).toHaveCount(1);
  await console.getByRole("button", { name: "Format", exact: true }).click();
  await console.getByRole("button", { name: "Run query" }).click();
  await expect(console.locator("pre")).toHaveText('"9"');
  await program.fill('exchange("move/start/key-room/2/false", #{actor: "agent", environment: "world"}, 1)');
  await console.getByRole("button", { name: "Assess exchange" }).click();
  await console.getByRole("button", { name: "Apply assessed exchange" }).click();
  await expect(trace(page)).toHaveAttribute("max", "1");
});

test("Sokoban opens with formatted exchange bindings and syntax colors", async ({ page }) => {
  await openSample(page, "sokoban");
  await page.getByRole("button", { name: "Source", exact: true }).click();
  const editor = page.getByRole("textbox", { name: "Rhai source" });
  const binding = editor.locator(".cm-line").filter({ hasText: /^\s*from: "cell\/8",$/ }).first();
  await expect(binding).toBeAttached();
  await binding.scrollIntoViewIfNeeded();
  await expect(binding.locator(".tok-propertyName")).toHaveText("from");
  await expect(binding.locator(".tok-string")).toHaveText('"cell/8"');
  for (const colorScheme of ["dark", "light"] as const) {
    await page.emulateMedia({ colorScheme });
    const key = await binding.locator(".tok-propertyName").evaluate(el => getComputedStyle(el).color);
    const value = await binding.locator(".tok-string").evaluate(el => getComputedStyle(el).color);
    expect(key).not.toBe(value);
  }
  await expect(page.getByRole("button", { name: "Discard edits" })).toHaveCount(0);
});
