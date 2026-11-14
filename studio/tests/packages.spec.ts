import { expect, test, type Page } from "@playwright/test";

/**
 * Studio on packages (P12), run against the native server and against the
 * engine in the browser's Worker alike: both engines offer the same
 * operations, so the same steps give the same pages. A package's refs are
 * fixed, so a test that moves a ref names one of its own at a package's
 * ref's point, a name no other run gives, since the native server's store
 * outlives a test.
 */

async function loadSample(page: Page, name: string) {
  await expect(page.getByRole("combobox", { name: "Sample package" })).toBeVisible({ timeout: 30_000 });
  // Choosing a sample loads it; the picker goes back to offering the others.
  await page.getByRole("combobox", { name: "Sample package" }).selectOption({ label: name });
  await expect(page.getByRole("status").filter({ hasText: `Loaded ${name} from packages/${name}.zip` })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("combobox", { name: "Sample package" })).toHaveValue("");
  await expect(page.getByRole("combobox", { name: "Package", exact: true })).toHaveValue(name);
}

const trace = (page: Page) => page.getByRole("slider", { name: "Trace position" });

/**
 * How many steps the history on screen has, once the point opened is
 * drawn: none where Studio says nothing has happened yet, in place of
 * playback.
 */
async function steps(page: Page): Promise<number> {
  await expect(page.locator(".point-cockpit")).toHaveAttribute("aria-busy", "false");
  await expect(trace(page).or(page.getByRole("region", { name: "Nothing to play yet" }))).toBeVisible();
  if (await page.getByRole("region", { name: "Nothing to play yet" }).isVisible()) return 0;
  return Number(await trace(page).getAttribute("max"));
}

/** Opens a ref, and gives how many steps its history has. */
async function openRef(page: Page, name: string): Promise<number> {
  await page.getByRole("combobox", { name: "Ref" }).selectOption(name);
  await expect(page.locator(".point-heading .eyebrow")).toContainText(`ref ${name}`);
  return await steps(page);
}

/** A ref name under `pkg` no other run gives. */
function ownRef(pkg: string): string {
  return `${pkg}/play/${Date.now().toString(36)}${Math.floor(Math.random() * 1e6).toString(36)}`;
}

/**
 * Opens the package's ref `shipped`, which is fixed, and names a ref of
 * one's own at its point, which opens; gives how many steps its history has.
 */
async function playFrom(page: Page, shipped: string, name: string): Promise<number> {
  const start = await openRef(page, shipped);
  await expect(page.getByLabel("Point on screen")).toContainText("On a package's ref");
  await page.getByRole("textbox", { name: "Name a ref here" }).fill(name);
  await page.getByRole("button", { name: "Name it" }).click();
  await expect(page.getByRole("status").filter({ hasText: `${name} names this point now` })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Ref" })).toHaveValue(name);
  await expect(page.locator(".point-heading .eyebrow")).toContainText(`ref ${name}`);
  expect(await steps(page)).toBe(start);
  return start;
}

test("loads a sample package and scrubs a ref's history", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("combobox", { name: "Package", exact: true })).toBeVisible();
  await loadSample(page, "sokoban");
  await expect(page.getByLabel("What this package defines")).toContainText("candidates sokoban/moves");
  expect(await openRef(page, "sokoban/micro/deadlock")).toBe(3);
  // A ref opens where it is: the end of its history.
  await expect(trace(page)).toHaveValue("3");
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await expect(trace(page)).toHaveValue("0");
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page.locator(".ticker-panel .panel-heading small")).toHaveText("1 / 3");
  await expect(page.locator(".transition-panel .assessment-applicable")).toContainText("applicable");
  await expect(page.locator(".stage .world-panel")).toBeVisible();
  await expect(page.locator(".stage .accounts-panel")).toBeVisible();
});

