import { describe, expect, test } from "vitest";
import { layoutTree, pathOf, type TreeBranch } from "./HistoryTree";

/** A history of six steps, a branch at step 2 of it and a twig at step 4 of the branch, and a later branch at step 5. */
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
});

describe("pathOf", () => {
  test("follows a selection's history back through every branch it grew from", () => {
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
