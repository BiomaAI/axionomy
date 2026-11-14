import type { components, operations } from "./generated/api";
import { api, type ViewDocument } from "./api";

/**
 * The engine's operations Studio uses (P12), the same whichever engine
 * serves them: the native Studio server over HTTP, or the engine in the
 * browser's Worker. Both run the same Rust functions (`axionomy-studio-api`)
 * and give the same JSON, so every type here is the OpenAPI contract's.
 */
type Schemas = components["schemas"];

export type PointId = Schemas["PointId"];
export type PackageList = Schemas["PackageList"];
export type PackageView = Schemas["PackageView"];
export type ExperimentView = Schemas["ExperimentView"];
export type ObjectiveView = Schemas["ObjectiveView"];
export type LoadedView = Schemas["LoadedView"];
export type ReleaseView = Schemas["ReleaseView"];
export type Candidates = Schemas["Candidates"];
export type Candidate = Schemas["Candidate"];
export type Exchange = Schemas["Exchange"];
export type Applied = Schemas["Applied"];
export type Balances = Schemas["Balances"];
export type StrategyRequest = Schemas["StrategyRequest"];
export type RanView = Schemas["RanView"];
export type StrategyEvent = Schemas["StrategyEvent"];
export type ExperimentEvent = Schemas["ExperimentEvent"];
export type ResultsView = Schemas["ResultsView"];
export type CompareRequest = Schemas["CompareRequest"];
export type ComparisonView = Schemas["ComparisonView"];
export type RefView = Schemas["RefView"];
export type RefFrames = Schemas["RefFrames"];
export type RefMoved = Schemas["RefMoved"];
export type SearchProgress = Schemas["GraphSearchProgress"];
export type ExperimentProgress = Schemas["ExperimentProgress"];
export type SamplePackages = Schemas["SamplePackages"];
export type HistoriesView = Schemas["HistoriesView"];
export type SegmentView = Schemas["SegmentView"];
export type ChangeView = Schemas["ChangeView"];
export type FramesQuery = operations["listRefFrames"]["parameters"]["query"];
export type DivergenceView = Schemas["DivergenceView"];
export type DivergenceQuery = operations["getDivergence"]["parameters"]["query"];

/** An operation refused, with the status the server answers it with. */
export class EngineRefusal extends Error {
  readonly status: number;
  /** Where the ref is, when a move found it elsewhere (409). */
  readonly moved?: RefMoved;

  constructor(status: number, body: unknown) {
    super(refusalMessage(body, status));
    this.name = "EngineRefusal";
    this.status = status;
    if (status === 409 && body && typeof body === "object" && "current" in body) this.moved = body as RefMoved;
  }
}

function refusalMessage(body: unknown, status: number): string {
  if (body && typeof body === "object" && "error" in body && typeof body.error === "string") return body.error;
  if (typeof body === "string" && body) return body;
  return `the engine refused the request (${status})`;
}

/**
 * How a package was got: loaded from its bytes; current, the URL answering
 * that it serves what the store loaded from it (304), so nothing was
 * downloaded; or kept, the URL unreachable and the store holding what it
 * served before.
 */
export type LoadedFrom =
  | { url?: string; got: "loaded"; status?: number; loaded: LoadedView }
  | { url: string; got: "current"; status: 304; packages: string[] }
  | { url: string; got: "kept"; message: string; packages: string[] };

/** The packages a load gave the store. */
export function loadedPackages(from: LoadedFrom): string[] {
  return from.got === "loaded" ? from.loaded.packages : from.packages;
}

/** A run under way: its end, and how to stop it. */
export type Running<T> = { result: Promise<T>; cancel(): void };

/** Where the engine keeps its store. */
export type StoreKept = { persistent: boolean; reason?: string };

