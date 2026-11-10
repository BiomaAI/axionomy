import {
  cancelStudy,
  createStudy,
  fetchStudy,
  fetchProblems,
  fetchStaticStudy,
  fetchStaticProblems,
  pauseStudy,
  resumeStudy,
  type ProblemDescriptor,
  type Study,
  type StudyRequest,
  type StudySummary,
  type StudioEvent,
} from "./api";

export type EngineKind = "native" | "browser" | "static";
export type EngineConnectivity = "checking" | "connected" | "disconnected" | "static";

export type EngineStudySubscription = {
  summary: StudySummary;
  close(): void;
};

export type EngineEventListener = (event: StudioEvent) => void | Promise<void>;

/**
 * The Studio depends on this capability boundary instead of HTTP directly.
 * Native HTTP/SSE and the browser Worker engine produce the same Rust-owned
 * requests, events, and studies; static playback intentionally cannot study.
 */
export interface EngineClient {
  readonly kind: EngineKind;
  readonly canStudy: boolean;
  readonly canPause: boolean;
  catalog(): Promise<ProblemDescriptor[]>;
  defaultStudy(problem: string): Promise<Study>;
  health(): Promise<boolean>;
  start(request: StudyRequest, listener: EngineEventListener, onError: (message: string) => void): Promise<EngineStudySubscription>;
  /** The study once it is published. */
  study(studyId: string): Promise<Study>;
  pause(studyId: string): Promise<StudySummary>;
  resume(studyId: string): Promise<StudySummary>;
  cancel(studyId: string): Promise<StudySummary>;
}

const eventKinds: StudioEvent["kind"][] = ["study_started", "progress", "thinking", "frame_appended", "study_published", "study_paused", "study_resumed", "study_completed", "study_cancelled", "study_failed"];

export const nativeEngine: EngineClient = {
  kind: "native",
  canStudy: true,
  canPause: true,
  catalog: fetchProblems,
  defaultStudy: fetchStaticStudy,
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
    const summary = await createStudy(request);
    const source = new EventSource(new URL(`api/studies/${encodeURIComponent(summary.id)}/events`, window.location.href));
    for (const kind of eventKinds) source.addEventListener(kind, (message) => {
      const event = JSON.parse((message as MessageEvent<string>).data) as StudioEvent;
      void listener(event);
    });
    source.onerror = () => {
      if (source.readyState !== EventSource.CLOSED) onError("event stream disconnected; reconnecting remains safe by sequence");
    };
    return { summary, close: () => source.close() };
  },
  study: fetchStudy,
  pause: pauseStudy,
  resume: resumeStudy,
  cancel: cancelStudy,
};

function unsupported(): never {
  throw new Error("Saved studies can be replayed but not recomputed. Start an engine to compute new ones.");
}

export const staticEngine: EngineClient = {
  kind: "static",
  canStudy: false,
  canPause: false,
  catalog: fetchStaticProblems,
  defaultStudy: fetchStaticStudy,
  async health() { return true; },
  async start() { return unsupported(); },
  async study() { return unsupported(); },
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
