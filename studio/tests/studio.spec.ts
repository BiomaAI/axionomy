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

test("opens a shared Work League replay at the exact leaderboard and step", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&document=work_league%3Amixed_field&view=replay&step=12&leaderboard=resource_efficiency&seed=17&budget=128");
  await expect(page.getByRole("combobox", { name: "Problem", exact: true })).toHaveValue("work_league");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Resource efficiency/ })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".leaderboard-step")).toContainText("12");
  await expect(page.locator(".leaderboard-entries article")).toHaveCount(4);
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page).toHaveURL(/step=13/);
  await expect(page.locator(".leaderboard-step")).toContainText("13");
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
