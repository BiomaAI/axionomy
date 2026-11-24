import type { PackageView, PointId, RefView, SegmentView, StrategyRequest } from "./packageEngine";
import { parsePoint } from "./packageTree";

/**
 * A strategy a link asks to play from its ref's point, or from an earlier
 * step of its history. What it leaves out is what the package asks of that
 * strategy there.
 */
export type LinkedStrategy = {
  strategy: string;
  /** The step it plays from; the ref's own when none. */
  from?: number;
} & Partial<Omit<StrategyRequest, "strategy">>;

/** What a link asks Studio to open. */
export type Link = {
  /** A package's URL to load first. */
  package?: string;
  /** A loaded package, by name. */
  name?: string;
  ref?: string;
  play?: LinkedStrategy;
  /** A point of the store it was written from. */
  point?: PointId;
  step?: number;
};

/** A whole number below 2^64, written in decimal. */
export function natural(text: string): boolean {
  return /^(0|[1-9]\d*)$/.test(text) && BigInt(text) < 2n ** 64n;
}

/**
 * The link a query string writes, and the parameters of it Studio does not
 * read: one it does not know, one whose value it cannot read, and one that
 * says something of what the link does not name, such as a seed without a
 * strategy.
 */
export function readLink(search: string): { link: Link; unread: string[] } {
  const query = new URLSearchParams(search);
  const read = new Set<string>();
  const value = (key: string, reads: (text: string) => boolean = (text) => text !== "") => {
    const text = query.get(key);
    if (text === null || !reads(text)) return undefined;
    read.add(key);
    return text;
  };
  const whole = (key: string) => {
    const text = value(key, (written) => natural(written) && Number.isSafeInteger(Number(written)));
    return text === undefined ? undefined : Number(text);
  };
  const link: Link = { package: value("package"), name: value("name"), ref: value("ref") };
  const strategy = link.ref === undefined ? undefined : value("strategy");
  if (strategy !== undefined) {
    link.play = { strategy, from: whole("from"), goal: value("goal"), seed: value("seed", natural), budget: value("budget", natural), turns: value("turns", natural) };
  }
  if (link.ref === undefined) link.point = value("point", (text) => /^run-[0-9a-f]+:\d+$/.test(text));
  if (link.ref !== undefined || link.point !== undefined) link.step = whole("step");
  return { link, unread: [...new Set(query.keys())].filter((key) => !read.has(key)) };
}

/** The query string that writes `link`, its `?` included; empty for one that names nothing. */
export function writeLink({ package: url, name, ref, play, point, step }: Link): string {
  const query = new URLSearchParams();
  const write = (key: string, value: string | number | undefined) => { if (value !== undefined) query.set(key, String(value)); };
  write("package", url);
  write("name", name);
  write("ref", ref);
  if (play) {
    write("strategy", play.strategy);
    write("from", play.from);
    write("goal", play.goal);
    write("seed", play.seed);
    write("budget", play.budget);
    write("turns", play.turns);
  }
  write("point", point);
  write("step", step);
  const written = query.toString();
  return written ? `?${written}` : "";
}

/** The effort Studio gives a strategy no experiment runs. */
export const DEFAULT_EFFORT = { seed: "17", budget: "1000000", turns: "1000" };

/**
 * What `pkg` asks of `strategy` at `ref`: the goal and the effort of the
 * first experiment that compares it from there, or else of the first that
 * compares it anywhere; a strategy no experiment compares is asked the first
 * goal with Studio's own effort. None for a strategy the package lacks, or
 * one it has no goal for.
 */
export function askedAt(pkg: PackageView, strategy: string, ref: string): StrategyRequest | undefined {
  if (!pkg.strategies.includes(strategy)) return undefined;
  const comparing = pkg.experiments.filter((experiment) => experiment.arms.some((arm) => arm.strategy === strategy));
  const there = comparing.find((experiment) => experiment.arms.some((arm) => arm.strategy === strategy && (arm.start ?? experiment.start) === ref)) ?? comparing[0];
  if (there) return { strategy, goal: there.goal, seed: there.seed, budget: there.budget, turns: there.turns };
  return pkg.goals[0] === undefined ? undefined : { strategy, goal: pkg.goals[0], ...DEFAULT_EFFORT };
}

/** Whether two runs of a strategy were asked the same. */
export function sameAsked(left: StrategyRequest, right: StrategyRequest): boolean {
  return left.strategy === right.strategy && left.goal === right.goal && left.seed === right.seed && left.budget === right.budget && left.turns === right.turns;
}

/** What a link to `strategy` played from `ref` asks of it: the package's own asking, but where the link says otherwise. */
export function askedBy(pkg: PackageView, ref: string, play: LinkedStrategy): StrategyRequest | undefined {
  const asked = askedAt(pkg, play.strategy, ref);
  return asked && { strategy: play.strategy, goal: play.goal ?? asked.goal, seed: play.seed ?? asked.seed, budget: play.budget ?? asked.budget, turns: play.turns ?? asked.turns };
}

/**
 * How a link names the branch `segment` so that any store can make it: the
 * strategy that made it, played from a ref the package ships. None for a
 * branch no strategy made, or one made from where no such ref reaches.
 */
export function playedLink(pkg: PackageView, segment: SegmentView): { ref: string; play: LinkedStrategy } | undefined {
  const { from, asked } = segment;
  if (!from || !asked) return undefined;
  const start = parsePoint(from);
  const step = (ref: RefView) => parsePoint(ref.point).step;
  // The ref on the run it left nearest after where it left it.
  const ref = pkg.refs
    .filter((candidate) => candidate.shipped_by && parsePoint(candidate.point).run === start.run && step(candidate) >= start.step)
    .sort((left, right) => step(left) - step(right))[0];
  if (!ref) return undefined;
  const usual = askedAt(pkg, asked.strategy, ref.name);
  if (!usual) return undefined;
  const unless = <T>(value: T, same: T) => value === same ? undefined : value;
  return {
    ref: ref.name,
    play: {
      strategy: asked.strategy,
      from: unless(start.step, step(ref)),
      goal: unless(asked.goal, usual.goal),
      seed: unless(asked.seed, usual.seed),
      budget: unless(asked.budget, usual.budget),
      turns: unless(asked.turns, usual.turns),
    },
  };
}
