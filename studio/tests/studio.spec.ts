import { expect, test, type Page } from "@playwright/test";

async function mazeGeometry(page: Page) {
  return page.locator(".graph-scene").evaluate(() => {
    const elements = [...document.querySelectorAll<HTMLElement>(".react-flow__node.structure-node, .react-flow__node.occupant-overlay, .react-flow__node.attachment-group")];
    const boxes = elements.map((element) => ({ id: element.dataset.id ?? element.textContent ?? "node", box: element.getBoundingClientRect() }));
    const overlaps: string[] = [];
    for (let left = 0; left < boxes.length; left += 1) {
      for (let right = left + 1; right < boxes.length; right += 1) {
        const a = boxes[left]; const b = boxes[right];
        if (a.box.left < b.box.right - 1 && a.box.right > b.box.left + 1 && a.box.top < b.box.bottom - 1 && a.box.bottom > b.box.top + 1) overlaps.push(`${a.id} / ${b.id}`);
      }
    }
    const clipped = [...document.querySelectorAll<HTMLElement>(".structure-label, .occupant-label, .occupant-status")]
      .filter((element) => element.scrollWidth > element.clientWidth + 1 || element.scrollHeight > element.clientHeight + 1)
      .map((element) => element.textContent ?? "label");
    return { overlaps, clipped };
  });
}

test("runs a search, receives events, and scrubs its replay", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /Maze · least energy/ })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 30_000 });
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page.locator(".assessment-applicable")).toContainText("applicable");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});

test("reads scaled quantities in their units and keeps the exact atoms", async ({ page }) => {
  await page.goto("/?problem=lending&instance=showcase&view=replay&step=0&seed=17&budget=128");
  // The market's base rate is 20 basis points, stored as 2 × 10^15 WAD atoms.
  const baseRate = page.locator(".account-card .balance", { hasText: "Base rate" }).locator(".amount");
  await expect(baseRate.first()).toBeAttached();
  await expect(baseRate.first()).toHaveText("0.2 %");
  await expect(baseRate.first()).toHaveAttribute("title", "2000000000000000 atoms");
  // Counted assets stay plain numbers.
  await expect(page.locator(".account-card .balance", { hasText: "Cash" }).locator(".amount")).toHaveCount(0);
});

test("turns saved Maze search evidence into a truthful, scrubbable expansion map", async ({ page }) => {
  await page.goto("/?problem=maze&instance=showcase&strategy=a-star&document=a-star&view=solve&step=0&seed=0&budget=128");
  await expect(page.locator(".solve-map")).toBeVisible();
  await expect(page.locator(".solve-summary")).toContainText("A* solution");
  await expect(page.locator(".react-flow__node.search-explored")).not.toHaveCount(0);
  await expect(page.locator(".react-flow__node.search-current")).toHaveCount(1);
  await expect(page.locator(".solve-map")).not.toContainText("Planned next");
  await expect(page.locator(".solve-map")).not.toContainText("Traversed route");
  await expect(page.locator(".solve-map")).not.toContainText("Current move");

  const scrubber = page.getByRole("slider", { name: "Solver observation position" });
  await scrubber.fill("0");
  await expect(page.locator('.react-flow__node[data-id="node/start"]')).toHaveClass(/search-current/);
  await expect(page.locator(".solve-map > header")).toContainText("Expanded Start");

  await page.goto("/?problem=maze&instance=showcase&strategy=dijkstra&document=dijkstra&view=solve&step=0&seed=0&budget=128");
  await expect(page.locator(".solve-summary")).toContainText("Dijkstra solution");
  await expect(page.locator(".solve-observation-card").first()).toContainText("Expanded Start");

  for (const [strategy, document, algorithm] of [
    ["breadth-first", "breadth-first", "Breadth-first search"],
    ["pareto-energy", "pareto-energy", "Exact Pareto search"],
  ]) {
    await page.goto(`/?problem=maze&instance=showcase&strategy=${strategy}&document=${encodeURIComponent(document)}&view=solve&step=0&seed=0&budget=128`);
    await expect(page.locator(".solve-summary")).toContainText(algorithm);
    await expect(page.locator('.react-flow__node[data-id="node/exit"]')).toHaveClass(/search-current/);
  }
});

