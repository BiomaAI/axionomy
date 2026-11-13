import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Followers, type FollowedEngine } from "./following";
import type { RefFrames } from "./packageEngine";

/** Refs that move one step at a time, each step a point `run-…:<step>`, as the Worker's engine answers for them. */
class Refs implements FollowedEngine {
  moves = 0n;
  asked = 0;
  private readonly at = new Map<string, number>();

  point(step: number) {
    return `run-0000000000000000000000000000000a:${step}`;
  }

  refer(name: string, step: number) {
    this.at.set(name, step);
    this.moves += 1n;
  }

  advance(name: string) {
    this.refer(name, (this.at.get(name) ?? 0) + 1);
  }

  forget(name: string) {
    this.at.delete(name);
    this.moves += 1n;
  }

  frames(name: string, query: string): string {
    this.asked += 1;
    const step = this.at.get(name);
    if (step === undefined) throw JSON.stringify({ status: 404, body: { error: `no ref \`${name}\` names a point` } });
    const after = Number((JSON.parse(query) as { after: string }).after.split(":")[1]);
    const frames = Array.from({ length: step - after }, (_, index) => ({ point: this.point(after + index + 1) }));
    return JSON.stringify({ name, point: this.point(step), frames });
  }
}

describe("Followers", () => {
  let refs: Refs;
  let followers: Followers;

  beforeEach(() => {
    vi.useFakeTimers();
    refs = new Refs();
    refs.refer("live", 0);
    refs.refer("other", 0);
    followers = new Followers(refs);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const settled = async <T,>(promise: Promise<T>) => {
    let done = false;
    void promise.then(() => { done = true; }, () => { done = true; });
    await Promise.resolve();
    return done;
  };

  it("answers at once when the ref has moved, or the follower will not wait", () => {
    refs.advance("live");
    const moved = followers.frames("live", { after: refs.point(0), wait_seconds: 20 }) as RefFrames;
    expect(moved.point).toBe(refs.point(1));
    expect(moved.frames.map((frame) => frame.point)).toEqual([refs.point(1)]);
    const idle = followers.frames("other", { after: refs.point(0) }) as RefFrames;
    expect(idle.frames).toEqual([]);
    expect(followers.size).toBe(0);
  });

  it("waits for a move with no timer but its deadline, and answers once the ref moves", async () => {
    const waiting = followers.frames("live", { after: refs.point(0), wait_seconds: 20 }) as Promise<RefFrames>;
    expect(waiting).toBeInstanceOf(Promise);
    expect(followers.size).toBe(1);
    // One timer, the wait's deadline: nothing asks again in the meantime.
    expect(vi.getTimerCount()).toBe(1);
    const asked = refs.asked;
    vi.advanceTimersByTime(19_000);
    followers.moved();
    expect(refs.asked).toBe(asked);
    expect(await settled(waiting)).toBe(false);

    // Another ref moving looks again, and keeps waiting.
    refs.advance("other");
    followers.moved();
    expect(refs.asked).toBe(asked + 1);
    expect(await settled(waiting)).toBe(false);

    refs.advance("live");
    followers.moved();
    const answer = await waiting;
    expect(answer.point).toBe(refs.point(1));
    expect(answer.frames.map((frame) => frame.point)).toEqual([refs.point(1)]);
    expect(followers.size).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("answers with no frames when the wait is over and the ref has not moved", async () => {
    const waiting = followers.frames("live", { after: refs.point(0), wait_seconds: 2 }) as Promise<RefFrames>;
    vi.advanceTimersByTime(1_999);
    expect(await settled(waiting)).toBe(false);
    vi.advanceTimersByTime(1);
    const answer = await waiting;
    expect(answer).toEqual({ name: "live", point: refs.point(0), frames: [] });
    expect(followers.size).toBe(0);
  });

  it("answers every follower of a ref that moves, and fails those whose ref is gone", async () => {
    const first = followers.frames("live", { after: refs.point(0), wait_seconds: 20 }) as Promise<RefFrames>;
    const second = followers.frames("live", { after: refs.point(0), wait_seconds: 5 }) as Promise<RefFrames>;
    const gone = followers.frames("other", { after: refs.point(0), wait_seconds: 20 }) as Promise<RefFrames>;
    refs.advance("live");
    refs.forget("other");
    followers.moved();
    expect((await first).point).toBe(refs.point(1));
    expect((await second).point).toBe(refs.point(1));
    await expect(gone).rejects.toMatch(/"status":404/);
    expect(followers.size).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
  });
});
