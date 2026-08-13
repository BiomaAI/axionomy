import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import { App } from "./App";

vi.mock("echarts/core", () => ({
  use: vi.fn(),
  init: () => ({ setOption: vi.fn(), on: vi.fn(), resize: vi.fn(), dispose: vi.fn() }),
}));

const problem = {
  key: "maze",
  title: "Key-door maze",
  summary: "A fixture problem",
  family: "pathfinding",
  default_instance: "showcase",
  instances: [{ key: "showcase", label: "Showcase", description: "A default interactive workload", profile: "showcase" }],
  default_strategy: "a_star",
  strategies: [{ key: "a_star", label: "Least energy", description: "A*", algorithm: "a*" }],
  capabilities: ["weighted_search"],
};

const artifact = {
  id: "maze:showcase:a_star:17:128",
  problem,
  instance: problem.instances[0],
  request: { problem: "maze", instance: "showcase", strategy: "a_star", seed: 17, budget: 128 },
  selected_document_id: "maze:a_star",
  assessed_proposals: [],
  documents: [{
    id: "maze:a_star",
    title: "Maze · least energy",
    description: "Replay fixture",
    source: { key: "maze", label: "Key-door maze" },
    model: { rates: [], goal: [], invariants: [] },
    initial: { index: 0, accounts: [], scene: null, leaderboards: [] },
    frames: [],
    objectives: [],
    pareto_fronts: [],
    proposals: [],
    telemetry: [],
    observations: [],
    solve_observations: [],
  }],
};

vi.mock("./api", async (importOriginal) => {
  const original = await importOriginal<typeof import("./api")>();
  return {
    ...original,
    fetchProblems: vi.fn(async () => [problem]),
    fetchStaticProblems: vi.fn(async () => [problem]),
    fetchStaticArtifact: vi.fn(async () => artifact),
  };
});

test("loads a full portable artifact and exposes the model workbench", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  expect(await screen.findByRole("heading", { name: "Maze · least energy" })).toBeInTheDocument();
  expect(screen.getByText("Rates, roles, goals & invariants")).toBeInTheDocument();
  expect(screen.getByText("replay verified")).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Axionomy" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Go to initial state" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Go to final state" })).toBeInTheDocument();
});