test("keeps every Maze replay frame aligned and tells the key-and-gate story", async ({ page }) => {
  test.setTimeout(45_000);
  const artifact = await (await page.request.get("/artifacts/maze.json")).json();
  const frames = artifact.documents.find((document: { id: string }) => document.id === "a-star").frames;
  const metricStep = (key: string, value: string) => frames.findIndex((frame: { after: { scene: { metrics: Array<{ key: string; value: string }> } } }) =>
    frame.after.scene.metrics.some((metric) => metric.key === key && metric.value === value)) + 1;
  const keyStep = metricStep("key-state", "carried");
  const openStep = metricStep("gate-state", "open");
  expect(keyStep).toBeGreaterThan(0);
  expect(openStep).toBeGreaterThan(keyStep);
  await page.goto("/?problem=maze&instance=showcase&strategy=a-star&document=a-star&view=replay&step=0&seed=0&budget=128");
  const scrubber = page.getByRole("slider", { name: "Trace position" });
  const frameCount = Number(await scrubber.getAttribute("max"));
  expect(frameCount).toBeGreaterThanOrEqual(10);
  await expect(page.locator(".react-flow__node.structure-node")).toHaveCount(15);
  await expect(page.locator(".react-flow__edge.path-candidate")).not.toHaveCount(0);

  await scrubber.fill("0");
  await expect(page.locator(".attachment-group").filter({ hasText: "Brass key" })).toBeVisible();
  await expect(page.locator(".occupant-inventory")).toHaveCount(0);
  await scrubber.fill(String(keyStep));
  await expect(page.locator(".occupant-inventory")).toContainText("Brass key");
  await scrubber.fill(String(openStep));
  await expect(page.locator(".occupant-inventory")).toHaveCount(0);
  await expect(page.locator('.react-flow__node[data-id="node/gate"]')).toContainText("open");

  await scrubber.fill("1");
  await expect(page.locator(".react-flow__edge.current")).toHaveCount(1);
  await scrubber.fill(String(keyStep));
  await expect(page.locator(".react-flow__edge.current")).toHaveCount(0);
  await expect(page.locator(".react-flow__edge.traversed")).not.toHaveCount(0);
});

test("keeps every frame of every Maze result geometrically sound", async ({ page }) => {
  test.setTimeout(60_000);
  const results = ["breadth-first", "dijkstra", "a-star", "pareto-energy", "pareto-time"];
  for (const result of results) {
    await page.goto(`/?problem=maze&instance=showcase&strategy=${result}&document=${result}&view=replay&step=0&seed=0&budget=128`);
    const scrubber = page.getByRole("slider", { name: "Trace position" });
    const frames = Number(await scrubber.getAttribute("max"));
    for (let step = 0; step <= frames; step += 1) {
      await scrubber.fill(String(step));
      await page.waitForTimeout(60);
      const geometry = await mazeGeometry(page);
      expect(geometry.overlaps, `${result} step ${step} nodes must not collide`).toEqual([]);
      expect(geometry.clipped, `${result} step ${step} labels must fit`).toEqual([]);
    }
  }
});

test("shows incremental tactical search and responsive pause control", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/");
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("connect-four");
  await page.getByRole("combobox", { name: "Instance (size)", exact: true }).selectOption("micro");
  await page.getByLabel("Seed").fill("42");
  await page.getByLabel("Budget").fill("32");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByRole("button", { name: "Running…" })).toBeDisabled();
  await expect(page.locator(".run-activity")).toContainText(/MCTS iterations/);
  await page.getByRole("button", { name: "Pause" }).click();
  await expect(page.getByText("Run paused", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Resume" }).click();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 50_000 });
  await expect(page.getByText(/Connect Four · Micro · Tactical MCTS self-play · seed 42 · budget 32/)).toBeVisible();
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await expect(page.locator(".grid-scene")).toBeVisible();
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});

test("loads a distinct static problem and its specialized renderer", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("exact-cover");
  await expect(page.getByRole("heading", { name: /Exact cover · Algorithm X/ })).toBeVisible();
  await expect(page.locator(".matrix-scene")).toBeVisible();
  await expect(page.getByText("Which subset contains which element", { exact: false })).toBeVisible();
});

test("shows endogenous AMM price discovery, exact curve evidence, and causal attribution", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/amm.json")).json();
  const steps = artifact.documents.find((document: { id: string }) => document.id === "market-day").frames.length;
  await page.goto(`/?problem=amm&instance=showcase&strategy=market-day&document=market-day&view=replay&step=${steps}&leaderboard=causal-price-contribution&seed=17&budget=128`);
  await expect(page.getByRole("heading", { name: "Living Market · endogenous price discovery" })).toBeVisible();
  await expect(page.locator(".market-scene")).toBeVisible();
  await expect(page.getByRole("img", { name: /Discovered price history/ })).toBeVisible();
  await expect(page.getByRole("img", { name: /Constant-product bonding curve/ })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Leaderboard ranking dimension" })).toHaveValue("causal-price-contribution");
  await expect(page.locator(".leaderboard-entries article")).toHaveCount(7);
  await expect(page.locator(".market-price-history circle")).toHaveCount(steps + 1);
  await expect(page.locator(".scene-metric").filter({ hasText: "Market day" })).toContainText("8 day");
  await expect(page.locator(".scene-metric").filter({ hasText: "Unmet daily needs" })).toBeVisible();

  await page.getByRole("button", { name: /Factory:/ }).click();
  await expect(page.locator(".account-card.focused")).toContainText("Factory");
  await expect(page.getByText("Demand more than the curve returns")).toBeVisible();
  await expect(page.getByText(/minimum output/).last()).toBeVisible();
});

