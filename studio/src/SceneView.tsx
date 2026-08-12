import { useEffect, useRef, useState } from "react";
import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
  type ReactFlowInstance,
} from "@xyflow/react";
import type { ExchangeFrame, Scene, SceneEntity, SceneSurface } from "./api";
import { SceneIcon } from "./SceneIcon";

export type ReplayMotionMode = "step" | "seek";

export function SceneView({ scene, previousScene, frame, motion = "step", onAccount }: { scene?: Scene | null; previousScene?: Scene | null; frame?: ExchangeFrame; motion?: ReplayMotionMode; onAccount?: (account: string) => void }) {
  if (!scene) {
    return <div className="empty-state">No picture for this problem — the accounts below still show everything.</div>;
  }
  return <div className="scene-composition">
    <SceneChrome scene={scene} />
    <SceneSurfaceView scene={scene} previousScene={previousScene} frame={frame} motion={motion} onAccount={onAccount} />
  </div>;
}

function SceneSurfaceView({ scene, previousScene, frame, motion, onAccount }: { scene: Scene; previousScene?: Scene | null; frame?: ExchangeFrame; motion: ReplayMotionMode; onAccount?: (account: string) => void }) {
  switch (scene.surface.kind) {
    case "graph": return <GraphScene scene={scene} previousScene={previousScene} frame={frame} motion={motion} surface={scene.surface} onAccount={onAccount} />;
    case "grid": return <GridScene scene={scene} surface={scene.surface} onAccount={onAccount} />;
    case "matrix": return <MatrixScene scene={scene} surface={scene.surface} onAccount={onAccount} />;
    case "timeline": return <TimelineScene scene={scene} surface={scene.surface} onAccount={onAccount} />;
  }
}

function SceneChrome({ scene }: { scene: Scene }) {
  if (scene.metrics.length === 0 && scene.legend.length === 0 && scene.annotations.length === 0) return null;
  return <div className="scene-chrome">
    {scene.metrics.length > 0 && <div className="scene-metrics">{scene.metrics.map((metric) => <div key={metric.key} className="scene-metric"><span>{metric.label}</span><strong>{metric.value}{metric.unit ? ` ${metric.unit}` : ""}</strong>{metric.previous !== null && metric.previous !== undefined && metric.previous !== metric.value && <small>{metric.previous} → {metric.value}</small>}</div>)}</div>}
    {scene.annotations.length > 0 && <div className="scene-annotations">{scene.annotations.map((annotation) => <span key={annotation.id} className={`tone-${annotation.tone}`}>{annotation.label}</span>)}</div>}
    {scene.legend.length > 0 && <div className="scene-legend" aria-label="Scene legend">{scene.legend.slice(0, 8).map((entry, index) => <span key={`${entry.glyph}:${entry.tone}:${index}`} className={`tone-${entry.tone}`}><SceneIcon glyph={entry.glyph} size={15} />{entry.label}</span>)}</div>}
  </div>;
}

type GraphSurface = Extract<SceneSurface, { kind: "graph" }>;
type EntityEffect = "consumed" | "produced" | "preserved" | "changed" | "none";
type RichNodeData = { label: string; entity?: SceneEntity; token?: boolean; effect?: EntityEffect; entering?: boolean; exiting?: boolean; moving?: boolean };

function RichNode({ data }: NodeProps) {
  const view = data as RichNodeData;
  const metrics = view.entity?.metrics.map((metric) => `${metric.label}: ${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`).join(" · ");
  return <div title={metrics || undefined} className={`rich-node ${view.token ? "entity-token" : ""} ${view.entity ? `tone-${view.entity.tone}` : ""} effect-${view.effect ?? "none"} ${view.entering ? "is-entering" : ""} ${view.exiting ? "is-exiting" : ""} ${view.moving ? "is-moving" : ""}`}>
    <Handle type="target" position={Position.Top} isConnectable={false} />
    {view.entity && <SceneIcon glyph={view.entity.glyph} size={view.token ? 18 : 25} />}
    <span>{view.label}</span>
    {view.entity?.status && <small>{view.entity.status.replaceAll("_", " ")}</small>}
    <Handle type="source" position={Position.Bottom} isConnectable={false} />
  </div>;
}

const nodeTypes = { rich: RichNode };