export interface PackageEngine {
  readonly kind: "native" | "browser";
  store(): Promise<StoreKept>;
  packages(): Promise<PackageList>;
  /** Loads a package file the visitor picked. */
  loadFile(file: Blob): Promise<LoadedFrom>;
  /** Loads the package at a URL. */
  loadUrl(url: string): Promise<LoadedFrom>;
  draw(point: PointId): Promise<ViewDocument>;
  /** The point at step `step` of `point`'s history. */
  ancestor(point: PointId, step: number): Promise<PointId>;
  /** Every account at `point` and what it holds there. */
  balances(point: PointId): Promise<Balances>;
  candidates(point: PointId, set: string): Promise<Candidates>;
  assess(point: PointId, exchange: Exchange): Promise<Candidate>;
  apply(point: PointId, exchanges: Exchange[]): Promise<Applied>;
  readRef(name: string): Promise<RefView>;
  createRef(name: string, point: PointId): Promise<RefView>;
  /**
   * Moves a ref forward; refused with `moved` when it is not at `expected`,
   * and with 403 for a ref a package ships, which is fixed.
   */
  advanceRef(name: string, expected: PointId, to: PointId): Promise<RefView>;
  /**
   * Deletes a ref from `expected`, giving the ref as it was; refused as a
   * move is. Its points stay.
   */
  deleteRef(name: string, expected: PointId): Promise<RefView>;
  /** The steps a ref was moved over after `query.after`, waiting up to `wait_seconds` for one. */
  frames(name: string, query: FramesQuery, signal?: AbortSignal): Promise<RefFrames>;
  runStrategy(point: PointId, request: StrategyRequest, onProgress: (progress: SearchProgress) => void): Running<RanView>;
  runExperiment(experiment: string, onProgress: (progress: ExperimentProgress) => void): Running<ResultsView>;
  /** The results an experiment last recorded, or undefined before it has run. */
  results(experiment: string): Promise<ResultsView | undefined>;
  compare(request: CompareRequest): Promise<ComparisonView>;
  /** The package's history tree: each history its refs are on and every branch grown from it. */
  histories(name: string): Promise<HistoriesView>;
  /**
   * Tells `listener` of every change to the store from now on, as it is
   * made, even while the run that made it goes on. A listener that may have
   * missed some is told `definitions`, to read everything again. Gives the
   * function that stops it.
   */
  onChange(listener: (change: ChangeView) => void): () => void;
  /**
   * Two points' histories compared: the last point both share, and for each
   * step after it in the window `query` asks for, the exchange each side
   * applied and the balances the two states differ in; where they are equal
   * again, and objectives at both ends. Refused with 400 when the histories
   * share no point.
   */
  divergence(query: DivergenceQuery): Promise<DivergenceView>;
}

/** The sample packages a static site serves beside Studio, each at `packages/<name>.zip`. */
export async function samplePackages(): Promise<{ name: string; url: string }[]> {
  const base = new URL(import.meta.env.BASE_URL, window.location.origin);
  const response = await fetch(new URL("packages/index.json", base), { cache: "no-store" });
  if (!response.ok) return [];
  const index = (await response.json()) as SamplePackages;
  return index.packages.map((name) => ({ name, url: new URL(`packages/${name}.zip`, base).href }));
}

/** What openapi-fetch gave, or the refusal it carried. */
function answered<T>({ data, error, response }: { data?: T; error?: unknown; response: Response }): T {
  if (data === undefined || error !== undefined) throw new EngineRefusal(response.status, error);
  return data;
}

/** A path for the URL of a ref or an experiment, one segment with its `/` written `%2F`. */
function segment(path: string): string {
  return encodeURIComponent(path);
}

/**
 * Posts `body` and reads the server-sent events its answer streams, telling
 * `onEvent` each; aborting `signal` closes the stream, which stops the run.
 */
async function streamed<E>(path: string, body: unknown, onEvent: (event: E) => void, signal: AbortSignal): Promise<void> {
  const response = await fetch(new URL(path, window.location.origin), {
    method: "POST",
    headers: { "content-type": "application/json", accept: "text/event-stream" },
    body: JSON.stringify(body ?? null),
    signal,
  });
  if (!response.ok || !response.body) {
    const text = await response.text();
    let parsed: unknown = text;
    try { parsed = JSON.parse(text); } catch { /* the body is text */ }
    throw new EngineRefusal(response.status, parsed);
  }
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffered = "";
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buffered += value;
    let end: number;
    while ((end = buffered.indexOf("\n\n")) >= 0) {
      const block = buffered.slice(0, end);
      buffered = buffered.slice(end + 2);
      const data = block.split("\n").filter((line) => line.startsWith("data:")).map((line) => line.slice(5).trimStart()).join("\n");
      if (data) onEvent(JSON.parse(data) as E);
    }
  }
}

/** What a run's events say of how far it is. */
type ProgressOf<E> = E extends { kind: "progress"; progress: infer P } ? P : never;

/** A run streamed natively: progress as it comes, then its end, or why it has none. */
function running<E extends { kind: string }, T>(path: string, body: unknown, onProgress: (progress: ProgressOf<E>) => void, end: (event: E) => T | undefined): Running<T> {
  const abort = new AbortController();
  const result = (async () => {
    let ended: T | undefined;
    let failure: EngineRefusal | undefined;
    await streamed<E>(path, body, (event) => {
      if (event.kind === "progress") onProgress((event as unknown as { progress: ProgressOf<E> }).progress);
      else if (event.kind === "failed") {
        const failed = event as unknown as { status: number; error: string };
        failure = new EngineRefusal(failed.status, { error: failed.error });
      } else ended = end(event);
    }, abort.signal);
    if (failure) throw failure;
    if (ended === undefined) throw new Error("the run ended without saying where");
    return ended;
  })();
  return { result, cancel: () => abort.abort() };
}

