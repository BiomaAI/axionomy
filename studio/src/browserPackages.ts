import type { ViewDocument } from "./api";
import {
  EngineRefusal,
  type Applied,
  type Balances,
  type Candidate,
  type Candidates,
  type CompareRequest,
  type ComparisonView,
  type Exchange,
  type FramesQuery,
  type LoadedFrom,
  type PackageEngine,
  type PackageList,
  type PointId,
  type RanView,
  type RefFrames,
  type RefView,
  type ResultsView,
  type Running,
  type SearchProgress,
  type StoreKept,
  type StrategyRequest,
} from "./packageEngine";

/**
 * The engine itself in the browser (P11, P12, D-047): the Worker holds an
 * `axionomy-engine` over a store on the origin private file system, or in
 * memory where that is unavailable, and offers the operations the native
 * Studio server does, giving the same JSON. One Worker at a time holds the
 * saved store.
 */

/** What the Worker is asked, each the operation of the same name. */
export type StoreRequest =
  | { kind: "open" }
  | { kind: "load_url"; url: string }
  | { kind: "load_bytes"; bytes: ArrayBuffer }
  | { kind: "packages" }
  | { kind: "experiments" }
  | { kind: "draw"; point: PointId }
  | { kind: "ancestor"; point: PointId; step: number }
  | { kind: "balances"; point: PointId }
  | { kind: "candidates"; point: PointId; set: string }
  | { kind: "assess"; point: PointId; exchange: Exchange }
  | { kind: "apply"; point: PointId; exchanges: Exchange[] }
  | { kind: "read_ref"; name: string }
  | { kind: "create_ref"; name: string; point: PointId }
  | { kind: "advance_ref"; name: string; expected: PointId; to: PointId }
  | { kind: "delete_ref"; name: string; expected: PointId }
  | { kind: "frames"; name: string; query: FramesQuery }
  | { kind: "run_strategy"; point: PointId; request: StrategyRequest }
  | { kind: "run_experiment"; experiment: string }
  | { kind: "results"; experiment: string }
  | { kind: "compare"; request: CompareRequest };

export type StoreMessage =
  | { kind: "store_reply"; id: number; ok: true; value: unknown }
  | { kind: "store_reply"; id: number; ok: false; status: number; body: unknown }
  | { kind: "store_progress"; id: number; progress: SearchProgress }
  | { kind: "store_begun"; id: number };

type Waiting = {
  /** What the Worker was asked, to ask the next one should it be terminated. */
  request: StoreRequest;
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  progress?: (progress: SearchProgress) => void;
  /** Whether the Worker has begun it; a run says so as it begins. */
  begun?: boolean;
};

/** A client of the Worker's engine. */
export class BrowserPackages implements PackageEngine {
  readonly kind = "browser" as const;
  private next = 1;
  private waiting = new Map<number, Waiting>();
  /** Runs cancelled before the Worker began them, each stopped as it begins. */
  private stopping = new Set<number>();
  private worker = this.spawn();

  /** Opens the store, if it is not open yet, and says where it is kept. */
  open(): Promise<StoreKept> {
    return this.request({ kind: "open" }) as Promise<StoreKept>;
  }

  store(): Promise<StoreKept> {
    return this.open();
  }

  /**
   * Loads the package at `url`, asking for it whole only when the store does
   * not hold what it served, and otherwise only whether it changed.
   */
  loadUrl(url: string | URL): Promise<LoadedFrom> {
    const absolute = new URL(url, window.location.href).href;
    return this.request({ kind: "load_url", url: absolute }) as Promise<LoadedFrom>;
  }

  async loadFile(file: Blob): Promise<LoadedFrom> {
    // Copied to the Worker, not transferred, so the next one can be asked again.
    const bytes = await file.arrayBuffer();
    return this.request({ kind: "load_bytes", bytes }) as Promise<LoadedFrom>;
  }

  packages(): Promise<PackageList> {
    return this.request({ kind: "packages" }) as Promise<PackageList>;
  }

  experiments(): Promise<string[]> {
    return this.request({ kind: "experiments" }) as Promise<string[]>;
  }

  draw(point: PointId): Promise<ViewDocument> {
    return this.request({ kind: "draw", point }) as Promise<ViewDocument>;
  }

  async ancestor(point: PointId, step: number): Promise<PointId> {
    return ((await this.request({ kind: "ancestor", point, step })) as { point: PointId }).point;
  }

  balances(point: PointId): Promise<Balances> {
    return this.request({ kind: "balances", point }) as Promise<Balances>;
  }

  candidates(point: PointId, set: string): Promise<Candidates> {
    return this.request({ kind: "candidates", point, set }) as Promise<Candidates>;
  }

  assess(point: PointId, exchange: Exchange): Promise<Candidate> {
    return this.request({ kind: "assess", point, exchange }) as Promise<Candidate>;
  }

  apply(point: PointId, exchanges: Exchange[]): Promise<Applied> {
    return this.request({ kind: "apply", point, exchanges }) as Promise<Applied>;
  }

  readRef(name: string): Promise<RefView> {
    return this.request({ kind: "read_ref", name }) as Promise<RefView>;
  }

  createRef(name: string, point: PointId): Promise<RefView> {
    return this.request({ kind: "create_ref", name, point }) as Promise<RefView>;
  }

