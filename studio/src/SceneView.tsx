import { useEffect, useRef, useState, type CSSProperties } from "react";
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
type HandleSide = "top" | "right" | "bottom" | "left";
type GraphHandle = { id: string; type: "source" | "target"; side: HandleSide; offset: number };
type RichNodeData = { label: string; entity?: SceneEntity; token?: boolean; effect?: EntityEffect; entering?: boolean; exiting?: boolean; moving?: boolean; handles?: GraphHandle[] };

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

function RichNode({ data }: NodeProps) {
  const view = data as RichNodeData;
  const metrics = view.entity?.metrics.map((metric) => `${metric.label}: ${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`).join(" · ");
  return <div title={metrics || undefined} className={`rich-node ${view.entity ? `tone-${view.entity.tone}` : ""}`}>
    {view.handles?.map((handle) => <Handle key={handle.id} id={handle.id} type={handle.type} position={HANDLE_POSITION[handle.side]} style={handleStyle(handle.side, handle.offset)} className="graph-node-handle" isConnectable={false} />)}
    <div className={`rich-node-lifecycle ${view.entering ? "is-entering" : ""} ${view.exiting ? "is-exiting" : ""}`}>
      <div className={`rich-node-effect effect-${view.effect ?? "none"}`}>
        <div className={`rich-node-content ${view.token ? "entity-token" : ""} ${view.moving ? "is-moving" : ""}`}>
          {view.entity && <SceneIcon glyph={view.entity.glyph} size={view.token ? 18 : 25} />}
          <span>{view.label}</span>
          {view.entity?.status && <small>{view.entity.status.replaceAll("_", " ")}</small>}
        </div>
      </div>
    </div>
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
  const routes = graphEdgeRoutes(surface, positions);
  const handles = new Map<string, GraphHandle[]>();
  for (const route of routes.values()) {
    const sourceHandles = handles.get(route.source) ?? [];
    sourceHandles.push({ id: route.sourceHandle, type: "source", side: route.sourceSide, offset: route.offset });
    handles.set(route.source, sourceHandles);
    const targetHandles = handles.get(route.target) ?? [];
    targetHandles.push({ id: route.targetHandle, type: "target", side: route.targetSide, offset: route.offset });
    handles.set(route.target, targetHandles);
  }
  const previousById = new Map(previousScene?.entities.map((entity) => [entity.id.key, entity]) ?? []);
  const entityAtNode = (key: string) => scene.entities.find((entity) => entity.anchor.kind === "graph_node" && entity.anchor.node === key && entity.id.key === key)
    ?? scene.entities.find((entity) => entity.anchor.kind === "graph_node" && entity.anchor.node === key);
  const nodes: Node[] = surface.nodes.map((node) => ({
    id: node.id.key,
    type: "rich",
    position: { x: node.x ?? 0, y: node.y ?? 0 },
    data: (() => { const entity = entityAtNode(node.id.key); return { label: node.id.label, entity, effect: entity ? composedEntityEffect(entity, previousById.get(entity.id.key), frame) : "none", handles: handles.get(node.id.key) } satisfies RichNodeData; })(),
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
  const occupied: GraphRect[] = surface.nodes.map((node) => ({ x: node.x ?? 0, y: node.y ?? 0, width: GRAPH_NODE_WIDTH, height: GRAPH_NODE_HEIGHT }));
  for (const overlay of overlays) {
    const position = graphEntityPosition(overlay.entity, surface, positions, overlay.position, occupied);
    occupied.push({ ...position, width: GRAPH_ENTITY_WIDTH, height: GRAPH_ENTITY_HEIGHT });
    nodes.push({ id: `entity:${overlay.entity.id.key}`, type: "rich", position, data: { label: overlay.entity.id.label, entity: overlay.entity, token: true, effect: composedEntityEffect(overlay.entity, previousById.get(overlay.entity.id.key), frame), entering: overlay.entering, exiting: overlay.exiting, moving: overlay.moving } satisfies RichNodeData, draggable: false, selectable: false, className: `entity-overlay motion-${motion}` });
  }
  const edges: Edge[] = surface.edges.map((edge) => {
    const semantic = scene.paths.find((path) => path.id === edge.id);
    const route = routes.get(edge.id);
    return {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      type: "smoothstep",
      sourceHandle: route?.sourceHandle,
      targetHandle: route?.targetHandle,
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

type GraphPoint = { x: number; y: number };
type GraphRect = GraphPoint & { width: number; height: number };
type GraphEdgeRoute = { source: string; target: string; sourceHandle: string; targetHandle: string; sourceSide: HandleSide; targetSide: HandleSide; offset: number };

const GRAPH_NODE_WIDTH = 88;
const GRAPH_NODE_HEIGHT = 72;
const GRAPH_ENTITY_WIDTH = 112;
const GRAPH_ENTITY_HEIGHT = 46;

export function graphEdgeSides(source: GraphPoint, target: GraphPoint): { source: HandleSide; target: HandleSide } {
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  if (Math.abs(dx) >= Math.abs(dy)) return dx >= 0 ? { source: "right", target: "left" } : { source: "left", target: "right" };
  return dy >= 0 ? { source: "bottom", target: "top" } : { source: "top", target: "bottom" };
}

function graphEdgeRoutes(surface: GraphSurface, positions: Map<string, GraphPoint>): Map<string, GraphEdgeRoute> {
  const groups = new Map<string, typeof surface.edges>();
  for (const edge of surface.edges) {
    const key = [edge.source, edge.target].sort().join("\u0000");
    groups.set(key, [...(groups.get(key) ?? []), edge]);
  }
  const routes = new Map<string, GraphEdgeRoute>();
  for (const group of groups.values()) {
    const ordered = [...group].sort((left, right) => left.id.localeCompare(right.id));
    ordered.forEach((edge, index) => {
      const source = positions.get(edge.source) ?? { x: 0, y: 0 };
      const target = positions.get(edge.target) ?? { x: 0, y: 1 };
      const sides = graphEdgeSides(source, target);
      routes.set(edge.id, {
        source: edge.source,
        target: edge.target,
        sourceHandle: `source:${edge.id}`,
        targetHandle: `target:${edge.id}`,
        sourceSide: sides.source,
        targetSide: sides.target,
        offset: (index + 1) / (ordered.length + 1),
      });
    });
  }
  return routes;
}

function anchorKey(anchor: SceneEntity["anchor"]): string {
  if (anchor.kind === "graph_node") return `node:${anchor.node}`;
  if (anchor.kind === "graph_edge") return `edge:${anchor.edge}:${anchor.progress ?? 0.5}`;
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

function graphEntityPosition(entity: SceneEntity, surface: GraphSurface, positions: Map<string, GraphPoint>, fallback: GraphPoint, occupied: GraphRect[]): GraphPoint {
  const hash = stableHash(entity.id.key);
  const anchor = graphAnchorPoint(entity, surface, positions) ?? { x: fallback.x + GRAPH_NODE_WIDTH / 2, y: fallback.y + GRAPH_NODE_HEIGHT / 2 };
  const candidates = entity.anchor.kind === "graph_edge"
    ? edgeEntityCandidates(entity, surface, positions, anchor, hash)
    : nodeEntityCandidates(anchor, hash);
  return candidates.find((candidate) => !occupied.some((rect) => rectanglesOverlap({ ...candidate, width: GRAPH_ENTITY_WIDTH, height: GRAPH_ENTITY_HEIGHT }, rect, 8))) ?? candidates[0];
}

function nodeEntityCandidates(anchor: GraphPoint, hash: number): GraphPoint[] {
  const candidates: GraphPoint[] = [];
  const first = hash % 12;
  for (const radius of [94, 142, 190]) {
    for (let step = 0; step < 12; step += 1) {
      const angle = ((first + step) % 12) * Math.PI / 6;
      candidates.push({
        x: Math.round(anchor.x + Math.cos(angle) * radius - GRAPH_ENTITY_WIDTH / 2),
        y: Math.round(anchor.y + Math.sin(angle) * radius - GRAPH_ENTITY_HEIGHT / 2),
      });
    }
  }
  return candidates;
}

function edgeEntityCandidates(entity: SceneEntity, surface: GraphSurface, positions: Map<string, GraphPoint>, anchor: GraphPoint, hash: number): GraphPoint[] {
  if (entity.anchor.kind !== "graph_edge") return nodeEntityCandidates(anchor, hash);
  const edgeId = entity.anchor.edge;
  const edge = surface.edges.find((candidate) => candidate.id === edgeId);
  const source = edge ? positions.get(edge.source) : undefined;
  const target = edge ? positions.get(edge.target) : undefined;
  if (!source || !target) return nodeEntityCandidates(anchor, hash);
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const length = Math.hypot(dx, dy) || 1;
  const normal = { x: -dy / length, y: dx / length };
  const sign = hash % 2 === 0 ? 1 : -1;
  return [24 * sign, -24 * sign, 42 * sign, -42 * sign, 0].map((distance) => ({
    x: Math.round(anchor.x + normal.x * distance - GRAPH_ENTITY_WIDTH / 2),
    y: Math.round(anchor.y + normal.y * distance - GRAPH_ENTITY_HEIGHT / 2),
  }));
}

function graphAnchorPoint(entity: SceneEntity, surface: GraphSurface, positions: Map<string, GraphPoint>): GraphPoint | undefined {
  const anchor = entity.anchor;
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

function rectanglesOverlap(left: GraphRect, right: GraphRect, padding: number): boolean {
  return left.x < right.x + right.width + padding
    && left.x + left.width + padding > right.x
    && left.y < right.y + right.height + padding
    && left.y + left.height + padding > right.y;
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
