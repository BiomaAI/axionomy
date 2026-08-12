import { describe, expect, test } from "vitest";
import type { ExchangeFrame, SceneEntity } from "./api";
import { sceneEntityEffect } from "./SceneView";

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
