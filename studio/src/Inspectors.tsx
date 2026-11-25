import { createContext, useContext, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { IconX } from "@tabler/icons-react";
import type { ViewDocument, ViewId } from "./api";
import type { components } from "./generated/api";

type Schemas = components["schemas"];

export function PanelHeading({ kicker, title, aside, action }: { kicker: ReactNode; title: ReactNode; aside?: ReactNode; action?: ReactNode }) {
  return <div className="panel-heading"><div><span>{kicker}</span><h2>{title}</h2></div><div className="panel-heading-side">{aside && <small>{aside}</small>}{action}</div></div>;
}

/** The state at `position` of a document's timeline: its initial state, or the state after that step. */
export function useSnapshot(document: ViewDocument | undefined, position: number) {
  return useMemo(() => {
    if (!document) return undefined;
    return position === 0 ? document.initial : document.frames[position - 1]?.after;
  }, [document, position]);
}

/**
 * Something Studio names: a value of the ontology (an account, an asset, a
 * rate) by its view's id, a package's ref, strategy, experiment or
 * objective by its path, or a step of the timeline on screen.
 */
export type Subject =
  | { kind: "account"; id: ViewId }
  | { kind: "asset"; id: ViewId }
  | { kind: "rate"; id: ViewId }
  | { kind: "ref"; path: Schemas["RefPath"] }
  | { kind: "strategy"; path: Schemas["StrategyPath"] }
  | { kind: "experiment"; path: Schemas["ExperimentPath"] }
  | { kind: "objective"; path: Schemas["ObjectivePath"] }
  | { kind: "step"; step: number };

export type SubjectKind = Subject["kind"];

const KINDS: Record<SubjectKind, string> = {
  account: "Account",
  asset: "Asset",
  rate: "Rate",
  ref: "Ref",
  strategy: "Strategy",
  experiment: "Experiment",
  objective: "Objective",
  step: "Step",
};

/** One subject's key: the same thing, wherever it is named, has the same key. */
export function subjectKey(subject: Subject): string {
  switch (subject.kind) {
    case "account": case "asset": case "rate": return `${subject.kind}:${subject.id.key}`;
    case "step": return `step:${subject.step}`;
    default: return `${subject.kind}:${subject.path}`;
  }
}

/** The path a subject is named by. */
export function subjectPath(subject: Subject): string {
  switch (subject.kind) {
    case "account": case "asset": case "rate": return subject.id.key;
    case "step": return String(subject.step);
    default: return subject.path;
  }
}

/** What a subject is called: its view's label, its path, or its step. */
export function subjectLabel(subject: Subject): string {
  switch (subject.kind) {
    case "account": case "asset": case "rate": return subject.id.label;
    case "step": return `step ${subject.step}`;
    default: return subject.path;
  }
}

/**
 * How the page inspects what it names. `inspect` draws a subject as it is
 * at the cursor, the step on screen; `pin` keeps its inspector open.
 * Without it, an {@link IdLink} is its label and nothing more.
 */
export type Inspection = {
  inspect: (subject: Subject) => ReactNode;
  pin: (subject: Subject) => void;
};

const Inspecting = createContext<Inspection | undefined>(undefined);
/** Inside a card: a link pins, it does not open another card. */
const InCard = createContext(false);

export function InspectionProvider({ value, children }: { value: Inspection; children: ReactNode }) {
  return <Inspecting.Provider value={value}>{children}</Inspecting.Provider>;
}

/** How long a pointer rests on a link before its card opens, and lingers after it leaves. */
const OPEN_AFTER = 250;
const CLOSE_AFTER = 160;

/**
 * An identifier, everywhere Studio names one: its label, with its path in
 * reach. A hover shows its card as it is at the cursor; a click pins its
 * inspector, which follows the cursor.
 */
export function IdLink({ subject, label, className }: { subject: Subject; label?: ReactNode; className?: string }) {
  const inspection = useContext(Inspecting);
  const inCard = useContext(InCard);
  const text = label ?? subjectLabel(subject);
  const path = subjectPath(subject);
  const classes = ["id-link", `id-${subject.kind}`, className].filter(Boolean).join(" ");
  const anchor = useRef<HTMLButtonElement>(null);
  const [at, setAt] = useState<{ left: number; top?: number; bottom?: number }>();
  const timer = useRef<number | undefined>(undefined);
  const cardId = useId();
  useEffect(() => () => window.clearTimeout(timer.current), []);
  useEffect(() => {
    if (!at) return;
    const gone = () => setAt(undefined);
    window.addEventListener("scroll", gone, { capture: true, passive: true });
    return () => window.removeEventListener("scroll", gone, { capture: true });
  }, [at]);
  if (!inspection) return <span className={`${classes} static`} title={path}>{text}</span>;
  const open = () => {
    window.clearTimeout(timer.current);
    if (inCard) return;
    timer.current = window.setTimeout(() => {
      const box = anchor.current?.getBoundingClientRect();
      if (!box) return;
      const left = Math.max(8, Math.min(box.left, window.innerWidth - CARD_WIDTH - 8));
      // Below the link where there is room, else above it.
      setAt(box.bottom + 320 < window.innerHeight ? { left, top: box.bottom + 6 } : { left, bottom: window.innerHeight - box.top + 6 });
    }, OPEN_AFTER);
  };
  const close = () => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setAt(undefined), CLOSE_AFTER);
  };
  return <>
    <button ref={anchor} type="button" className={classes} data-path={path} aria-describedby={at ? cardId : undefined}
      onClick={(event) => { event.stopPropagation(); window.clearTimeout(timer.current); setAt(undefined); inspection.pin(subject); }}
      onMouseEnter={open} onMouseLeave={close} onFocus={open} onBlur={close}>{text}</button>
    {at && createPortal(<div id={cardId} role="tooltip" className="id-card" style={{ left: at.left, top: at.top, bottom: at.bottom, width: CARD_WIDTH }}
      onMouseEnter={() => window.clearTimeout(timer.current)} onMouseLeave={close}>
      <InCard.Provider value={true}>
        <InspectorHeading subject={subject} />
        <div className="inspector-body">{inspection.inspect(subject)}</div>
      </InCard.Provider>
    </div>, document.body)}
  </>;
}

