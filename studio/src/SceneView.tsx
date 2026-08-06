import {
  Background,
  Controls,
  MarkerType,
  ReactFlow,
  type Edge,
  type Node,
} from "@xyflow/react";
import type { Scene } from "./api";

export function SceneView({ scene }: { scene?: Scene | null }) {
  if (!scene) {
    return <div className="empty-state">No domain projection supplied; the accounts remain fully inspectable.</div>;
  }
  switch (scene.kind) {
    case "graph":
      return <GraphScene scene={scene} />;
    case "grid":
      return <GridScene scene={scene} />;
    case "matrix":
      return <MatrixScene scene={scene} />;
    case "timeline":
      return <TimelineScene scene={scene} />;
  }
}

function GraphScene({ scene }: { scene: Extract<Scene, { kind: "graph" }> }) {
  const nodes: Node[] = scene.nodes.map((node) => ({
    id: node.id.key,
    position: { x: node.x ?? 0, y: node.y ?? 0 },
    data: { label: node.id.label },
    className: node.classes.join(" "),
    draggable: false,
  }));
  const edges: Edge[] = scene.edges.map((edge) => ({
    id: edge.id,
    source: edge.source,
    target: edge.target,
    type: "straight",
    label: edge.label,
    className: edge.classes.join(" "),
    markerEnd: { type: MarkerType.ArrowClosed },
  }));
  return (
    <div className="graph-scene" role="img" aria-label={scene.title}>
      <ReactFlow nodes={nodes} edges={edges} fitView minZoom={0.35} maxZoom={2} nodesConnectable={false} elementsSelectable={false}>
        <Background gap={22} size={1} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}

function GridScene({ scene }: { scene: Extract<Scene, { kind: "grid" }> }) {
  return (
    <div
      className="grid-scene"
      role="grid"
      aria-label={scene.title}
      style={{ gridTemplateColumns: `repeat(${scene.width}, minmax(42px, 1fr))`, gridTemplateRows: `repeat(${scene.height}, minmax(42px, 1fr))` }}
    >
      {scene.cells.map((cell) => (
        <div
          role="gridcell"
          aria-label={`${cell.x}, ${cell.y}: ${cell.label}`}
          key={`${cell.x}:${cell.y}`}
          className={cell.classes.join(" ")}
          style={{ gridColumn: cell.x + 1, gridRow: cell.y + 1 }}
        >
          <span>{cell.label}</span>
          <small>{cell.x},{cell.y}</small>
        </div>
      ))}
    </div>
  );
}

function MatrixScene({ scene }: { scene: Extract<Scene, { kind: "matrix" }> }) {
  const cell = (row: string, column: string) => scene.cells.find((candidate) => candidate.row === row && candidate.column === column);
  return (
    <div className="matrix-scene">
      <table aria-label={scene.title}>
        <thead><tr><th>Set</th>{scene.columns.map((column) => <th key={column.key}>{column.label}</th>)}</tr></thead>
        <tbody>
          {scene.rows.map((row) => <tr key={row.key}>
            <th>{row.label}</th>
            {scene.columns.map((column) => {
              const value = cell(row.key, column.key);
              return <td key={column.key} className={value?.classes.join(" ")}>{value?.label ?? "·"}</td>;
            })}
          </tr>)}
        </tbody>
      </table>
    </div>
  );
}

function TimelineScene({ scene }: { scene: Extract<Scene, { kind: "timeline" }> }) {
  const maximum = Math.max(1, ...scene.spans.map((span) => span.end), scene.cursor ?? 0);
  return (
    <div className="timeline-scene" role="img" aria-label={scene.title}>
      <div className="timeline-axis"><span>0</span><span>{maximum}</span></div>
      {scene.lanes.map((lane) => <div className="timeline-lane" key={lane.id.key}>
        <strong>{lane.id.label}</strong>
        <div className="timeline-track">
          {scene.spans.filter((span) => span.lane === lane.id.key).map((span) => <div
            key={span.id}
            className={`timeline-span ${span.classes.join(" ")}`}
            style={{ left: `${(span.start / maximum) * 100}%`, width: `${Math.max(2, ((span.end - span.start) / maximum) * 100)}%` }}
          >{span.label}</div>)}
          {scene.cursor !== null && scene.cursor !== undefined && <i className="timeline-cursor" style={{ left: `${(scene.cursor / maximum) * 100}%` }} aria-label={`Current time ${scene.cursor}`} />}
        </div>
      </div>)}
    </div>
  );
}