/**
 * The native server's changes, one stream for every listener, open while
 * any listens. A stream that reconnects may have missed changes, so its
 * listeners are told to read everything again.
 */
const nativeChanges = (() => {
  const listeners = new Set<(change: ChangeView) => void>();
  let source: EventSource | undefined;
  const tell = (change: ChangeView) => { for (const listener of listeners) listener(change); };
  return (listener: (change: ChangeView) => void) => {
    listeners.add(listener);
    if (!source) {
      let opened = false;
      source = new EventSource(new URL("/api/changes", window.location.origin));
      source.addEventListener("change", (event) => tell(JSON.parse((event as MessageEvent<string>).data) as ChangeView));
      source.addEventListener("open", () => {
        if (opened) tell({ kind: "definitions" });
        opened = true;
      });
    }
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) { source?.close(); source = undefined; }
    };
  };
})();

/** The native Studio server's engine, over HTTP. */
export const nativePackages: PackageEngine = {
  kind: "native",
  async store() { return { persistent: true }; },
  async packages() { return answered(await api.GET("/api/packages")); },
  async loadFile(file) {
    const response = await fetch(new URL("/api/packages", window.location.origin), {
      method: "POST",
      headers: { "content-type": "application/octet-stream" },
      body: file,
    });
    const body: unknown = await response.json().catch(() => undefined);
    if (!response.ok) throw new EngineRefusal(response.status, body);
    return { got: "loaded", loaded: body as LoadedView };
  },
  async loadUrl(url) {
    // The server loads bytes; the page fetches them.
    const response = await fetch(url, { cache: "no-store" });
    if (!response.ok) throw new Error(`${url} answered ${response.status} ${response.statusText}`.trim());
    const loaded = await nativePackages.loadFile(await response.blob());
    return loaded.got === "loaded" ? { url, got: "loaded", status: response.status, loaded: loaded.loaded } : loaded;
  },
  async draw(point) {
    return answered(await api.GET("/api/points/{point}/document", { params: { path: { point } } }));
  },
  async ancestor(point, step) {
    return answered(await api.GET("/api/points/{point}/ancestor", { params: { path: { point }, query: { step } } })).point;
  },
  async balances(point) {
    // The contract's balances: openapi-fetch reads a basket's pairs as arrays.
    return answered(await api.GET("/api/points/{point}/balances", { params: { path: { point } } })) as Balances;
  },
  async candidates(point, set) {
    return answered(await api.GET("/api/points/{point}/candidates", { params: { path: { point }, query: { set } } }));
  },
  async assess(point, exchange) {
    return answered(await api.POST("/api/points/{point}/assess", { params: { path: { point } }, body: { exchange } }));
  },
  async apply(point, exchanges) {
    return answered(await api.POST("/api/points/{point}/apply", { params: { path: { point } }, body: { exchanges } }));
  },
  async readRef(name) {
    return answered(await api.GET("/api/refs/{name}", { params: { path: { name } } }));
  },
  async createRef(name, point) {
    return answered(await api.POST("/api/refs", { body: { name, point } }));
  },
  async advanceRef(name, expected, to) {
    return answered(await api.POST("/api/refs/{name}/advance", { params: { path: { name } }, body: { expected, to } }));
  },
  async deleteRef(name, expected) {
    return answered(await api.DELETE("/api/refs/{name}", { params: { path: { name }, query: { expected } } }));
  },
  async frames(name, query, signal) {
    // The contract's frames: openapi-fetch reads their receipts' pairs as arrays.
    return answered(await api.GET("/api/refs/{name}/frames", { params: { path: { name }, query }, signal })) as RefFrames;
  },
  runStrategy(point, request, onProgress) {
    return running<StrategyEvent, RanView>(`/api/points/${point}/run`, request, onProgress, (event) => event.kind === "ran" ? event.ran : undefined);
  },
  runExperiment(experiment, onProgress) {
    return running<ExperimentEvent, ResultsView>(`/api/experiments/${segment(experiment)}/run`, null, onProgress, (event) => event.kind === "results" ? event.results : undefined);
  },
  async results(experiment) {
    const answer = await api.GET("/api/experiments/{experiment}/results", { params: { path: { experiment } } });
    if (answer.response.status === 404) return undefined;
    return answered(answer);
  },
  async compare(request) {
    return answered(await api.POST("/api/compare", { body: request }));
  },
  async histories(name) {
    return answered(await api.GET("/api/packages/{name}/histories", { params: { path: { name } } }));
  },
  onChange: nativeChanges,
  async divergence(query) {
    return answered(await api.GET("/api/divergence", { params: { query } }));
  },
};
