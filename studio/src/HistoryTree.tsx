import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

/**
 * A history tree, drawn as a timeline: steps run left to right, each branch
 * on a row of its own below the branch it grew from, joined to it at the
 * step it branched at. It knows nothing of what the steps are: branches,
 * their steps and labels in, the step a person picks out. Long stretches
 * without a branch, a label or the cursor are drawn short, their steps
 * counted, so a history of thousands of steps stays one screen wide.
 */
export type TreeBranch = {
  id: string;
  /** The branch it grew from and the step it branched at; none for a history's first branch. */
  parent?: { id: string; step: number };
  /** The step it starts from: its parent's step it branched at, or its own first. */
  first: number;
  /** Its last step. */
  last: number;
  /** What it is, in words: its name in a hover and to a screen reader. */
  title: string;
  tone?: "named" | "trial" | "branch";
  /** Names written beside steps of it, such as a ref at the step it names. */
  labels?: TreeLabel[];
};

export type TreeLabel = { step: number; text: string; tone?: "named" | "fixed" | "trial"; onClick?: () => void };

/** Where a person is: a branch's history up to `end`, shown at step `cursor` of it. */
export type TreeSelection = { id: string; end: number; cursor: number };

/** How much of a stretch is drawn step by step; a longer one is drawn this long, its steps counted. */
const DRAWN_GAP = 8;
const UNIT = 24;
const ROW = 30;
const PAD_X = 16;
const PAD_Y = 18;
const LABEL_SPACE = 220;

export type Laid = {
  rows: Map<string, number>;
  /** The x of a step, compressed stretches included. */
  x: (step: number) => number;
  /** The step nearest to `x`. */
  stepAt: (x: number) => number;
  /** The stretches drawn short: from, to and how many steps each holds. */
  gaps: { from: number; to: number }[];
  width: number;
  height: number;
  ordered: TreeBranch[];
};

/**
 * Rows and columns: each branch's row comes after the rows of everything
 * grown from the branches before it (its parent's earlier children first,
 * by the step they branched at), and steps keep their order left to right,
 * a stretch longer than {@link DRAWN_GAP} with nothing in it drawn short.
 */
export function layoutTree(branches: TreeBranch[], selection?: TreeSelection): Laid {
  const byId = new Map(branches.map((branch) => [branch.id, branch]));
  const children = new Map<string, TreeBranch[]>();
  const roots: TreeBranch[] = [];
  for (const branch of branches) {
    if (branch.parent && byId.has(branch.parent.id)) children.set(branch.parent.id, [...(children.get(branch.parent.id) ?? []), branch]);
    else roots.push(branch);
  }
  const ordered: TreeBranch[] = [];
  const visit = (branch: TreeBranch) => {
    ordered.push(branch);
    for (const child of [...(children.get(branch.id) ?? [])].sort((left, right) => (left.parent?.step ?? 0) - (right.parent?.step ?? 0))) visit(child);
  };
  roots.forEach(visit);
  const rows = new Map(ordered.map((branch, index) => [branch.id, index]));

  const keys = new Set<number>([0]);
  for (const branch of branches) {
    keys.add(branch.first);
    keys.add(branch.last);
    for (const label of branch.labels ?? []) keys.add(label.step);
  }
  if (selection) { keys.add(selection.end); keys.add(selection.cursor); }
  const sorted = [...keys].sort((left, right) => left - right);
  const starts: number[] = [];
  const gaps: { from: number; to: number }[] = [];
  let at = 0;
  for (let index = 0; index < sorted.length; index++) {
    starts.push(at);
    const next = sorted[index + 1];
    if (next === undefined) break;
    const gap = next - sorted[index];
    if (gap > DRAWN_GAP) gaps.push({ from: sorted[index], to: next });
    at += Math.min(gap, DRAWN_GAP) * UNIT;
  }
  const x = (step: number) => {
    let index = sorted.findIndex((key) => key >= step);
    if (index < 0) index = sorted.length - 1;
    if (sorted[index] === step || index === 0) return PAD_X + starts[index];
    const from = sorted[index - 1];
    const to = sorted[index];
    const span = starts[index] - starts[index - 1];
    return PAD_X + starts[index - 1] + ((step - from) / (to - from)) * span;
  };
  const stepAt = (position: number) => {
    const offset = position - PAD_X;
    for (let index = 0; index < sorted.length - 1; index++) {
      if (offset <= starts[index + 1]) {
        const span = starts[index + 1] - starts[index];
        const fraction = span === 0 ? 0 : (offset - starts[index]) / span;
        return Math.round(sorted[index] + Math.max(0, fraction) * (sorted[index + 1] - sorted[index]));
      }
    }
    return sorted.at(-1) ?? 0;
  };
  return {
    rows,
    x,
    stepAt,
    gaps,
    width: PAD_X * 2 + at + LABEL_SPACE,
    height: PAD_Y * 2 + Math.max(0, ordered.length - 1) * ROW,
    ordered,
  };
}

