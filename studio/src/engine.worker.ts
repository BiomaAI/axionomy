/// <reference lib="webworker" />
import init, { Engine, catalog as wasmCatalog, protocol, study as wasmStudy } from "./generated/wasm/axionomy_studio_wasm";
import type { ProblemDescriptor, Study, StudyRequest, StudioEvent } from "./api";
import type { StoreMessage, StoreRequest } from "./browserPackages";
import { Followers } from "./following";
import type { LoadedFrom, LoadedView, StoreKept } from "./packageEngine";

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
      post({ kind: "store_reply", id, ok: false, ...refused(cause) });
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

let store: Promise<{ engine: Engine; opened: StoreKept; followers: Followers }> | undefined;

function opened() {
  store ??= openStore().then((kept) => ({ ...kept, followers: new Followers(kept.engine) })).catch((cause: unknown) => {
    store = undefined;
    throw cause;
  });
  return store;
}

/** The engine's JSON answer, read. */
function read(answer: string): unknown {
  return JSON.parse(answer);
}

/**
 * The operation `request` names, as the native server serves it, with the
 * same JSON. The engine throws a refusal as the JSON of `{status, body}`.
 * Followers waiting for a ref look again once it has served anything that
 * moved one.
 */
async function serve(id: number, request: StoreRequest): Promise<unknown> {
  const { engine, opened: where, followers } = await opened();
  try {
    return await served(engine, where, followers, id, request);
  } finally {
    followers.moved();
  }
}

async function served(engine: Engine, where: StoreKept, followers: Followers, id: number, request: StoreRequest): Promise<unknown> {
  const progress = (watched: string) => post({ kind: "store_progress", id, progress: JSON.parse(watched) });
  switch (request.kind) {
    case "open":
      return where;
    case "load_url":
      return loadUrl(engine, request.url);
    case "load_bytes": {
      const loaded = read(engine.load(new Uint8Array(request.bytes), undefined, undefined, undefined)) as LoadedView;
      return { got: "loaded", loaded } satisfies LoadedFrom;
    }
    case "packages":
      return read(engine.packages());
    case "experiments":
      return read(engine.experiments());
    case "draw":
      return read(engine.draw(request.point));
    case "ancestor":
      return read(engine.ancestor(request.point, JSON.stringify({ step: request.step })));
    case "balances":
      return read(engine.balances(request.point));
    case "candidates":
      return read(engine.candidates(request.point, JSON.stringify({ set: request.set })));
    case "assess":
      return read(engine.assess(request.point, JSON.stringify({ exchange: request.exchange })));
    case "apply":
      return read(engine.apply(request.point, JSON.stringify({ exchanges: request.exchanges })));
    case "read_ref":
      return read(engine.readRef(request.name));
    case "create_ref":
      return read(engine.createRef(JSON.stringify({ name: request.name, point: request.point })));
    case "advance_ref":
      return read(engine.advanceRef(request.name, JSON.stringify({ expected: request.expected, to: request.to })));
    case "frames":
      return followers.frames(request.name, request.query);
    case "run_strategy":
      return read(engine.runStrategy(request.point, JSON.stringify(request.request), progress));
    case "run_experiment":
      return read(engine.runExperiment(request.experiment, progress));
    case "results":
      return read(engine.results(request.experiment));
    case "compare":
      return read(engine.compare(JSON.stringify(request.request)));
  }
}

/** A refusal's status and body: as the engine threw it, or a failure. */
function refused(cause: unknown): { status: number; body: unknown } {
  if (typeof cause === "string") {
    try {
      const thrown = JSON.parse(cause) as { status?: unknown; body?: unknown };
      if (typeof thrown.status === "number") return { status: thrown.status, body: thrown.body };
    } catch {
      // Not a refusal the engine wrote.
    }
  }
  return { status: 500, body: { error: described(cause) } };
}

/**
 * The Worker's store: on the origin private file system, where it stays
 * across reloads, or in memory, saying why, when that is unavailable, another
 * tab holds it, or it holds a store this version cannot open (which is left
 * as it is).
 */
async function openStore(): Promise<{ engine: Engine; opened: StoreKept }> {
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

/** What the store loaded from a URL: what the response said, and the packages it held. */
type Source = { validators: { etag?: string | null; last_modified?: string | null }; packages: string[] };

/**
 * Loads the package at `url`. When the store holds what the URL served
 * before, the request carries that response's validators, so a URL serving
 * the same answers 304 and nothing is downloaded; a URL serving something
 * else answers with it, and it is loaded. A cross-origin host that does not
 * allow the conditional headers is asked again plainly.
 */
async function loadUrl(engine: Engine, url: string): Promise<LoadedFrom> {
  const known = engine.source(url);
  const source = known === undefined ? undefined : (JSON.parse(known) as Source);
  const validators = source?.validators;
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
      if (source) return { url, got: "kept", message: described(cause), packages: source.packages };
      throw new Error(`${url} could not be fetched: ${described(conditional)}`);
    }
  }
  if (response.status === 304 && source) return { url, got: "current", status: 304, packages: source.packages };
  if (!response.ok) throw new Error(`${url} answered ${response.status} ${response.statusText}`.trim());
  const bytes = new Uint8Array(await response.arrayBuffer());
  const loaded = read(engine.load(bytes, url, response.headers.get("ETag") ?? undefined, response.headers.get("Last-Modified") ?? undefined)) as LoadedView;
  return { url, got: "loaded", status: response.status, loaded };
}

function described(cause: unknown) {
  return cause instanceof Error ? cause.message : String(cause);
}

export {};
