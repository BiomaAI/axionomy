import { fetchStaticStudy, type ProblemDescriptor, type Study, type StudyFetchOptions, type StudyRequest, type StudySummary, type StudioEvent } from "./api";
import type { EngineClient, EngineEventListener, EngineStudySubscription } from "./engine";

type WorkerMessage =
  | { kind: "ready"; protocol: string; catalog: ProblemDescriptor[] }
  | { kind: "event"; event: StudioEvent }
  | { kind: "study"; studyId: string; study: Study };

class BrowserEngine implements EngineClient {
  readonly kind = "browser" as const;
  readonly canStudy = true;
  readonly canPause = false;
  private worker?: Worker;
  private catalogValue?: ProblemDescriptor[];
  private ready!: Promise<ProblemDescriptor[]>;
  private resolveReady!: (catalog: ProblemDescriptor[]) => void;
  private rejectReady!: (error: Error) => void;
  private studies = new Map<string, Study>();
  private started = 0;
  private active?: { id: string; request: StudyRequest; listener: EngineEventListener; lastSequence: number };

  private spawn() {
    const worker = new Worker(new URL("./engine.worker.ts", import.meta.url), { type: "module", name: "axionomy-engine" });
    this.worker = worker;
    this.ready = new Promise((resolve, reject) => { this.resolveReady = resolve; this.rejectReady = reject; });
    worker.onmessage = (message: MessageEvent<WorkerMessage>) => {
      const data = message.data;
      if (data.kind === "ready") {
        if (data.protocol !== "axionomy-studio-worker/1") this.rejectReady(new Error(`unsupported browser engine protocol ${data.protocol}`));
        else { this.catalogValue = data.catalog; this.resolveReady(data.catalog); }
      } else if (data.kind === "study") {
        this.studies.set(data.studyId, data.study);
      } else if (this.active && data.event.study_id === this.active.id) {
        this.active.lastSequence = Math.max(this.active.lastSequence, data.event.sequence);
        void this.active.listener(data.event);
      }
    };
    worker.onerror = (event) => this.rejectReady(new Error(event.message || "browser engine worker failed"));
    worker.postMessage({ kind: "initialize" });
  }

  private ensureSpawned() {
    if (!this.worker) this.spawn();
  }

  async health() {
    try { this.ensureSpawned(); await this.ready; return true; } catch { return false; }
  }

  async catalog() { this.ensureSpawned(); return this.catalogValue ?? this.ready; }
  defaultStudy(problem: string, options?: StudyFetchOptions) { return fetchStaticStudy(problem, options); }

  async start(request: StudyRequest, listener: EngineEventListener, _onError: (message: string) => void): Promise<EngineStudySubscription> {
    this.ensureSpawned();
    await this.ready;
    this.started += 1;
    const id = `study-${this.started}`;
    const summary: StudySummary = { id, request, status: "studying", completed: 0 };
    this.active = { id, request, listener, lastSequence: 0 };
    this.worker?.postMessage({ kind: "study", studyId: id, request });
    return { summary, close: () => { if (this.active?.id === id) this.active = undefined; } };
  }

  async study(studyId: string) {
    const study = this.studies.get(studyId);
    if (!study) throw new Error(`browser study ${studyId} has not been published`);
    return study;
  }

  async cancel(studyId: string): Promise<StudySummary> {
    if (!this.active || this.active.id !== studyId) throw new Error(`browser study ${studyId} is not active`);
    const { request, listener, lastSequence } = this.active;
    this.worker?.terminate();
    this.worker = undefined;
    this.active = undefined;
    void listener({ kind: "study_cancelled", study_id: studyId, sequence: lastSequence + 1 });
    this.spawn();
    return { id: studyId, request, status: "cancelled", completed: 0, message: "cancelled by terminating the isolated Worker" };
  }

  async pause(): Promise<StudySummary> { throw new Error("Browser studies do not yet support resumable pause; cancellation remains immediate and isolated."); }
  async resume(): Promise<StudySummary> { throw new Error("Browser studies do not yet support resumable pause."); }
}

export const browserEngine: EngineClient = new BrowserEngine();