const CARD_WIDTH = 340;

/** What an inspector is headed by: its kind's word, then its label, or a step's number. */
function headingLabel(subject: Subject): string {
  return subject.kind === "step" ? String(subject.step) : subjectLabel(subject);
}

function InspectorHeading({ subject, onClose }: { subject: Subject; onClose?: () => void }) {
  return <div className="inspector-heading">
    <div>
      <span>{KINDS[subject.kind]}</span>
      <strong>{headingLabel(subject)}</strong>
      {subject.kind !== "step" && subjectPath(subject) !== subjectLabel(subject) && <code>{subjectPath(subject)}</code>}
    </div>
    {onClose && <button type="button" className="inspector-close" aria-label={`Close ${subjectLabel(subject)}`} onClick={onClose}><IconX size={14} stroke={2} aria-hidden="true" /></button>}
  </div>;
}

/**
 * The inspectors pinned open, latest last, each drawn as its subject is at
 * the cursor, so each follows the step on screen. × closes one; Esc closes
 * the one it is pressed in, or the latest. The page makes room for the dock
 * (`.docked`), so it covers nothing.
 */
export function InspectorDock({ pinned, inspect, onClose }: { pinned: Subject[]; inspect: (subject: Subject) => ReactNode; onClose: (subject: Subject) => void }) {
  const latest = pinned.at(-1);
  useEffect(() => {
    if (!latest) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const inside = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-inspector]") : null;
      const closing = pinned.find((subject) => subjectKey(subject) === inside?.dataset.inspector) ?? latest;
      event.preventDefault();
      onClose(closing);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pinned, latest, onClose]);
  if (pinned.length === 0) return null;
  return <aside className="inspector-dock" aria-label="Inspectors">
    {pinned.map((subject) => <section key={subjectKey(subject)} className="inspector" data-inspector={subjectKey(subject)} aria-label={`${KINDS[subject.kind]} ${headingLabel(subject)}`}>
      <InspectorHeading subject={subject} onClose={() => onClose(subject)} />
      <div className="inspector-body">{inspect(subject)}</div>
    </section>)}
  </aside>;
}
