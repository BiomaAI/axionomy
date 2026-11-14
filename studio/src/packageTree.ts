import type { TreeBranch, TreeLabel } from "./HistoryTree";
import type { PackageView, PointId, ResultsView, SegmentView } from "./packageEngine";

/** A point's run and step: `run-…:3` is run `run-…`, step 3. */
export function parsePoint(point: PointId): { run: string; step: number } {
  const [run, step] = point.split(":");
  return { run, step: Number(step) };
}

/** A label short enough for the tree: its first 28 characters, an ellipsis after. */
function shortened(text: string): string {
  return text.length > 28 ? `${text.slice(0, 27)}…` : text;
}

/** A path's last part, as a label writes it: `maze/a-star` is `a-star`. */
export function lastPart(path: string): string {
  return path.split("/").at(-1) ?? path;
}

/** The trial of an experiment that made a branch. */
export type TrialOf = {
  experiment: string;
  strategy: string;
  index: number;
  trial: ResultsView["arms"][number]["trials"][number];
  objectives: ResultsView["objectives"];
};

/** What a branch is, as the tree tells it. */
export type BranchInfo = {
  /** The refs naming points of it, and whether a package ships each (fixed). */
  refs: { name: string; step: number; fixed: boolean }[];
  /** The experiment trial that made it, when one did. */
  trial?: TrialOf;
  /** What made it in this tab, when Studio knows: a short label, and in words. */
  note?: Note;
};

/** What made a branch in this tab: a label for the tree, and the words of a hover. */
export type Note = { label: string; words: string };

/**
 * The history tree of a package as {@link TreeBranch}es: one per segment,
 * joined at the step it branched at, labelled with the refs naming points
 * of it and, at its tip, the experiment trial that made it.
 */
export function branchesOf(
  segments: SegmentView[],
  pkg: PackageView | undefined,
  recorded: ResultsView[],
  notes: ReadonlyMap<string, Note>,
  openRef: (name: string) => void,
): { branches: TreeBranch[]; info: Map<string, BranchInfo> } {
  const info = new Map<string, BranchInfo>();
  const infoOf = (run: string) => {
    let known = info.get(run);
    if (!known) { known = { refs: [] }; info.set(run, known); }
    return known;
  };
  for (const ref of pkg?.refs ?? []) {
    const { run, step } = parsePoint(ref.point);
    infoOf(run).refs.push({ name: ref.name, step, fixed: Boolean(ref.shipped_by) });
  }
  for (const results of recorded) {
    for (const arm of results.arms) {
      arm.trials.forEach((trial, index) => {
        // A trial that applied nothing ends where it started: no branch of its own.
        if (trial.end === arm.start) return;
        infoOf(parsePoint(trial.end).run).trial = { experiment: results.experiment, strategy: arm.arm.strategy, index, trial, objectives: results.objectives };
      });
    }
  }
  for (const [run, note] of notes) infoOf(run).note = note;
  const branches = segments.map((segment): TreeBranch => {
    const tip = parsePoint(segment.tip);
    const from = segment.from ? parsePoint(segment.from) : undefined;
    const known = info.get(tip.run);
    const labels: TreeLabel[] = (known?.refs ?? []).map((ref) => ({ step: ref.step, text: ref.name, tone: ref.fixed ? "fixed" : "named", onClick: () => openRef(ref.name) }));
    const trial = known?.trial;
    if (trial) labels.push({ step: tip.step, text: `${lastPart(trial.strategy)}${trial.index > 0 ? ` · trial ${trial.index + 1}` : ""}`, tone: "trial" });
    else if (known?.note) labels.push({ step: tip.step, text: shortened(known.note.label), tone: "trial" });
    const named = known?.refs.find((ref) => ref.step === tip.step);
    return {
      id: tip.run,
      parent: from ? { id: from.run, step: from.step } : undefined,
      first: from?.step ?? 0,
      last: tip.step,
      title: named?.name ?? (trial ? `${lastPart(trial.strategy)}, trial ${trial.index + 1} of ${trial.experiment}` : known?.note?.words ?? (from ? "A branch" : "A history")),
      tone: named ? "named" : trial ? "trial" : "branch",
      labels,
    };
  });
  return { branches, info };
}
