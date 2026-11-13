import type { ViewDocument } from "./api";

/**
 * The engine itself in the browser (P11, D-047): the Worker holds an
 * `axionomy-engine` over a store on the origin private file system, or in
 * memory where that is unavailable, loads packages from URLs and runs their
 * experiments. The types below are the JSON the Worker's Rust writes; Studio
 * moves onto them in P12.
 */

/** What a package says of itself (D-048). */
export type Manifest = {
  name: string;
  version: { major: number; minor: number; patch: number };
  /** The store format the package is written in. */
  format: number;
  /** The script host API its scripts are written for. */
  host_api: number;
  refs: string[];
  definitions: string[];
};

/** What loading a package did: its manifests, and the names the store already gave something else, which it kept. */
export type Loaded = {
  manifests: Manifest[];
  kept: { refs: string[]; definitions: string[] };
};

/** A point of the store: the state after `step` steps of a run's history. */
export type Point = { run: string; step: number };

export type Ended =
  | "Reached"
  | "Stopped"
  | "Horizon"
  | "Exhausted"
  | "Spent"
  | "Interrupted"
  | { Refused: { turn: number } };

/**
 * One seeded run of an arm. Its seed, a 64-bit number JSON gives JavaScript
 * inexactly, is left out.
 */
export type Trial = {
  end: Point;
  ended: Ended;
  /** The experiment's objectives at `end`, exact, in its order. */
  values: string[];
};

export type ArmResult = {
  arm: { strategy: string; start: string | null };
  start: Point;
  trials: Trial[];
};

/** An experiment's results: every arm's trials, and the store group its runs were recorded in. */
export type Results = { group: string; arms: ArmResult[] };

export type SearchProgress = {
  expanded: number;
  generated: number;
  frontier: number;
  visited: number;
  solution_depth: number | null;
};

/** Where the Worker keeps its store, and why in memory when it does. */
export type Opened = { persistent: boolean; reason?: string };

/**
 * How a package was got from its URL: downloaded and loaded; current, the
 * URL answering that it serves what the store loaded from it (304), so
 * nothing was downloaded; or kept, the URL unreachable and the store holding
 * what it served before.
 */
export type Fetched =
  | { url: string; got: "downloaded"; status: number; loaded: Loaded }
  | { url: string; got: "current"; status: 304 }
  | { url: string; got: "kept"; message: string };

export type StoreRequest =
  | { kind: "open" }
  | { kind: "load_url"; url: string }
  | { kind: "packages" }
  | { kind: "experiments" }
  | { kind: "run_experiment"; experiment: string }
  | { kind: "results"; experiment: string }
  | { kind: "draw"; point: Point };

export type StoreMessage =
  | { kind: "store_reply"; id: number; ok: true; value: unknown }
  | { kind: "store_reply"; id: number; ok: false; message: string }
  | { kind: "store_progress"; id: number; progress: SearchProgress };

type Waiting = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  progress?: (progress: SearchProgress) => void;
};

/** A client of the Worker's engine. One Worker at a time holds the store. */
export class BrowserPackages {
  private next = 1;
  private waiting = new Map<number, Waiting>();
  private worker = this.spawn();

  /** Opens the store, if it is not open yet, and says where it is kept. */
  open(): Promise<Opened> {
    return this.request({ kind: "open" }) as Promise<Opened>;
  }

  /**
   * Loads the package at `url`, asking for it whole only when the store does
   * not hold what it served, and otherwise only whether it changed.
   */
  loadUrl(url: string | URL): Promise<Fetched> {
    const absolute = new URL(url, window.location.href).href;
    return this.request({ kind: "load_url", url: absolute }) as Promise<Fetched>;
  }

  packages(): Promise<Manifest[]> {
    return this.request({ kind: "packages" }) as Promise<Manifest[]>;
  }

  experiments(): Promise<string[]> {
    return this.request({ kind: "experiments" }) as Promise<string[]>;
  }

  runExperiment(experiment: string, progress?: (progress: SearchProgress) => void): Promise<Results> {
    return this.request({ kind: "run_experiment", experiment }, progress) as Promise<Results>;
  }

  /** The results the experiment last recorded, or undefined before it has run. */
  results(experiment: string): Promise<Results | undefined> {
    return this.request({ kind: "results", experiment }) as Promise<Results | undefined>;
  }

  draw(point: Point): Promise<ViewDocument> {
    return this.request({ kind: "draw", point }) as Promise<ViewDocument>;
  }

  /**
   * Stops whatever the Worker is doing by terminating it, failing every
   * request waiting on it, and starts another, which opens the store again.
   * Writes the stopped Worker had not made durable are lost, never the
   * store's consistency.
   */
  cancel() {
    this.worker.terminate();
    this.fail(new Error("cancelled by terminating the Worker"));
    this.worker = this.spawn();
  }

  /** Stops the Worker for good, releasing the store. */
  terminate() {
    this.worker.terminate();
    this.fail(new Error("the Worker was terminated"));
  }

  private spawn() {
    const worker = new Worker(new URL("./engine.worker.ts", import.meta.url), { type: "module", name: "axionomy-store" });
    worker.onmessage = (message: MessageEvent<StoreMessage>) => {
      const data = message.data;
      const waiting = this.waiting.get(data.id);
      if (!waiting) return;
      if (data.kind === "store_progress") {
        waiting.progress?.(data.progress);
        return;
      }
      this.waiting.delete(data.id);
      if (data.ok) waiting.resolve(data.value);
      else waiting.reject(new Error(data.message));
    };
    worker.onerror = (event) => this.fail(new Error(event.message || "the store's Worker failed"));
    return worker;
  }

  private request(request: StoreRequest, progress?: (progress: SearchProgress) => void) {
    const id = this.next++;
    return new Promise<unknown>((resolve, reject) => {
      this.waiting.set(id, { resolve, reject, progress });
      this.worker.postMessage({ kind: "store", id, request });
    });
  }

  private fail(error: Error) {
    for (const waiting of this.waiting.values()) waiting.reject(error);
    this.waiting.clear();
  }
}
