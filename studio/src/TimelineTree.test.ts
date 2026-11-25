import { describe, expect, test } from "vitest";
import { layoutTree, pathOf, type TreeBranch } from "./TimelineTree";

/** A timeline of six steps, a branch at step 2 of it and a twig at step 4 of the branch, and a later branch at step 5. */
const branches: TreeBranch[] = [
  { id: "trunk", first: 0, last: 6, title: "trunk" },
  { id: "late", parent: { id: "trunk", step: 5 }, first: 5, last: 7, title: "late" },
  { id: "branch", parent: { id: "trunk", step: 2 }, first: 2, last: 5, title: "branch" },
  { id: "twig", parent: { id: "branch", step: 4 }, first: 4, last: 6, title: "twig" },
];

describe("layoutTree", () => {
  test("puts each branch below everything grown from the branches before it, earlier forks first", () => {
    const laid = layoutTree(branches);
    expect(laid.ordered.map((branch) => branch.id)).toEqual(["trunk", "branch", "twig", "late"]);
    expect([...laid.rows.entries()]).toEqual([["trunk", 0], ["branch", 1], ["twig", 2], ["late", 3]]);
  });

  test("sets first timelines of one group close, and a gap before the next group", () => {
    const laid = layoutTree([
      { id: "micro", first: 0, last: 0, title: "micro", group: "micro" },
      { id: "micro-field", first: 0, last: 0, title: "micro/field", group: "micro" },
      { id: "branch", parent: { id: "micro-field", step: 0 }, first: 0, last: 2, title: "branch" },
      { id: "showcase", first: 0, last: 0, title: "showcase", group: "showcase" },
    ]);
    const row = laid.y("micro-field") - laid.y("micro");
    expect(laid.y("branch") - laid.y("micro-field")).toBe(row);
    expect(laid.y("showcase") - laid.y("branch")).toBeGreaterThan(row);
    expect(laid.height).toBeGreaterThan(laid.y("showcase"));
  });

  test("keeps steps in order left to right and finds the step under a position", () => {
    const laid = layoutTree(branches);
    for (let step = 0; step < 7; step++) expect(laid.x(step + 1)).toBeGreaterThan(laid.x(step));
    for (let step = 0; step <= 7; step++) expect(laid.stepAt(laid.x(step))).toBe(step);
    expect(laid.gaps).toEqual([]);
  });

  test("draws a long stretch with nothing in it short, and counts it", () => {
    const long: TreeBranch[] = [{ id: "season", first: 0, last: 1000, title: "season", labels: [{ step: 400, text: "mid" }] }];
    const laid = layoutTree(long);
    expect(laid.gaps).toEqual([{ from: 0, to: 400 }, { from: 400, to: 1000 }]);
    expect(laid.x(1000) - laid.x(0)).toBeLessThan(500);
    expect(laid.stepAt(laid.x(400))).toBe(400);
  });

  test("is as wide as its widest label, however short its timelines", () => {
    const named = (text: string) => ({ step: 0, text });
    const short = layoutTree([{ id: "a", first: 0, last: 0, title: "a", labels: [named("a")] }]);
    const long = layoutTree([
      { id: "a", first: 0, last: 0, title: "a", labels: [named("work-league/showcase/sustainable-field")] },
      { id: "b", first: 0, last: 0, title: "b", labels: [named("work-league/stress"), named("work-league/stress/throughput-field")] },
    ]);
    // Each label is a pill of about 6.6px a character: the two side by side end furthest right.
    expect(long.width).toBeGreaterThan(long.x(0) + 6.6 * ("work-league/stress".length + "work-league/stress/throughput-field".length));
    expect(short.width).toBeLessThan(long.width);
  });
});

describe("pathOf", () => {
  test("follows a selection's timeline back through every branch it grew from", () => {
    const path = pathOf(branches, { id: "twig", end: 6, cursor: 3 });
    expect([...path.entries()]).toEqual([
      ["twig", { from: 4, to: 6 }],
      ["branch", { from: 2, to: 4 }],
      ["trunk", { from: 0, to: 2 }],
    ]);
  });

  test("ends where the selection ends, short of its branch's last step", () => {
    expect([...pathOf(branches, { id: "trunk", end: 3, cursor: 3 }).entries()]).toEqual([["trunk", { from: 0, to: 3 }]]);
  });
});
