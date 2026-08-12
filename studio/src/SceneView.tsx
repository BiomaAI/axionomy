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
import type { Scene, SceneEntity, SceneSurface } from "./api";
import { SceneIcon } from "./SceneIcon";

export function SceneView({ scene, onAccount }: { scene?: Scene | null; onAccount?: (account: string) => void }) {
  if (!scene) {
    return <div className="empty-state">No picture for this problem — the accounts below still show everything.</div>;
  }
  return <div className="scene-composition">
    <SceneChrome scene={scene} />
    <SceneSurfaceView scene={scene} onAccount={onAccount} />
  </div>;
}

function SceneSurfaceView({ scene, onAccount }: { scene: Scene; onAccount?: (account: string) => void }) {
  switch (scene.surface.kind) {
    case "graph": return <GraphScene scene={scene} surface={scene.surface} onAccount={onAccount} />;
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
type RichNodeData = { label: string; entity?: SceneEntity; token?: boolean };

function RichNode({ data }: NodeProps) {
  const view = data as RichNodeData;
  return <div className={`rich-node ${view.token ? "entity-token" : ""} ${view.entity ? `tone-${view.entity.tone}` : ""}`}>
    <Handle type="target" position={Position.Top} isConnectable={false} />
    {view.entity && <SceneIcon glyph={view.entity.glyph} size={view.token ? 18 : 25} />}
    <span>{view.label}</span>
    {view.entity?.status && <small>{view.entity.status.replaceAll("_", " ")}</small>}
    <Handle type="source" position={Position.Bottom} isConnectable={false} />
  </div>;
}

const nodeTypes = { rich: RichNode };

function GraphScene({ scene, surface, onAccount }: { scene: Scene; surface: GraphSurface; onAccount?: (account: string) => void }) {
  const host = useRef<HTMLDivElement>(null);
  const [flow, setFlow] = useState<ReactFlowInstance | null>(null);
  // The stage sizes the canvas (viewport height, theater mode, breakpoints),
  // so the picture re-fits whenever its container changes size.
  useEffect(() => {
    if (!flow || !host.current) return;
    let pending = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(() => { void flow.fitView({ padding: 0.15 }); });
    });
    observer.observe(host.current);
    return () => { observer.disconnect(); cancelAnimationFrame(pending); };
  }, [flow]);
  const positions = new Map(surface.nodes.map((node) => [node.id.key, { x: node.x ?? 0, y: node.y ?? 0 }]));
  const entityAtNode = (key: string) => scene.entities.find((entity) => entity.anchor.kind === "graph_node" && entity.anchor.node === key && entity.id.key === key)
    ?? scene.entities.find((entity) => entity.anchor.kind === "graph_node" && entity.anchor.node === key);
  const nodes: Node[] = surface.nodes.map((node) => ({
    id: node.id.key,
    type: "rich",
    position: { x: node.x ?? 0, y: node.y ?? 0 },
    data: { label: node.id.label, entity: entityAtNode(node.id.key) } satisfies RichNodeData,
    className: node.classes.join(" "),
    draggable: false,
  }));
  const overlays: Array<{ entity: SceneEntity; position: { x: number; y: number } }> = [];
  for (const entity of scene.entities) {
    if (surface.nodes.some((node) => node.id.key === entity.id.key)) continue;
    let position: { x: number; y: number } | undefined;
    if (entity.anchor.kind === "graph_node") position = positions.get(entity.anchor.node);
    if (entity.anchor.kind === "graph_edge") {
      const edgeId = entity.anchor.edge;
      const edge = surface.edges.find((candidate) => candidate.id === edgeId);
      const source = edge ? positions.get(edge.source) : undefined;
      const target = edge ? positions.get(edge.target) : undefined;
      if (source && target) {
        const progress = entity.anchor.progress ?? 0.5;
        position = { x: source.x + (target.x - source.x) * progress, y: source.y + (target.y - source.y) * progress };
      }
    }
    if (!position) continue;
    overlays.push({ entity, position });
  }
  const colocated = new Map<string, number>();
  for (const overlay of overlays) {
    // Entities that share a node or route position form a readable vertical stack.
    // Quantizing the anchor also prevents distinct, crossing graph edges with the
    // same midpoint from placing interactive tokens on top of one another.
    const anchor = `${Math.round(overlay.position.x / 24)}:${Math.round(overlay.position.y / 24)}`;
    const index = colocated.get(anchor) ?? 0;
    colocated.set(anchor, index + 1);
    nodes.push({ id: `entity:${overlay.entity.id.key}`, type: "rich", position: { x: overlay.position.x, y: overlay.position.y + 88 + index * 52 }, data: { label: overlay.entity.id.label, entity: overlay.entity, token: true } satisfies RichNodeData, draggable: false, selectable: false, className: "entity-overlay" });
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
      animated: semantic?.status === "current" || semantic?.status === "candidate",
      markerEnd: { type: MarkerType.ArrowClosed },
    };
  });
  return <div className="graph-scene" ref={host} role="img" aria-label={scene.title}>
    <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} fitView fitViewOptions={{ padding: 0.15 }} minZoom={0.25} maxZoom={2} nodesConnectable={false} nodesDraggable={false} elementsSelectable onInit={setFlow} onNodeClick={(_, node) => { const account = (node.data as RichNodeData).entity?.account; if (account) onAccount?.(account); }}>
      <Background gap={22} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  </div>;
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
