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
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /Maze · least energy/ })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("New artifact computed and loaded")).toBeVisible();
  await page.getByRole("tab", { name: /Step-by-step replay/ }).click();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await expect(page.locator(".assessment-applicable")).toContainText("applicable");
  await expect(page.locator(".replay-proof")).toContainText("replay verified");
});

test("turns saved Maze search evidence into a truthful, scrubbable expansion map", async ({ page }) => {
  await page.goto("/?problem=maze&instance=showcase&strategy=a_star&document=maze%3Aa_star&view=solve&step=0&seed=0&budget=128");
  await expect(page.locator(".solve-map")).toBeVisible();
  await expect(page.locator(".solve-summary")).toContainText("A* solution");
  await expect(page.locator(".react-flow__node.search-explored")).not.toHaveCount(0);
  await expect(page.locator(".react-flow__node.search-current")).toHaveCount(1);
  await expect(page.locator(".solve-map")).not.toContainText("Planned next");
  await expect(page.locator(".solve-map")).not.toContainText("Traversed route");
  await expect(page.locator(".solve-map")).not.toContainText("Current move");

  const scrubber = page.getByRole("slider", { name: "Solver observation position" });
  await scrubber.fill("0");
  await expect(page.locator('.react-flow__node[data-id="node:start"]')).toHaveClass(/search-current/);
  await expect(page.locator(".solve-map > header")).toContainText("Expanded Start");

  await page.goto("/?problem=maze&instance=showcase&strategy=dijkstra&document=maze%3Adijkstra&view=solve&step=0&seed=0&budget=128");
  await expect(page.locator(".solve-summary")).toContainText("Dijkstra solution");
  await expect(page.locator(".solve-observation-card").first()).toContainText("Expanded Start");

  for (const [strategy, document, algorithm] of [
    ["breadth_first", "maze:breadth_first", "Breadth-first search"],
    ["pareto_energy", "maze:pareto_energy", "Exact Pareto search"],
  ]) {
    await page.goto(`/?problem=maze&instance=showcase&strategy=${strategy}&document=${encodeURIComponent(document)}&view=solve&step=0&seed=0&budget=128`);
    await expect(page.locator(".solve-summary")).toContainText(algorithm);
    await expect(page.locator('.react-flow__node[data-id="node:exit"]')).toHaveClass(/search-current/);
  }
});

test("keeps every Maze replay frame aligned and tells the key-and-gate story", async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto("/?problem=maze&instance=showcase&strategy=a_star&document=maze%3Aa_star&view=replay&step=0&seed=0&budget=128");
  const scrubber = page.getByRole("slider", { name: "Trace position" });
  const frameCount = Number(await scrubber.getAttribute("max"));
  expect(frameCount).toBeGreaterThanOrEqual(10);
  await expect(page.locator(".react-flow__node.structure-node")).toHaveCount(15);
  await expect(page.locator(".react-flow__edge.path-candidate")).not.toHaveCount(0);

  await scrubber.fill("0");
  await expect(page.locator(".attachment-group").filter({ hasText: "Brass key" })).toBeVisible();
  await expect(page.locator(".occupant-inventory")).toHaveCount(0);
  await scrubber.fill("4");
  await expect(page.locator(".occupant-inventory")).toContainText("Brass key");
  await scrubber.fill("8");
  await expect(page.locator(".occupant-inventory")).toHaveCount(0);
  await expect(page.locator('.react-flow__node[data-id="node:gate"]')).toContainText("open");

  await scrubber.fill("1");
  await expect(page.locator(".react-flow__edge.current")).toHaveCount(1);
  await scrubber.fill("4");
  await expect(page.locator(".react-flow__edge.current")).toHaveCount(0);
  await expect(page.locator(".react-flow__edge.traversed")).not.toHaveCount(0);
});