test("shows overlapping Work League activity, service occupancy, and missed deadlines", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/work-league.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "mixed-field");
  const busyStep = document.frames.findIndex((frame: { after: { scene: { metrics: Array<{ key: string; value: string }> } } }) =>
    frame.after.scene.metrics.some((metric) => metric.key === "activities" && Number(metric.value) >= 2)) + 1;
  expect(busyStep).toBeGreaterThan(0);
  await page.goto(`/?problem=work-league&instance=showcase&strategy=mixed-field&document=mixed-field&view=replay&step=${busyStep}`);
  await expect(page.locator(".scene-metric").filter({ hasText: "Simultaneous activities" })).toContainText(/[2-4] workers/);
  await expect(page.locator(".scene-metric").filter({ hasText: "World time" })).toBeVisible();
  await expect(page.locator(".graph-scene")).toContainText("bay");
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Missed deadlines" })).toContainText(/[1-9]\d* jobs/);
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-work-league.png" });
});

test("shows an executed adaptive Logistics journey instead of a two-step prefix", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/logistics.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "mcts");
  expect(document.frames.length).toBeGreaterThan(2);
  expect(document.objectives.some((objective: { key: string; value: string }) => objective.key === "delivered" && Number(objective.value) > 0)).toBe(true);
  await page.goto(`/?problem=logistics&instance=showcase&strategy=mcts&document=mcts&view=replay&step=${document.frames.length}`);
  await expect(page.getByRole("heading", { name: "Logistics · adaptive delivery" })).toBeVisible();
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});

test("renders the generic cockpit for every canonical problem without browser errors", async ({ page }) => {
  test.setTimeout(60_000);
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.goto("/");
  const problems = [
    "maze", "sokoban", "exact-cover", "bridge", "scheduling", "workshop", "amm",
    "marketplace", "logistics", "connect-four", "rescue", "mission",
    "perishables", "work-league",
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
  await page.goto("/?problem=work-league&instance=showcase&strategy=mixed-field&document=mixed-field&view=replay&step=12&leaderboard=resource-efficiency&seed=17&budget=128");
  await expect(page.getByRole("combobox", { name: "Problem", exact: true })).toHaveValue("work-league");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Leaderboard ranking dimension" })).toHaveValue("resource-efficiency");
  await expect(page.locator(".leaderboard-step")).toContainText("12");
  await expect(page.locator(".leaderboard-entries article")).toHaveCount(4);
  await page.getByRole("combobox", { name: "Leaderboard ranking dimension" }).selectOption("least-waste");
  await expect(page).toHaveURL(/leaderboard=least-waste/);
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page).toHaveURL(/step=13/);
  await expect(page.locator(".leaderboard-step")).toContainText("13");
});

test("keeps replay shortcuts scoped to the cockpit and preserves economic wording", async ({ page }) => {
  await page.goto("/?problem=work-league&instance=showcase&strategy=mixed-field&document=mixed-field&view=replay&step=12&leaderboard=resource-efficiency&seed=17&budget=128");
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
  await expect(comparison).toHaveAttribute("open", "");
  await expect(comparison).toContainText("Initial → final state");
  await summary.focus();
  await page.keyboard.press("Space");
  await expect(page.getByRole("button", { name: "Play" })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(comparison).not.toHaveAttribute("open", "");
  await page.keyboard.press("Enter");
  await expect(comparison).toHaveAttribute("open", "");
  await expect(page.getByRole("button", { name: "Play" })).toBeVisible();
});

test("keeps the replay cockpit within a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?problem=work-league&instance=showcase&strategy=mixed-field&document=mixed-field&view=replay&step=12&leaderboard=resource-efficiency&seed=17&budget=128");
  await expect(page.locator(".stage .world-panel")).toBeVisible();
  await expect(page.locator(".stage .accounts-panel")).toBeVisible();
  await expect(page.getByRole("button", { name: "Next exchange" })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});

