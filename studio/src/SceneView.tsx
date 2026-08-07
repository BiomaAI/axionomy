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
} from "@xyflow/react";
import type { Scene, SceneEntity, SceneSurface } from "./api";
import { SceneIcon } from "./SceneIcon";

export function SceneView({ scene }: { scene?: Scene | null }) {
  if (!scene) {
    return <div className="empty-state">No domain projection supplied; the accounts remain fully inspectable.</div>;
  }
  return <div className="scene-composition">
    <SceneChrome scene={scene} />
    <SceneSurfaceView scene={scene} />
  </div>;
}

function SceneSurfaceView({ scene }: { scene: Scene }) {
  switch (scene.surface.kind) {
    case "graph": return <GraphScene scene={scene} surface={scene.surface} />;
    case "grid": return <GridScene scene={scene} surface={scene.surface} />;
    case "matrix": return <MatrixScene scene={scene} surface={scene.surface} />;
    case "timeline": return <TimelineScene scene={scene} surface={scene.surface} />;
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

function GraphScene({ scene, surface }: { scene: Scene; surface: GraphSurface }) {
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
  for (const [index, entity] of scene.entities.entries()) {
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
    nodes.push({ id: `entity:${entity.id.key}`, type: "rich", position: { x: position.x + (index % 3) * 34, y: position.y + 74 + Math.floor(index / 3) * 30 }, data: { label: entity.id.label, entity, token: true } satisfies RichNodeData, draggable: false, selectable: false, className: "entity-overlay" });
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
  return <div className="graph-scene" role="img" aria-label={scene.title}>
    <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} fitView minZoom={0.25} maxZoom={2} nodesConnectable={false} nodesDraggable={false} elementsSelectable>
      <Background gap={22} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  </div>;
}

type GridSurface = Extract<SceneSurface, { kind: "grid" }>;

function GridScene({ scene, surface }: { scene: Scene; surface: GridSurface }) {
  const entities = (x: number, y: number) => scene.entities.filter((entity) => entity.anchor.kind === "grid_cell" && entity.anchor.x === x && entity.anchor.y === y);
  return <div className="grid-scene" role="grid" aria-label={scene.title} style={{ gridTemplateColumns: `repeat(${surface.width}, minmax(42px, 1fr))`, gridTemplateRows: `repeat(${surface.height}, minmax(42px, 1fr))` }}>
    {surface.cells.map((cell) => <div role="gridcell" aria-label={`${cell.x}, ${cell.y}: ${cell.label}`} key={`${cell.x}:${cell.y}`} className={cell.classes.join(" ")} style={{ gridColumn: cell.x + 1, gridRow: cell.y + 1 }}>
      <div className="grid-entities">{entities(cell.x, cell.y).map((entity) => <span key={entity.id.key} className={`grid-entity tone-${entity.tone}`} title={entity.id.label}><SceneIcon glyph={entity.glyph} size={24} /><b>{entity.id.label}</b></span>)}</div>
      {entities(cell.x, cell.y).length === 0 && <span>{cell.label}</span>}
      <small>{cell.x},{cell.y}</small>
    </div>)}
  </div>;
}

type MatrixSurface = Extract<SceneSurface, { kind: "matrix" }>;

function MatrixScene({ scene, surface }: { scene: Scene; surface: MatrixSurface }) {
  const cell = (row: string, column: string) => surface.cells.find((candidate) => candidate.row === row && candidate.column === column);
  const entities = (row: string, column: string) => scene.entities.filter((entity) => entity.anchor.kind === "matrix_cell" && entity.anchor.row === row && entity.anchor.column === column);
  return <div className="matrix-scene"><table aria-label={scene.title}><thead><tr><th>Set</th>{surface.columns.map((column) => <th key={column.key}>{column.label}</th>)}</tr></thead><tbody>
    {surface.rows.map((row) => <tr key={row.key}><th>{row.label}</th>{surface.columns.map((column) => {
      const value = cell(row.key, column.key); const occupants = entities(row.key, column.key);
      return <td key={column.key} className={value?.classes.join(" ")}>{occupants.map((entity) => <span key={entity.id.key} className={`matrix-entity tone-${entity.tone}`} title={entity.id.label}><SceneIcon glyph={entity.glyph} size={18} /></span>)}{occupants.length === 0 ? value?.label ?? "·" : null}</td>;
    })}</tr>)}
  </tbody></table></div>;
}

type TimelineSurface = Extract<SceneSurface, { kind: "timeline" }>;

function TimelineScene({ scene, surface }: { scene: Scene; surface: TimelineSurface }) {
  const maximum = Math.max(1, ...surface.spans.map((span) => span.end), surface.cursor ?? 0);
  const entity = (id: string) => scene.entities.find((candidate) => candidate.id.key === id);
  const queued = scene.entities.filter((candidate) => candidate.anchor.kind === "unanchored");
  return <div className="timeline-scene" role="img" aria-label={scene.title}>
    {queued.length > 0 && <div className="timeline-queue"><strong>Ready queue</strong>{queued.map((item) => <span key={item.id.key} className={`tone-${item.tone}`}><SceneIcon glyph={item.glyph} size={15} />{item.id.label}</span>)}</div>}
    <div className="timeline-axis"><span>0</span><span>{maximum}</span></div>
    {surface.lanes.map((lane) => <div className="timeline-lane" key={lane.id.key}><strong>{lane.id.label}</strong><div className="timeline-track">
      {surface.spans.filter((span) => span.lane === lane.id.key).map((span) => { const item = entity(span.id); return <div key={span.id} className={`timeline-span ${span.classes.join(" ")} ${item ? `tone-${item.tone}` : ""}`} style={{ left: `${(span.start / maximum) * 100}%`, width: `${Math.max(2, ((span.end - span.start) / maximum) * 100)}%` }}>{item && <SceneIcon glyph={item.glyph} size={15} />}{span.label}</div>; })}
      {surface.cursor !== null && surface.cursor !== undefined && <i className="timeline-cursor" style={{ left: `${(surface.cursor / maximum) * 100}%` }} aria-label={`Current time ${surface.cursor}`} />}
    </div></div>)}
  </div>;
}