test("applies a candidate at a ref's point, moving the ref, and anywhere else makes a branch", async ({ page }) => {
  await page.goto("/");
  await loadSample(page, "maze");
  // The package's ref is fixed: at its point, applying makes a branch.
  await openRef(page, "maze/showcase");
  const panel = page.locator(".action-panel");
  await expect(panel.locator(".panel-heading")).toContainText("maze/showcase is fixed: applying makes a branch");
  await expect(panel.locator(".candidate-applicable").first()).toBeVisible();
  const mine = ownRef("maze");
  const start = await playFrom(page, "maze/showcase", mine);
  await expect(panel.locator(".panel-heading")).toContainText(`At ${mine}'s point`);
  await expect(panel.locator(".panel-heading")).toContainText(`Applying moves ${mine}`);
  // Each candidate is shown assessed before anything is applied.
  const applicable = panel.locator(".candidate-applicable");
  await expect(applicable.first()).toBeVisible();
  await applicable.first().click();
  await expect(panel.locator(".candidate-detail .assessment-applicable")).toBeVisible();
  await panel.getByRole("button", { name: `Apply and move ${mine}` }).click();
  await expect(page.getByRole("status").filter({ hasText: `${mine} moved to step ${start + 1}` })).toBeVisible();
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await expect(page.getByRole("combobox", { name: "Ref" })).toHaveValue(mine);

  // Back at step 0 the point is not where the ref is: applying there branches.
  await page.getByRole("button", { name: "Go to initial state" }).click();
  await expect(panel.locator(".panel-heading")).toContainText("At step 0");
  await expect(panel.locator(".panel-heading")).toContainText("Applying makes a branch");
  await panel.locator(".candidate-applicable").last().click();
  await panel.getByRole("button", { name: "Apply as a branch" }).click();
  await expect(page.getByRole("status").filter({ hasText: "a branch, a new point that no ref names" })).toBeVisible();
  await expect(page.locator(".point-heading .eyebrow")).toContainText("Branch");
  await expect(page.getByLabel("Point on screen")).toContainText("Not on any ref");
  await expect(trace(page)).toHaveAttribute("max", "1");
  // The ref stayed where the first move took it, and the package's where it shipped.
  expect(await openRef(page, mine)).toBe(start + 1);
  expect(await openRef(page, "maze/showcase")).toBe(start);
  expect(await openRef(page, mine)).toBe(start + 1);

  // A written exchange is assessed before it can be applied.
  await panel.locator(".exchange-editor summary").click();
  await panel.getByLabel("Exchange as JSON").fill('{"rate": "nowhere", "bindings": {}, "units": "1"}');
  await expect(panel.getByRole("button", { name: `Apply and move ${mine}` })).toBeDisabled();
  await panel.getByRole("button", { name: "Assess", exact: true }).click();
  await expect(panel.locator(".exchange-editor .assessment-invalid")).toBeVisible();
  await expect(panel.getByRole("button", { name: `Apply and move ${mine}` })).toBeDisabled();

  // One's own ref is deleted; the point it named stays on screen.
  await page.getByRole("button", { name: `Delete ${mine}` }).click();
  await expect(page.getByRole("status").filter({ hasText: `${mine} is deleted` })).toBeVisible();
  await expect(page.getByLabel("Point on screen")).toContainText("Not on any ref");
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await expect(page.getByRole("combobox", { name: "Ref" }).locator("option", { hasText: mine })).toHaveCount(0);
  // A package's ref offers no deleting.
  await openRef(page, "maze/showcase");
  await expect(page.getByRole("button", { name: "Delete maze/showcase" })).toHaveCount(0);
});

test("continues from here with a strategy, showing where it ended and its thinking", async ({ page }) => {
  await page.goto("/");
  await loadSample(page, "maze");
  const start = await openRef(page, "maze/showcase");
  const panel = page.locator(".action-panel");
  const ended = page.getByLabel("Where the strategy ended");
  const continueWith = async (strategy: string) => {
    await panel.getByRole("tab", { name: "Continue from here" }).click();
    await panel.getByRole("combobox", { name: "Strategy" }).selectOption(strategy);
    // The goal and the effort are those of the experiment that runs the strategy.
    await expect(panel.getByRole("combobox", { name: "Goal" })).toHaveValue("maze/solved");
    await expect(panel.getByRole("textbox", { name: "Seed" })).toHaveValue("17");
    // Maze's strategies search, so its experiments give them no turns.
    await expect(panel.getByRole("textbox", { name: "Turns" })).toHaveValue("0");
    await panel.getByRole("button", { name: "Run strategy" }).click();
    await expect(ended).toContainText("Reached its goal", { timeout: 30_000 });
  };
  await continueWith("maze/a-star");
  await expect(ended.getByLabel("Thinking")).toContainText("Expanded");
  await ended.getByRole("button", { name: "Open where it ended" }).click();
  await expect(page.locator(".point-heading .eyebrow")).toContainText("Where a strategy ended");
  expect(await steps(page)).toBeGreaterThan(start);

  // The package's ref stays where it shipped; from a ref of one's own at
  // its point, the ref can move to where the strategy ended.
  const mine = ownRef("maze");
  expect(await playFrom(page, "maze/showcase", mine)).toBe(start);
  await continueWith("maze/dijkstra");
  await ended.getByRole("button", { name: `Move ${mine} there` }).click();
  await expect(page.getByRole("status").filter({ hasText: new RegExp(`${mine} moved to step \\d+`) })).toBeVisible();
  await expect(page.locator(".point-heading .eyebrow")).toContainText(`ref ${mine}`);
  expect(await steps(page)).toBeGreaterThan(start);
  expect(await openRef(page, "maze/showcase")).toBe(start);
});

test("follows a ref and receives the steps it is moved over", async ({ page }) => {
  await page.goto("/");
  await loadSample(page, "maze");
  const mine = ownRef("maze");
  const start = await playFrom(page, "maze/stress", mine);
  const panel = page.locator(".action-panel");
  await panel.getByRole("tab", { name: "Follow" }).click();
  await panel.getByRole("button", { name: `Follow ${mine}` }).click();
  // Following goes on while this page acts at the ref's point.
  await panel.getByRole("tab", { name: "Act" }).click();
  await panel.locator(".candidate-applicable").first().click();
  await panel.getByRole("button", { name: `Apply and move ${mine}` }).click();
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await panel.getByRole("tab", { name: "Follow" }).click();
  await expect(panel.getByRole("list", { name: "Delivered steps" })).toContainText(`step ${start + 1}`);
  await expect(panel.getByRole("button", { name: "Stop following" })).toBeVisible();
});

