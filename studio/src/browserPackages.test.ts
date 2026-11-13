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

  it("cancels a run by terminating the Worker running it, and asks the next one everything else waiting", async () => {
    const packages = new BrowserPackages();
    const [first] = FakeWorker.spawned;
    const run = packages.runStrategy(point, { strategy: "game", goal: "over", seed: "17", budget: "1", turns: "10" }, () => {});
    const following = packages.frames("live", { after: point, wait_seconds: 20 });
    const reading = packages.readRef("live");
    first.says({ kind: "store_begun", id: first.asked("run_strategy") });

    run.cancel();
    await expect(run.result).rejects.toThrow(/cancelled/);
    expect(first.terminated).toBe(true);
    expect(FakeWorker.spawned).toHaveLength(2);
    const next = FakeWorker.spawned[1];
    // Everything else, asked again with the same ids, in the same order.
    expect(next.posted.map((posted) => posted.request.kind)).toEqual(["frames", "read_ref"]);
    expect(next.posted.map((posted) => posted.id)).toEqual(first.posted.slice(1).map((posted) => posted.id));

    // The terminated Worker is not heard; the next one answers.
    first.says({ kind: "store_reply", id: next.asked("read_ref"), ok: true, value: { name: "live", point: "stale" } });
    next.says({ kind: "store_reply", id: next.asked("read_ref"), ok: true, value: { name: "live", point } });
    next.says({ kind: "store_reply", id: next.asked("frames"), ok: true, value: { name: "live", point, frames: [] } });
    expect(await reading).toEqual({ name: "live", point });
    expect(await following).toEqual({ name: "live", point, frames: [] });
  });

  it("leaves the Worker to finish what it is doing before a run cancelled, and stops the run as it begins", async () => {
    const packages = new BrowserPackages();
    const [first] = FakeWorker.spawned;
    const drawing = packages.draw(point);
    const run = packages.runExperiment("maze/micro", () => {});
    run.cancel();
    await expect(run.result).rejects.toThrow(/cancelled/);
    expect(first.terminated).toBe(false);
    first.says({ kind: "store_reply", id: first.asked("draw"), ok: true, value: { frames: [] } });
    expect(await drawing).toEqual({ frames: [] });
    expect(first.terminated).toBe(false);
    first.says({ kind: "store_begun", id: first.asked("run_experiment") });
    expect(first.terminated).toBe(true);
    // Nothing was waiting: the next Worker is asked nothing.
    expect(FakeWorker.spawned).toHaveLength(2);
    expect(FakeWorker.spawned[1].posted).toEqual([]);
  });
});
