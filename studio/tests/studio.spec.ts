import { expect, test } from "@playwright/test";

test("runs a search, receives events, and scrubs its replay", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /Maze · least energy/ })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible();
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page.locator(".assessment-applicable")).toContainText("applicable");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});

test("shows incremental stochastic progress and responsive pause control", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("logistics");
  await page.getByLabel("Seed").fill("42");
  await page.getByLabel("Budget").fill("64");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByRole("button", { name: "Running…" })).toBeDisabled();
  await expect(page.locator(".run-activity")).toContainText(/policy rollouts|MCTS iterations/);
  await page.getByRole("button", { name: "Pause" }).click();
  await expect(page.getByText("Run paused", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Resume" }).click();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText(/Stochastic logistics · Showcase · Reliable route · seed 42 · budget 64/)).toBeVisible();
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await expect(page.getByText("Moves that should be refused")).toBeVisible();
  await expect(page.getByText("Missing account binding for role `Vehicle`.")).toBeVisible();
  await expect(page.getByText("Missing account binding for role `Order`.")).toBeVisible();
});

test("loads a distinct static problem and its specialized renderer", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("exact_cover");
  await expect(page.getByRole("heading", { name: /Exact cover · Algorithm X/ })).toBeVisible();
  await expect(page.locator(".matrix-scene")).toBeVisible();
  await expect(page.getByText("Which subset contains which element", { exact: false })).toBeVisible();
});

test("renders the generic cockpit for every canonical problem without browser errors", async ({ page }) => {
  test.setTimeout(60_000);
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.goto("/");
  const problems = [
    "maze", "sokoban", "exact_cover", "bridge", "scheduling", "workshop",
    "marketplace", "logistics", "connect_four", "rescue", "mission",
    "perishables", "work_league",
  ];
  for (const problem of problems) {
    await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption(problem);
    await expect(page.locator(".document-heading .eyebrow")).toContainText(problem);
    await expect(page.locator(".stage .world-panel")).toBeVisible();
    await expect(page.locator(".stage .accounts-panel")).toBeVisible();
    await expect(page.getByText("Rates, roles, goals & invariants")).toBeVisible();
  }
  expect(browserErrors).toEqual([]);
});

test("opens a shared Work League replay at the exact leaderboard and step", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&document=work_league%3Amixed_field&view=replay&step=12&leaderboard=resource_efficiency&seed=17&budget=128");
  await expect(page.getByRole("combobox", { name: "Problem", exact: true })).toHaveValue("work_league");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Leaderboard ranking dimension" })).toHaveValue("resource_efficiency");
  await expect(page.locator(".leaderboard-step")).toContainText("12");
  await expect(page.locator(".leaderboard-entries article")).toHaveCount(4);
  await page.getByRole("combobox", { name: "Leaderboard ranking dimension" }).selectOption("least_waste");
  await expect(page).toHaveURL(/leaderboard=least_waste/);
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page).toHaveURL(/step=13/);
  await expect(page.locator(".leaderboard-step")).toContainText("13");
});

test("keeps replay shortcuts scoped to the cockpit and preserves economic wording", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&document=work_league%3Amixed_field&view=replay&step=12&leaderboard=resource_efficiency&seed=17&budget=128");
  await expect(page.locator(".stage .world-panel")).toBeVisible();
  await expect(page.locator(".stage .accounts-panel")).toBeVisible();
  await expect(page.getByText(/unchanged assets? in this replay/).first()).toBeVisible();
  await expect(page.getByText(/configuration assets?/)).toHaveCount(0);

  await page.keyboard.press("ArrowRight");
  await expect(page.locator(".leaderboard-step")).toContainText("13");
  await page.getByRole("tab", { name: /How it was solved/ }).click();
  await page.keyboard.press("ArrowRight");
  await expect(page).toHaveURL(/step=13/);

  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  const comparison = page.locator(".strategy-comparison");
  const summary = comparison.locator("summary");
  await summary.focus();
  await page.keyboard.press("Space");
  await expect(page.getByRole("button", { name: "Play" })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(comparison).toHaveAttribute("open", "");
});

test("keeps the replay cockpit within a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&document=work_league%3Amixed_field&view=replay&step=12&leaderboard=resource_efficiency&seed=17&budget=128");
  await expect(page.locator(".stage .world-panel")).toBeVisible();
  await expect(page.locator(".stage .accounts-panel")).toBeVisible();
  await expect(page.getByRole("button", { name: "Next exchange" })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});

test("keeps Logistics graph travel smooth when the OS requests reduced motion", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=logistics%3Areliable&view=replay&step=1&seed=0&budget=128");
  const vehicle = page.locator('.react-flow__node[data-id="entity:vehicle:fleet-1"]');
  await expect(vehicle).toBeVisible();
  const before = await vehicle.boundingBox();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await page.waitForTimeout(60);
  const during = await vehicle.boundingBox();
  const animations = await vehicle.evaluate((element) => element.getAnimations().filter((animation) => animation.playState === "running").length);
  expect(animations).toBeGreaterThan(0);
  expect(during?.x).not.toBe(before?.x);
  await page.waitForTimeout(300);
  const after = await vehicle.boundingBox();
  expect(after?.x).not.toBe(before?.x);
  expect(after?.x).not.toBe(during?.x);
});

test("browser history restores a prior problem deep link", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&view=replay&step=7&leaderboard=contract_value&seed=17&budget=128");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("maze");
  await expect(page.getByRole("heading", { name: /Maze · least energy/ })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await expect(page.locator(".leaderboard-step")).toContainText("7");
});

test("streams replay-verified Work League standings before publishing the artifact", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=micro&strategy=mixed_field&view=solve&step=0&seed=17&budget=32");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.locator(".live-standings")).toContainText("Live verified frame", { timeout: 15_000 });
  await expect(page.locator(".live-standings")).toContainText("Contract value");
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 30_000 });
});
