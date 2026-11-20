import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

/** A tab: the key it opens, its words, and whether there is anything behind it yet. */
export type Tab<K extends string> = { key: K; label: ReactNode; disabled?: boolean };

/**
 * A row of tabs. The arrow keys move between those that open, Home and End
 * to the first and the last; the tab open is the one Tab reaches.
 */
export function TabStrip<K extends string>({ id, label, tabs, open, onOpen }: {
  /** What the tabs' and their panels' ids start with. */
  id: string;
  label: string;
  tabs: Tab<K>[];
  open?: K;
  onOpen: (key: K) => void;
}) {
  const strip = useRef<HTMLDivElement>(null);
  const enabled = tabs.filter((tab) => !tab.disabled);
  const focusable = open !== undefined && enabled.some((tab) => tab.key === open) ? open : enabled[0]?.key;
  const onKey = (event: KeyboardEvent<HTMLButtonElement>, key: K) => {
    const at = enabled.findIndex((tab) => tab.key === key);
    const next = event.key === "ArrowRight" ? enabled[(at + 1) % enabled.length]
      : event.key === "ArrowLeft" ? enabled[(at - 1 + enabled.length) % enabled.length]
      : event.key === "Home" ? enabled[0]
      : event.key === "End" ? enabled.at(-1)
      : undefined;
    if (!next) return;
    event.preventDefault();
    onOpen(next.key);
    strip.current?.querySelector<HTMLButtonElement>(`#${CSS.escape(`${id}-tab-${next.key}`)}`)?.focus();
  };
  return <div ref={strip} role="tablist" aria-label={label} className="tab-strip">
    {tabs.map((tab) => <button key={tab.key} type="button" role="tab" id={`${id}-tab-${tab.key}`} aria-controls={`${id}-panel-${tab.key}`}
      aria-selected={tab.key === open} tabIndex={tab.key === focusable ? 0 : -1} disabled={tab.disabled}
      onClick={() => onOpen(tab.key)} onKeyDown={(event) => onKey(event, tab.key)}>{tab.label}</button>)}
  </div>;
}

/**
 * The panels of a {@link TabStrip}: each drawn once it is first opened and
 * kept, hidden, while another is open, so what is typed or picked in it
 * stays.
 */
export function TabPanels<K extends string>({ id, open, panels }: { id: string; open?: K; panels: Record<K, () => ReactNode> }) {
  const [seen, setSeen] = useState<ReadonlySet<K>>(() => new Set(open === undefined ? [] : [open]));
  useEffect(() => {
    if (open !== undefined && !seen.has(open)) setSeen(new Set(seen).add(open));
  }, [open, seen]);
  return <>{(Object.keys(panels) as K[]).filter((key) => key === open || seen.has(key)).map((key) =>
    <div key={key} role="tabpanel" id={`${id}-panel-${key}`} aria-labelledby={`${id}-tab-${key}`} className="tab-panel" hidden={key !== open}>{panels[key]()}</div>)}</>;
}

/** How tall the box a handle resizes is drawn. */
function drawn(handle: HTMLElement): number {
  return handle.parentElement?.getBoundingClientRect().height ?? 0;
}

/**
 * A bar along an edge of a box that resizes it: dragged, or with the arrow
 * keys once focused. `edge` is the box's edge it sits on: dragging the top
 * edge up makes the box taller, the bottom edge down likewise.
 */
export function ResizeHandle({ edge, label, size, min, max, onResize }: {
  edge: "top" | "bottom";
  label: string;
  /** The box's height, or the most it may grow to. */
  size: number;
  min: number;
  max: number;
  onResize: (size: number) => void;
}) {
  const drag = useRef<{ from: number; size: number } | undefined>(undefined);
  const clamp = (value: number) => Math.round(Math.min(max, Math.max(min, value)));
  const grows = edge === "top" ? -1 : 1;
  return <div className={`resize-handle resize-${edge}`} role="separator" aria-orientation="horizontal" aria-label={label}
    aria-valuenow={size} aria-valuemin={min} aria-valuemax={max} tabIndex={0}
    onPointerDown={(event) => {
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      // From the height it is drawn at, which may be less than its limit.
      drag.current = { from: event.clientY, size: drawn(event.currentTarget) };
    }}
    onPointerMove={(event) => {
      if (drag.current) onResize(clamp(drag.current.size + grows * (event.clientY - drag.current.from)));
    }}
    onPointerUp={() => { drag.current = undefined; }}
    onPointerCancel={() => { drag.current = undefined; }}
    onKeyDown={(event) => {
      const step = event.shiftKey ? 64 : 16;
      const now = drawn(event.currentTarget);
      if (event.key === "ArrowUp") { event.preventDefault(); onResize(clamp(now - grows * step)); }
      else if (event.key === "ArrowDown") { event.preventDefault(); onResize(clamp(now + grows * step)); }
    }} />;
}
