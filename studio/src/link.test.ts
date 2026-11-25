import { describe, expect, test } from "vitest";
import { askedAt, askedBy, playedLink, readLink, writeLink } from "./link";
import type { PackageView } from "./packageEngine";

const root = "run-00000000000000000000000000000001";
const branch = "run-00000000000000000000000000000002";

const pkg = {
  name: "maze",
  refs: [
    { name: "maze/micro", point: `${root}:0`, shipped_by: "maze" },
    { name: "maze/mine", point: `${branch}:6` },
  ],
  experiments: [
    { experiment: "maze/stress", start: "maze/stress", goal: "maze/solved", arms: [{ strategy: "maze/a-star" }], seed: "3", budget: "9", turns: "0" },
    { experiment: "maze/micro", start: "maze/micro", goal: "maze/solved", arms: [{ strategy: "maze/a-star" }], seed: "17", budget: "100", turns: "0" },
  ],
  strategies: ["maze/a-star", "maze/uncompared"],
  goals: ["maze/solved"],
} as unknown as PackageView;

const asked = { strategy: "maze/a-star", goal: "maze/solved", seed: "17", budget: "100", turns: "0" };

describe("readLink", () => {
  test("reads a strategy played from a ref, at a step", () => {
    expect(readLink("?package=packages/maze.zip&name=maze&ref=maze/micro&strategy=maze/a-star&seed=4&step=2")).toEqual({
      link: { package: "packages/maze.zip", name: "maze", ref: "maze/micro", play: { strategy: "maze/a-star", from: undefined, goal: undefined, seed: "4", budget: undefined, turns: undefined }, point: undefined, step: 2 },
      unread: [],
    });
  });

  test("says which parameters mean nothing, and reads none of them", () => {
    const { link, unread } = readLink("?budget=128&document=marketplace:market_clearing&instance=showcase&problem=marketplace&seed=17&step=4&strategy=market_clearing&view=replay");
    expect(Object.values(link).filter((value) => value !== undefined)).toEqual([]);
    expect(unread).toEqual(["budget", "document", "instance", "problem", "seed", "step", "strategy", "view"]);
  });

  test("does not read a value it cannot use", () => {
    const { link, unread } = readLink(`?ref=maze/micro&strategy=maze/a-star&seed=-1&budget=many&step=1.5&point=${branch}:6&name=`);
    expect(link.play).toEqual({ strategy: "maze/a-star" });
    expect(link.step).toBeUndefined();
    // A ref and a point name two things: the ref is opened.
    expect(unread).toEqual(["seed", "budget", "step", "point", "name"]);
  });

  test("reads what writeLink writes", () => {
    for (const link of [
      { name: "maze", ref: "maze/micro", step: 0 },
      { name: "maze", point: `${branch}:6`, step: 3 },
      { package: "https://example.test/a.zip?b=c&d", name: "maze", ref: "maze/micro", play: { strategy: "maze/a-star", from: 2, goal: "maze/other", seed: "18446744073709551615", budget: "5", turns: "1" }, step: 4 },
    ]) expect(readLink(writeLink(link))).toEqual({ link: expect.objectContaining(link), unread: [] });
    expect(writeLink({})).toBe("");
  });
});

test("a strategy is asked what the experiment from its ref asks, or the first that compares it", () => {
  expect(askedAt(pkg, "maze/a-star", "maze/micro")).toEqual(asked);
  expect(askedAt(pkg, "maze/a-star", "maze/showcase")?.seed).toBe("3");
  expect(askedAt(pkg, "maze/uncompared", "maze/micro")?.goal).toBe("maze/solved");
  expect(askedAt(pkg, "market_clearing", "maze/micro")).toBeUndefined();
  expect(askedBy(pkg, "maze/micro", { strategy: "maze/a-star", seed: "4" })).toEqual({ ...asked, seed: "4" });
});

describe("playedLink", () => {
  test("names the strategy and the ref it played from, and only what was asked otherwise than usual", () => {
    expect(playedLink(pkg, { from: `${root}:0`, tip: `${branch}:6`, asked })).toEqual({ ref: "maze/micro", play: { strategy: "maze/a-star" } });
    expect(playedLink(pkg, { from: `${root}:0`, tip: `${branch}:6`, asked: { ...asked, seed: "4" } })?.play).toEqual({ strategy: "maze/a-star", seed: "4" });
  });

  test("names the step it played from when it is earlier than the ref's", () => {
    const later = { ...pkg, refs: [{ name: "maze/micro", point: `${root}:5`, shipped_by: "maze" }] } as unknown as PackageView;
    expect(playedLink(later, { from: `${root}:2`, tip: `${branch}:6`, asked })?.play.from).toBe(2);
    expect(playedLink(later, { from: `${root}:7`, tip: `${branch}:9`, asked })).toBeUndefined();
  });

  test("names no branch another store could not make", () => {
    // A move made it.
    expect(playedLink(pkg, { from: `${root}:0`, tip: `${branch}:1` })).toBeUndefined();
    // It left a branch only this store holds, which a ref of one's own is on.
    expect(playedLink(pkg, { from: `${branch}:6`, tip: `${root}:9`, asked })).toBeUndefined();
  });
});
