/// <reference lib="webworker" />
import init, { Engine, catalog as wasmCatalog, protocol, study as wasmStudy } from "./generated/wasm/axionomy_studio_wasm";
import type { ProblemDescriptor, Study, StudyRequest, StudioEvent } from "./api";
import type { Fetched, Loaded, Opened, StoreMessage, StoreRequest } from "./browserPackages";

type Command =
  | { kind: "initialize" }
  | { kind: "study"; studyId: string; request: StudyRequest }
  | { kind: "store"; id: number; request: StoreRequest };

type WorkerMessage =
  | { kind: "ready"; protocol: string; catalog: ProblemDescriptor[] }
  | { kind: "event"; event: StudioEvent }
  | { kind: "study"; studyId: string; study: Study }
  | StoreMessage;

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
  if (message.data.kind === "store") {
    const { id, request } = message.data;
    try {
      post({ kind: "store_reply", id, ok: true, value: await serve(id, request) });
    } catch (cause) {
      post({ kind: "store_reply", id, ok: false, message: described(cause) });
    }
    return;
  }
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
    post({ kind: "event", event: { kind: "study_failed", study_id: studyId, sequence: Number.MAX_SAFE_INTEGER, message: described(cause) } });
  }
};

/** The file in the origin private file system that holds the Worker's store. */
const STORE_FILE = "axionomy.redb";
/** How long a Worker waits for the Worker of a reloaded page to let go of the store. */
const HANDLE_ATTEMPTS = 30;
const HANDLE_RETRY_MS = 100;

let store: Promise<{ engine: Engine; opened: Opened }> | undefined;

function opened() {
  store ??= openStore().catch((cause: unknown) => {
    store = undefined;
    throw cause;
  });
  return store;
}

async function serve(id: number, request: StoreRequest): Promise<unknown> {
  const { engine, opened: where } = await opened();
  switch (request.kind) {
    case "open":
      return where;
    case "load_url":
      return loadUrl(engine, request.url);
    case "packages":
      return JSON.parse(engine.packages());
    case "experiments":
      return JSON.parse(engine.experiments());
    case "run_experiment":
      return JSON.parse(engine.runExperiment(request.experiment, (progress: string) => {
        post({ kind: "store_progress", id, progress: JSON.parse(progress) });
      }));
    case "results": {
      const results = engine.results(request.experiment);
      return results === undefined ? undefined : JSON.parse(results);
    }
    case "draw":
      return JSON.parse(engine.draw(JSON.stringify(request.point)));
  }
}

/**
 * The Worker's store: on the origin private file system, where it stays
 * across reloads, or in memory, saying why, when that is unavailable, another
 * tab holds it, or it holds a store this version cannot open (which is left
 * as it is).
 */
async function openStore(): Promise<{ engine: Engine; opened: Opened }> {
  await initialize();
  const { handle, reason } = await accessHandle();
  if (!handle) return { engine: Engine.open(undefined), opened: { persistent: false, reason } };
  try {
    const engine = Engine.open(handle);
    return { engine, opened: { persistent: engine.persistent } };
  } catch (cause) {
    return { engine: Engine.open(undefined), opened: { persistent: false, reason: `the saved store cannot be opened: ${described(cause)}` } };
  }
}

async function accessHandle(): Promise<{ handle?: FileSystemSyncAccessHandle; reason?: string }> {
  let file: FileSystemFileHandle;
  try {
    const root = await navigator.storage.getDirectory();
    file = await root.getFileHandle(STORE_FILE, { create: true });
  } catch (cause) {
    return { reason: `the origin private file system is unavailable: ${described(cause)}` };
  }
  // The Worker of a page just reloaded may hold the file a moment longer.
  for (let attempt = 1; ; attempt++) {
    try {
      return { handle: await file.createSyncAccessHandle() };
    } catch (cause) {
      const held = cause instanceof DOMException && cause.name === "NoModificationAllowedError";
      if (!held) return { reason: `the origin private file system is unavailable: ${described(cause)}` };
      if (attempt >= HANDLE_ATTEMPTS) return { reason: "another tab holds the saved store" };
      await new Promise((resolve) => setTimeout(resolve, HANDLE_RETRY_MS));
    }
  }
}

type Validators = { etag?: string | null; last_modified?: string | null };

/**
 * Loads the package at `url`. When the store holds what the URL served
 * before, the request carries that response's validators, so a URL serving
 * the same answers 304 and nothing is downloaded; a URL serving something
 * else answers with it, and it is loaded. A cross-origin host that does not
 * allow the conditional headers is asked again plainly.
 */
async function loadUrl(engine: Engine, url: string): Promise<Fetched> {
  const known = engine.source(url);
  const validators = known === undefined ? undefined : (JSON.parse(known) as Validators);
  const headers = new Headers();
  if (validators?.etag) headers.set("If-None-Match", validators.etag);
  else if (validators?.last_modified) headers.set("If-Modified-Since", validators.last_modified);
  let response: Response;
  try {
    // The store is the cache: the browser's own is never asked.
    response = await fetch(url, { headers, cache: "no-store" });
  } catch (conditional) {
    try {
      response = await fetch(url, { cache: "no-store" });
    } catch (cause) {
      if (validators) return { url, got: "kept", message: described(cause) };
      throw new Error(`${url} could not be fetched: ${described(conditional)}`);
    }
  }
  if (response.status === 304 && validators) return { url, got: "current", status: 304 };
  if (!response.ok) throw new Error(`${url} answered ${response.status} ${response.statusText}`.trim());
  const bytes = new Uint8Array(await response.arrayBuffer());
  const loaded = JSON.parse(engine.load(bytes, url, response.headers.get("ETag"), response.headers.get("Last-Modified"))) as Loaded;
  return { url, got: "downloaded", status: response.status, loaded };
}

function described(cause: unknown) {
  return cause instanceof Error ? cause.message : String(cause);
}

export {};
