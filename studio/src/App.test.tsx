import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import { App, DEFAULT_BUDGET, DEFAULT_SEED, ThinkingWorkspace } from "./App";
import { nativeEngine, type EngineEventListener } from "./engine";
import { fetchProblems, fetchStaticProblems, fetchStaticStudy, type ProblemDescriptor, type Scene, type Study } from "./api";
import openapi from "../openapi.json";

vi.mock("echarts/core", () => ({
  use: vi.fn(),
  init: () => ({ setOption: vi.fn(), on: vi.fn(), resize: vi.fn(), dispose: vi.fn() }),
}));

const problem = {
  key: "maze",
  title: "Key-door maze",
  summary: "A fixture problem",
  family: { key: "pathfinding", label: "Pathfinding" },
  default_instance: "showcase",
  instances: [
    { key: "micro", label: "Micro", description: "The smallest workload" },
    { key: "showcase", label: "Showcase", description: "A default interactive workload" },
  ],
  default_strategy: "a-star",
  strategies: [{ key: "a-star", label: "Least energy", description: "A*", algorithm: "a*" }],
  capabilities: [{ key: "weighted-search", label: "Cost-guided search" }],
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

test("a saved study still downloading disables Study and cannot replace a newer study", async () => {
  let finishPreview!: (study: Study) => void;
  const preview = new Promise<Study>((resolve) => { finishPreview = resolve; });
  let listener!: EngineEventListener;
  const result = { ...study, key: { ...study.key, seed: 18 }, documents: [{ ...study.documents[0], title: "New computed branch" }] } as Study;
  const loaded = { ...study, documents: [{ ...study.documents[0], title: "Study from a file" }] } as Study;
  const health = vi.spyOn(nativeEngine, "health").mockResolvedValue(true);
  const defaults = vi.mocked(fetchStaticStudy).mockReturnValueOnce(preview);
  const stored = vi.spyOn(nativeEngine, "study").mockResolvedValue(result);
  const start = vi.spyOn(nativeEngine, "start").mockImplementation(async (request, onEvent) => {
    listener = onEvent;
    return { summary: { id: "study-1", request, status: "studying", completed: 0 }, close() {} };
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  await waitFor(() => expect(defaults).toHaveBeenCalled());
  expect(await screen.findByText("Loading the saved study for Key-door maze…")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByText("Engine connected")).toBeInTheDocument());
  expect(screen.getByRole("button", { name: /^Study$/ })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Copy link" })).toBeDisabled();
  await userEvent.upload(screen.getByLabelText("Load study"), new File([JSON.stringify(loaded)], "study.json", { type: "application/json" }));
  expect(await screen.findByRole("heading", { name: "Study from a file" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Study$/ })).toBeEnabled();
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
  for (const spy of [health, stored, start]) spy.mockRestore();
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
  expect(await screen.findByText("The linked outcome “dijkstra” is not in the saved Key-door maze study; opened Least energy instead.")).toBeInTheDocument();
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
type Labeled = [key: string, label: string];
const labeled = ([key, label]: Labeled) => ({ key, label });
const observation = (sequence: number, phase: Labeled, kind: Observation["kind"], label: string, algorithm: Labeled = phase): Observation => ({
  sequence, phase: labeled(phase), algorithm: labeled(algorithm), kind, label, completed: sequence, total: 4, metrics: [],
});
const preparing: Labeled = ["prepare", "Preparing"];
const recording: Labeled = ["documents", "Recording documents"];

test("saved thinking starts on the last search checkpoint, not on bookkeeping", () => {
  render(<ThinkingWorkspace active={false} observations={[
    observation(0, preparing, "phase", "Preparing the Showcase instance of Key-door maze"),
    observation(1, ["a-star-frontier", "A* frontier"], "frontier", "Expanded Start", ["a-star", "A*"]),
    observation(2, ["a-star-solution", "A* solution"], "incumbent", "Solution accepted at Exit", ["a-star", "A*"]),
    observation(3, recording, "document", "Recorded the replay “Maze · least energy”"),
  ]} />);
  expect(screen.getByText("2 search checkpoints")).toBeInTheDocument();
  expect(screen.getByRole("slider", { name: "Thinking position" })).toHaveValue("2");
  expect(screen.getByText("A* solution")).toBeInTheDocument();
  expect(screen.getByText("A*")).toBeInTheDocument();
  expect(screen.getByRole("button", { current: "step" })).toHaveTextContent("Solution accepted at Exit");
  expect(screen.getByText("Frontier · Solutions")).toBeInTheDocument();
});

test("a strategy that records no search says so instead of listing bookkeeping", () => {
  render(<ThinkingWorkspace active={false} observations={[
    observation(0, preparing, "phase", "Preparing the Showcase instance of Exact cover"),
    observation(1, recording, "document", "Recorded the replay “Exact cover · Algorithm X”"),
  ]} />);
  expect(screen.getByText("This strategy records no search")).toBeInTheDocument();
  expect(screen.getByText("0 search checkpoints")).toBeInTheDocument();
  expect(screen.queryByText("Preparing the Showcase instance of Exact cover")).not.toBeInTheDocument();
  expect(screen.queryByRole("slider", { name: "Thinking position" })).not.toBeInTheDocument();
});

/**
 * A problem registered outside the canonical suite, with a family, capability,
 * instance and phase of its own: Studio has never heard of any of them, and
 * shows each in the words the catalog and study give it.
 */
test("a problem Studio has never heard of reads in its own words", async () => {
  const bakery = {
    key: "bakery",
    title: "Corner bakery",
    summary: "Turn flour into bread before the shop opens.",
    family: { key: "home-baking", label: "Home baking" },
    default_instance: "one-tray",
    instances: [{ key: "one-tray", label: "One tray", description: "Three units of flour and one oven" }],
    default_strategy: "whole-tray",
    strategies: [{ key: "whole-tray", label: "Bake the whole tray", description: "Bake a loaf from every unit of flour.", algorithm: "batch baking" }],
    capabilities: [{ key: "proofing", label: "Proofing dough" }],
  };
  const baked = {
    key: { problem: "bakery", instance: "one-tray", strategy: "whole-tray", seed: 17, budget: 128 },
    problem: bakery,
    instance: bakery.instances[0],
    selected_document: "whole-tray",
    assessed_proposals: [],
    documents: [{
      ...study.documents[0],
      id: "whole-tray",
      title: "Bakery · the whole tray",
      source: { key: "bakery", label: "Corner bakery" },
      thinking: [observation(1, ["kneading", "Kneading the dough"], "rollout", "Kneaded loaf 1", ["by-hand", "Kneading by hand"])],
    }],
  } as Study;
  const catalog = [vi.mocked(fetchStaticProblems), vi.mocked(fetchProblems)].map((fetch) => fetch.mockResolvedValue([bakery, problem] as ProblemDescriptor[]));
  const saved = vi.mocked(fetchStaticStudy).mockImplementation(async (key) => key === "bakery" ? baked : study as Study);
  const cleanup = renderAt("/?problem=bakery");
  expect(await screen.findByRole("heading", { name: "Bakery · the whole tray" })).toBeInTheDocument();
  expect(screen.getByText("Home baking")).toBeInTheDocument();
  expect(screen.getByRole("list", { name: "What this problem exercises" })).toHaveTextContent("Proofing dough");
  expect(screen.getByRole("combobox", { name: "Instance (size)" })).toHaveValue("one-tray");
  expect(screen.getByRole("option", { name: "One tray" })).toBeInTheDocument();
  await waitFor(() => expect(window.location.search).toContain("instance=one-tray"));
  await userEvent.click(screen.getByRole("tab", { name: /Thinking/ }));
  expect(screen.getAllByText("Kneading the dough").length).toBeGreaterThan(0);
  expect(screen.getByText("Kneading by hand")).toBeInTheDocument();
  cleanup();
  for (const fetch of catalog) fetch.mockResolvedValue([problem, lending] as ProblemDescriptor[]);
  saved.mockImplementation(async () => study as Study);
});

test("a link that names no problem opens the catalog's first, without a notice", async () => {
  const catalog = [vi.mocked(fetchStaticProblems), vi.mocked(fetchProblems)].map((fetch) => fetch.mockResolvedValue([lending, problem] as ProblemDescriptor[]));
  const cleanup = renderAt("/");
  await waitFor(() => expect(screen.getByRole("combobox", { name: "Problem" })).toHaveValue("lending"));
  expect(screen.getByRole("combobox", { name: "Strategy" })).toHaveValue("least-interest");
  await waitFor(() => expect(window.location.search).toContain("problem=lending"));
  expect(screen.queryByText(/does not exist/)).not.toBeInTheDocument();
  cleanup();
  for (const fetch of catalog) fetch.mockResolvedValue([problem, lending] as ProblemDescriptor[]);
});

test("a search map draws any graph scene under its own title, without the replay's route legend", () => {
  const legend = (label: string, path?: "current" | "candidate") => ({ label, glyph: "move", tone: "neutral", ...(path ? { path } : {}) });
  const scene = {
    title: "Orchard rows",
    surface: {
      kind: "graph",
      nodes: [{ id: { key: "row/a", label: "Row A" }, classes: [], x: 0, y: 0 }, { id: { key: "row/b", label: "Row B" }, classes: [], x: 200, y: 0 }],
      edges: [{ id: "lane/a-b", source: "row/a", target: "row/b", classes: ["current"] }],
    },
    entities: [],
    paths: [],
    annotations: [],
    metrics: [],
    legend: [legend("Row"), legend("Walking now", "current"), legend("Still to pick", "candidate")],
  } as unknown as Scene;
  const visited = { ...observation(1, ["picking", "Picking"], "frontier", "Picked Row A"), subjects: [{ key: "row/a", label: "Row A" }] };
  render(<ThinkingWorkspace active={false} scene={scene} observations={[visited]} />);
  const map = screen.getByRole("region", { name: "Search expansion map" });
  expect(map.querySelector(".graph-scene")).toHaveAttribute("aria-label", "Orchard rows");
  expect(map).toHaveTextContent("Row");
  expect(map).not.toHaveTextContent("Walking now");
  expect(map).not.toHaveTextContent("Still to pick");
});
