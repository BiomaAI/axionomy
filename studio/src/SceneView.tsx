import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import {
  Background,
  BaseEdge,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  getSmoothStepPath,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeChange,
  type NodeProps,
  type ReactFlowInstance,
  useUpdateNodeInternals,
} from "@xyflow/react";
import type { ExchangeFrame, Scene, SceneEntity, SceneSurface } from "./api";
import { formatQuantity } from "./Quantity";
import { SceneIcon } from "./SceneIcon";

export type ReplayMotionMode = "step" | "seek";

export function SceneView({ scene, previousScene, history = [], frame, motion = "step", onAccount }: { scene?: Scene | null; previousScene?: Scene | null; history?: Scene[]; frame?: ExchangeFrame; motion?: ReplayMotionMode; onAccount?: (account: string) => void }) {
  // The chrome wraps rather than clipping, so the picture is laid out below
  // however many rows it takes.
  const chrome = useRef<HTMLDivElement>(null);
  const [chromeHeight, setChromeHeight] = useState(0);
  useLayoutEffect(() => {
    const node = chrome.current;
    if (!node) { setChromeHeight(0); return; }
    const measure = () => setChromeHeight(Math.ceil(node.getBoundingClientRect().height));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, [scene !== null && scene !== undefined && hasChrome(scene)]);
  if (!scene) {
    return <div className="empty-state">No picture for this problem — the accounts below still show everything.</div>;
  }
  return <div className="scene-composition" style={{ "--scene-top": `${chromeHeight}px` } as CSSProperties}>
    <SceneChrome scene={scene} lane={chrome} onAccount={onAccount} />
    <SceneSurfaceView scene={scene} previousScene={previousScene} history={history} frame={frame} motion={motion} onAccount={onAccount} topInset={chromeHeight} />
  </div>;
}

function hasChrome(scene: Scene): boolean {
  return scene.metrics.length > 0 || scene.legend.length > 0 || scene.annotations.length > 0 || scene.entities.some((entity) => entity.role === "context");
}

function SceneSurfaceView({ scene, previousScene, history, frame, motion, onAccount, topInset }: { scene: Scene; previousScene?: Scene | null; history: Scene[]; frame?: ExchangeFrame; motion: ReplayMotionMode; onAccount?: (account: string) => void; topInset: number }) {
  switch (scene.surface.kind) {
    case "graph": return <GraphScene scene={scene} previousScene={previousScene} frame={frame} motion={motion} surface={scene.surface} onAccount={onAccount} topInset={topInset} />;
    case "grid": return <GridScene scene={scene} previousScene={previousScene} frame={frame} motion={motion} surface={scene.surface} onAccount={onAccount} />;
    case "matrix": return <MatrixScene scene={scene} surface={scene.surface} onAccount={onAccount} />;
    case "timeline": return <TimelineScene scene={scene} surface={scene.surface} onAccount={onAccount} />;
    case "reserves": return <ReservesScene scene={scene} previousScene={previousScene} history={history} frame={frame} surface={scene.surface} onAccount={onAccount} />;
  }
}

type ReservesSurface = Extract<SceneSurface, { kind: "reserves" }>;
type ReservePool = ReservesSurface["pool"];
type ReserveCurve = NonNullable<ReservePool["curve"]>;

function reservesSurfaces(scenes: Scene[]): ReservesSurface[] {
  return scenes.flatMap((candidate) => candidate.surface.kind === "reserves" ? [candidate.surface] : []);
}

/** A number to place on a chart; text is always written from the exact value. */
function exactNumber(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

/** The pool's ratio to plot: its atoms ÷ 10^scale. */
function ratioNumber(pool: ReservePool): number {
  return exactNumber(pool.ratio.value) / 10 ** pool.ratio.format.scale;
}

/** The pool's ratio written exactly, without its unit. */
function ratioText(pool: ReservePool): string {
  return formatQuantity(pool.ratio.value, { ...pool.ratio.format, unit: null });
}

function signed(value: number, symbol: string): string {
  return `${value > 0 ? "+" : ""}${value.toLocaleString()} ${symbol}`;
}

/**
 * A pool of two reserves and the participants who exchange against it. The
 * problem writes every caption; this draws the shape: the pool and its ratio
 * in the middle, participants around it, the ratio's history and, when the
 * pool keeps one, the curve its reserves move along.
 */
function ReservesScene({ scene, previousScene, history, frame, surface, onAccount }: { scene: Scene; previousScene?: Scene | null; history: Scene[]; frame?: ExchangeFrame; surface: ReservesSurface; onAccount?: (account: string) => void }) {
  const { pool, participants } = surface;
  const previousPools = reservesSurfaces(history);
  const pools = previousPools.at(-1)?.pool.ratio.value === pool.ratio.value
    ? previousPools
    : [...previousPools, surface];
  const base = exactNumber(pool.base.quantity);
  const quote = exactNumber(pool.quote.quantity);
  const baseMax = Math.max(1, ...pools.map((candidate) => exactNumber(candidate.pool.base.quantity)));
  const quoteMax = Math.max(1, ...pools.map((candidate) => exactNumber(candidate.pool.quote.quantity)));
  // The participants this step's receipt changed are the ones taking part.
  const changed = new Set(frame?.receipt.deltas.filter((delta) => delta.consumed.length > 0 || delta.produced.length > 0).map((delta) => delta.account.key) ?? []);
  const active = participants.filter((participant) => changed.has(participant.account));
  // The state before this exchange is the previous snapshot; a replay holds each state once.
  const previousPool = frame && previousScene?.surface.kind === "reserves" ? previousScene.surface.pool : undefined;
  const baseDelta = previousPool ? base - exactNumber(previousPool.base.quantity) : 0;
  const quoteDelta = previousPool ? quote - exactNumber(previousPool.quote.quantity) : 0;
  const poolStyle = {
    "--base-fill": `${Math.max(18, base / baseMax * 100)}%`,
    "--quote-fill": `${Math.max(18, quote / quoteMax * 100)}%`,
  } as CSSProperties;
  const step = frame?.cues[0]?.label ?? frame?.exchange.rate.label;
  const stepLabel = frame ? [active[0]?.id.label, step].filter(Boolean).join(" · ") : "Initial reserves";
  const deltas = [baseDelta !== 0 ? signed(baseDelta, pool.base.symbol) : "", quoteDelta !== 0 ? signed(quoteDelta, pool.quote.symbol) : ""].filter(Boolean).join(" · ");

  return <div className={`reserves-scene${pool.curve ? "" : " no-curve"}`} aria-label={scene.title}>
    <div className="reserves-world">
      <svg className="reserves-links" viewBox="0 0 1000 600" role="img" aria-label={`Participants connected to ${pool.id.label}`}>
        <title>Who exchanges against the pool</title>
        {participants.map((participant) => <line key={participant.id.key} x1={participant.x * 10} y1={participant.y * 6} x2="500" y2="285" className={changed.has(participant.account) ? "is-active" : undefined} />)}
      </svg>
      <div className="reserves-step-line" role="status"><span>{stepLabel}</span>{frame && <strong>{deltas}</strong>}</div>
      {participants.map((participant) => <button
        type="button"
        key={participant.id.key}
        className={`reserves-participant tone-${participant.tone}${changed.has(participant.account) ? " is-active" : ""}`}
        style={{ left: `${participant.x}%`, top: `${participant.y}%` }}
        onClick={() => onAccount?.(participant.account)}
        aria-label={`${participant.id.label}: ${participant.base} ${pool.base.asset.label}, ${participant.quote} ${pool.quote.asset.label}${participant.status ? `, ${participant.status}` : ""}`}
      >
        <SceneIcon glyph={participant.glyph} size={17} />
        <span><b>{participant.id.label}</b><small>{Number(participant.base).toLocaleString()} {pool.base.symbol} · {Number(participant.quote).toLocaleString()} {pool.quote.symbol}</small></span>
      </button>)}
      <button type="button" className="reserves-pool" style={poolStyle} onClick={() => onAccount?.(pool.account)}>
        <span className="reserves-pool-caption">{pool.caption}</span>
        <strong>{ratioText(pool)}</strong>
        {pool.ratio.format.unit && <small>{pool.ratio.format.unit}</small>}
        <div className="reserves-bars">
          <span className="reserves-bar base"><i /><b>{base.toLocaleString()}</b><small>{pool.base.asset.label}</small></span>
          <span className="reserves-bar quote"><i /><b>{quote.toLocaleString()}</b><small>{pool.quote.asset.label}</small></span>
        </div>
      </button>
    </div>
    <div className="reserves-analysis">
      <RatioHistory pools={pools.map((candidate) => candidate.pool)} />
      {pool.curve && <ReserveCurveChart pool={pool} curve={pool.curve} />}
    </div>
  </div>;
}

function RatioHistory({ pools }: { pools: ReservePool[] }) {
  const width = 520;
  const height = 132;
  const padding = { left: 42, right: 18, top: 20, bottom: 25 };
  const values = pools.map(ratioNumber);
  const low = Math.min(...values);
  const high = Math.max(...values);
  const spread = Math.max(.001, high - low);
  const x = (index: number) => padding.left + (values.length <= 1 ? 0 : index / (values.length - 1) * (width - padding.left - padding.right));
  const y = (value: number) => padding.top + (high - value) / spread * (height - padding.top - padding.bottom);
  const path = values.map((value, index) => `${index === 0 ? "M" : "L"}${x(index).toFixed(1)},${y(value).toFixed(1)}`).join(" ");
  const current = pools.at(-1);
  const currentValue = values.at(-1) ?? 0;
  const label = current?.ratio.label ?? "Ratio";
  const written = current ? formatQuantity(current.ratio.value, current.ratio.format) : "";
  return <svg className="reserves-ratio-history" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`${label} history, currently ${written}`}>
    <title>{label} through accepted exchanges</title>
    <desc>The ratio of the pool's two reserves, after each exchange that moved it.</desc>
    <text x="12" y="14" className="chart-title">{label}</text>
    <line x1={padding.left} y1={padding.top} x2={padding.left} y2={height - padding.bottom} className="reserves-axis" />
    <line x1={padding.left} y1={height - padding.bottom} x2={width - padding.right} y2={height - padding.bottom} className="reserves-axis" />
    <text x={padding.left - 6} y={padding.top + 4} textAnchor="end">{high.toFixed(1)}</text>
    <text x={padding.left - 6} y={height - padding.bottom + 4} textAnchor="end">{low.toFixed(1)}</text>
    <text x={padding.left} y={height - 7}>initial</text>
    <text x={width - padding.right} y={height - 7} textAnchor="end">step {Math.max(0, values.length - 1)}</text>
    <path d={path} className="reserves-ratio-path" />
    {values.map((value, index) => <circle key={index} cx={x(index)} cy={y(value)} r={index === values.length - 1 ? 4 : 2} className={index === values.length - 1 ? "is-current" : undefined} />)}
    <text x={Math.min(width - 54, x(values.length - 1) + 8)} y={Math.max(28, y(currentValue) - 7)} className="chart-value">{current ? ratioText(current) : ""}</text>
  </svg>;
}

/** The reserve pairs a constant-product pool accepts, and where its reserves stand. */
function ReserveCurveChart({ pool, curve }: { pool: ReservePool; curve: ReserveCurve }) {
  const width = 420;
  const height = 132;
  const padding = { left: 38, right: 20, top: 20, bottom: 25 };
  const reserve = Math.max(1, exactNumber(pool.base.quantity));
  const quote = Math.max(1, exactNumber(pool.quote.quantity));
  const product = Math.max(1, exactNumber(curve.product));
  const minX = reserve * .42;
  const maxX = reserve * 1.85;
  const samples = Array.from({ length: 48 }, (_, index) => minX + index / 47 * (maxX - minX));
  const quotes = samples.map((value) => product / value);
  const minY = Math.min(...quotes);
  const maxY = Math.max(...quotes);
  const sx = (value: number) => padding.left + (value - minX) / (maxX - minX) * (width - padding.left - padding.right);
  const sy = (value: number) => padding.top + (maxY - value) / (maxY - minY) * (height - padding.top - padding.bottom);
  const path = samples.map((value, index) => `${index === 0 ? "M" : "L"}${sx(value).toFixed(1)},${sy(quotes[index]).toFixed(1)}`).join(" ");
  return <svg className="reserves-curve" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Constant-product curve at ${reserve.toLocaleString()} ${pool.base.asset.label} and ${quote.toLocaleString()} ${pool.quote.asset.label}`}>
    <title>Constant-product reserve curve</title>
    <desc>The point moves along the curve as participants exchange against the pool.</desc>
    <text x="12" y="14" className="chart-title">{curve.label}</text>
    <line x1={padding.left} y1={padding.top} x2={padding.left} y2={height - padding.bottom} className="reserves-axis" />
    <line x1={padding.left} y1={height - padding.bottom} x2={width - padding.right} y2={height - padding.bottom} className="reserves-axis" />
    <text x="4" y={padding.top + 4}>{pool.quote.asset.label}</text>
    <text x={width - padding.right} y={height - 7} textAnchor="end">{pool.base.asset.label} reserve</text>
    <path d={path} className="reserves-curve-path" />
    <line x1={sx(reserve)} x2={sx(reserve)} y1={sy(quote)} y2={height - padding.bottom} className="reserves-guide" />
    <line x1={padding.left} x2={sx(reserve)} y1={sy(quote)} y2={sy(quote)} className="reserves-guide" />
    <circle cx={sx(reserve)} cy={sy(quote)} r="5" className="is-current" />
    <text x={Math.min(width - 92, sx(reserve) + 9)} y={Math.max(29, sy(quote) - 7)} className="chart-value">{reserve.toLocaleString()} {pool.base.symbol} · {quote.toLocaleString()} {pool.quote.symbol}</text>
  </svg>;
}

function SceneChrome({ scene, lane, onAccount }: { scene: Scene; lane: RefObject<HTMLDivElement | null>; onAccount?: (account: string) => void }) {
  if (!hasChrome(scene)) return null;
  // What bears on the whole scenario rides in the lane, above the picture,
  // so it never covers a part of it.
  const context = scene.entities.filter((entity) => entity.role === "context");
  return <div className="scene-chrome" ref={lane}>
    {scene.metrics.length > 0 && <div className="scene-metrics">{scene.metrics.map((metric) => {
      const changed = metric.previous !== null && metric.previous !== undefined && metric.previous !== metric.value;
      // Every metric keeps a line for its change, so the lane's height, and
      // the picture fitted below it, hold still from step to step.
      return <div key={metric.key} className="scene-metric"><span>{metric.label}</span><strong>{metric.value}{metric.unit ? ` ${metric.unit}` : ""}</strong>{changed ? <small>{metric.previous} → {metric.value}</small> : <small className="unchanged" aria-hidden="true">{"\u00a0"}</small>}</div>;
    })}</div>}
    {scene.annotations.length > 0 && <ul className="scene-annotations" aria-label="Scene notes">{scene.annotations.map((annotation) => <li key={annotation.id} className={`tone-${annotation.tone}`}>{annotation.label}</li>)}</ul>}
    {scene.legend.length > 0 && <ul className="scene-legend" aria-label="Scene legend">{scene.legend.slice(0, 8).map((entry, index) => <li key={`${entry.glyph}:${entry.tone}:${index}`} className={`tone-${entry.tone}`}><SceneIcon glyph={entry.glyph} size={15} />{entry.label}</li>)}</ul>}
    {context.length > 0 && <ul className="scene-context" aria-label="Scenario context">{context.map((entity) => <li key={entity.id.key}><button type="button" className={`tone-${entity.tone}`} title={[entity.status, scene.surface.kind === "graph" ? contextRelation(entity, scene.surface) : undefined, entityDetail(entity)].filter(Boolean).join(" · ")} disabled={!entity.account} onClick={() => entity.account && onAccount?.(entity.account)}><SceneIcon glyph={entity.glyph} size={14} /><b>{entity.id.label}</b>{entity.status && <small>{entity.status}</small>}</button></li>)}</ul>}
  </div>;
}

type GraphSurface = Extract<SceneSurface, { kind: "graph" }>;
type FitPadding = Record<"top" | "right" | "bottom" | "left", `${number}px`>;
type EntityEffect = "consumed" | "produced" | "preserved" | "changed" | "none";
type HandleSide = "top" | "right" | "bottom" | "left";
type GraphHandle = { id: string; type: "source" | "target"; side: HandleSide; offset: number };
type StructureNodeData = { kind: "structure"; label: string; entity?: SceneEntity; states: SceneEntity[]; effect: EntityEffect; handles?: GraphHandle[]; focusAccount?: string };
type EntityAttachment = { entity: SceneEntity; effect: EntityEffect };
type OccupantNodeData = { kind: "occupant"; entity: SceneEntity; attachments: EntityAttachment[]; effect: EntityEffect; entering: boolean; exiting: boolean; moving: boolean; focusAccount?: string };
type AttachmentNodeData = { kind: "attachments"; label: string; entities: SceneEntity[]; effects: Record<string, EntityEffect>; focusAccount?: string; onAccount?: (account: string) => void };
type SceneNodeData = StructureNodeData | OccupantNodeData | AttachmentNodeData;

const HANDLE_POSITION: Record<HandleSide, Position> = {
  top: Position.Top,
  right: Position.Right,
  bottom: Position.Bottom,
  left: Position.Left,
};

function handleStyle(side: HandleSide, offset: number): CSSProperties {
  const percentage = `${Math.round(offset * 100)}%`;
  return side === "top" || side === "bottom" ? { left: percentage } : { top: percentage };
}

function effectClass(effect: EntityEffect): string {
  return `rich-node-effect effect-${effect}`;
}

function entityDetail(entity: SceneEntity): string | undefined {
  const metric = entity.metrics[0];
  if (metric) return `${metric.value}${metric.unit ? ` ${metric.unit}` : ""} ${metric.label}`;
  return entity.status?.replaceAll("_", " ");
}

function StructureNode({ data }: NodeProps) {
  const view = data as StructureNodeData;
  const intrinsic = view.entity && (view.entity.metrics.length > 0 || (view.entity.status && !["location", "resource", "place", "node", "machine", "facility"].includes(view.entity.status))) ? view.entity : undefined;
  const state = view.states[0] ?? intrinsic;
  const entity = state ?? view.entity;
  const visibleStates = [...view.states, ...(intrinsic ? [intrinsic] : [])];
  const metrics = visibleStates.flatMap((item) => item.metrics.map((metric) => `${item.id.label} · ${metric.label}: ${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`)).join(" · ");
  const title = [view.label, ...visibleStates.map((item) => item.status), metrics].filter(Boolean).join(" · ");
  return <div title={title || undefined} className={`rich-node structure-content ${entity ? `tone-${entity.tone}` : ""}`}>
    {view.handles?.map((handle) => <Handle key={handle.id} id={handle.id} type={handle.type} position={HANDLE_POSITION[handle.side]} style={handleStyle(handle.side, handle.offset)} className="graph-node-handle" isConnectable={false} />)}
    <div className={effectClass(view.effect)}>
      <div className="rich-node-content">
        {entity && <SceneIcon glyph={entity.glyph} size={21} />}
        <span className="structure-label">{view.label}</span>
        {state && <small className="structure-state">{entityDetail(state)}</small>}
        {state?.metrics.length && state.status ? <small className="structure-state structure-phase">{state.status}</small> : null}
        {view.states.length > 1 && <small className="structure-state-count">+{view.states.length - 1} linked</small>}
      </div>
    </div>
  </div>;
}

function OccupantNode({ data }: NodeProps) {
  const view = data as OccupantNodeData;
  const metrics = view.entity.metrics.map((metric) => `${metric.label}: ${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`).join(" · ");
  const inventory = view.attachments.map(({ entity }) => `${entity.id.label}${entity.status ? ` (${entity.status})` : ""}`).join(" · ");
  const title = [view.entity.id.label, metrics, inventory].filter(Boolean).join(" · ");
  return <div title={title || undefined} className={`rich-node tone-${view.entity.tone}`}>
    {view.entity.anchor.kind === "graph_node" && RELATION_SIDES.map((side) => <Handle key={side} id={`relation-target:${side}`} type="target" position={HANDLE_POSITION[side]} className="relation-handle" isConnectable={false} />)}
    <div className={`rich-node-lifecycle ${view.entering ? "is-entering" : ""} ${view.exiting ? "is-exiting" : ""}`}>
      <div className={effectClass(view.effect)}>
        <div className={`rich-node-content occupant-content ${view.moving ? "is-moving" : ""}`}>
          <SceneIcon glyph={view.entity.glyph} size={16} />
          <span className="occupant-label">{view.entity.id.label}</span>
          {view.entity.status && <small className="occupant-status">{view.entity.status.replaceAll("_", " ")}</small>}
          {view.attachments.length > 0 && <div className="occupant-inventory" aria-label={`${view.entity.id.label} inventory`}>{view.attachments.map(({ entity }) => <span key={entity.id.key} title={`${entity.id.label}${entity.status ? ` · ${entity.status}` : ""}`} className={`tone-${entity.tone}`}><SceneIcon glyph={entity.glyph} size={14} /><small>{entity.id.label}</small></span>)}</div>}
        </div>
      </div>
    </div>
  </div>;
}

function AttachmentNode({ data }: NodeProps) {
  const view = data as AttachmentNodeData;
  return <div className="attachment-collection">
    {RELATION_SIDES.map((side) => <Handle key={side} id={`relation-target:${side}`} type="target" position={HANDLE_POSITION[side]} className="relation-handle" isConnectable={false} />)}
    <header><span>{view.label}</span><b>{view.entities.length}</b></header>
    <div className="attachment-rows">
      {view.entities.slice(0, 4).map((entity) => <button type="button" key={entity.id.key} className={`${effectClass(view.effects[entity.id.key] ?? "none")} tone-${entity.tone}`} disabled={!entity.account} onClick={(event) => { event.stopPropagation(); if (entity.account) view.onAccount?.(entity.account); }}>
        <SceneIcon glyph={entity.glyph} size={15} />
        <span>{entity.id.label}</span>
        <small title={[entity.status?.replaceAll("_", " "), entity.metrics.length ? entityDetail(entity) : undefined].filter(Boolean).join(" · ")}>{[entity.status?.replaceAll("_", " "), entity.metrics.length ? entityDetail(entity) : undefined].filter(Boolean).join(" · ")}</small>
      </button>)}
      {view.entities.length > 4 && <div className="attachment-overflow">+{view.entities.length - 4} more</div>}
    </div>
  </div>;
}

const nodeTypes = { structure: StructureNode, occupant: OccupantNode, attachments: AttachmentNode };
const RELATION_SIDES: HandleSide[] = ["top", "right", "bottom", "left"];

/**
 * React Flow keeps where a node's handles are with the node's measured size,
 * which carries over from render to render: a node whose handles changed is
 * measured again, so its lines meet the handles it has now.
 */
function HandlesMeasured({ handles }: { handles: ReadonlyMap<string, string> }) {
  const measure = useUpdateNodeInternals();
  const seen = useRef<ReadonlyMap<string, string>>(new Map());
  useEffect(() => {
    const changed = [...handles].filter(([id, key]) => seen.current.has(id) && seen.current.get(id) !== key).map(([id]) => id);
    seen.current = handles;
    if (changed.length > 0) measure(changed);
  });
  return null;
}

function GraphScene({ scene, previousScene, frame, motion, surface, onAccount, topInset }: { scene: Scene; previousScene?: Scene | null; frame?: ExchangeFrame; motion: ReplayMotionMode; surface: GraphSurface; onAccount?: (account: string) => void; topInset: number }) {
  const host = useRef<HTMLDivElement>(null);
  const [flow, setFlow] = useState<ReactFlowInstance | null>(null);
  // Nodes are made afresh each render, and React Flow hides a node that
  // comes without its measured size until it measures it again, which a
  // render can race past and leave it hidden. Each node comes with the size
  // last measured; a change in size is measured as it happens, and a change
  // in its handles by HandlesMeasured.
  const [sizes, setSizes] = useState<ReadonlyMap<string, { width: number; height: number }>>(new Map());
  const measured = (changes: NodeChange[]) => {
    const now = changes.flatMap((change) => change.type === "dimensions" && change.dimensions ? [[change.id, change.dimensions] as const] : []);
    if (now.length === 0) return;
    setSizes((current) => {
      const next = new Map(current);
      for (const [id, size] of now) next.set(id, size);
      return next;
    });
  };
  // The picture fits below the chrome lane, whatever its height.
  const padding: FitPadding = { top: `${topInset + 14}px`, right: "24px", bottom: "24px", left: "24px" };
  const fitPadding = useRef(padding);
  fitPadding.current = padding;
  // The stage sizes the canvas (viewport height, theater mode, breakpoints),
  // so the picture re-fits whenever its container changes size.
  useEffect(() => {
    if (!flow || !host.current) return;
    let pending = 0;
    let lastWidth = 0;
    let lastHeight = 0;
    const observer = new ResizeObserver((entries) => {
      const box = entries[0]?.contentRect;
      if (!box || box.width < 40 || box.height < 40) return;
      if (Math.abs(box.width - lastWidth) < 1 && Math.abs(box.height - lastHeight) < 1) return;
      lastWidth = box.width;
      lastHeight = box.height;
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(() => { void flow.fitView({ padding: fitPadding.current }); });
    });
    observer.observe(host.current);
    return () => { observer.disconnect(); cancelAnimationFrame(pending); };
  }, [flow]);
  const positions = new Map(surface.nodes.map((node) => [node.id.key, { x: node.x ?? 0, y: node.y ?? 0 }]));
  const routes = graphEdgeRoutes(surface.edges, positions);
  const handles = new Map<string, GraphHandle[]>();
  for (const route of routes.values()) {
    const sourceHandles = handles.get(route.source) ?? [];
    sourceHandles.push({ id: route.sourceHandle, type: "source", side: route.sourceSide, offset: route.sourceOffset });
    handles.set(route.source, sourceHandles);
    const targetHandles = handles.get(route.target) ?? [];
    targetHandles.push({ id: route.targetHandle, type: "target", side: route.targetSide, offset: route.targetOffset });
    handles.set(route.target, targetHandles);
  }
  const previousById = new Map(previousScene?.entities.map((entity) => [entity.id.key, entity]) ?? []);
  const structureById = new Map(scene.entities.filter((entity) => entity.role === "structure").map((entity) => [entity.id.key, entity]));
  const statesByNode = groupBy(scene.entities.filter((entity) => entity.role === "state" && entity.anchor.kind === "graph_node"), (entity) => entity.anchor.kind === "graph_node" ? entity.anchor.node : "");
  // Nodes, lines and their labels are taken; markers and item lists find room around them.
  const occupied: GraphRect[] = [
    ...surface.nodes.map((node) => ({ x: node.x ?? 0, y: node.y ?? 0, width: GRAPH_NODE_WIDTH, height: GRAPH_NODE_HEIGHT })),
    ...surface.edges.flatMap((edge) => {
      const route = routes.get(edge.id);
      return route ? graphLineRects(edge.label, route, positions) : [];
    }),
  ];
  const relationEdges: Edge[] = [];
  const relationHandles = new Map<string, GraphHandle[]>();
  const nodes: Node[] = [];

  const attachments = groupBy(scene.entities.filter((entity) => entity.role === "attachment" && entity.anchor.kind !== "entity"), (entity) => anchorKey(entity.anchor));
  for (const [key, entities] of [...attachments.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    const anchor = entities[0]?.anchor;
    if (!anchor) continue;
    const height = attachmentHeight(entities.length);
    const point = semanticAttachmentPosition(anchor, surface, positions, occupied, ATTACHMENT_WIDTH, height);
    if (!point) continue;
    occupied.push({ ...point, width: ATTACHMENT_WIDTH, height });
    const nodeId = `attachments:${key}`;
    const label = attachmentLabel(anchor, surface, entities.length);
    const effects = Object.fromEntries(entities.map((entity) => [entity.id.key, composedEntityEffect(entity, previousById.get(entity.id.key), frame)]));
    const effect = strongestEffect(Object.values(effects));
    nodes.push({ id: nodeId, type: "attachments", position: point, data: { kind: "attachments", label, entities, effects, focusAccount: entities.find((entity) => entity.account)?.account ?? undefined, onAccount } satisfies AttachmentNodeData, draggable: false, selectable: true, className: `attachment-group motion-${motion} effect-${effect}` });
    if (anchor.kind === "graph_node") addRelation(anchor.node, nodeId, point, "attachment", relationHandles, relationEdges, positions);
  }

  // Preserve the outgoing attachment's docking footprint for one transition.
  // Otherwise an occupant can snap into the vacated space on the same frame
  // that the attachment is picked up, producing avoidable layout motion.
  const previousAttachments = groupBy(previousScene?.entities.filter((entity) => entity.role === "attachment" && entity.anchor.kind !== "entity") ?? [], (entity) => anchorKey(entity.anchor));
  for (const [key, entities] of previousAttachments) {
    if (attachments.has(key)) continue;
    const anchor = entities[0]?.anchor;
    if (!anchor) continue;
    const height = attachmentHeight(entities.length);
    const point = semanticAttachmentPosition(anchor, surface, positions, occupied, ATTACHMENT_WIDTH, height);
    if (point) occupied.push({ ...point, width: ATTACHMENT_WIDTH, height });
  }

  const occupants: Array<{ entity: SceneEntity; entering: boolean; exiting: boolean; moving: boolean }> = scene.entities.filter((entity) => entity.role === "occupant").map((entity) => {
    const previous = previousById.get(entity.id.key);
    return { entity, entering: Boolean(previousScene && !previous), exiting: false, moving: Boolean(previous && anchorKey(previous.anchor) !== anchorKey(entity.anchor)) };
  });
  if (previousScene) {
    for (const entity of previousScene.entities.filter((item) => item.role === "occupant")) {
      if (!scene.entities.some((candidate) => candidate.id.key === entity.id.key)) occupants.push({ entity, entering: false, exiting: true, moving: false });
    }
  }
  occupants.sort((left, right) => left.entity.id.key.localeCompare(right.entity.id.key));
  const entityAttachments = groupBy(scene.entities.filter((entity) => entity.role === "attachment" && entity.anchor.kind === "entity"), (entity) => entity.anchor.kind === "entity" ? entity.anchor.entity : "");
  const previousEntityAttachments = groupBy(previousScene?.entities.filter((entity) => entity.role === "attachment" && entity.anchor.kind === "entity") ?? [], (entity) => entity.anchor.kind === "entity" ? entity.anchor.entity : "");
  const occupantSlots = new Map<string, number>();
  for (const view of occupants) {
    const baseKey = anchorKey(view.entity.anchor);
    const slot = occupantSlots.get(baseKey) ?? 0;
    occupantSlots.set(baseKey, slot + 1);
    const point = semanticOccupantPosition(view.entity, slot, surface, positions, occupied);
    if (!point) continue;
    occupied.push({ ...point, width: OCCUPANT_WIDTH, height: OCCUPANT_HEIGHT });
    const nodeId = `entity:${view.entity.id.key}`;
    const attached = (entityAttachments.get(view.entity.id.key) ?? []).map((entity) => ({ entity, effect: composedEntityEffect(entity, previousById.get(entity.id.key), frame) }));
    const removedAttachmentEffects = (previousEntityAttachments.get(view.entity.id.key) ?? []).filter((entity) => !attached.some((candidate) => candidate.entity.id.key === entity.id.key)).map((entity) => sceneEntityEffect(entity, frame));
    const effect = strongestEffect([composedEntityEffect(view.entity, previousById.get(view.entity.id.key), frame), ...attached.map((candidate) => candidate.effect), ...removedAttachmentEffects]);
    nodes.push({ id: nodeId, type: "occupant", position: point, data: { kind: "occupant", entity: view.entity, attachments: attached, effect, entering: view.entering, exiting: view.exiting, moving: view.moving, focusAccount: view.entity.account ?? undefined } satisfies OccupantNodeData, draggable: false, selectable: true, className: `occupant-overlay motion-${motion} effect-${effect}` });
    if (view.entity.anchor.kind === "graph_node") addRelation(view.entity.anchor.node, nodeId, point, "occupant", relationHandles, relationEdges, positions);
  }

  for (const node of surface.nodes) {
    const structure = structureById.get(node.id.key);
    const states = statesByNode.get(node.id.key) ?? [];
    const effects = [structure, ...states].filter((entity): entity is SceneEntity => Boolean(entity)).map((entity) => composedEntityEffect(entity, previousById.get(entity.id.key), frame));
    const effect = strongestEffect(effects);
    nodes.push({
      id: node.id.key,
      type: "structure",
      position: { x: node.x ?? 0, y: node.y ?? 0 },
      data: { kind: "structure", label: node.id.label, entity: structure, states, effect, handles: [...(handles.get(node.id.key) ?? []), ...(relationHandles.get(node.id.key) ?? [])], focusAccount: states.find((entity) => entity.account)?.account ?? structure?.account ?? undefined } satisfies StructureNodeData,
      className: ["structure-node", `motion-${motion}`, `effect-${effect}`, ...node.classes].join(" "),
      draggable: false,
    });
  }

  const edges: Edge[] = surface.edges.map((edge) => {
    const semantic = scene.paths.find((path) => path.id === edge.id);
    const route = routes.get(edge.id);
    return {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      type: "scene",
      data: { parallel: route?.parallel ?? 0 } satisfies SceneEdgeData,
      sourceHandle: route?.sourceHandle,
      targetHandle: route?.targetHandle,
      label: edge.label,
      className: [...edge.classes, semantic ? `path-${semantic.status}` : ""].filter(Boolean).join(" "),
      animated: semantic?.status === "current" || semantic?.status === "candidate" || edge.classes.includes("selected") || edge.classes.includes("completed"),
      markerEnd: { type: MarkerType.ArrowClosed },
    };
  });
  const layoutKey = nodes.map((node) => `${node.id}:${node.position.x}:${node.position.y}:${node.type === "attachments" ? attachmentHeight((node.data as AttachmentNodeData).entities.length) : ""}`).join("|");
  // A different graph (another package, another view) is fitted afresh;
  // the same graph a step later moves only if something left the frame.
  const surfaceKey = surface.nodes.map((node) => node.id.key).join("|");
  const fittedSurface = useRef(surfaceKey);
  useEffect(() => {
    if (!flow || !host.current) return;
    let pending = requestAnimationFrame(() => {
      pending = requestAnimationFrame(() => {
        const box = host.current?.getBoundingClientRect();
        if (!box) return;
        if (fittedSurface.current !== surfaceKey) {
          fittedSurface.current = surfaceKey;
          void flow.fitView({ padding: fitPadding.current });
          return;
        }
        const viewport = flow.getViewport();
        const clipped = flow.getNodes().some((node) => {
          const x = node.position.x * viewport.zoom + viewport.x;
          const y = node.position.y * viewport.zoom + viewport.y;
          const width = (node.measured?.width ?? GRAPH_NODE_WIDTH) * viewport.zoom;
          const height = (node.measured?.height ?? GRAPH_NODE_HEIGHT) * viewport.zoom;
          return x < 8 || y < topInset + 6 || x + width > box.width - 8 || y + height > box.height - 8;
        });
        if (clipped) void flow.fitView({ padding: fitPadding.current, duration: motion === "step" ? 180 : 0 });
      });
    });
    return () => cancelAnimationFrame(pending);
  }, [flow, layoutKey, motion, surfaceKey, topInset]);
  return <div className={`graph-scene motion-${motion}`} data-motion={motion} ref={host} aria-label={scene.title}>
    {/* The wheel scrolls the page; the controls, a pinch, or Ctrl with the wheel zoom the picture. */}
    <ReactFlow nodes={nodes.map((node) => sizes.has(node.id) ? { ...node, measured: sizes.get(node.id) } : node)} onNodesChange={measured} edges={[...edges, ...relationEdges]} nodeTypes={nodeTypes} edgeTypes={edgeTypes} fitView fitViewOptions={{ padding }} minZoom={0.2} maxZoom={2} zoomOnScroll={false} preventScrolling={false} nodesConnectable={false} nodesDraggable={false} elementsSelectable onInit={setFlow} onNodeClick={(_, node) => { const account = (node.data as SceneNodeData).focusAccount; if (account) onAccount?.(account); }}>
      <Background gap={22} size={1} />
      <Controls showInteractive={false} fitViewOptions={{ padding }} />
      <HandlesMeasured handles={new Map(nodes.map((node) => [node.id, node.type === "structure" ? ((node.data as StructureNodeData).handles ?? []).map((handle) => handle.id).join(" ") : ""]))} />
    </ReactFlow>
  </div>;
}

type GraphPoint = { x: number; y: number };
type GraphRect = GraphPoint & { width: number; height: number };
type GraphEdgeRoute = { source: string; target: string; sourceHandle: string; targetHandle: string; sourceSide: HandleSide; targetSide: HandleSide; sourceOffset: number; targetOffset: number; parallel: number };

const GRAPH_NODE_WIDTH = 88;
const GRAPH_NODE_HEIGHT = 72;
const OCCUPANT_WIDTH = 136;
const OCCUPANT_HEIGHT = 42;
const ATTACHMENT_WIDTH = 168;

export function graphEdgeSides(source: GraphPoint, target: GraphPoint): { source: HandleSide; target: HandleSide } {
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  if (Math.abs(dx) >= Math.abs(dy)) return dx >= 0 ? { source: "right", target: "left" } : { source: "left", target: "right" };
  return dy >= 0 ? { source: "bottom", target: "top" } : { source: "top", target: "bottom" };
}

// Every line meeting a side of a node gets its own place on that side, in
// the order of the nodes at the lines' other ends, so no two lines share an
// end. Lines between the same two nodes are numbered around zero (`parallel`)
// so each can bend at its own place and keep its label clear of the others.
export function graphEdgeRoutes(edges: GraphSurface["edges"], positions: Map<string, GraphPoint>): Map<string, GraphEdgeRoute> {
  const at = (node: string, fallback: GraphPoint) => positions.get(node) ?? fallback;
  const pairs = new Map<string, string[]>();
  for (const edge of edges) {
    const key = [edge.source, edge.target].sort().join("\u0000");
    pairs.set(key, [...(pairs.get(key) ?? []), edge.id]);
  }
  const parallel = new Map<string, number>();
  for (const ids of pairs.values()) [...ids].sort().forEach((id, index) => parallel.set(id, index - (ids.length - 1) / 2));
  const sides = new Map(edges.map((edge) => [edge.id, graphEdgeSides(at(edge.source, { x: 0, y: 0 }), at(edge.target, { x: 0, y: 1 }))]));
  const ends = new Map<string, Array<{ key: string; along: number; id: string }>>();
  for (const edge of edges) {
    const side = sides.get(edge.id);
    if (!side) continue;
    for (const [end, node, other, nodeSide] of [["source", edge.source, edge.target, side.source], ["target", edge.target, edge.source, side.target]] as const) {
      const far = at(other, { x: 0, y: 0 });
      const list = ends.get(`${node}\u0000${nodeSide}`) ?? [];
      list.push({ key: `${end}:${edge.id}`, along: nodeSide === "left" || nodeSide === "right" ? far.y : far.x, id: edge.id });
      ends.set(`${node}\u0000${nodeSide}`, list);
    }
  }
  const offsets = new Map<string, number>();
  for (const list of ends.values()) {
    list.sort((left, right) => left.along - right.along || left.id.localeCompare(right.id));
    list.forEach((end, index) => offsets.set(end.key, (index + 1) / (list.length + 1)));
  }
  const routes = new Map<string, GraphEdgeRoute>();
  for (const edge of edges) {
    const side = sides.get(edge.id);
    if (!side) continue;
    routes.set(edge.id, {
      source: edge.source,
      target: edge.target,
      sourceHandle: `source:${edge.id}`,
      targetHandle: `target:${edge.id}`,
      sourceSide: side.source,
      targetSide: side.target,
      sourceOffset: offsets.get(`source:${edge.id}`) ?? .5,
      targetOffset: offsets.get(`target:${edge.id}`) ?? .5,
      parallel: parallel.get(edge.id) ?? 0,
    });
  }
  return routes;
}

type SceneEdgeData = { parallel: number };
const PARALLEL_GAP = 18;

// A step line whose bend moves with its place among parallel lines. Lines
// that fall (or rise) to the right bend outermost first, so they never cross,
// and their labels, drawn on the bend, stand apart.
function SceneEdge({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, label, markerEnd, style, interactionWidth, data }: EdgeProps<Edge<SceneEdgeData>>) {
  const across = sourcePosition === Position.Left || sourcePosition === Position.Right;
  const bend = parallelBend(data?.parallel ?? 0, { x: sourceX, y: sourceY }, { x: targetX, y: targetY });
  const [path, labelX, labelY] = getSmoothStepPath({
    sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition,
    ...(across ? { centerX: (sourceX + targetX) / 2 + bend } : { centerY: (sourceY + targetY) / 2 + bend }),
  });
  return <BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} interactionWidth={interactionWidth} label={label} labelX={labelX} labelY={labelY} />;
}

const edgeTypes = { scene: SceneEdge };

function parallelBend(parallel: number, source: GraphPoint, target: GraphPoint): number {
  const sense = Math.sign(target.x - source.x) * Math.sign(target.y - source.y) || 1;
  return -parallel * PARALLEL_GAP * sense;
}

function handlePoint(node: GraphPoint, side: HandleSide, offset: number): GraphPoint {
  if (side === "left") return { x: node.x, y: node.y + GRAPH_NODE_HEIGHT * offset };
  if (side === "right") return { x: node.x + GRAPH_NODE_WIDTH, y: node.y + GRAPH_NODE_HEIGHT * offset };
  if (side === "top") return { x: node.x + GRAPH_NODE_WIDTH * offset, y: node.y };
  return { x: node.x + GRAPH_NODE_WIDTH * offset, y: node.y + GRAPH_NODE_HEIGHT };
}

const LABEL_CHARACTER_WIDTH = 5.4;
const LABEL_HEIGHT = 16;

const LINE_WIDTH = 6;

// The room a line takes, as `SceneEdge` draws it: its three straight runs
// (out, across at its bend, in) and its label, which sits on the bend.
export function graphLineRects(label: string | null | undefined, route: GraphEdgeRoute, positions: Map<string, GraphPoint>): GraphRect[] {
  const source = handlePoint(positions.get(route.source) ?? { x: 0, y: 0 }, route.sourceSide, route.sourceOffset);
  const target = handlePoint(positions.get(route.target) ?? { x: 0, y: 0 }, route.targetSide, route.targetOffset);
  const bend = parallelBend(route.parallel, source, target);
  const across = route.sourceSide === "left" || route.sourceSide === "right";
  const center = { x: (source.x + target.x) / 2 + (across ? bend : 0), y: (source.y + target.y) / 2 + (across ? 0 : bend) };
  const corners = across
    ? [source, { x: center.x, y: source.y }, { x: center.x, y: target.y }, target]
    : [source, { x: source.x, y: center.y }, { x: target.x, y: center.y }, target];
  const runs = corners.slice(1).map((end, index) => {
    const start = corners[index];
    return { x: Math.min(start.x, end.x) - LINE_WIDTH / 2, y: Math.min(start.y, end.y) - LINE_WIDTH / 2, width: Math.abs(end.x - start.x) + LINE_WIDTH, height: Math.abs(end.y - start.y) + LINE_WIDTH };
  });
  if (!label) return runs;
  const width = label.length * LABEL_CHARACTER_WIDTH + 8;
  return [...runs, { x: center.x - width / 2, y: center.y - LABEL_HEIGHT / 2, width, height: LABEL_HEIGHT }];
}

function anchorKey(anchor: SceneEntity["anchor"]): string {
  if (anchor.kind === "graph_node") return `node:${anchor.node}`;
  if (anchor.kind === "graph_edge") return `edge:${anchor.edge}:${anchor.progress ?? 0.5}`;
  if (anchor.kind === "entity") return `entity:${anchor.entity}`;
  return JSON.stringify(anchor);
}

function stableHash(key: string): number {
  let hash = 2166136261;
  for (const character of key) {
    hash ^= character.charCodeAt(0);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return hash;
}

function groupBy<T>(items: T[], key: (item: T) => string): Map<string, T[]> {
  const groups = new Map<string, T[]>();
  for (const item of items) groups.set(key(item), [...(groups.get(key(item)) ?? []), item]);
  return groups;
}

function attachmentHeight(count: number): number {
  return 30 + Math.min(count, 4) * 42 + (count > 4 ? 22 : 0);
}

function attachmentLabel(anchor: SceneEntity["anchor"], surface: GraphSurface, count: number): string {
  const noun = count === 1 ? "item" : "items";
  if (anchor.kind === "graph_node") return `${surface.nodes.find((node) => node.id.key === anchor.node)?.id.label ?? anchor.node} · ${noun}`;
  if (anchor.kind === "graph_edge") return `On ${surface.edges.find((edge) => edge.id === anchor.edge)?.label ?? "route"} · ${noun}`;
  return noun;
}

function graphAnchorPoint(anchor: SceneEntity["anchor"], surface: GraphSurface, positions: Map<string, GraphPoint>): GraphPoint | undefined {
  if (anchor.kind === "graph_node") {
    const position = positions.get(anchor.node);
    return position ? { x: position.x + GRAPH_NODE_WIDTH / 2, y: position.y + GRAPH_NODE_HEIGHT / 2 } : undefined;
  }
  if (anchor.kind !== "graph_edge") return undefined;
  const edge = surface.edges.find((candidate) => candidate.id === anchor.edge);
  const source = edge ? positions.get(edge.source) : undefined;
  const target = edge ? positions.get(edge.target) : undefined;
  if (!source || !target) return undefined;
  const progress = anchor.progress ?? 0.5;
  return {
    x: source.x + GRAPH_NODE_WIDTH / 2 + (target.x - source.x) * progress,
    y: source.y + GRAPH_NODE_HEIGHT / 2 + (target.y - source.y) * progress,
  };
}

function semanticAttachmentPosition(anchor: SceneEntity["anchor"], surface: GraphSurface, positions: Map<string, GraphPoint>, occupied: GraphRect[], width: number, height: number): GraphPoint | undefined {
  const point = graphAnchorPoint(anchor, surface, positions);
  if (!point) return undefined;
  let baseAngle = -Math.PI / 2;
  if (anchor.kind === "graph_node") {
    const neighbors = surface.edges.flatMap((edge) => edge.source === anchor.node ? [edge.target] : edge.target === anchor.node ? [edge.source] : []);
    if (neighbors.length > 0) {
      const average = neighbors.reduce((sum, key) => { const target = positions.get(key); return target ? { x: sum.x + target.x + GRAPH_NODE_WIDTH / 2, y: sum.y + target.y + GRAPH_NODE_HEIGHT / 2 } : sum; }, { x: 0, y: 0 });
      baseAngle = Math.atan2(point.y - average.y / neighbors.length, point.x - average.x / neighbors.length);
    } else {
      baseAngle = (stableHash(anchor.node) % 8) * Math.PI / 4;
    }
  } else if (anchor.kind === "graph_edge") {
    const edge = surface.edges.find((candidate) => candidate.id === anchor.edge);
    const source = edge ? positions.get(edge.source) : undefined;
    const target = edge ? positions.get(edge.target) : undefined;
    if (source && target) baseAngle = Math.atan2(target.y - source.y, target.x - source.x) - Math.PI / 2;
  }
  const candidates: GraphPoint[] = [];
  for (const radius of [118, 172, 226]) {
    for (const offset of [0, .6, -.6, 1.2, -1.2, Math.PI]) {
      const angle = baseAngle + offset;
      candidates.push({ x: Math.round(point.x + Math.cos(angle) * radius - width / 2), y: Math.round(point.y + Math.sin(angle) * radius - height / 2) });
    }
  }
  return candidates.find((candidate) => !occupied.some((rect) => rectanglesOverlap({ ...candidate, width, height }, rect, 12))) ?? candidates[0];
}

function semanticOccupantPosition(entity: SceneEntity, slot: number, surface: GraphSurface, positions: Map<string, GraphPoint>, occupied: GraphRect[]): GraphPoint | undefined {
  const point = graphAnchorPoint(entity.anchor, surface, positions);
  if (!point) return undefined;
  if (entity.anchor.kind === "graph_edge") {
    const edgeId = entity.anchor.edge;
    const edge = surface.edges.find((candidate) => candidate.id === edgeId);
    const source = edge ? positions.get(edge.source) : undefined;
    const target = edge ? positions.get(edge.target) : undefined;
    const angle = source && target ? Math.atan2(target.y - source.y, target.x - source.x) - Math.PI / 2 : -Math.PI / 2;
    const preferred = { x: Math.round(point.x + Math.cos(angle) * 34 - OCCUPANT_WIDTH / 2), y: Math.round(point.y + Math.sin(angle) * 34 - OCCUPANT_HEIGHT / 2) };
    if (!occupied.some((rect) => rectanglesOverlap({ ...preferred, width: OCCUPANT_WIDTH, height: OCCUPANT_HEIGHT }, rect, 8))) return preferred;
  }
  const baseAngle = -Math.PI / 2 + slot * Math.PI / 2;
  const candidates: GraphPoint[] = [];
  for (const radius of [100, 145, 190]) {
    for (const offset of [0, Math.PI / 2, -Math.PI / 2, Math.PI]) {
      const angle = baseAngle + offset;
      candidates.push({ x: Math.round(point.x + Math.cos(angle) * radius - OCCUPANT_WIDTH / 2), y: Math.round(point.y + Math.sin(angle) * radius - OCCUPANT_HEIGHT / 2) });
    }
  }
  return candidates.find((candidate) => !occupied.some((rect) => rectanglesOverlap({ ...candidate, width: OCCUPANT_WIDTH, height: OCCUPANT_HEIGHT }, rect, 8))) ?? candidates[0];
}

function addRelation(source: string, target: string, targetPosition: GraphPoint, role: "occupant" | "attachment", relationHandles: Map<string, GraphHandle[]>, edges: Edge[], positions: Map<string, GraphPoint>): void {
  const sourcePoint = positions.get(source);
  if (!sourcePoint) return;
  const targetCenter = role === "occupant" ? { x: targetPosition.x + OCCUPANT_WIDTH / 2, y: targetPosition.y + OCCUPANT_HEIGHT / 2 } : { x: targetPosition.x + ATTACHMENT_WIDTH / 2, y: targetPosition.y + 20 };
  const sourceCenter = { x: sourcePoint.x + GRAPH_NODE_WIDTH / 2, y: sourcePoint.y + GRAPH_NODE_HEIGHT / 2 };
  const sides = graphEdgeSides(sourceCenter, targetCenter);
  const id = `relation:${source}:${target}`;
  relationHandles.set(source, [...(relationHandles.get(source) ?? []), { id: `source:${id}`, type: "source", side: sides.source, offset: .5 }]);
  edges.push({ id, source, target, sourceHandle: `source:${id}`, targetHandle: `relation-target:${sides.target}`, type: "straight", className: `relation-edge ${role}-relation`, selectable: false });
}

function strongestEffect(effects: EntityEffect[]): EntityEffect {
  for (const effect of ["produced", "consumed", "changed", "preserved"] as EntityEffect[]) if (effects.includes(effect)) return effect;
  return "none";
}

function contextRelation(entity: SceneEntity, surface: GraphSurface): string {
  if (entity.anchor.kind === "graph_node") {
    const nodeId = entity.anchor.node;
    return `Affects ${surface.nodes.find((node) => node.id.key === nodeId)?.id.label ?? nodeId}`;
  }
  if (entity.anchor.kind === "graph_edge") {
    const edgeId = entity.anchor.edge;
    return `Affects ${surface.edges.find((edge) => edge.id === edgeId)?.label ?? edgeId}`;
  }
  if (entity.anchor.kind === "entity") return `Attached to ${entity.anchor.entity}`;
  return "Applies to this scenario";
}

function rectanglesOverlap(left: GraphRect, right: GraphRect, padding: number): boolean {
  return left.x < right.x + right.width + padding
    && left.x + left.width + padding > right.x
    && left.y < right.y + right.height + padding
    && left.y + left.height + padding > right.y;
}

export function sceneEntityEffect(entity: SceneEntity, frame?: ExchangeFrame): EntityEffect {
  if (!frame) return "none";
  const references = entity.evidence ?? (entity.account ? [{ kind: "account" as const, account: entity.account }] : []);
  let preserved = false;
  let changed = false;
  for (const reference of references) {
    const delta = frame.receipt.deltas.find((candidate) => candidate.account.key === reference.account);
    if (!delta) continue;
    const matches = (assets: typeof delta.consumed) => reference.kind === "account" ? assets.length > 0 : assets.some((item) => item.asset.key === reference.asset);
    if (matches(delta.produced)) return "produced";
    if (matches(delta.consumed)) return "consumed";
    if (matches(delta.preserved)) preserved = true;
    if (reference.kind === "account") {
      changed = changed || delta.consumed.length > 0 || delta.produced.length > 0;
    }
  }
  if (preserved) return "preserved";
  return changed ? "changed" : "none";
}

function composedEntityEffect(entity: SceneEntity, previous: SceneEntity | undefined, frame?: ExchangeFrame): EntityEffect {
  const economic = sceneEntityEffect(entity, frame);
  if (economic !== "none") return economic;
  if (previous && (previous.status !== entity.status || previous.tone !== entity.tone || JSON.stringify(previous.metrics) !== JSON.stringify(entity.metrics))) return "changed";
  return "none";
}

type GridSurface = Extract<SceneSurface, { kind: "grid" }>;

const GRID_GAP_PX = 3;

function gridCellCenter(index: number, count: number) {
  const percentage = ((index + .5) / count) * 100;
  const gapCorrection = GRID_GAP_PX * (index - (count - 1) / 2) / count;
  return `calc(${percentage}% + ${gapCorrection}px)`;
}

function GridScene({ scene, previousScene, frame, motion, surface, onAccount }: { scene: Scene; previousScene?: Scene | null; frame?: ExchangeFrame; motion: ReplayMotionMode; surface: GridSurface; onAccount?: (account: string) => void }) {
  const current = scene.entities.filter((entity) => entity.anchor.kind === "grid_cell");
  const previous = previousScene?.entities.filter((entity) => entity.anchor.kind === "grid_cell") ?? [];
  const currentById = new Map(current.map((entity) => [entity.id.key, entity]));
  const previousById = new Map(previous.map((entity) => [entity.id.key, entity]));
  const entities = [...current, ...previous.filter((entity) => !currentById.has(entity.id.key))];
  const changedAccounts = new Set(frame?.receipt.deltas.filter((delta) => delta.consumed.length > 0 || delta.produced.length > 0).map((delta) => delta.account.key) ?? []);
  const paths = scene.paths.flatMap((path) => {
    const points = path.anchors.flatMap((anchor) => anchor.kind === "grid_cell" ? [`${anchor.x + .5},${anchor.y + .5}`] : []);
    return points.length > 1 ? [{ ...path, points: points.join(" ") }] : [];
  });
  const boardStyle = {
    "--grid-width": surface.width,
    "--grid-height": surface.height,
    "--grid-gap": `${GRID_GAP_PX}px`,
    gridTemplateColumns: `repeat(${surface.width}, minmax(0, 1fr))`,
    gridTemplateRows: `repeat(${surface.height}, minmax(0, 1fr))`,
  } as CSSProperties;
  return <div className="grid-scene">
    <div className="grid-board" role="grid" aria-label={scene.title} style={boardStyle}>
      {surface.cells.map((cell) => <button type="button" role="gridcell" aria-label={`${cell.label} at column ${cell.x + 1}, row ${cell.y + 1}`} key={`${cell.x}:${cell.y}`} className={`grid-cell ${cell.classes.join(" ")} ${cell.account && changedAccounts.has(cell.account) ? "is-changing" : ""}`} style={{ gridColumn: cell.x + 1, gridRow: cell.y + 1 }} disabled={!cell.account} onClick={() => cell.account && onAccount?.(cell.account)} />)}
      {paths.length > 0 && <svg className="grid-paths" viewBox={`0 0 ${surface.width} ${surface.height}`} preserveAspectRatio="none" aria-label="Paths through the grid">{paths.map((path) => <polyline key={path.id} points={path.points} className={`grid-path path-${path.status}`} vectorEffect="non-scaling-stroke"><title>{path.label}</title></polyline>)}</svg>}
      <div className="grid-entities-layer">
        {entities.map((entity) => {
          const shown = currentById.get(entity.id.key) ?? entity;
          const prior = previousById.get(entity.id.key);
          const anchor = shown.anchor.kind === "grid_cell" ? shown.anchor : prior?.anchor.kind === "grid_cell" ? prior.anchor : undefined;
          if (!anchor) return null;
          const entering = !prior && currentById.has(entity.id.key);
          const exiting = !currentById.has(entity.id.key);
          const moving = prior?.anchor.kind === "grid_cell" && (prior.anchor.x !== anchor.x || prior.anchor.y !== anchor.y);
          const effect = composedEntityEffect(shown, prior, frame);
          const status = shown.status ? `status-${shown.status.replaceAll(/[^a-zA-Z0-9_-]/g, "-").toLowerCase()}` : "";
          const style = {
            left: gridCellCenter(anchor.x, surface.width),
            top: gridCellCenter(anchor.y, surface.height),
          };
          return <button type="button" key={entity.id.key} aria-label={shown.id.label} data-grid-x={anchor.x} data-grid-y={anchor.y} className={`grid-entity tone-${shown.tone} ${status} motion-${motion} ${effectClass(effect)} ${entering ? "is-entering" : ""} ${exiting ? "is-exiting" : ""} ${moving ? "is-moving" : ""}`} style={style} disabled={!shown.account} onClick={() => shown.account && onAccount?.(shown.account)}><SceneIcon glyph={shown.glyph} size={25} /><b>{shown.id.label}</b>{shown.status && <small>{shown.status.replaceAll("_", " ")}</small>}</button>;
        })}
      </div>
    </div>
  </div>;
}

type MatrixSurface = Extract<SceneSurface, { kind: "matrix" }>;

function MatrixScene({ scene, surface, onAccount }: { scene: Scene; surface: MatrixSurface; onAccount?: (account: string) => void }) {
  const cell = (row: string, column: string) => surface.cells.find((candidate) => candidate.row === row && candidate.column === column);
  const entities = (row: string, column: string) => scene.entities.filter((entity) => entity.anchor.kind === "matrix_cell" && entity.anchor.row === row && entity.anchor.column === column);
  return <div className="matrix-scene"><table aria-label={scene.title}><thead><tr><td />{surface.columns.map((column) => <th key={column.key}>{column.label}</th>)}</tr></thead><tbody>
    {surface.rows.map((row) => <tr key={row.key}><th>{row.label}</th>{surface.columns.map((column) => {
      const value = cell(row.key, column.key); const occupants = entities(row.key, column.key);
      return <td key={column.key} className={value?.classes.join(" ")}>{occupants.map((entity) => <button type="button" key={entity.id.key} className={`matrix-entity tone-${entity.tone}`} title={entity.id.label} disabled={!entity.account} onClick={() => entity.account && onAccount?.(entity.account)}><SceneIcon glyph={entity.glyph} size={18} /></button>)}{occupants.length === 0 ? value?.label ?? "·" : null}</td>;
    })}</tr>)}
  </tbody></table></div>;
}

type TimelineSurface = Extract<SceneSurface, { kind: "timeline" }>;

function TimelineScene({ scene, surface, onAccount }: { scene: Scene; surface: TimelineSurface; onAccount?: (account: string) => void }) {
  const maximum = Math.max(1, ...surface.spans.map((span) => span.end), surface.cursor ?? 0);
  const entity = (id: string) => scene.entities.find((candidate) => candidate.id.key === id);
  const queued = scene.entities.filter((candidate) => candidate.anchor.kind === "unanchored");
  return <div className="timeline-scene" role="group" aria-label={scene.title}>
    {queued.length > 0 && <div className="timeline-queue"><strong>Waiting work</strong>{queued.map((item) => <button type="button" key={item.id.key} className={`tone-${item.tone}`} title={item.status ?? undefined} disabled={!item.account} onClick={() => item.account && onAccount?.(item.account)}><SceneIcon glyph={item.glyph} size={15} /><span>{item.id.label}{item.status && <small>{item.status}</small>}</span></button>)}</div>}
    <div className="timeline-axis"><span>0</span><span>{maximum}</span></div>
    {surface.lanes.map((lane) => <div className="timeline-lane" key={lane.id.key}><strong>{lane.id.label}</strong><div className="timeline-track">
      {surface.spans.filter((span) => span.lane === lane.id.key).map((span) => { const item = entity(span.id); return <div key={span.id} title={`${span.label} · ${span.start}–${span.end}${item?.status ? ` · ${item.status}` : ""}`} className={`timeline-span ${span.classes.join(" ")} ${item ? `tone-${item.tone}` : ""}`} style={{ left: `${(span.start / maximum) * 100}%`, width: `${Math.max(2, ((span.end - span.start) / maximum) * 100)}%` }}>{item && <SceneIcon glyph={item.glyph} size={15} />}{span.label}</div>; })}
      {surface.cursor !== null && surface.cursor !== undefined && <i className="timeline-cursor" style={{ left: `${(surface.cursor / maximum) * 100}%` }} aria-label={`Current time ${surface.cursor}`} />}
    </div><div className="timeline-state-list">{scene.entities.filter((item) => item.anchor.kind === "timeline" && item.anchor.lane === lane.id.key).map((item) => <button type="button" key={item.id.key} className={`tone-${item.tone}`} disabled={!item.account} onClick={() => item.account && onAccount?.(item.account)}><SceneIcon glyph={item.glyph} size={14} /><span>{item.id.label}</span><small>{item.status}</small></button>)}</div></div>)}
  </div>;
}