test("animates Logistics travel without reflowing the replay layout", async ({ page }) => {
  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=reliable&view=replay&step=8&seed=0&budget=128");
  const vehicle = page.locator('.react-flow__node[data-id="entity:van/cold"]');
  const depot = page.locator('.react-flow__node[data-id="depot"]');
  const stage = page.locator(".stage");
  // Expanded comparisons may put playback below the fold. Clicking Next can
  // scroll it into view; measure document geometry, not viewport scrolling.
  const stageLayout = () => stage.evaluate((element) => {
    const box = element.getBoundingClientRect();
    return { x: box.x + window.scrollX, y: box.y + window.scrollY, width: box.width, height: box.height };
  });
  await expect(vehicle).toBeVisible();
  await expect(depot).toBeVisible();
  const before = await vehicle.boundingBox();
  const stageBefore = await stageLayout();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await page.waitForTimeout(60);
  const during = await vehicle.boundingBox();
  const stageDuring = await stageLayout();
  const motion = await vehicle.evaluate((element) => ({
    animations: element.getAnimations().filter((animation) => animation.playState === "running").length,
    transitions: element.getAnimations().filter((animation) => animation instanceof CSSTransition && animation.transitionProperty === "transform").length,
    duration: getComputedStyle(element).transitionDuration,
    easing: getComputedStyle(element).transitionTimingFunction,
    radius: getComputedStyle(element).borderRadius,
  }));
  const depotEffect = await page.locator('.react-flow__node[data-id="hub"]').evaluate((element) => ({
    animationNames: element.getAnimations().map((animation) => animation instanceof CSSAnimation ? animation.animationName : ""),
    radius: getComputedStyle(element).borderRadius,
  }));
  const innerEffect = await vehicle.locator(".rich-node-effect").evaluate((element) => ({
    animations: element.getAnimations().length,
    shadow: getComputedStyle(element).boxShadow,
  }));
  expect(motion.animations).toBeGreaterThan(0);
  expect(motion.transitions).toBeGreaterThan(0);
  expect(motion.duration).toContain("0.62s");
  expect(motion.easing).toContain("cubic-bezier(0.2, 0.8, 0.2, 1)");
  expect(motion.radius).toBe("999px");
  expect(depotEffect.radius).toBe("50%");
  expect(innerEffect.animations).toBe(0);
  expect(innerEffect.shadow).toBe("none");
  expect(Math.hypot(during!.x - before!.x, during!.y - before!.y)).toBeGreaterThan(0.1);
  await page.waitForTimeout(300);
  const after = await vehicle.boundingBox();
  const stageAfter = await stageLayout();
  expect(Math.hypot(after!.x - before!.x, after!.y - before!.y)).toBeGreaterThan(0.1);
  expect(Math.hypot(after!.x - during!.x, after!.y - during!.y)).toBeGreaterThan(0.1);
  for (const current of [stageDuring, stageAfter]) {
    expect(current).not.toBeNull();
    expect(stageBefore).not.toBeNull();
    if (current && stageBefore) {
      expect(Math.abs(current.x - stageBefore.x)).toBeLessThan(1);
      expect(Math.abs(current.y - stageBefore.y)).toBeLessThan(1);
      expect(Math.abs(current.width - stageBefore.width)).toBeLessThan(1);
      expect(Math.abs(current.height - stageBefore.height)).toBeLessThan(1);
    }
  }
});

test("keeps every Sokoban replay entity centered on its authoritative cell", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?problem=sokoban&instance=showcase&strategy=a-star&document=a-star&view=replay&step=0&seed=0&budget=128");

  const frameCount = Number(await page.getByRole("slider", { name: "Trace position" }).getAttribute("max"));
  for (let step = 0; step <= frameCount; step += 1) {
    if (step > 0) {
      await page.getByRole("button", { name: "Next exchange" }).click();
      await page.waitForTimeout(720);
    }

    const alignment = await page.locator(".grid-entity").evaluateAll((entities) => {
      const cells = [...document.querySelectorAll<HTMLElement>(".grid-cell")];
      const board = document.querySelector<HTMLElement>(".grid-board");
      const width = Number(board && getComputedStyle(board).getPropertyValue("--grid-width"));
      return entities.map((entity) => {
        const element = entity as HTMLElement;
        const x = Number(element.dataset.gridX);
        const y = Number(element.dataset.gridY);
        const cell = cells[y * width + x];
        const entityRect = element.getBoundingClientRect();
        const cellRect = cell.getBoundingClientRect();
        return {
          label: element.getAttribute("aria-label"),
          position: getComputedStyle(element).position,
          error: Math.hypot(
            entityRect.left + entityRect.width / 2 - (cellRect.left + cellRect.width / 2),
            entityRect.top + entityRect.height / 2 - (cellRect.top + cellRect.height / 2),
          ),
        };
      });
    });

    for (const entity of alignment) {
      expect(entity.position, `step ${step}: ${entity.label} must remain out of document flow`).toBe("absolute");
      expect(entity.error, `step ${step}: ${entity.label} must be centered on its cell`).toBeLessThan(0.1);
    }
  }
});

test("gives graph semantics depth, distinct shapes, and direction-aware ports", async ({ page }) => {
  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=reliable&view=replay&step=2&seed=0&budget=128");
  const depot = page.locator('.react-flow__node[data-id="depot"]');
  const vehicle = page.locator('.react-flow__node[data-id="entity:van/cold"]');
  await expect(depot).toBeVisible();
  await expect(vehicle).toBeVisible();
  const appearance = await depot.evaluate((element) => ({
    radius: getComputedStyle(element).borderRadius,
    shadow: getComputedStyle(element).boxShadow,
  }));
  expect(appearance.radius).toBe("50%");
  expect(appearance.shadow).not.toBe("none");
  await expect(depot.locator('[data-handleid="source:road/depot/hub"]')).toHaveAttribute("data-handlepos", "right");
  await expect(vehicle).toHaveCSS("border-radius", "999px");
  const trunk = await page.locator('.react-flow__edge[data-id="road/depot/hub"] .react-flow__edge-path').getAttribute("d");
  const express = await page.locator('.react-flow__edge[data-id="road/depot/north"] .react-flow__edge-path').getAttribute("d");
  expect(trunk).not.toBe(express);
});