test("cancels a game mid-run, and nothing else notices", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/");
  await loadSample(page, "connect-four");
  // The package's ref is fixed: the game is followed and moved on one of its own.
  const live = ownRef("connect-four");
  const start = await playFrom(page, "connect-four/stress", live);
  const panel = page.locator(".action-panel");
  await panel.getByRole("tab", { name: "Follow" }).click();
  await panel.getByRole("button", { name: `Follow ${live}` }).click();
  await panel.getByRole("tab", { name: "Continue from here" }).click();
  await panel.getByRole("combobox", { name: "Strategy" }).selectOption("connect-four/mcts-32/self-play");
  await panel.getByRole("button", { name: "Run strategy" }).click();
  // A game reports its turns, and its players' searches, as it plays.
  await expect(panel.locator(".activity-progress")).toContainText(/\d+ expanded · \d+ generated/, { timeout: 60_000 });
  await panel.getByRole("button", { name: "Cancel" }).click();
  await expect(panel.locator(".continue [role=alert]")).toContainText("Cancelled: the strategy stopped");
  await expect(panel.getByRole("button", { name: "Run strategy" })).toBeEnabled();
  // The follower never noticed: it is still waiting, and hears the next move.
  await expect(panel.locator(".follow [role=alert]")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Stop following", includeHidden: true })).toBeAttached();
  await panel.getByRole("tab", { name: "Act" }).click();
  await panel.locator(".candidate-applicable").first().click();
  await panel.getByRole("button", { name: `Apply and move ${live}` }).click();
  await expect(trace(page)).toHaveAttribute("max", String(start + 1));
  await panel.getByRole("tab", { name: "Follow" }).click();
  await expect(panel.getByRole("list", { name: "Delivered steps" })).toContainText(`step ${start + 1}`);
  await expect(panel.locator(".follow [role=alert]")).toHaveCount(0);
});

test("runs an experiment and compares its trials", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/");
  await loadSample(page, "maze");
  await page.getByRole("tab", { name: /Experiments/ }).click();
  const card = page.getByRole("group", { name: "Experiment maze/micro" });
  await card.getByRole("button", { name: "Run experiment" }).click();
  const results = card.getByRole("table", { name: "Results of maze/micro" });
  await expect(results).toBeVisible({ timeout: 60_000 });
  await expect(card.getByRole("button", { name: "Run experiment" })).toBeEnabled({ timeout: 60_000 });
  await expect(results.locator("tbody tr")).toHaveCount(5);
  await expect(results).toContainText("maze/a-star");
  await expect(results).toContainText("reached");
  await expect(results.locator("tbody tr.selected")).toHaveCount(1);
  await expect(results.locator("tbody")).toContainText("on the front");
  await expect(card).toContainText("the pick is the best by maze/energy, then maze/time");
  await card.getByRole("combobox", { name: "Pick by" }).selectOption("maze/time");
  await expect(card).toContainText("the pick is the best by maze/time, then maze/energy");
  // A trial's end opens like any point.
  await results.locator("tbody tr").first().getByRole("button", { name: "Open" }).click();
  await expect(page.locator(".point-heading .eyebrow")).toContainText("A trial's end");
  expect(await steps(page)).toBeGreaterThan(0);
});

test("a link names a package by its URL and opens it", async ({ page }) => {
  await page.goto("/?package=/packages/sokoban.zip");
  await expect(page.getByRole("combobox", { name: "Package", exact: true })).toHaveValue("sokoban", { timeout: 30_000 });
  await expect(page.getByRole("combobox", { name: "Ref" })).toHaveValue("sokoban/micro");
  await expect(page.locator(".point-heading h1")).toBeVisible();
  // The link keeps naming the URL, so reloading opens it again.
  await expect(page).toHaveURL(/package=%2Fpackages%2Fsokoban\.zip/);
  await page.getByRole("combobox", { name: "Ref" }).selectOption("sokoban/micro/deadlock");
  await expect(page).toHaveURL(/ref=sokoban%2Fmicro%2Fdeadlock/);
  await page.reload();
  await expect(page.getByRole("combobox", { name: "Ref" })).toHaveValue("sokoban/micro/deadlock", { timeout: 30_000 });
  await expect(trace(page)).toHaveAttribute("max", "3");
});

test("keeps the packages workspace within a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?package=/packages/sokoban.zip&name=sokoban&ref=sokoban%2Fmicro%2Fdeadlock");
  await expect(page.getByRole("combobox", { name: "Ref" })).toHaveValue("sokoban/micro/deadlock", { timeout: 30_000 });
  expect(await steps(page)).toBe(3);
  await expect(page.locator(".action-panel .candidate").first()).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});
