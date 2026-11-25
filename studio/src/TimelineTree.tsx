import { useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

/**
 * A tree of timelines: steps run left to right, each branch
 * on a row of its own below the branch it grew from, joined to it at the
 * step it branched at. It knows nothing of what the steps are: branches,
 * their steps and labels in, the step a person picks out. Long stretches
 * without a branch, a label or the cursor are drawn short, their steps
 * counted, so a timeline of thousands of steps stays one screen wide.
 */
export type TreeBranch = {
  id: string;
  /** The branch it grew from and the step it branched at; none for a timeline's first branch. */
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
  /** What it is drawn together with: first timelines of one group sit close, a gap before the next group. */
  group?: string;
};

/** A name beside a step: as written, and in full where it is written short. */
export type TreeLabel = { step: number; text: string; name?: string; tone?: "named" | "fixed" | "trial"; onClick?: () => void };

/** Where a person is: a branch's timeline up to `end`, shown at step `cursor` of it. */
export type TreeSelection = { id: string; end: number; cursor: number };

/** How much of a stretch is drawn step by step; a longer one is drawn this long, its steps counted. */
const DRAWN_GAP = 8;
const UNIT = 24;
const ROW = 30;
const PAD_X = 16;
const PAD_Y = 18;
/** The space between groups of first timelines. */
const GROUP_GAP = 12;
/** The radius of the corner a branch turns from its parent's trunk onto its own row. */
const CORNER = 8;
/** How far dashes keep from the steps and the count they run between. */
const DASH_CLEAR = 8;
/** How long a dash is, before its round ends; dashes are about this far apart. */
const DASH = 3;
const DASH_EVERY = 8;

/** A stretch of a row from `x1` to `x2`: solid, or dashed by `dash`. */
type Piece = { x1: number; x2: number; dash?: string };

/** Dashes from `x1` to `x2`, as many as fit about {@link DASH_EVERY} apart, the first at `x1` and the last at `x2`. */
function dashes(x1: number, x2: number): Piece {
  const length = Math.max(0, x2 - x1);
  const count = Math.max(1, Math.round((length + DASH_EVERY - DASH) / DASH_EVERY));
  const gap = count > 1 ? (length - count * DASH) / (count - 1) : length;
  return { x1, x2, dash: `${DASH} ${gap.toFixed(3)}` };
}

/** How wide a label is drawn: its text in the tree's 11px monospace, padded. */
function labelWidth(text: string): number {
  return text.length * 6.6 + 12;
}

/** How far right of its step each label starts: labels at one step sit side by side. */
function labelOffsets(labels: TreeLabel[]): number[] {
  const taken = new Map<number, number>();
  return labels.map((label) => {
    const offset = taken.get(label.step) ?? 10;
    taken.set(label.step, offset + labelWidth(label.text) + 4);
    return offset;
  });
}

export type Laid = {
  rows: Map<string, number>;
  /** The y of a branch's row. */
  y: (id: string) => number;
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
  // Each row a row below the last, and a gap more where a first timeline starts another group.
  const ys = new Map<string, number>();
  let rowY = PAD_Y;
  let group: string | undefined;
  ordered.forEach((branch, index) => {
    const root = !branch.parent || !byId.has(branch.parent.id);
    if (index > 0) rowY += ROW + (root && branch.group !== group ? GROUP_GAP : 0);
    if (root) group = branch.group;
    ys.set(branch.id, rowY);
  });

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
  // The tree is as wide as its last step, or the end of its widest label.
  let right = PAD_X + at;
  for (const branch of branches) {
    const labels = branch.labels ?? [];
    labelOffsets(labels).forEach((offset, index) => { right = Math.max(right, x(labels[index].step) + offset + labelWidth(labels[index].text)); });
  }
  return {
    rows,
    y: (id) => ys.get(id) ?? PAD_Y,
    x,
    stepAt,
    gaps,
    width: right + PAD_X,
    height: rowY + PAD_Y,
    ordered,
  };
}

/** The branches a selection's timeline runs along, each with the steps of it the timeline takes. */
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

/** The branch a step of a selection's timeline is drawn on. */
function cursorBranch(path: Map<string, { from: number; to: number }>, step: number, roots: Set<string>): string | undefined {
  for (const [id, range] of path) {
    if (step > range.from && step <= range.to) return id;
    if (roots.has(id) && step >= range.from && step <= range.to) return id;
  }
  return undefined;
}

export function TimelineTree({ branches, selection, onSelect, describe, label, height }: {
  branches: TreeBranch[];
  selection?: TreeSelection;
  /** A step of a branch picked out. */
  onSelect: (id: string, step: number) => void;
  /** What a hover over a step of a branch says. */
  describe?: (branch: TreeBranch, step: number) => ReactNode;
  label: string;
  /** How tall it is drawn; the rest of it scrolls. */
  height?: number;
}) {
  const laid = useMemo(() => layoutTree(branches, selection), [branches, selection]);
  const path = useMemo(() => pathOf(branches, selection), [branches, selection]);
  const roots = useMemo(() => new Set(branches.filter((branch) => !branch.parent).map((branch) => branch.id)), [branches]);
  const host = useRef<HTMLDivElement>(null);
  // The card of the step hovered stays where that step is, so the pointer
  // can move onto it and read its words; it goes once the pointer leaves both.
  const [hover, setHover] = useState<{ branch: TreeBranch; step: number }>();
  // The tree scrolling inside itself moves the card with its step.
  const [, redraw] = useReducer((count: number) => count + 1, 0);
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
  const y = laid.y;
  const compressed = (step: number) => laid.gaps.some((gap) => step > gap.from && step < gap.to);
  const point = (branch: TreeBranch, step: number) => { stay(); setHover({ branch, step }); };
  const onTrack = (branch: TreeBranch, event: React.MouseEvent<SVGElement>) => {
    const svg = event.currentTarget.ownerSVGElement ?? (event.currentTarget as unknown as SVGSVGElement);
    const box = svg.getBoundingClientRect();
    const own = branch.parent ? branch.first + 1 : branch.first;
    onSelect(branch.id, Math.min(branch.last, Math.max(own, laid.stepAt(event.clientX - box.left))));
  };
  const cursorOn = selection ? cursorBranch(path, selection.cursor, roots) : undefined;
  // A timeline opened is scrolled into view where the tree scrolls; as it
  // plays, the tree stays where it was put.
  const cursorAt = selection && cursorOn ? { x: laid.x(selection.cursor), y: y(cursorOn) } : undefined;
  const opened = useRef(cursorAt);
  opened.current = cursorAt;
  useEffect(() => {
    const box = host.current;
    const at = opened.current;
    if (!box || !at) return;
    const margin = ROW;
    if (at.y < box.scrollTop + margin || at.y > box.scrollTop + box.clientHeight - margin) box.scrollTop = at.y - box.clientHeight / 2;
    if (at.x < box.scrollLeft + margin || at.x > box.scrollLeft + box.clientWidth - margin) box.scrollLeft = at.x - box.clientWidth / 2;
  }, [selection?.id, selection?.end]);
  const frame = host.current?.getBoundingClientRect();
  // Where a branch leaves its parent's row: down the parent's trunk at the
  // step it branched at, then round a corner onto its own row.
  const fork = (branch: TreeBranch) => branch.parent && laid.rows.has(branch.parent.id) ? { x: laid.x(branch.parent.step), y: y(branch.parent.id) } : undefined;
  const start = (branch: TreeBranch) => { const from = fork(branch); return from ? from.x + CORNER : laid.x(branch.first); };
  /**
   * The stretches of `branch`'s row from step `from` to step `to`: solid,
   * and over each stretch drawn short, dashes from the steps either side up
   * to its count of steps in the middle.
   */
  const pieces = (branch: TreeBranch, from: number, to: number) => {
    const out: Piece[] = [];
    let at = from === branch.first ? start(branch) : laid.x(from);
    for (const gap of laid.gaps) {
      if (gap.from < from || gap.to > to) continue;
      if (laid.x(gap.from) > at) out.push({ x1: at, x2: laid.x(gap.from) });
      const count = counting(gap);
      out.push(dashes(laid.x(gap.from) + DASH_CLEAR, count.left - DASH_CLEAR), dashes(count.right + DASH_CLEAR, laid.x(gap.to) - DASH_CLEAR));
      at = laid.x(gap.to);
    }
    if (laid.x(to) > at) out.push({ x1: at, x2: laid.x(to) });
    return out;
  };
  /** Where a stretch drawn short says how many steps it holds. */
  const counting = (gap: { from: number; to: number }) => {
    const middle = (laid.x(gap.from) + laid.x(gap.to)) / 2;
    const words = `${gap.to - gap.from - 1} steps`;
    const width = words.length * 5.6;
    return { words, middle, left: middle - width / 2, right: middle + width / 2 };
  };
  // The last branch to leave a row at a step draws the trunk down to it;
  // those before it leave the trunk at their own rows.
  const lastOff = new Map<string, string>();
  for (const branch of laid.ordered) if (branch.parent && fork(branch)) lastOff.set(`${branch.parent.id}:${branch.parent.step}`, branch.id);
  /**
   * A branch's row, from `from` to `to`: from where it leaves its parent
   * (from `top`, or from its corner), round its corner and on along its
   * first solid stretch in one stroke, so nothing joins where it turns; then
   * its other stretches, dashed where drawn short.
   */
  const row = (branch: TreeBranch, from: number, to: number, on: boolean, top?: number) => {
    const at = fork(branch);
    const rowY = y(branch.id);
    const stretches = pieces(branch, from, to);
    const turned = at && from === branch.first;
    const along = turned && stretches[0] && !stretches[0].dash ? stretches.shift() : undefined;
    const className = `tree-edge${on ? " on-path" : ""}`;
    return <g key={branch.id}>
      {turned && <path className={className} d={`M ${at.x} ${top ?? rowY - CORNER} V ${rowY - CORNER} A ${CORNER} ${CORNER} 0 0 0 ${at.x + CORNER} ${rowY}${along ? ` H ${along.x2}` : ""}`} />}
      {stretches.map((piece, index) => <line key={index} className={className} strokeDasharray={piece.dash} x1={piece.x1} x2={piece.x2} y1={rowY} y2={rowY} />)}
    </g>;
  };
  return <div className="timeline-tree" ref={host} role="group" aria-label={label} style={height === undefined ? undefined : { height }} onScroll={() => { if (hover) redraw(); }}>
    <svg width={laid.width} height={laid.height} role="presentation">
      <g className="tree-edges">
        {laid.ordered.map((branch) => row(branch, branch.first, branch.last, false, branch.parent && lastOff.get(`${branch.parent.id}:${branch.parent.step}`) === branch.id ? fork(branch)?.y : undefined))}
      </g>
      <g className="tree-edges">
        {laid.ordered.flatMap((branch) => {
          const on = path.get(branch.id);
          return on ? [row(branch, Math.max(branch.first, on.from), Math.min(branch.last, on.to), true, fork(branch)?.y)] : [];
        })}
      </g>
      <g className="tree-counts">
        {laid.ordered.flatMap((branch) => laid.gaps.filter((gap) => gap.from >= branch.first && gap.to <= branch.last).map((gap) => {
          const count = counting(gap);
          return <text key={`${branch.id}:${gap.from}`} className="tree-gap" x={count.middle} y={y(branch.id) + 3} textAnchor="middle">{count.words}</text>;
        }))}
      </g>
      {laid.ordered.map((branch) => {
        const row = y(branch.id);
        const own = branch.parent ? branch.first + 1 : branch.first;
        const on = path.get(branch.id);
        const tone = `tree-branch tone-${branch.tone ?? "branch"}${on ? " on-path" : ""}`;
        const steps: number[] = [];
        for (let step = own; step <= branch.last; step++) if (!compressed(step) || step === branch.last) steps.push(step);
        return <g key={branch.id} className={tone}>
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
          {(branch.labels ?? []).map((label, index, labels) => {
            const lx = laid.x(label.step) + labelOffsets(labels)[index];
            return <g key={`${label.text}:${index}`} className={`tree-label tone-${label.tone ?? "named"}${label.onClick ? " clickable" : ""}`} transform={`translate(${lx} ${row - 9})`} onClick={label.onClick} role={label.onClick ? "button" : undefined} aria-label={label.onClick ? `Open ${label.name ?? label.text}` : undefined}>
              {label.name && label.name !== label.text && <title>{label.name}</title>}
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
