import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import { App, DEFAULT_BUDGET, DEFAULT_SEED, ThinkingWorkspace } from "./App";
import { nativeEngine, type EngineEventListener } from "./engine";
import type { Study } from "./api";
import openapi from "../openapi.json";

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
  instances: [
    { profile: "micro", label: "Micro", description: "The smallest workload" },
    { profile: "showcase", label: "Showcase", description: "A default interactive workload" },
  ],
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
  instance: problem.instances[1],
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
    thinking: [],
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

function renderAt(url: string) {
  window.history.replaceState(null, "", url);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  return () => { view.unmount(); client.clear(); window.history.replaceState(null, "", "/"); };
}

test("Studio asks for the service's default seed and budget", () => {
  const request = openapi.components.schemas.StudyRequest.properties;
  expect(DEFAULT_SEED).toBe(request.seed.default);
  expect(DEFAULT_BUDGET).toBe(request.budget.default);
});

test("a link without a seed or budget opens the saved study it names", async () => {
  const cleanup = renderAt("/?problem=maze");
  expect(await screen.findByRole("heading", { name: "Maze · least energy" })).toBeInTheDocument();
  expect(screen.getByRole("spinbutton", { name: "Seed" })).toHaveValue(DEFAULT_SEED);
  expect(screen.getByRole("spinbutton", { name: "Budget" })).toHaveValue(DEFAULT_BUDGET);
  await waitFor(() => expect(window.location.search).toContain(`seed=${DEFAULT_SEED}`));
  expect(window.location.search).toContain(`budget=${DEFAULT_BUDGET}`);
  expect(screen.getByLabelText("Study on screen")).toHaveTextContent(/Saved study.*Showcase.*Least energy.*17.*128/);
  expect(screen.queryByText("The controls ask for a study that is not loaded")).not.toBeInTheDocument();
  cleanup();
});

test("a link to a study that was never saved says which study is on screen instead", async () => {
  const cleanup = renderAt("/?problem=maze&instance=micro&seed=4");
  expect(await screen.findByText("The controls ask for a study that is not loaded")).toBeInTheDocument();
  expect(screen.getByText(/You asked for Key-door maze · Micro · Least energy · seed 4 · budget 128\. There is no saved study for it, so this is the saved study Showcase · Least energy · seed 17 · budget 128\./)).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Instance (size)" })).toHaveValue("micro");
  cleanup();
});

test("a link to a missing outcome or past the last step says what opened instead", async () => {
  const cleanup = renderAt("/?problem=maze&document=dijkstra&step=5");
  expect(await screen.findByText("The linked outcome “dijkstra” is not in the saved Key-door maze study; opened Maze · least energy instead.")).toBeInTheDocument();
  expect(await screen.findByText("The link’s step 5 is past the end of this replay, which has 0 steps; Studio opened its final state.")).toBeInTheDocument();
  cleanup();
});

test("a link with an unreadable number says so and uses the default", async () => {
  const cleanup = renderAt("/?problem=maze&seed=abc&budget=0");
  expect(await screen.findByText(`The link’s seed “abc” is not a whole number; Studio used seed ${DEFAULT_SEED}.`)).toBeInTheDocument();
  expect(screen.getByText(`The link’s budget “0” is not a whole number of at least 1; Studio used budget ${DEFAULT_BUDGET}.`)).toBeInTheDocument();
  expect(screen.getByRole("spinbutton", { name: "Seed" })).toHaveValue(DEFAULT_SEED);
  cleanup();
});

type Observation = import("./api").SearchObservation;
const observation = (sequence: number, phase: string, kind: Observation["kind"], label: string): Observation => ({
  sequence, phase, algorithm: phase, kind, label, completed: sequence, total: 4, metrics: [],
});

test("saved thinking starts on the last search checkpoint, not on bookkeeping", () => {
  render(<ThinkingWorkspace active={false} observations={[
    observation(0, "prepare", "phase", "Preparing the Showcase instance of Key-door maze"),
    observation(1, "a-star-frontier", "frontier", "Expanded Start"),
    observation(2, "a-star-solution", "incumbent", "Solution accepted at Exit"),
    observation(3, "documents", "document", "Recorded the replay “Maze · least energy”"),
  ]} />);
  expect(screen.getByText("2 search checkpoints")).toBeInTheDocument();
  expect(screen.getByRole("slider", { name: "Thinking position" })).toHaveValue("2");
  expect(screen.getByText("A* solution")).toBeInTheDocument();
  expect(screen.getByRole("button", { current: "step" })).toHaveTextContent("Solution accepted at Exit");
  expect(screen.getByText("Frontier · Solutions")).toBeInTheDocument();
});

test("a strategy that records no search says so instead of listing bookkeeping", () => {
  render(<ThinkingWorkspace active={false} observations={[
    observation(0, "prepare", "phase", "Preparing the Showcase instance of Exact cover"),
    observation(1, "documents", "document", "Recorded the replay “Exact cover · Algorithm X”"),
  ]} />);
  expect(screen.getByText("This strategy records no search")).toBeInTheDocument();
  expect(screen.getByText("0 search checkpoints")).toBeInTheDocument();
  expect(screen.queryByText("Preparing the Showcase instance of Exact cover")).not.toBeInTheDocument();
  expect(screen.queryByRole("slider", { name: "Thinking position" })).not.toBeInTheDocument();
});
