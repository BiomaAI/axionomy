import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserPackages, type StoreMessage, type StoreRequest } from "./browserPackages";

/** A Worker that answers nothing by itself: the test reads what it is asked and says what it answers. */
class FakeWorker {
  static spawned: FakeWorker[] = [];
  readonly posted: { id: number; request: StoreRequest }[] = [];
  terminated = false;
  onmessage: ((message: MessageEvent<StoreMessage>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;

  constructor() {
    FakeWorker.spawned.push(this);
  }

  postMessage(message: { kind: "store"; id: number; request: StoreRequest }) {
    this.posted.push({ id: message.id, request: message.request });
  }

  terminate() {
    this.terminated = true;
  }

  /** Says `data`, as the Worker posts it. */
  says(data: StoreMessage) {
    this.onmessage?.({ data } as MessageEvent<StoreMessage>);
  }

  asked(kind: StoreRequest["kind"]) {
    const asked = this.posted.find((posted) => posted.request.kind === kind);
    if (!asked) throw new Error(`the Worker was not asked to ${kind}`);
    return asked.id;
  }
}

const point = "run-0000000000000000000000000000000a:0";

describe("BrowserPackages", () => {
  beforeEach(() => {
    FakeWorker.spawned = [];
    vi.stubGlobal("Worker", FakeWorker);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("asks the Worker for a follower's frames once, and waits for its answer", async () => {
    const packages = new BrowserPackages();
    const [worker] = FakeWorker.spawned;
    const following = packages.frames("live", { after: point, wait_seconds: 20 });
    expect(worker.posted.map((posted) => posted.request)).toEqual([{ kind: "frames", name: "live", query: { after: point, wait_seconds: 20 } }]);
    worker.says({ kind: "store_reply", id: worker.asked("frames"), ok: true, value: { name: "live", point, frames: [] } });
    expect(await following).toEqual({ name: "live", point, frames: [] });
  });

  it("stops waiting for a follower's answer when it aborts", async () => {
    const packages = new BrowserPackages();
    const [worker] = FakeWorker.spawned;
    const abort = new AbortController();
    const following = packages.frames("live", { after: point, wait_seconds: 20 }, abort.signal);
    abort.abort(new Error("stopped following"));
    await expect(following).rejects.toThrow("stopped following");
    // An answer that comes later is no one's.
    worker.says({ kind: "store_reply", id: worker.asked("frames"), ok: true, value: { name: "live", point, frames: [] } });
    expect(worker.posted).toHaveLength(1);
  });
});