test("keeps graph node titles and metadata in separate readable regions", async ({ page }) => {
  await page.goto("/?problem=work-league&instance=showcase&strategy=mixed-field&document=mixed-field&view=replay&step=12&seed=17&budget=128");

  const occupants = page.locator(".react-flow__node.occupant-overlay");
  await expect(occupants.first()).toBeVisible();
  const occupantLayout = await occupants.evaluateAll((nodes) => {
    const rect = (element: Element | null) => {
      if (!element) return null;
      const box = element.getBoundingClientRect();
      return { x: box.x, y: box.y, width: box.width, height: box.height };
    };
    const intersects = (left: ReturnType<typeof rect>, right: ReturnType<typeof rect>) => Boolean(
      left && right
      && left.x < right.x + right.width && left.x + left.width > right.x
      && left.y < right.y + right.height && left.y + left.height > right.y
    );

    return nodes.map((node) => {
      const icon = node.querySelector(".occupant-content > svg");
      const label = node.querySelector<HTMLElement>(".occupant-label");
      const status = node.querySelector<HTMLElement>(".occupant-status");
      const iconBox = rect(icon);
      const labelBox = rect(label);
      const statusBox = rect(status);
      return {
        label: label?.textContent,
        labelFont: label ? getComputedStyle(label).fontSize : null,
        labelClipped: Boolean(label && label.scrollWidth > label.clientWidth + 1),
        overlaps: intersects(iconBox, labelBox) || intersects(iconBox, statusBox) || intersects(labelBox, statusBox),
      };
    });
  });

  expect(occupantLayout).toHaveLength(4);
  for (const occupant of occupantLayout) {
    expect(occupant.labelFont, `${occupant.label} must use diagram typography`).toBe("8px");
    expect(occupant.labelClipped, `${occupant.label} must remain readable`).toBe(false);
    expect(occupant.overlaps, `${occupant.label} content regions must not overlap`).toBe(false);
  }

  await page.goto("/?problem=marketplace&instance=showcase&strategy=market-clearing&document=market-clearing&view=replay&step=4&seed=17&budget=128");
  const labels = page.locator(".structure-label");
  await expect(labels.first()).toBeVisible();
  const structureLabels = await labels.evaluateAll((labels) => labels.map((label) => {
    const element = label as HTMLElement;
    return {
      label: element.textContent,
      font: getComputedStyle(element).fontSize,
      clipped: element.scrollWidth > element.clientWidth + 1 || element.scrollHeight > element.clientHeight + 1,
    };
  }));
  for (const structure of structureLabels) {
    expect(structure.font, `${structure.label} must use diagram typography`).toBe("8px");
    expect(structure.clipped, `${structure.label} must fit its node`).toBe(false);
  }
});

test("uses the generic motion compositor across every graph problem", async ({ page }) => {
  test.setTimeout(60_000);
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  const problems = ["maze", "bridge", "workshop", "marketplace", "logistics", "mission", "rescue", "work-league"];
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

test("keeps active graph structures legible across every problem and replay frame", async ({ page }) => {
  test.setTimeout(180_000);
  await page.emulateMedia({ colorScheme: "dark" });
  const problems = ["maze", "bridge", "workshop", "marketplace", "logistics", "mission", "rescue", "work-league"];
  let auditedNodes = 0;

  for (const problem of problems) {
    const response = await page.request.get(`/artifacts/${problem}.json`);
    expect(response.ok(), `${problem} artifact must be available`).toBe(true);
    const artifact = await response.json() as {
      documents: Array<{ id: string; frames: unknown[]; initial: { scene: { surface: { kind: string } } } }>;
    };

    for (const document of artifact.documents.filter((candidate) => candidate.initial.scene.surface.kind === "graph")) {
      const strategy = document.id.slice(document.id.indexOf(":") + 1);
      await page.goto(`/?problem=${problem}&instance=showcase&strategy=${strategy}&document=${encodeURIComponent(document.id)}&view=replay&step=0&seed=17&budget=128`);
      await expect(page.locator(".graph-scene")).toBeVisible();

      for (let step = 0; step <= document.frames.length; step += 1) {
        const audit = await page.evaluate(async (position) => {
          const slider = document.querySelector<HTMLInputElement>('input[aria-label="Trace position"]');
          if (!slider) throw new Error("Trace position slider is missing");
          const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
          setter?.call(slider, String(position));
          slider.dispatchEvent(new Event("input", { bubbles: true }));
          slider.dispatchEvent(new Event("change", { bubbles: true }));
          await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));

          const channels = (value: string): [number, number, number] => {
            const match = value.match(/[\d.]+/g);
            if (!match || match.length < 3) throw new Error(`Cannot parse color ${value}`);
            return [Number(match[0]), Number(match[1]), Number(match[2])];
          };
          const luminance = (value: string): number => {
            const linear = channels(value).map((channel) => {
              const normalized = channel / 255;
              return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
            });
            return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
          };
          const contrast = (foreground: string, background: string): number => {
            const lighter = Math.max(luminance(foreground), luminance(background));
            const darker = Math.min(luminance(foreground), luminance(background));
            return (lighter + 0.05) / (darker + 0.05);
          };

          const failures: string[] = [];
          const nodes = [...document.querySelectorAll<HTMLElement>(".react-flow__node.structure-node.current")];
          for (const node of nodes) {
            const background = getComputedStyle(node).backgroundColor;
            const targets = [
              ...node.querySelectorAll<HTMLElement>(".structure-label, .structure-state, .structure-state-count"),
              ...node.querySelectorAll<SVGElement>(".rich-node-content > svg"),
            ];
            for (const target of targets) {
              const ratio = contrast(getComputedStyle(target).color, background);
              const minimum = target instanceof SVGElement ? 3 : 4.5;
              const className = typeof target.className === "string" ? target.className : target.className.baseVal;
              if (ratio < minimum) failures.push(`${node.dataset.id ?? "node"} ${className} ${ratio.toFixed(2)}:1`);
            }
          }
          return { nodeCount: nodes.length, failures };
        }, step);

        auditedNodes += audit.nodeCount;
        expect(audit.failures, `${document.id} replay step ${step}`).toEqual([]);
      }
    }
  }

  expect(auditedNodes, "the audit must exercise light-filled active structures").toBeGreaterThan(0);
});

