import { fetchStaticArtifact, type ProblemDescriptor, type RunArtifact, type RunRequest, type RunSummary, type StudioEvent } from "./api";
import type { EngineClient, EngineEventListener, EngineRunSubscription } from "./engine";

type WorkerMessage =
  | { kind: "ready"; protocol: string; catalog: ProblemDescriptor[] }
  | { kind: "event"; event: StudioEvent }
  | { kind: "artifact"; artifact: RunArtifact };

class BrowserEngine implements EngineClient {
  readonly kind = "browser" as const;
  readonly canRun = true;
  readonly canPause = false;
  private worker?: Worker;
  private catalogValue?: ProblemDescriptor[];
  private ready!: Promise<ProblemDescriptor[]>;
  private resolveReady!: (catalog: ProblemDescriptor[]) => void;
  private rejectReady!: (error: Error) => void;
  private artifacts = new Map<string, RunArtifact>();
  private active?: { id: string; request: RunRequest; listener: EngineEventListener; lastSequence: number };

  private spawn() {
    const worker = new Worker(new URL("./engine.worker.ts", import.meta.url), { type: "module", name: "axionomy-engine" });
    this.worker = worker;
    this.ready = new Promise((resolve, reject) => { this.resolveReady = resolve; this.rejectReady = reject; });
    worker.onmessage = (message: MessageEvent<WorkerMessage>) => {
      const data = message.data;
      if (data.kind === "ready") {
        if (data.protocol !== "axionomy-studio-worker/1") this.rejectReady(new Error(`unsupported browser engine protocol ${data.protocol}`));
        else { this.catalogValue = data.catalog; this.resolveReady(data.catalog); }
      } else if (data.kind === "artifact") {
        this.artifacts.set(data.artifact.id, data.artifact);
      } else if (this.active && data.event.run_id === this.active.id) {
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
  defaultArtifact(problem: string) { return fetchStaticArtifact(problem); }

  async start(request: RunRequest, listener: EngineEventListener, _onError: (message: string) => void): Promise<EngineRunSubscription> {
    this.ensureSpawned();
    await this.ready;
    const id = `browser-${Date.now().toString(36)}-${request.seed}`;
    const summary: RunSummary = { id, request, status: "running", completed: 0 };
    this.active = { id, request, listener, lastSequence: 0 };
    this.worker?.postMessage({ kind: "run", runId: id, request });
    return { summary, close: () => { if (this.active?.id === id) this.active = undefined; } };
  }

  async artifact(artifactId: string) {
    const artifact = this.artifacts.get(artifactId);
    if (!artifact) throw new Error(`browser artifact ${artifactId} was not published`);
    return artifact;
  }

  async cancel(runId: string): Promise<RunSummary> {
    if (!this.active || this.active.id !== runId) throw new Error(`browser run ${runId} is not active`);
    const { request, listener, lastSequence } = this.active;
    this.worker?.terminate();
    this.worker = undefined;
    this.active = undefined;
    void listener({ kind: "run_cancelled", run_id: runId, sequence: lastSequence + 1 });
    this.spawn();
    return { id: runId, request, status: "cancelled", completed: 0, message: "cancelled by terminating the isolated Worker" };
  }

  async pause(): Promise<RunSummary> { throw new Error("Browser runs do not yet support resumable pause; cancellation remains immediate and isolated."); }
  async resume(): Promise<RunSummary> { throw new Error("Browser runs do not yet support resumable pause."); }
}

export const browserEngine: EngineClient = new BrowserEngine();
