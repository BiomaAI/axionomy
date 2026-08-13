import { expect, test } from "@playwright/test";

test("runs the Rust engine in a Worker and loads its verified artifact", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("logistics");
  await page.getByLabel("Budget").fill("8");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("Live solver observations")).toBeVisible();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByText("Saved solver observations")).toBeVisible();
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await expect(page.getByText("Vehicle 1", { exact: true }).first()).toBeVisible();
  await expect(page.locator(".graph-scene")).toBeVisible();
  await page.locator(".react-flow__node.occupant-overlay").filter({ hasText: "Vehicle 1" }).click();
  await expect(page.locator(".account-card.focused")).toContainText("Vehicle");
});

test("cancels browser computation by terminating its isolated Worker", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("logistics");
  await page.getByRole("combobox", { name: "Instance (size)", exact: true }).selectOption("stress");
  await page.getByLabel("Budget").fill("2048");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Run", exact: true })).toBeEnabled();
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible();
});

test("browser Worker streams the same verified Work League frames", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=micro&strategy=mixed_field&view=solve&step=0&seed=17&budget=32");
  await expect(page.getByText("Running in your browser", { exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.locator(".live-standings")).toContainText("Live verified frame", { timeout: 30_000 });
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 60_000 });
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await expect(page.getByText("Who is winning depends on what you value")).toBeVisible();
});
