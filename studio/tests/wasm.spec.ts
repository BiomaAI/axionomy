import { expect, test } from "@playwright/test";

test("runs the Rust engine in a Worker and loads its verified artifact", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("browser engine ready", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByLabel("Canonical problem").selectOption("logistics");
  await page.getByLabel("Budget").fill("8");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("Live solver observations")).toBeVisible();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByText("Retained solver observations")).toBeVisible();
  await page.getByRole("tab", { name: /Verified replay/ }).click();
  await expect(page.getByText("Vehicle 1", { exact: true }).first()).toBeVisible();
  await expect(page.locator(".graph-scene")).toBeVisible();
});

test("cancels browser computation by terminating its isolated Worker", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("browser engine ready", { exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByLabel("Canonical problem").selectOption("logistics");
  await page.getByLabel("Instance").selectOption("stress");
  await page.getByLabel("Budget").fill("2048");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Run", exact: true })).toBeEnabled();
  await expect(page.getByText("browser engine ready", { exact: true })).toBeVisible();
});
