import {
  cancelRun,
  createRun,
  fetchArtifact,
  fetchProblems,
  fetchStaticArtifact,
  fetchStaticProblems,
  pauseRun,
  resumeRun,
  type ProblemDescriptor,
  type RunArtifact,
  type RunRequest,
  type RunSummary,
  type StudioEvent,
} from "./api";

export type EngineKind = "native" | "browser" | "static";
export type EngineConnectivity = "checking" | "connected" | "disconnected" | "static";

export type EngineRunSubscription = {
  summary: RunSummary;
  close(): void;
};

export type EngineEventListener = (event: StudioEvent) => void | Promise<void>;

/**
 * The Studio depends on this capability boundary instead of HTTP directly.
 * Native HTTP/SSE and the browser Worker engine produce the same Rust-owned
 * requests, events, and artifacts; static playback intentionally cannot run.
 */
export interface EngineClient {
  readonly kind: EngineKind;
  readonly canRun: boolean;
  readonly canPause: boolean;
  catalog(): Promise<ProblemDescriptor[]>;
  defaultArtifact(problem: string): Promise<RunArtifact>;
  health(): Promise<boolean>;
  start(request: RunRequest, listener: EngineEventListener, onError: (message: string) => void): Promise<EngineRunSubscription>;
  artifact(artifactId: string): Promise<RunArtifact>;
  pause(runId: string): Promise<RunSummary>;
  resume(runId: string): Promise<RunSummary>;
  cancel(runId: string): Promise<RunSummary>;
}

const eventKinds: StudioEvent["kind"][] = ["run_started", "progress", "search_observation", "frame_appended", "artifact_published", "run_paused", "run_resumed", "run_completed", "run_cancelled", "run_failed"];

export const nativeEngine: EngineClient = {
  kind: "native",
  canRun: true,
  canPause: true,
  catalog: fetchProblems,
  defaultArtifact: fetchStaticArtifact,
  async health() {
    try {
      const response = await fetch(new URL("api/health", window.location.href), {
        cache: "no-store",
        signal: AbortSignal.timeout(1_500),
      });
      if (!response.ok) return false;
      const health = await response.json() as { status?: string; engine?: string };
      return health.status === "ok" && health.engine === "native";
    } catch {
      return false;
    }
  },
  async start(request, listener, onError) {
    const summary = await createRun(request);
    const source = new EventSource(new URL(`api/runs/${encodeURIComponent(summary.id)}/events`, window.location.href));
    for (const kind of eventKinds) source.addEventListener(kind, (message) => {
      const event = JSON.parse((message as MessageEvent<string>).data) as StudioEvent;
      void listener(event);
    });
    source.onerror = () => {
      if (source.readyState !== EventSource.CLOSED) onError("event stream disconnected; reconnecting remains safe by sequence");
    };
    return { summary, close: () => source.close() };
  },
  artifact: fetchArtifact,
  pause: pauseRun,
  resume: resumeRun,
  cancel: cancelRun,
};

function unsupported(): never {
  throw new Error("Saved results can be replayed but not re-run. Start an engine to compute new ones.");
}

export const staticEngine: EngineClient = {
  kind: "static",
  canRun: false,
  canPause: false,
  catalog: fetchStaticProblems,
  defaultArtifact: fetchStaticArtifact,
  async health() { return true; },
  async start() { return unsupported(); },
  async artifact() { return unsupported(); },
  async pause() { return unsupported(); },
  async resume() { return unsupported(); },
  async cancel() { return unsupported(); },
};

export function connectionLabel(kind: EngineKind, connectivity: EngineConnectivity): string {
  if (kind === "browser") return connectivity === "connected" ? "Running in your browser" : "Browser engine unavailable";
  if (kind === "static") return "Replay only";
  if (connectivity === "checking") return "Checking for an engine";
  return connectivity === "connected" ? "Engine connected" : "Engine disconnected";
}
