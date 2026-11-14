import React from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { ExchangeFrame, Scene, SceneEntity } from "./api";
import { SceneView, graphEdgeRoutes, graphEdgeSides, graphLineRects, sceneEntityEffect } from "./SceneView";

const entity = {
  id: { key: "stock/wood", label: "Wood" },
  glyph: "material",
  anchor: { kind: "graph_node", node: "wood" },
  role: "state",
  tone: "active",
  evidence: [{ kind: "balance", account: "workshop/account/workshop", asset: "workshop/asset/wood" }],
  metrics: [],
} as SceneEntity;

function frame(consumed: string[] = [], produced: string[] = [], preserved: string[] = []): ExchangeFrame {
  const assets = (keys: string[]) => keys.map((key) => ({ asset: { key, label: key }, quantity: "1" }));
  return {
    receipt: {
      deltas: [{
        account: { key: "workshop/account/workshop", label: "Workshop" },
        consumed: assets(consumed),
        produced: assets(produced),
        preserved: assets(preserved),
      }],
    },
  } as ExchangeFrame;
}

describe("sceneEntityEffect", () => {
  test("matches an exact linked balance instead of any account delta", () => {
    expect(sceneEntityEffect(entity, frame(["workshop/asset/labor"]))).toBe("none");
    expect(sceneEntityEffect(entity, frame(["workshop/asset/wood"]))).toBe("consumed");
  });

  test("distinguishes produced and preserved economic evidence", () => {
    expect(sceneEntityEffect(entity, frame([], ["workshop/asset/wood"]))).toBe("produced");
    expect(sceneEntityEffect(entity, frame([], [], ["workshop/asset/wood"]))).toBe("preserved");
  });
});

describe("graphEdgeSides", () => {
  test("routes horizontal edges through the facing sides of their nodes", () => {
    expect(graphEdgeSides({ x: 40, y: 100 }, { x: 300, y: 120 })).toEqual({ source: "right", target: "left" });
    expect(graphEdgeSides({ x: 300, y: 120 }, { x: 40, y: 100 })).toEqual({ source: "left", target: "right" });
  });

  test("routes vertical edges through the facing sides of their nodes", () => {
    expect(graphEdgeSides({ x: 100, y: 20 }, { x: 120, y: 280 })).toEqual({ source: "bottom", target: "top" });
    expect(graphEdgeSides({ x: 120, y: 280 }, { x: 100, y: 20 })).toEqual({ source: "top", target: "bottom" });
  });
});

describe("graphEdgeRoutes", () => {
  const line = (id: string, source: string, target: string) => ({ id, source, target, label: id, classes: [] });
  const positions = new Map([["wood", { x: 0, y: 100 }], ["scrap", { x: 300, y: 0 }], ["chair", { x: 300, y: 260 }]]);

  test("gives every line its own place on a side, in the order of where it goes", () => {
    const routes = graphEdgeRoutes([line("to-chair", "wood", "chair"), line("to-scrap", "wood", "scrap")], positions);
    expect(routes.get("to-scrap")).toMatchObject({ sourceSide: "right", sourceOffset: 1 / 3 });
    expect(routes.get("to-chair")).toMatchObject({ sourceSide: "right", sourceOffset: 2 / 3 });
  });

  test("numbers the lines between two nodes around zero, whichever way they run", () => {
    const routes = graphEdgeRoutes([line("basic", "wood", "chair"), line("batch", "wood", "chair"), line("back", "chair", "wood"), line("scrap", "wood", "scrap")], positions);
    expect([routes.get("back")?.parallel, routes.get("basic")?.parallel, routes.get("batch")?.parallel, routes.get("scrap")?.parallel]).toEqual([-1, 0, 1, 0]);
  });

  test("parallel lines bend apart, so their labels do not meet", () => {
    const routes = graphEdgeRoutes([line("basic", "wood", "chair"), line("batch", "wood", "chair")], positions);
    const label = (id: string) => graphLineRects("recipe", routes.get(id)!, positions).at(-1)!;
    const [basic, batch] = [label("basic"), label("batch")];
    const overlap = basic.x < batch.x + batch.width && batch.x < basic.x + basic.width && basic.y < batch.y + batch.height && batch.y < basic.y + basic.height;
    expect(overlap).toBe(false);
  });
});

describe("reserves scene", () => {
  // A pool no sample problem has: every caption comes from the scene.
  const pool = (water: string, grain: string, ratio: string) => ({
    id: { key: "mill/pool", label: "Water / grain mill" },
    account: "mill/account/pool",
    caption: "Mill pond · x · y = k",
    base: { asset: { key: "mill/asset/water", label: "Water" }, quantity: water, symbol: "W" },
    quote: { asset: { key: "mill/asset/grain", label: "Grain" }, quantity: grain, symbol: "G" },
    ratio: { label: "Grain per bucket", value: ratio, format: { scale: 2, digits: 2, unit: "grain / water" } },
    curve: { kind: "constant_product" as const, label: "Pond curve · xy = k", product: "200000" },
  });
  const scene = (water: string, grain: string, ratio: string) => ({
    title: "The mill",
    surface: {
      kind: "reserves",
      pool: pool(water, grain, ratio),
      participants: [{
        id: { key: "mill/miller", label: "Miller" },
        account: "mill/account/miller",
        glyph: "person",
        tone: "neutral",
        x: 15,
        y: 50,
        base: "3",
        quote: "40",
      }],
    },
    entities: [],
    paths: [],
    annotations: [],
    metrics: [],
    legend: [],
  }) as Scene;

  test("writes the pool in the words its scene gives and links participants to their accounts", () => {
    const onAccount = vi.fn();
    const before = scene("100", "2000", "2000");
    render(React.createElement(SceneView, { scene: before, history: [before], onAccount }));
    expect(screen.getByRole("button", { name: /Mill pond · x · y = k.*20\.00.*grain \/ water.*100.*Water.*2,000.*Grain/ })).toBeInTheDocument();
    screen.getByRole("button", { name: "Miller: 3 Water, 40 Grain" }).click();
    expect(onAccount).toHaveBeenCalledWith("mill/account/miller");
    expect(screen.getByRole("img", { name: "Grain per bucket history, currently 20.00 grain / water" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Constant-product curve at 100 Water and 2,000 Grain" })).toBeInTheDocument();
    expect(screen.getByText("Pond curve · xy = k")).toBeInTheDocument();
  });

  test("names the step and the participant whose account it changed, and the reserves' change", () => {
    const before = scene("100", "2000", "2000");
    const after = scene("110", "1820", "1654");
    const step = {
      cues: [{ label: "Pour water for grain" }],
      exchange: { rate: { key: "mill/rate/pour", label: "Pour" } },
      receipt: { deltas: [{ account: { key: "mill/account/miller", label: "Miller" }, consumed: [{ asset: { key: "mill/asset/water", label: "Water" }, quantity: "10" }], produced: [{ asset: { key: "mill/asset/grain", label: "Grain" }, quantity: "180" }], preserved: [] }] },
    } as unknown as ExchangeFrame;
    render(React.createElement(SceneView, { scene: after, previousScene: before, history: [before, after], frame: step }));
    expect(screen.getByRole("status")).toHaveTextContent("Miller · Pour water for grain+10 W · -180 G");
    expect(screen.getByRole("button", { name: /^Miller:/ })).toHaveClass("is-active");
  });
});
