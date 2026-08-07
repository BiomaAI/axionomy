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
} from "./api";

export type EngineKind = "native" | "browser" | "static";
export type EngineConnectivity = "checking" | "connected" | "disconnected" | "static";

/**
 * The Studio depends on this capability boundary instead of HTTP directly.
 * Native HTTP/SSE and the browser Worker engine must produce the same Rust-
 * owned requests, events, and artifacts; static playback intentionally cannot
 * execute.
 */
export interface EngineClient {
  readonly kind: EngineKind;
  readonly canRun: boolean;
  catalog(): Promise<ProblemDescriptor[]>;
  defaultArtifact(problem: string): Promise<RunArtifact>;
  health(): Promise<boolean>;
  createRun(request: RunRequest): Promise<RunSummary>;
  eventsUrl(runId: string): string;
  artifact(artifactId: string): Promise<RunArtifact>;
  pause(runId: string): Promise<RunSummary>;
  resume(runId: string): Promise<RunSummary>;
  cancel(runId: string): Promise<RunSummary>;
}

export const nativeEngine: EngineClient = {
  kind: "native",
  canRun: true,
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
  createRun,
  eventsUrl: (runId) => new URL(`api/runs/${encodeURIComponent(runId)}/events`, window.location.href).toString(),
  artifact: fetchArtifact,
  pause: pauseRun,
  resume: resumeRun,
  cancel: cancelRun,
};

function unsupported(): never {
  throw new Error("Static playback cannot execute a search. Start the native engine or use the browser engine.");
}

export const staticEngine: EngineClient = {
  kind: "static",
  canRun: false,
  catalog: fetchStaticProblems,
  defaultArtifact: fetchStaticArtifact,
  async health() { return true; },
  async createRun() { return unsupported(); },
  eventsUrl: unsupported,
  async artifact() { return unsupported(); },
  async pause() { return unsupported(); },
  async resume() { return unsupported(); },
  async cancel() { return unsupported(); },
};

export function connectionLabel(kind: EngineKind, connectivity: EngineConnectivity): string {
  if (kind === "browser") return connectivity === "connected" ? "browser engine ready" : "browser engine unavailable";
  if (kind === "static") return "static playback";
  if (connectivity === "checking") return "checking native engine";
  return connectivity === "connected" ? "native engine connected" : "native engine disconnected";
}
