import { vi } from "vitest";
import { coalesced, shortPoint } from "./PackagesApp";

test("a point reads briefly as its run's last digits and its step", () => {
  expect(shortPoint("run-000000000000feed0000000000000007:12")).toBe("run-…000007:12");
});

test("a run's progress is drawn at most once a while, the latest of it", () => {
  vi.useFakeTimers();
  try {
    const shown: number[] = [];
    const progress = coalesced((value: number) => shown.push(value));
    for (const value of [1, 2, 3]) progress.push(value);
    expect(shown).toEqual([]);
    vi.advanceTimersByTime(200);
    expect(shown).toEqual([3]);
    progress.push(4);
    progress.stop();
    vi.advanceTimersByTime(400);
    // Stopped with the run, it draws nothing after it.
    expect(shown).toEqual([3]);
  } finally {
    vi.useRealTimers();
  }
});