test("composes state, attachments, occupants, and context by relationship", async ({ page }) => {
  await page.goto("/?problem=workshop&instance=showcase&strategy=minimum-waste&document=minimum-waste&view=replay&step=0&seed=17&budget=128");
  const wood = page.locator('.react-flow__node[data-id="wood"]');
  await expect(wood).toContainText(/10 units wood/i);
  await expect(page.locator('.react-flow__node[data-id="entity:stock/wood"]')).toHaveCount(0);

  await page.goto("/?problem=work-league&instance=showcase&strategy=mixed-field&document=mixed-field&view=replay&step=0&seed=17&budget=128");
  await expect(page.locator(".attachment-group").filter({ hasText: "North · items" })).toContainText("Job 1");
  await expect(page.locator(".react-flow__edge.attachment-relation").first()).toBeVisible();
  await expect(page.locator(".occupant-overlay").filter({ hasText: "Atlas" })).toBeVisible();

  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=reliable&view=replay&step=0&seed=17&budget=128");
  await expect(page.locator(".attachment-group").filter({ hasText: "North customer · item" })).toContainText("Order 1");
  await page.goto("/?problem=mission&instance=showcase&strategy=coordinated&view=replay&step=4");
  await expect(page.locator(".scene-context")).toContainText("Travelling to East");
});

test("browser history restores a prior problem deep link", async ({ page }) => {
  await page.goto("/?problem=work-league&instance=showcase&strategy=mixed-field&view=replay&step=7&leaderboard=contract-value&seed=17&budget=128");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await page.getByRole("combobox", { name: "Problem", exact: true }).selectOption("maze");
  await expect(page.getByRole("heading", { name: /Maze · least energy/ })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await expect(page.locator(".leaderboard-step")).toContainText("7");
});

test("streams replay-verified Work League standings before publishing the artifact", async ({ page }) => {
  await page.goto("/?problem=work-league&instance=micro&strategy=mixed-field&view=solve&step=0&seed=17&budget=32");
  await expect(page.getByRole("heading", { name: "Work League · Mixed policy field" })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.locator(".live-standings")).toContainText(/(?:Live|Last) verified frame/, { timeout: 15_000 });
  await expect(page.locator(".live-standings")).toContainText("Contract value");
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible({ timeout: 30_000 });
});


test("exposes coupled warehouse mechanisms and the far-side exit", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/sokoban.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "a-star");
  const held = document.frames.findIndex((frame: { after: { scene: { metrics: Array<{ key: string; value: string }> } } }) =>
    frame.after.scene.metrics.some((metric) => metric.key === "held-plates" && metric.value === "1")) + 1;
  expect(held).toBeGreaterThan(0);
  await page.goto(`/?problem=sokoban&instance=showcase&strategy=a-star&view=replay&step=${held}`);
  await expect(page.locator(".scene-metric").filter({ hasText: "Crates holding plates" })).toContainText("1");
  await expect(page.locator('.grid-entity[aria-label*="Door 1"]')).toBeVisible();
  await expect(page.locator('.grid-entity[aria-label*="Exit"]')).toBeVisible();
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Doors latched open" })).toContainText("1");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  const sceneBox = await page.locator(".grid-scene").boundingBox();
  const boardBox = await page.locator(".grid-board").boundingBox();
  expect(boardBox!.y).toBeGreaterThanOrEqual(sceneBox!.y);
  expect(boardBox!.y + boardBox!.height).toBeLessThanOrEqual(sceneBox!.y + sceneBox!.height);
  await page.locator(".grid-scene").screenshot({ path: "/tmp/axionomy-depth-sokoban.png" });
});

