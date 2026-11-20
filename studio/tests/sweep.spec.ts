import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";

/**
 * The sweep: every strategy Studio offers, at every ref of every sample, run
 * from that ref in the engine in the browser. Each run ends saying what it
 * came to, a goal reached or a caution, never a failure; the page throws
 * nothing, logs no error, and React Flow never fails to draw a line. The
 * samples run side by side, as many as the project allows, each in a
 * browser context, and so a store, of its own.
 */
const samples: string[] = JSON.parse(readFileSync(new URL("../public/packages/index.json", import.meta.url), "utf8")).packages.map((sample: { name: string }) => sample.name);

test.describe.configure({ mode: "parallel" });

const tree = (page: Page) => page.getByRole("group", { name: "History tree" });
const strategy = (page: Page) => page.locator(".action-panel").getByRole("region", { name: "Strategy", exact: true });

/**
 * Opens `ref` from its label in the tree, and waits for its history to be on
 * screen, at its end. The pointer leaves the tree first: a step it rests on
 * as the tree grows opens a card that may lie over the label.
 */
async function openRef(page: Page, ref: string) {
  await page.mouse.move(0, 0);
  await tree(page).getByRole("button", { name: `Open ${ref}`, exact: true }).click();
  await expect(page.locator(".action-panel .panel-heading h2")).toHaveText(new RegExp(`of ${ref.replaceAll("/", "\\/")}$`), { timeout: 60_000 });
  await expect(page.locator(".point-cockpit")).toHaveAttribute("aria-busy", "false", { timeout: 60_000 });
}

for (const sample of samples) {
  test(`every strategy offered at every ref of ${sample} runs`, async ({ page }) => {
    test.setTimeout(600_000);
    const problems: string[] = [];
    page.on("pageerror", (error) => problems.push(`thrown: ${error.message}`));
    page.on("console", (message) => {
      if (message.type() === "error" || message.text().includes("[React Flow]")) problems.push(`${message.type()}: ${message.text()}`);
    });
    await page.goto(`/?package=packages/${sample}.zip`);
    await expect(page.getByRole("combobox", { name: "Package", exact: true })).toHaveValue(sample, { timeout: 60_000 });
    const labels = tree(page).getByRole("button", { name: /^Open / });
    await expect(labels.first()).toBeVisible({ timeout: 60_000 });
    const refs = (await labels.evaluateAll((all) => all.map((label) => label.getAttribute("aria-label") ?? ""))).map((label) => label.slice("Open ".length));
    const failed: string[] = [];
    let ran = 0;
    for (const ref of refs) {
      await openRef(page, ref);
      await page.locator(".action-panel").getByRole("tab", { name: "Strategy", exact: true }).click();
      const offered = await strategy(page).getByRole("combobox", { name: "Strategy" }).locator("option").evaluateAll((all) => all.map((option) => (option as HTMLOptionElement).value)).catch(() => [] as string[]);
      for (const offer of offered) {
        // From the ref's own point each time: a run opens the branch it made.
        await openRef(page, ref);
        await strategy(page).getByRole("combobox", { name: "Strategy" }).selectOption(offer);
        await strategy(page).getByRole("button", { name: "Run", exact: true }).click();
        // A run clears what was said, then says what it came to as it ends;
        // the branch it made then plays, which is not what is swept.
        const said = page.locator(".activity-said");
        await expect(said).toBeVisible({ timeout: 240_000 });
        await page.getByRole("button", { name: "Pause" }).click({ timeout: 1_000 }).catch(() => undefined);
        const tone = await said.getAttribute("class");
        if (tone?.includes("tone-error")) failed.push(`${ref} · ${offer}: ${await said.textContent()}`);
        ran += 1;
      }
    }
    expect(failed).toEqual([]);
    expect(problems).toEqual([]);
    // Every sample offers something to run somewhere.
    expect(ran).toBeGreaterThan(0);
  });
}
