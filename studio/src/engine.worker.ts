/// <reference lib="webworker" />
import init, { catalog as wasmCatalog, protocol, study as wasmStudy } from "./generated/wasm/axionomy_studio_wasm";
import type { ProblemDescriptor, Study, StudyRequest, StudioEvent } from "./api";

type Command =
  | { kind: "initialize" }
  | { kind: "study"; studyId: string; request: StudyRequest };

type WorkerMessage =
  | { kind: "ready"; protocol: string; catalog: ProblemDescriptor[] }
  | { kind: "event"; event: StudioEvent }
  | { kind: "study"; studyId: string; study: Study };

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
    const { studyId, request } = message.data;
    let sequence = 0;
    const study = JSON.parse(wasmStudy(JSON.stringify(request), studyId, (eventJson: string) => {
      const event = JSON.parse(eventJson) as StudioEvent;
      sequence = Math.max(sequence, event.sequence + 1);
      post({ kind: "event", event });
    })) as Study;
    // The study is stored before completion is announced so the client can
    // load it synchronously when handling the ordered completion event.
    post({ kind: "study", studyId, study });
    post({ kind: "event", event: { kind: "study_published", study_id: studyId, sequence: sequence++, key: study.key, documents: study.documents.length } });
    post({ kind: "event", event: { kind: "study_completed", study_id: studyId, sequence, document: study.selected_document } });
  } catch (cause) {
    // Studies are numbered from 1, so a failure before any study never matches one.
    const studyId = message.data.kind === "study" ? message.data.studyId : "study-0";
    post({ kind: "event", event: { kind: "study_failed", study_id: studyId, sequence: Number.MAX_SAFE_INTEGER, message: cause instanceof Error ? cause.message : String(cause) } });
  }
};

export {};
