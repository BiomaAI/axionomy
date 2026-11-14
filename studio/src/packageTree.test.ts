import { describe, expect, test } from "vitest";
import type { PackageView, ResultsView } from "./packageEngine";
import { branchesOf, lastPart, parsePoint } from "./packageTree";

const root = "run-00000000000000000000000000000001";
const solved = "run-00000000000000000000000000000002";
const trial = "run-00000000000000000000000000000003";
const mine = "run-00000000000000000000000000000004";

const pkg = {
  name: "maze",
  refs: [
    { name: "maze/micro", point: `${root}:0`, shipped_by: "maze" },
    { name: "maze/mine", point: `${mine}:2` },
  ],
  experiments: [],
} as unknown as PackageView;

const results = {
  experiment: "maze/micro",
  objectives: [{ objective: "maze/energy", direction: "minimize" }],
  arms: [{
    arm: { strategy: "maze/a-star" },
    start: `${root}:0`,
    trials: [
      { seed: "17", end: `${trial}:6`, ended: { kind: "reached" }, values: ["6"] },
      // A trial that applied nothing ends where it started, and makes no branch.
      { seed: "18", end: `${root}:0`, ended: { kind: "exhausted" }, values: ["0"] },
    ],
  }],
} as unknown as ResultsView;

const segments = [
  { tip: `${root}:0` },
  { from: `${root}:0`, tip: `${solved}:4` },
  { from: `${root}:0`, tip: `${trial}:6` },
  { from: `${solved}:1`, tip: `${mine}:2` },
];

describe("branchesOf", () => {
  test("joins each segment at the step it branched at, and names it by what made it", () => {
    const notes = new Map([[solved, { label: "a-star", words: "a-star, from step 0" }]]);
    const { branches, info } = branchesOf(segments, pkg, [results], notes, () => undefined);
    expect(branches.map(({ id, parent, first, last, title, tone }) => ({ id, parent, first, last, title, tone }))).toEqual([
      { id: root, parent: undefined, first: 0, last: 0, title: "maze/micro", tone: "named" },
      { id: solved, parent: { id: root, step: 0 }, first: 0, last: 4, title: "a-star, from step 0", tone: "branch" },
      { id: trial, parent: { id: root, step: 0 }, first: 0, last: 6, title: "a-star, trial 1 of maze/micro", tone: "trial" },
      { id: mine, parent: { id: solved, step: 1 }, first: 1, last: 2, title: "maze/mine", tone: "named" },
    ]);
    expect(branches[0].labels?.map(({ text, tone }) => ({ text, tone }))).toEqual([{ text: "maze/micro", tone: "fixed" }]);
    expect(branches[2].labels?.map(({ text }) => text)).toEqual(["a-star"]);
    expect(info.get(trial)?.trial?.trial.values).toEqual(["6"]);
    // The empty trial labels nothing of the history it started from.
    expect(info.get(root)?.trial).toBeUndefined();
  });

  test("opens a ref from its label", () => {
    const opened: string[] = [];
    const { branches } = branchesOf(segments, pkg, [], new Map(), (name) => opened.push(name));
    branches[3].labels?.[0].onClick?.();
    expect(opened).toEqual(["maze/mine"]);
  });
});

test("points and paths read as the tree writes them", () => {
  expect(parsePoint(`${root}:12`)).toEqual({ run: root, step: 12 });
  expect(lastPart("maze/a-star")).toBe("a-star");
});
