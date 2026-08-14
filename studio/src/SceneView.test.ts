import React from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { ExchangeFrame, Scene, SceneEntity } from "./api";
import { SceneView, graphEdgeSides, sceneEntityEffect } from "./SceneView";

const entity = {
  id: { key: "stock:wood", label: "Wood" },
  glyph: "material",
  anchor: { kind: "graph_node", node: "wood" },
  role: "state",
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

describe("Living Market scene", () => {
  test("renders exact reserves and links actor selection to its account", () => {
    const onAccount = vi.fn();
    const scene = {
      title: "The Living Market",
      surface: {
        kind: "market",
        pool: {
          id: { key: "amm:pool", label: "Energy / Credit AMM" },
          base_asset: { key: "amm:asset:energy", label: "Energy" },
          quote_asset: { key: "amm:asset:credit", label: "Credit" },
          base_reserve: "10000",
          quote_reserve: "100000",
          price_milli: "10000",
          product: "1000000000",
          issued_liquidity: "10000",
          fee_numerator: 997,
          fee_denominator: 1000,
          account: "amm:account:pool",
        },
        actors: [{
          id: { key: "amm:actor:factory", label: "Factory" },
          account: "amm:account:actor-factory",
          glyph: "machine",
          tone: "neutral",
          x: 15,
          y: 50,
          energy: "0",
          credit: "40000",
          liquidity: "0",
          utility: "0",
        }],
      },
      entities: [],
      paths: [],
      annotations: [],
      metrics: [],
      legend: [],
    } as Scene;

    render(React.createElement(SceneView, { scene, history: [scene], onAccount }));
    expect(screen.getByRole("button", { name: /AMM pool.*10.000.*10,000.*100,000/ })).toBeInTheDocument();
    screen.getByRole("button", { name: /Factory: 0 energy, 40000 credit/ }).click();
    expect(onAccount).toHaveBeenCalledWith("amm:account:actor-factory");
    expect(screen.getByRole("img", { name: /Discovered price history/ })).toBeInTheDocument();
  });
});