test("exposes tactical threats and distinct Connect Four opponents", async ({ page }) => {
  await page.goto("/?problem=connect-four&instance=showcase&strategy=search-opponent&document=search-opponent&view=replay&step=0");
  await expect(page.getByRole("heading", { name: "Connect Four · against a depth-five opponent" })).toBeVisible();
  await expect(page.locator(".scene-metric").filter({ hasText: "Double-threat moves" })).toBeVisible();
  await expect(page.locator(".scene-metric").filter({ hasText: "Opponent threats" })).toBeVisible();
  const slider = page.getByRole("slider", { name: "Trace position" });
  await slider.fill(await slider.getAttribute("max") ?? "0");
  await expect(page.locator(".scene-metric").filter({ hasText: "Next player" })).toContainText("finished");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});


test("shows workshop contract choices and three production objectives", async ({ page }) => {
  await page.goto("/?problem=workshop&instance=showcase&strategy=minimum-waste&document=pareto-revenue&view=replay&step=10");
  await expect(page.getByRole("heading", { name: "Workshop Pareto · highest revenue" })).toBeVisible();
  await expect(page.locator(".scene-metric").filter({ hasText: "Contracts delivered" })).toContainText("2");
  await expect(page.locator(".scene-metric").filter({ hasText: "Revenue" })).toContainText("35 credit");
  await expect(page.locator(".attachment-group")).toContainText(["delivered and paid"]);
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-workshop.png" });
});

test("shows simultaneous bridge traffic and deadline outcomes", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/bridge.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "adaptive");
  const overlap = document.frames.findIndex((frame: { after: { scene: { entities: Array<{ status?: string }> } } }) =>
    frame.after.scene.entities.some((entity) => entity.status?.startsWith("crossing until")) && frame.after.scene.entities.some((entity) => entity.status?.startsWith("detouring until"))) + 1;
  expect(overlap).toBeGreaterThan(0);
  await page.goto(`/?problem=bridge&instance=showcase&strategy=auction&document=adaptive&view=replay&step=${overlap}`);
  await expect(page.getByRole("heading", { name: "Bridge · responsive routing" })).toBeVisible();
  await expect(page.locator(".occupant-overlay").filter({ hasText: "Agent A" })).toBeVisible();
  await expect(page.locator('.react-flow__node[data-id="detour"]')).toContainText("Paid alternate route");
  await expect(page.locator(".scene-metric").filter({ hasText: "Bridge capacity" })).toContainText("reserved / occupied");
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Late trips" })).toContainText("0");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});

test("shows funded marketplace contracts before delivery settlement", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/marketplace.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "market-clearing");
  const inTransit = document.frames.findIndex((frame: { after: { scene: { metrics: Array<{ key: string; value: string }> } } }) =>
    frame.after.scene.metrics.some((metric) => metric.key === "in-transit" && Number(metric.value) > 0)) + 1;
  expect(inTransit).toBeGreaterThan(0);
  await page.goto(`/?problem=marketplace&instance=showcase&strategy=market-clearing&view=replay&step=${inTransit}`);
  await expect(page.locator(".scene-metric").filter({ hasText: "Funds held in contracts" })).not.toContainText(/^Funds held in contracts0$/);
  await expect(page.locator(".graph-scene")).toContainText("in transit; funds held");
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Settled orders" })).toContainText("4");
  await expect(page.locator(".scene-metric").filter({ hasText: "Funds held in contracts" })).toContainText("0");
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-marketplace.png" });
});

test("shows executed scheduling time and real crew occupancy", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/scheduling.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "bounded-optimizer");
  const occupied = document.frames.findIndex((frame: { after: { scene: { metrics: Array<{ key: string; value: string }> } } }) =>
    frame.after.scene.metrics.some((metric) => metric.key === "busy-labor" && Number(metric.value) > 0)) + 1;
  expect(occupied).toBeGreaterThan(0);
  await page.goto(`/?problem=scheduling&instance=showcase&strategy=bounded-optimizer&view=replay&step=${occupied}`);
  await expect(page.locator(".timeline-scene")).toBeVisible();
  await expect(page.locator(".scene-metric").filter({ hasText: "Occupied crew" })).toBeVisible();
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Occupied crew" })).toContainText("0");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  await expect(page.locator(".timeline-state-list button")).toHaveCount(6);
  await page.locator(".timeline-scene").screenshot({ path: "/tmp/axionomy-depth-scheduling.png", animations: "disabled" });
});

