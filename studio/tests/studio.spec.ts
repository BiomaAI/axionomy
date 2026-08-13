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
  const motion = await vehicle.evaluate((element) => ({
    animations: element.getAnimations().filter((animation) => animation.playState === "running").length,
    duration: getComputedStyle(element).transitionDuration,
    easing: getComputedStyle(element).transitionTimingFunction,
  }));
  expect(motion.animations).toBeGreaterThan(0);
  expect(motion.duration).toContain("0.62s");
  expect(motion.easing).toContain("cubic-bezier(0.2, 0.8, 0.2, 1)");
  expect(during?.x).not.toBe(before?.x);
  await page.waitForTimeout(300);
  const after = await vehicle.boundingBox();
  expect(after?.x).not.toBe(before?.x);
  expect(after?.x).not.toBe(during?.x);
});

test("gives graph semantics depth, distinct shapes, and direction-aware ports", async ({ page }) => {
  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=logistics%3Areliable&view=replay&step=2&seed=0&budget=128");
  const depot = page.locator('.react-flow__node[data-id="location:Depot"]');
  const vehicle = page.locator('.react-flow__node[data-id="entity:vehicle:fleet-1"]');
  await expect(depot).toBeVisible();
  await expect(vehicle).toBeVisible();
  const appearance = await depot.evaluate((element) => ({
    radius: getComputedStyle(element).borderRadius,
    shadow: getComputedStyle(element).boxShadow,
  }));
  expect(appearance.radius).toBe("50%");
  expect(appearance.shadow).not.toBe("none");
  await expect(depot.locator('[data-handleid="source:route:DirectOut"]')).toHaveAttribute("data-handlepos", "right");
  await expect(depot.locator('[data-handleid="target:route:DirectBack"]')).toHaveAttribute("data-handlepos", "right");
  await expect(vehicle).toHaveCSS("border-radius", "999px");
  const outbound = await page.locator('.react-flow__edge[data-id="route:DirectOut"] .react-flow__edge-path').getAttribute("d");
  const inbound = await page.locator('.react-flow__edge[data-id="route:DirectBack"] .react-flow__edge-path').getAttribute("d");
  expect(outbound).not.toBe(inbound);
});

test("uses the generic motion compositor across every graph problem", async ({ page }) => {
  test.setTimeout(60_000);
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  const problems = ["maze", "bridge", "workshop", "marketplace", "logistics", "mission", "rescue", "work_league"];
  for (const problem of problems) {
    await page.goto(`/?problem=${problem}&instance=micro&view=replay&step=0&seed=17&budget=8`);
    await expect(page.locator(".graph-scene")).toBeVisible();
    await expect(page.locator(".react-flow__node.structure-node").first()).toBeVisible();
    await expect(page.locator(".occupant-overlay, .attachment-group, .structure-state, .scene-context").first()).toBeVisible();
    await expect(page.locator(".react-flow__node.entity-overlay")).toHaveCount(0);
    await page.getByRole("button", { name: "Next exchange" }).click();
    await expect(page.locator('.graph-scene[data-motion="step"]')).toBeVisible();
  }
  expect(browserErrors).toEqual([]);
});

test("composes state, attachments, occupants, and context by relationship", async ({ page }) => {
  await page.goto("/?problem=workshop&instance=showcase&strategy=minimum_waste&document=workshop%3Aminimum_waste&view=replay&step=0&seed=17&budget=128");
  const wood = page.locator('.react-flow__node[data-id="wood"]');
  await expect(wood).toContainText(/18 units wood/i);
  await expect(page.locator('.react-flow__node[data-id="entity:stock:wood"]')).toHaveCount(0);

  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&document=work_league%3Amixed_field&view=replay&step=0&seed=17&budget=128");
  await expect(page.locator(".attachment-group").filter({ hasText: "North · items" })).toContainText("Job 1");
  await expect(page.locator(".react-flow__edge.attachment-relation").first()).toBeVisible();
  await expect(page.locator(".occupant-overlay").filter({ hasText: "Atlas" })).toBeVisible();

  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=logistics%3Areliable&view=replay&step=0&seed=17&budget=128");
  await expect(page.locator(".scene-context")).toContainText("Travel conditions");
  await expect(page.locator(".attachment-group").filter({ hasText: "Depot · items" })).toContainText("Order A");
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