  advanceRef(name: string, expected: PointId, to: PointId): Promise<RefView> {
    return this.request({ kind: "advance_ref", name, expected, to }) as Promise<RefView>;
  }

  deleteRef(name: string, expected: PointId): Promise<RefView> {
    return this.request({ kind: "delete_ref", name, expected }) as Promise<RefView>;
  }

  /**
   * The Worker answers once the ref has moved from `query.after`, or when
   * the wait is over: every move of a ref goes through its engine, so it
   * hears of each and looks again then, asking nothing in the meantime.
   * Aborting `signal` stops waiting for its answer.
   */
  frames(name: string, query: FramesQuery, signal?: AbortSignal): Promise<RefFrames> {
    return this.request({ kind: "frames", name, query }, undefined, signal) as Promise<RefFrames>;
  }

  runStrategy(point: PointId, request: StrategyRequest, onProgress: (progress: SearchProgress) => void): Running<RanView> {
    return this.running<RanView>({ kind: "run_strategy", point, request }, onProgress);
  }

  runExperiment(experiment: string, onProgress?: (progress: SearchProgress) => void): Running<ResultsView> {
    return this.running<ResultsView>({ kind: "run_experiment", experiment }, onProgress);
  }

  async results(experiment: string): Promise<ResultsView | undefined> {
    try {
      return (await this.request({ kind: "results", experiment })) as ResultsView;
    } catch (cause) {
      if (cause instanceof EngineRefusal && cause.status === 404) return undefined;
      throw cause;
    }
  }

  compare(request: CompareRequest): Promise<ComparisonView> {
    return this.request({ kind: "compare", request }) as Promise<ComparisonView>;
  }

  /** Stops the Worker for good, releasing the store. */
  terminate() {
    this.worker.terminate();
    this.fail(new Error("the Worker was terminated"));
  }

  /**
   * A run in the Worker, and how to cancel it. A synchronous WebAssembly
   * run reads no message until it returns, and no flag in shared memory
   * reaches it without cross-origin isolation, which GitHub Pages cannot
   * give: cancelling terminates the Worker while it runs the run.
   */
  private running<T>(request: StoreRequest, progress?: (progress: SearchProgress) => void): Running<T> {
    const id = this.next++;
    const result = this.send(id, request, progress) as Promise<T>;
    return { result, cancel: () => this.cancel(id) };
  }

  /**
   * Fails the run `id` as cancelled and stops it: at once when the Worker
   * has begun it, and otherwise as it begins, the Worker meanwhile left to
   * finish what it is doing.
   */
  private cancel(id: number) {
    const run = this.waiting.get(id);
    if (!run) return;
    this.waiting.delete(id);
    run.reject(new Error("cancelled by terminating the Worker"));
    if (run.begun) this.restart();
    else this.stopping.add(id);
  }

  /**
   * Terminates the Worker, running a run cancelled, and starts another,
   * which opens the store again, asking it everything still waiting, in
   * order. None of it had begun but waits and fetches, which write nothing:
   * the Worker serves one request at a time. The store keeps whatever the
   * terminated Worker had committed: it writes through its file's handle as
   * it goes, and redb opens a file a commit was cut short in as it was
   * before that commit.
   */
  private restart() {
    this.worker.terminate();
    this.stopping.clear();
    this.worker = this.spawn();
    for (const [id, waiting] of this.waiting) {
      waiting.begun = false;
      this.worker.postMessage({ kind: "store", id, request: waiting.request });
    }
  }

  private spawn() {
    const worker = new Worker(new URL("./engine.worker.ts", import.meta.url), { type: "module", name: "axionomy-store" });
    worker.onmessage = (message: MessageEvent<StoreMessage>) => {
      // A Worker terminated is no longer heard: the next one answers.
      if (worker !== this.worker) return;
      const data = message.data;
      if (data.kind === "store_begun") {
        if (this.stopping.has(data.id)) return this.restart();
        const begun = this.waiting.get(data.id);
        if (begun) begun.begun = true;
        return;
      }
      const waiting = this.waiting.get(data.id);
      if (!waiting) return;
      if (data.kind === "store_progress") {
        waiting.progress?.(data.progress);
        return;
      }
      this.waiting.delete(data.id);
      if (data.ok) waiting.resolve(data.value);
      else waiting.reject(new EngineRefusal(data.status, data.body));
    };
    worker.onerror = (event) => {
      if (worker === this.worker) this.fail(new Error(event.message || "the store's Worker failed"));
    };
    return worker;
  }

  private request(request: StoreRequest, progress?: (progress: SearchProgress) => void, signal?: AbortSignal) {
    return this.send(this.next++, request, progress, signal);
  }

  private send(id: number, request: StoreRequest, progress?: (progress: SearchProgress) => void, signal?: AbortSignal) {
    return new Promise<unknown>((resolve, reject) => {
      if (signal?.aborted) return reject(signal.reason);
      this.waiting.set(id, { request, resolve, reject, progress });
      signal?.addEventListener("abort", () => {
        this.waiting.delete(id);
        reject(signal.reason);
      }, { once: true });
      this.worker.postMessage({ kind: "store", id, request });
    });
  }

  private fail(error: Error) {
    for (const waiting of this.waiting.values()) waiting.reject(error);
    this.waiting.clear();
  }
}
