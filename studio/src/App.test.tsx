import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import { App } from "./App";
import { nativeEngine, type EngineEventListener } from "./engine";
import type { Study } from "./api";

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
  instances: [{ profile: "showcase", label: "Showcase", description: "A default interactive workload" }],
  default_strategy: "a-star",
  strategies: [{ key: "a-star", label: "Least energy", description: "A*", algorithm: "a*" }],
  capabilities: ["weighted_search"],
};

const lending = {
  ...problem,
  key: "lending",
  title: "Lending market",
  default_strategy: "least-interest",
  strategies: [{ key: "least-interest", label: "Least interest", description: "Dijkstra", algorithm: "dijkstra" }],
};

const study = {
  key: { problem: "maze", instance: "showcase", strategy: "a-star", seed: 17, budget: 128 },
  problem,
  instance: problem.instances[0],
  selected_document: "a-star",
  assessed_proposals: [],
  documents: [{
    id: "a-star",
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
    fetchProblems: vi.fn(async () => [problem, lending]),
    fetchStaticProblems: vi.fn(async () => [problem, lending]),
    fetchStaticStudy: vi.fn(async () => study),
  };
});

test("loads a full portable study and exposes the model workbench", async () => {
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
  let finishPreview!: (study: Study) => void;
  const preview = new Promise<Study>((resolve) => { finishPreview = resolve; });
  let listener!: EngineEventListener;
  const result = { ...study, key: { ...study.key, seed: 18 }, documents: [{ ...study.documents[0], title: "New computed branch" }] } as Study;
  const health = vi.spyOn(nativeEngine, "health").mockResolvedValue(true);
  const defaults = vi.spyOn(nativeEngine, "defaultStudy").mockReturnValue(preview);
  const stored = vi.spyOn(nativeEngine, "study").mockResolvedValue(result);
  const start = vi.spyOn(nativeEngine, "start").mockImplementation(async (request, onEvent) => {
    listener = onEvent;
    return { summary: { id: "study-1", request, status: "studying", completed: 0 }, close() {} };
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  await waitFor(() => expect(defaults).toHaveBeenCalled());
  await userEvent.click(screen.getByRole("button", { name: /^Study$/ }));
  await act(async () => {
    await listener({ kind: "study_completed", study_id: "study-1", sequence: 1, document: result.selected_document });
  });
  expect(stored).toHaveBeenCalledWith("study-1");
  expect(await screen.findByRole("heading", { name: "New computed branch" })).toBeInTheDocument();
  await act(async () => { finishPreview(study as Study); await preview; });
  expect(screen.getByRole("heading", { name: "New computed branch" })).toBeInTheDocument();
  view.unmount();
  client.clear();
  for (const spy of [health, defaults, stored, start]) spy.mockRestore();
});

test("a link that omits options opens the problem's defaults without a warning", async () => {
  window.history.replaceState(null, "", "/?problem=lending");
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  await waitFor(() => expect(screen.getByRole("combobox", { name: "Strategy" })).toHaveValue("least-interest"));
  expect(screen.getByRole("combobox", { name: "Instance (size)" })).toHaveValue("showcase");
  expect(screen.queryByText(/linked options were not valid/)).not.toBeInTheDocument();
  view.unmount();
  client.clear();
  window.history.replaceState(null, "", "/");
});

test("a link with an option the problem lacks falls back with a warning", async () => {
  window.history.replaceState(null, "", "/?problem=lending&strategy=a-star");
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  expect(await screen.findByText("Some linked options were not valid for Lending market; Studio selected its defaults.")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Strategy" })).toHaveValue("least-interest");
  view.unmount();
  client.clear();
  window.history.replaceState(null, "", "/");
});