function GraphScene({ scene, previousScene, frame, motion, surface, onAccount }: { scene: Scene; previousScene?: Scene | null; frame?: ExchangeFrame; motion: ReplayMotionMode; surface: GraphSurface; onAccount?: (account: string) => void }) {
  const host = useRef<HTMLDivElement>(null);
  const [flow, setFlow] = useState<ReactFlowInstance | null>(null);
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
      pending = requestAnimationFrame(() => { void flow.fitView({ padding: 0.15 }); });
    });
    observer.observe(host.current);
    return () => { observer.disconnect(); cancelAnimationFrame(pending); };
  }, [flow]);
  const positions = new Map(surface.nodes.map((node) => [node.id.key, { x: node.x ?? 0, y: node.y ?? 0 }]));
  const previousById = new Map(previousScene?.entities.map((entity) => [entity.id.key, entity]) ?? []);
  const entityAtNode = (key: string) => scene.entities.find((entity) => entity.anchor.kind === "graph_node" && entity.anchor.node === key && entity.id.key === key)
    ?? scene.entities.find((entity) => entity.anchor.kind === "graph_node" && entity.anchor.node === key);
  const nodes: Node[] = surface.nodes.map((node) => ({
    id: node.id.key,
    type: "rich",
    position: { x: node.x ?? 0, y: node.y ?? 0 },
    data: (() => { const entity = entityAtNode(node.id.key); return { label: node.id.label, entity, effect: entity ? composedEntityEffect(entity, previousById.get(entity.id.key), frame) : "none" } satisfies RichNodeData; })(),
    className: node.classes.join(" "),
    draggable: false,
  }));
  const overlays: Array<{ entity: SceneEntity; position: { x: number; y: number }; entering: boolean; exiting: boolean; moving: boolean }> = [];
  for (const entity of scene.entities) {
    if (surface.nodes.some((node) => node.id.key === entity.id.key)) continue;
    const position = anchorPosition(entity, surface, positions);
    if (!position) continue;
    const previous = previousById.get(entity.id.key);
    overlays.push({ entity, position, entering: Boolean(previousScene && !previous), exiting: false, moving: Boolean(previous && anchorKey(previous.anchor) !== anchorKey(entity.anchor)) });
  }
  if (previousScene) {
    for (const entity of previousScene.entities) {
      if (scene.entities.some((candidate) => candidate.id.key === entity.id.key) || surface.nodes.some((node) => node.id.key === entity.id.key)) continue;
      const position = anchorPosition(entity, surface, positions);
      if (position) overlays.push({ entity, position, entering: false, exiting: true, moving: false });
    }
  }
  overlays.sort((left, right) => left.entity.id.key.localeCompare(right.entity.id.key));
  for (const overlay of overlays) {
    // A stable identity owns one deterministic orbit slot. Colocated entities
    // therefore remain legible without shifting when a peer appears or leaves.
    const slot = stableEntitySlot(overlay.entity.id.key);
    nodes.push({ id: `entity:${overlay.entity.id.key}`, type: "rich", position: { x: overlay.position.x + slot.x, y: overlay.position.y + slot.y }, data: { label: overlay.entity.id.label, entity: overlay.entity, token: true, effect: composedEntityEffect(overlay.entity, previousById.get(overlay.entity.id.key), frame), entering: overlay.entering, exiting: overlay.exiting, moving: overlay.moving } satisfies RichNodeData, draggable: false, selectable: false, className: `entity-overlay motion-${motion}` });
  }
  const edges: Edge[] = surface.edges.map((edge) => {
    const semantic = scene.paths.find((path) => path.id === edge.id);
    return {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      type: "smoothstep",
      label: edge.label,
      className: [...edge.classes, semantic ? `path-${semantic.status}` : ""].filter(Boolean).join(" "),
      animated: semantic?.status === "current" || semantic?.status === "candidate" || edge.classes.includes("selected") || edge.classes.includes("completed"),
      markerEnd: { type: MarkerType.ArrowClosed },
    };
  });
  return <div className={`graph-scene motion-${motion}`} data-motion={motion} ref={host} role="img" aria-label={scene.title}>
    <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} fitView fitViewOptions={{ padding: 0.15 }} minZoom={0.25} maxZoom={2} nodesConnectable={false} nodesDraggable={false} elementsSelectable onInit={setFlow} onNodeClick={(_, node) => { const account = (node.data as RichNodeData).entity?.account; if (account) onAccount?.(account); }}>
      <Background gap={22} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  </div>;
}

function anchorKey(anchor: SceneEntity["anchor"]): string {
  if (anchor.kind === "graph_node") return `node:${anchor.node}`;
  if (anchor.kind === "graph_edge") return `edge:${anchor.edge}:${anchor.progress ?? 0.5}`;
  return JSON.stringify(anchor);
}