test("keeps every frame of every Maze result geometrically sound", async ({ page }) => {
  test.setTimeout(60_000);
  const results = ["breadth_first", "dijkstra", "a_star", "pareto_energy", "pareto_time"];
  for (const result of results) {
    await page.goto(`/?problem=maze&instance=showcase&strategy=${result}&document=${encodeURIComponent(`maze:${result}`)}&view=replay&step=0&seed=0&budget=128`);
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

test("animates Logistics travel without reflowing the replay layout", async ({ page }) => {
  await page.goto("/?problem=logistics&instance=showcase&strategy=reliable&document=logistics%3Areliable&view=replay&step=1&seed=0&budget=128");
  const vehicle = page.locator('.react-flow__node[data-id="entity:vehicle:fleet-1"]');
  const depot = page.locator('.react-flow__node[data-id="location:Depot"]');
  const stage = page.locator(".stage");
  await expect(vehicle).toBeVisible();
  await expect(depot).toBeVisible();
  const before = await vehicle.boundingBox();
  const stageBefore = await stage.boundingBox();
  await page.getByRole("button", { name: "Next exchange" }).click();
  await page.waitForTimeout(60);
  const during = await vehicle.boundingBox();
  const stageDuring = await stage.boundingBox();
  const motion = await vehicle.evaluate((element) => ({
    animations: element.getAnimations().filter((animation) => animation.playState === "running").length,
    animationNames: element.getAnimations().map((animation) => animation instanceof CSSAnimation ? animation.animationName : ""),
    duration: getComputedStyle(element).transitionDuration,
    easing: getComputedStyle(element).transitionTimingFunction,
    radius: getComputedStyle(element).borderRadius,
    markerRadius: getComputedStyle(element, "::after").borderRadius,
    marker: getComputedStyle(element, "::after").content,
  }));
  const depotEffect = await depot.evaluate((element) => ({
    animationNames: element.getAnimations().map((animation) => animation instanceof CSSAnimation ? animation.animationName : ""),
    radius: getComputedStyle(element).borderRadius,
  }));
  const innerEffect = await vehicle.locator(".rich-node-effect").evaluate((element) => ({
    animations: element.getAnimations().length,
    shadow: getComputedStyle(element).boxShadow,
  }));
  expect(motion.animations).toBeGreaterThan(0);
  expect(motion.animationNames).toContain("node-effect-produced");
  expect(motion.duration).toContain("0.62s");
  expect(motion.easing).toContain("cubic-bezier(0.2, 0.8, 0.2, 1)");
  expect(motion.radius).toBe("999px");
  expect(motion.markerRadius).toBe("999px");
  expect(motion.marker).toBe('"+"');
  expect(depotEffect.animationNames).toContain("node-effect-changed");
  expect(depotEffect.radius).toBe("50%");
  expect(innerEffect.animations).toBe(0);
  expect(innerEffect.shadow).toBe("none");
  expect(during?.x).not.toBe(before?.x);
  await page.waitForTimeout(300);
  const after = await vehicle.boundingBox();
  const stageAfter = await stage.boundingBox();
  expect(after?.x).not.toBe(before?.x);
  expect(after?.x).not.toBe(during?.x);
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
  test.setTimeout(30_000);
  await page.goto("/?problem=sokoban&instance=showcase&strategy=a_star&document=sokoban%3Aa_star&view=replay&step=0&seed=0&budget=128");

  for (let step = 0; step <= 15; step += 1) {
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

test("keeps graph node titles and metadata in separate readable regions", async ({ page }) => {
  await page.goto("/?problem=work_league&instance=showcase&strategy=mixed_field&document=work_league%3Amixed_field&view=replay&step=12&seed=17&budget=128");

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

  await page.goto("/?problem=marketplace&instance=showcase&strategy=market_clearing&document=marketplace%3Amarket_clearing&view=replay&step=4&seed=17&budget=128");
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

test("keeps active graph structures legible across every problem and replay frame", async ({ page }) => {
  test.setTimeout(180_000);
  await page.emulateMedia({ colorScheme: "dark" });
  const problems = ["maze", "bridge", "workshop", "marketplace", "logistics", "mission", "rescue", "work_league"];
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