test("shows split cold-chain cohorts, inherited exposure and shared fuel", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/perishables.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "storage-buffer");
  const split = document.frames.findIndex((frame: { exchange: { rate: { key: string } } }) => frame.exchange.rate.key.includes("split")) + 1;
  expect(split).toBeGreaterThan(0);
  await page.goto(`/?problem=perishables&instance=showcase&strategy=outage&document=storage-buffer&view=replay&step=${split}`);
  await expect(page.getByRole("heading", { name: "Perishables · rebalance cold storage" })).toBeVisible();
  await expect(page.locator(".attachment-group")).toContainText(["Cohort 3"]);
  await expect(page.locator(".scene-metric").filter({ hasText: "Fuel left" })).toBeVisible();
  await expect(page.locator(".graph-scene")).toContainText("Heat exposure");
  const geometry = await mazeGeometry(page);
  expect(geometry.overlaps).toEqual([]);
  expect(geometry.clipped).toEqual([]);
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Food delivered" })).toContainText("10");
  await expect(page.locator(".scene-metric").filter({ hasText: "Unmet obligations" })).toContainText("2");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  await expect.poll(() => page.locator(".graph-scene").evaluate((scene) => {
    const viewport = scene.getBoundingClientRect();
    return [...scene.querySelectorAll<HTMLElement>(".react-flow__node")].filter((node) => {
      const box = node.getBoundingClientRect();
      return box.left < viewport.left - 1 || box.right > viewport.right + 1 || box.top < viewport.top - 1 || box.bottom > viewport.bottom + 1;
    }).length;
  })).toBe(0);
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-cold-chain.png", animations: "disabled" });
});

test("reveals rescued patients only after physical discovery and records hospital outcomes", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/rescue.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "stabilize-first");
  await page.goto("/?problem=rescue&instance=showcase&strategy=observe-then-follow&document=stabilize-first&view=replay&step=0");
  await expect(page.getByRole("heading", { name: "Rescue · stabilize before transport" })).toBeVisible();
  await expect(page.locator(".attachment-group").filter({ hasText: "Patient" })).toHaveCount(0);
  const carrying = document.frames.findIndex((frame: { after: { scene: { entities: Array<{ status?: string }> } } }) => frame.after.scene.entities.some((entity) => entity.status === "In transit")) + 1;
  expect(carrying).toBeGreaterThan(0);
  const slider = page.getByRole("slider", { name: "Trace position" });
  await slider.fill(String(carrying));
  await expect(page.locator(".occupant-overlay")).toContainText("Transporting one patient");
  await expect(page.locator(".attachment-group")).toContainText(["Vitality"]);
  await slider.fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Patients evacuated" })).toContainText("2");
  await expect(page.locator(".scene-metric").filter({ hasText: "Patients lost" })).toContainText("1");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-rescue.png", animations: "disabled" });
});

test("shows independent mission travel, private briefings and cooperative extraction", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/mission.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "coordinated");
  const discovery = document.frames.findIndex((frame: { exchange: { rate: { label: string } } }) => frame.exchange.rate.label.startsWith("Scout discovers Casualty"));
  const briefing = document.frames.findIndex((frame: { exchange: { rate: { label: string } } }) => frame.exchange.rate.label.startsWith("Brief Medic on Casualty"));
  expect(briefing).toBeGreaterThan(discovery);
  const knows = (frame: typeof document.frames[number], agent: number) => frame.observations[agent].visible_accounts.some((account: { account: { label: string } }) => account.account.label === "Casualty 1");
  expect(knows(document.frames[discovery], 0)).toBe(true);
  expect(knows(document.frames[discovery], 1)).toBe(false);
  expect(knows(document.frames[briefing], 1)).toBe(true);
  const busy = document.frames.findIndex((frame: { after: { scene: { entities: Array<{ status?: string }> } } }) => frame.after.scene.entities.filter((entity) => entity.status?.startsWith("Joint extraction until")).length === 2) + 1;
  expect(busy).toBeGreaterThan(0);
  await page.goto(`/?problem=mission&instance=showcase&strategy=coordinated&view=replay&step=${busy}`);
  await expect(page.getByRole("heading", { name: "Mission · independent agents, shared plan" })).toBeVisible();
  await expect(page.locator(".scene-metric").filter({ hasText: "Teammates occupied" })).toContainText("2 / 2");
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Objectives complete" })).toContainText("2");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-mission.png", animations: "disabled" });
});

test("shows fleet cargo staging, shared-road capacity and funded delivery outcomes", async ({ page }) => {
  const artifact = await (await page.request.get("/artifacts/logistics.json")).json();
  const document = artifact.documents.find((candidate: { id: string }) => candidate.id === "mcts");
  const staged = document.frames.findIndex((frame: { exchange: { rate: { label: string } } }) => frame.exchange.rate.label.includes("stages Order")) + 1;
  expect(staged).toBeGreaterThan(0);
  await page.goto(`/?problem=logistics&instance=showcase&strategy=mcts&document=mcts&view=replay&step=${staged}`);
  await expect(page.locator(".graph-scene")).toContainText("Staged at hub for another van");
  await expect(page.locator(".scene-metric").filter({ hasText: "Hub handoffs staged" })).toContainText("1");
  await page.getByRole("slider", { name: "Trace position" }).fill(String(document.frames.length));
  await expect(page.locator(".scene-metric").filter({ hasText: "Orders delivered" })).toContainText("4");
  await expect(page.locator(".scene-metric").filter({ hasText: "Orders on time" })).toContainText("4");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
  await page.locator(".graph-scene").screenshot({ path: "/tmp/axionomy-depth-fleet.png", animations: "disabled" });
});
