import { describe, expect, test } from "vitest";
import type { ExchangeFrame, SceneEntity } from "./api";
import { graphEdgeSides, sceneEntityEffect } from "./SceneView";

const entity = {
  id: { key: "stock:wood", label: "Wood" },
  glyph: "material",
  anchor: { kind: "graph_node", node: "wood" },
  tone: "active",
  evidence: [{ kind: "balance", account: "workshop:account:workshop", asset: "workshop:asset:wood" }],
  metrics: [],
} as SceneEntity;

function frame(consumed: string[] = [], produced: string[] = [], preserved: string[] = []): ExchangeFrame {
  const assets = (keys: string[]) => keys.map((key) => ({ asset: { key, label: key }, quantity: "1" }));
  return {
    receipt: {
      deltas: [{
        account: { key: "workshop:account:workshop", label: "Workshop" },
        consumed: assets(consumed),
        produced: assets(produced),
        preserved: assets(preserved),
      }],
    },
  } as ExchangeFrame;
}

describe("sceneEntityEffect", () => {
  test("matches an exact linked balance instead of any account delta", () => {
    expect(sceneEntityEffect(entity, frame(["workshop:asset:labor"]))).toBe("none");
    expect(sceneEntityEffect(entity, frame(["workshop:asset:wood"]))).toBe("consumed");
  });

  test("distinguishes produced and preserved economic evidence", () => {
    expect(sceneEntityEffect(entity, frame([], ["workshop:asset:wood"]))).toBe("produced");
    expect(sceneEntityEffect(entity, frame([], [], ["workshop:asset:wood"]))).toBe("preserved");
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