/** How wide a label is drawn: its text in the tree's 11px monospace, padded. */
function labelWidth(text: string): number {
  return text.length * 6.6 + 12;
}

/** The branches a selection's history runs along, each with the steps of it the history takes. */
export function pathOf(branches: TreeBranch[], selection: TreeSelection | undefined): Map<string, { from: number; to: number }> {
  const path = new Map<string, { from: number; to: number }>();
  if (!selection) return path;
  const byId = new Map(branches.map((branch) => [branch.id, branch]));
  let branch = byId.get(selection.id);
  let to = selection.end;
  while (branch) {
    path.set(branch.id, { from: branch.first, to });
    if (!branch.parent) break;
    to = branch.parent.step;
    branch = byId.get(branch.parent.id);
  }
  return path;
}

/** The branch a step of a selection's history is drawn on. */
function cursorBranch(path: Map<string, { from: number; to: number }>, step: number, roots: Set<string>): string | undefined {
  for (const [id, range] of path) {
    if (step > range.from && step <= range.to) return id;
    if (roots.has(id) && step >= range.from && step <= range.to) return id;
  }
  return undefined;
}

export function HistoryTree({ branches, selection, onSelect, describe, label }: {
  branches: TreeBranch[];
  selection?: TreeSelection;
  /** A step of a branch picked out. */
  onSelect: (id: string, step: number) => void;
  /** What a hover over a step of a branch says. */
  describe?: (branch: TreeBranch, step: number) => ReactNode;
  label: string;
}) {
  const laid = useMemo(() => layoutTree(branches, selection), [branches, selection]);
  const path = useMemo(() => pathOf(branches, selection), [branches, selection]);
  const roots = useMemo(() => new Set(branches.filter((branch) => !branch.parent).map((branch) => branch.id)), [branches]);
  const host = useRef<HTMLDivElement>(null);
  // The card of the step hovered stays where that step is, so the pointer
  // can move onto it and read its words; it goes once the pointer leaves both.
  const [hover, setHover] = useState<{ branch: TreeBranch; step: number }>();
  const leaving = useRef<number | undefined>(undefined);
  const stay = () => window.clearTimeout(leaving.current);
  const leave = () => { stay(); leaving.current = window.setTimeout(() => setHover(undefined), 220); };
  useEffect(() => stay, []);
  // A card is where its step was drawn: once the page moves, it goes.
  useEffect(() => {
    if (!hover) return;
    const gone = () => setHover(undefined);
    window.addEventListener("scroll", gone, { passive: true });
    return () => window.removeEventListener("scroll", gone);
  }, [hover]);
  const y = (id: string) => PAD_Y + (laid.rows.get(id) ?? 0) * ROW;
  const compressed = (step: number) => laid.gaps.some((gap) => step > gap.from && step < gap.to);
  const point = (branch: TreeBranch, step: number) => { stay(); setHover({ branch, step }); };
  const onTrack = (branch: TreeBranch, event: React.MouseEvent<SVGElement>) => {
    const svg = event.currentTarget.ownerSVGElement ?? (event.currentTarget as unknown as SVGSVGElement);
    const box = svg.getBoundingClientRect();
    const own = branch.parent ? branch.first + 1 : branch.first;
    onSelect(branch.id, Math.min(branch.last, Math.max(own, laid.stepAt(event.clientX - box.left))));
  };
  const cursorOn = selection ? cursorBranch(path, selection.cursor, roots) : undefined;
  const frame = host.current?.getBoundingClientRect();
  return <div className="history-tree" ref={host} role="group" aria-label={label} onScroll={() => setHover(undefined)}>
    <svg width={laid.width} height={laid.height} role="presentation">
      {laid.ordered.map((branch) => {
        const row = y(branch.id);
        const own = branch.parent ? branch.first + 1 : branch.first;
        const on = path.get(branch.id);
        const tone = `tree-branch tone-${branch.tone ?? "branch"}${on ? " on-path" : ""}`;
        const steps: number[] = [];
        for (let step = own; step <= branch.last; step++) if (!compressed(step) || step === branch.last) steps.push(step);
        return <g key={branch.id} className={tone}>
          {branch.parent && laid.rows.has(branch.parent.id) && <path className={`tree-link${on ? " on-path" : ""}`} d={`M ${laid.x(branch.parent.step)} ${y(branch.parent.id)} V ${row - 8} Q ${laid.x(branch.parent.step)} ${row} ${laid.x(branch.parent.step) + 8} ${row} H ${laid.x(own)}`} />}
          <line className="tree-line" x1={laid.x(branch.first)} x2={laid.x(branch.last)} y1={row} y2={row} />
          {on && <line className="tree-line path-line" x1={laid.x(Math.max(branch.first, on.from))} x2={laid.x(Math.min(branch.last, on.to))} y1={row} y2={row} />}
          {laid.gaps.filter((gap) => gap.from >= branch.first && gap.to <= branch.last).map((gap) => {
            // A stretch drawn short says on it how many steps it holds.
            const middle = (laid.x(gap.from) + laid.x(gap.to)) / 2;
            const words = `${gap.to - gap.from - 1} steps`;
            const width = words.length * 5.6 + 8;
            return <g key={gap.from}>
              <line className="tree-line gap-line" x1={laid.x(gap.from) + 6} x2={laid.x(gap.to) - 6} y1={row} y2={row} />
              <rect className="tree-gap-back" x={middle - width / 2} y={row - 7} width={width} height={14} />
              <text className="tree-gap" x={middle} y={row + 3} textAnchor="middle">{words}</text>
            </g>;
          })}
          <rect className="tree-track" x={laid.x(branch.first) - UNIT / 2} y={row - ROW / 2} width={laid.x(branch.last) - laid.x(branch.first) + UNIT} height={ROW}
            onClick={(event) => onTrack(branch, event)} />
          {steps.map((step) => {
            const tip = step === branch.last;
            const onPath = on && step >= on.from && step <= on.to;
            return <circle key={step} className={`tree-step${tip ? " tip" : ""}${onPath ? " on-path" : ""}`} cx={laid.x(step)} cy={row} r={tip ? 5.5 : 3.5}
              tabIndex={0} role="button" aria-label={`Step ${step} of ${branch.title}`}
              onClick={() => onSelect(branch.id, step)} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); onSelect(branch.id, step); } }}
              onMouseEnter={() => point(branch, step)} onMouseLeave={leave} onFocus={() => point(branch, step)} onBlur={leave} />;
          })}
          {(branch.labels ?? []).map((label, index) => {
            // Labels at one step sit side by side.
            const before = (branch.labels ?? []).slice(0, index).filter((other) => other.step === label.step);
            const lx = laid.x(label.step) + 10 + before.reduce((sum, other) => sum + labelWidth(other.text) + 4, 0);
            return <g key={`${label.text}:${index}`} className={`tree-label tone-${label.tone ?? "named"}${label.onClick ? " clickable" : ""}`} transform={`translate(${lx} ${row - 9})`} onClick={label.onClick} role={label.onClick ? "button" : undefined} aria-label={label.onClick ? `Open ${label.text}` : undefined}>
              <rect height={18} width={labelWidth(label.text)} rx={9} />
              <text x={6} y={12.5}>{label.text}</text>
            </g>;
          })}
        </g>;
      })}
      {selection && cursorOn && <circle className="tree-cursor" cx={laid.x(selection.cursor)} cy={y(cursorOn)} r={9} />}
    </svg>
    {hover && describe && frame && createPortal(<div className="tree-hover" role="tooltip" onMouseEnter={stay} onMouseLeave={leave} onClick={() => setHover(undefined)}
      style={{ left: Math.min(window.innerWidth - 340, Math.max(8, frame.left - (host.current?.scrollLeft ?? 0) + laid.x(hover.step) - 12)), top: frame.top - (host.current?.scrollTop ?? 0) + y(hover.branch.id) + 12 }}>{describe(hover.branch, hover.step)}</div>, document.body)}
  </div>;
}
