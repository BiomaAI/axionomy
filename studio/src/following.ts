import type { FramesQuery, PointId, RefFrames } from "./packageEngine";

/**
 * What following asks of the Worker's engine: the frames of a ref after a
 * point, at once, as the engine's JSON, and how many times a ref has moved.
 */
export interface FollowedEngine {
  frames(name: string, query: string): string;
  readonly moves: bigint;
}

type Follower = {
  name: string;
  query: string;
  after: PointId;
  timer: ReturnType<typeof setTimeout>;
  answer(frames: RefFrames): void;
  fail(cause: unknown): void;
};

/**
 * Followers of refs waiting in the browser's Worker (P10, P12). The native
 * server waits on the engine with a condition variable, which blocks a
 * thread; the Worker has one thread, but every move of a ref goes through
 * its engine, so it knows when one moves. A follower whose ref is still
 * where it last looked waits, holding no thread and asking nothing, until
 * the Worker hears of a move and looks again, or its wait is over.
 */
export class Followers {
  private readonly waiting = new Set<Follower>();
  private seen: bigint;

  constructor(private readonly engine: FollowedEngine) {
    this.seen = engine.moves;
  }

  /**
   * The steps the ref `name` was moved over after `query.after`: at once
   * when there are some or the follower will not wait, else once the ref
   * moves, or with none when `query.wait_seconds` are over. The engine
   * checks the query, its wait among it, as the server does.
   */
  frames(name: string, query: FramesQuery): RefFrames | Promise<RefFrames> {
    const asked = JSON.stringify(query);
    const now = this.look(name, asked);
    const wait = (query.wait_seconds ?? 0) * 1_000;
    if (now.point !== query.after || wait <= 0) return now;
    return new Promise<RefFrames>((answer, fail) => {
      const follower: Follower = {
        name,
        query: asked,
        after: query.after,
        timer: setTimeout(() => this.settle(follower), wait),
        answer,
        fail,
      };
      this.waiting.add(follower);
    });
  }

  /**
   * Looks again for every waiting follower once a ref has moved since the
   * last look, answering those whose ref is no longer where they last
   * looked. Call it after anything that may move a ref.
   */
  moved() {
    const moves = this.engine.moves;
    if (moves === this.seen) return;
    this.seen = moves;
    for (const follower of this.waiting) {
      let now: RefFrames;
      try {
        now = this.look(follower.name, follower.query);
      } catch (cause) {
        this.settle(follower, { cause });
        continue;
      }
      if (now.point !== follower.after) this.settle(follower, { frames: now });
    }
  }

  /** How many followers are waiting. */
  get size() {
    return this.waiting.size;
  }

  private look(name: string, query: string): RefFrames {
    return JSON.parse(this.engine.frames(name, query)) as RefFrames;
  }

  /** Answers `follower` with what it is owed, or with what is there now. */
  private settle(follower: Follower, owed?: { frames: RefFrames } | { cause: unknown }) {
    clearTimeout(follower.timer);
    this.waiting.delete(follower);
    if (owed && "cause" in owed) return follower.fail(owed.cause);
    try {
      follower.answer(owed?.frames ?? this.look(follower.name, follower.query));
    } catch (cause) {
      follower.fail(cause);
    }
  }
}
