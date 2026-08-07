/// <reference lib="webworker" />
import init, { catalog as wasmCatalog, protocol, run as wasmRun } from "./generated/wasm/axionomy_studio_wasm";
import type { ProblemDescriptor, RunArtifact, RunRequest, StudioEvent } from "./api";

type Command =
  | { kind: "initialize" }
  | { kind: "run"; runId: string; request: RunRequest };

type WorkerMessage =
  | { kind: "ready"; protocol: string; catalog: ProblemDescriptor[] }
  | { kind: "event"; event: StudioEvent }
  | { kind: "artifact"; artifact: RunArtifact };

const scope = self as DedicatedWorkerGlobalScope;
let initialized: Promise<void> | undefined;

function initialize() {
  initialized ??= init().then(() => undefined);
  return initialized;
}

function post(message: WorkerMessage) {
  scope.postMessage(message);
}

scope.onmessage = async (message: MessageEvent<Command>) => {
  try {
    await initialize();
    if (message.data.kind === "initialize") {
      post({ kind: "ready", protocol: protocol(), catalog: JSON.parse(wasmCatalog()) as ProblemDescriptor[] });
      return;
    }
    const { runId, request } = message.data;
    let sequence = 0;
    const artifact = JSON.parse(wasmRun(JSON.stringify(request), runId, (eventJson: string) => {
      const event = JSON.parse(eventJson) as StudioEvent;
      sequence = Math.max(sequence, event.sequence + 1);
      post({ kind: "event", event });
    })) as RunArtifact;
    // Artifact storage is announced before completion so the client can load
    // it synchronously when handling the ordered completion event.
    post({ kind: "artifact", artifact });
    post({ kind: "event", event: { kind: "artifact_published", run_id: runId, sequence: sequence++, artifact_id: artifact.id, documents: artifact.documents.length } });
    post({ kind: "event", event: { kind: "run_completed", run_id: runId, sequence, artifact_id: artifact.id, document_id: artifact.selected_document_id } });
  } catch (cause) {
    const runId = message.data.kind === "run" ? message.data.runId : "browser-initialization";
    post({ kind: "event", event: { kind: "run_failed", run_id: runId, sequence: Number.MAX_SAFE_INTEGER, message: cause instanceof Error ? cause.message : String(cause) } });
  }
};

export {};