function stableEntitySlot(key: string): { x: number; y: number } {
  let hash = 2166136261;
  for (const character of key) {
    hash ^= character.charCodeAt(0);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  const index = hash % 16;
  const angle = (index % 8) * Math.PI / 4;
  const radius = index < 8 ? 66 : 112;
  return { x: Math.round(Math.cos(angle) * radius), y: 48 + Math.round(Math.sin(angle) * radius) };
}

function anchorPosition(entity: SceneEntity, surface: GraphSurface, positions: Map<string, { x: number; y: number }>): { x: number; y: number } | undefined {
  const anchor = entity.anchor;
  if (anchor.kind === "graph_node") return positions.get(anchor.node);
  if (anchor.kind !== "graph_edge") return undefined;
  const edge = surface.edges.find((candidate) => candidate.id === anchor.edge);
  const source = edge ? positions.get(edge.source) : undefined;
  const target = edge ? positions.get(edge.target) : undefined;
  if (!source || !target) return undefined;
  const progress = anchor.progress ?? 0.5;
  return { x: source.x + (target.x - source.x) * progress, y: source.y + (target.y - source.y) * progress };
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

function GridScene({ scene, surface, onAccount }: { scene: Scene; surface: GridSurface; onAccount?: (account: string) => void }) {
  const entities = (x: number, y: number) => scene.entities.filter((entity) => entity.anchor.kind === "grid_cell" && entity.anchor.x === x && entity.anchor.y === y);
  return <div className="grid-scene" role="grid" aria-label={scene.title} style={{ gridTemplateColumns: `repeat(${surface.width}, minmax(42px, 1fr))`, gridTemplateRows: `repeat(${surface.height}, minmax(42px, 1fr))` }}>
    {surface.cells.map((cell) => <div role="gridcell" aria-label={`${cell.x}, ${cell.y}: ${cell.label}`} key={`${cell.x}:${cell.y}`} className={cell.classes.join(" ")} style={{ gridColumn: cell.x + 1, gridRow: cell.y + 1 }}>
      <div className="grid-entities">{entities(cell.x, cell.y).map((entity) => <button type="button" key={entity.id.key} className={`grid-entity tone-${entity.tone}`} title={entity.id.label} disabled={!entity.account} onClick={() => entity.account && onAccount?.(entity.account)}><SceneIcon glyph={entity.glyph} size={24} /><b>{entity.id.label}</b></button>)}</div>
      {entities(cell.x, cell.y).length === 0 && <span>{cell.label}</span>}
      <small>{cell.x},{cell.y}</small>
    </div>)}
  </div>;
}

type MatrixSurface = Extract<SceneSurface, { kind: "matrix" }>;

function MatrixScene({ scene, surface, onAccount }: { scene: Scene; surface: MatrixSurface; onAccount?: (account: string) => void }) {
  const cell = (row: string, column: string) => surface.cells.find((candidate) => candidate.row === row && candidate.column === column);
  const entities = (row: string, column: string) => scene.entities.filter((entity) => entity.anchor.kind === "matrix_cell" && entity.anchor.row === row && entity.anchor.column === column);
  return <div className="matrix-scene"><table aria-label={scene.title}><thead><tr><th>Set</th>{surface.columns.map((column) => <th key={column.key}>{column.label}</th>)}</tr></thead><tbody>
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
  return <div className="timeline-scene" role="img" aria-label={scene.title}>
    {queued.length > 0 && <div className="timeline-queue"><strong>Ready queue</strong>{queued.map((item) => <button type="button" key={item.id.key} className={`tone-${item.tone}`} disabled={!item.account} onClick={() => item.account && onAccount?.(item.account)}><SceneIcon glyph={item.glyph} size={15} />{item.id.label}</button>)}</div>}
    <div className="timeline-axis"><span>0</span><span>{maximum}</span></div>
    {surface.lanes.map((lane) => <div className="timeline-lane" key={lane.id.key}><strong>{lane.id.label}</strong><div className="timeline-track">
      {surface.spans.filter((span) => span.lane === lane.id.key).map((span) => { const item = entity(span.id); return <div key={span.id} className={`timeline-span ${span.classes.join(" ")} ${item ? `tone-${item.tone}` : ""}`} style={{ left: `${(span.start / maximum) * 100}%`, width: `${Math.max(2, ((span.end - span.start) / maximum) * 100)}%` }}>{item && <SceneIcon glyph={item.glyph} size={15} />}{span.label}</div>; })}
      {surface.cursor !== null && surface.cursor !== undefined && <i className="timeline-cursor" style={{ left: `${(surface.cursor / maximum) * 100}%` }} aria-label={`Current time ${surface.cursor}`} />}
    </div></div>)}
  </div>;
}
