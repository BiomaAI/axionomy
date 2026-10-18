import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import { App } from "./App";
import { nativeEngine, type EngineEventListener } from "./engine";
import type { RunArtifact } from "./api";

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

test("a late saved preview cannot replace a newly computed result", async () => {
  let finishPreview!: (artifact: RunArtifact) => void;
  const preview = new Promise<RunArtifact>((resolve) => { finishPreview = resolve; });
  let listener!: EngineEventListener;
  const result = { ...artifact, id: "computed-branch", documents: [{ ...artifact.documents[0], title: "New computed branch" }] } as RunArtifact;
  const health = vi.spyOn(nativeEngine, "health").mockResolvedValue(true);
  const defaults = vi.spyOn(nativeEngine, "defaultArtifact").mockReturnValue(preview);
  const stored = vi.spyOn(nativeEngine, "artifact").mockResolvedValue(result);
  const start = vi.spyOn(nativeEngine, "start").mockImplementation(async (request, onEvent) => {
    listener = onEvent;
    return { summary: { id: "test-run", request, status: "running", completed: 0 }, close() {} };
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  await waitFor(() => expect(defaults).toHaveBeenCalled());
  await userEvent.click(screen.getByRole("button", { name: /^Run$/ }));
  await act(async () => {
    await listener({ kind: "run_completed", run_id: "test-run", sequence: 1, artifact_id: result.id, document_id: result.selected_document_id });
  });
  expect(await screen.findByRole("heading", { name: "New computed branch" })).toBeInTheDocument();
  await act(async () => { finishPreview(artifact as RunArtifact); await preview; });
  expect(screen.getByRole("heading", { name: "New computed branch" })).toBeInTheDocument();
  view.unmount();
  client.clear();
  for (const spy of [health, defaults, stored, start]) spy.mockRestore();
});
