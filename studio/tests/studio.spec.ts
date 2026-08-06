import { expect, test } from "@playwright/test";

test("runs a search, receives events, and scrubs its replay", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /Maze Pareto/ })).toBeVisible();
  await page.getByRole("button", { name: "Run search" }).click();
  await expect(page.getByText("Run completed and replay verified")).toBeVisible();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page.getByText("Assessment: applicable")).toBeVisible();
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});
