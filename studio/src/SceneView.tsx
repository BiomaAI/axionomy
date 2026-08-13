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
    case "grid": return <GridScene scene={scene} previousScene={previousScene} frame={frame} motion={motion} surface={scene.surface} onAccount={onAccount} />;
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
type StructureNodeData = { kind: "structure"; label: string; entity?: SceneEntity; states: SceneEntity[]; effect: EntityEffect; handles?: GraphHandle[]; focusAccount?: string };
type OccupantNodeData = { kind: "occupant"; entity: SceneEntity; effect: EntityEffect; entering: boolean; exiting: boolean; moving: boolean; focusAccount?: string };
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
  const state = view.states[0];
  const entity = state ?? view.entity;
  const metrics = view.states.flatMap((item) => item.metrics.map((metric) => `${item.id.label} · ${metric.label}: ${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`)).join(" · ");
  return <div title={metrics || undefined} className={`rich-node structure-content ${entity ? `tone-${entity.tone}` : ""}`}>
    {view.handles?.map((handle) => <Handle key={handle.id} id={handle.id} type={handle.type} position={HANDLE_POSITION[handle.side]} style={handleStyle(handle.side, handle.offset)} className="graph-node-handle" isConnectable={false} />)}
    <div className={effectClass(view.effect)}>
      <div className="rich-node-content">
        {entity && <SceneIcon glyph={entity.glyph} size={25} />}
        <span>{view.label}</span>
        {state && <small className="structure-state">{entityDetail(state)}</small>}
        {view.states.length > 1 && <small className="structure-state-count">+{view.states.length - 1} linked</small>}
      </div>
    </div>
  </div>;
}

function OccupantNode({ data }: NodeProps) {
  const view = data as OccupantNodeData;
  const metrics = view.entity.metrics.map((metric) => `${metric.label}: ${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`).join(" · ");
  return <div title={metrics || undefined} className={`rich-node tone-${view.entity.tone}`}>
    {view.entity.anchor.kind === "graph_node" && RELATION_SIDES.map((side) => <Handle key={side} id={`relation-target:${side}`} type="target" position={HANDLE_POSITION[side]} className="relation-handle" isConnectable={false} />)}
    <div className={`rich-node-lifecycle ${view.entering ? "is-entering" : ""} ${view.exiting ? "is-exiting" : ""}`}>
      <div className={effectClass(view.effect)}>
        <div className={`rich-node-content occupant-content ${view.moving ? "is-moving" : ""}`}>
          <SceneIcon glyph={view.entity.glyph} size={19} />
          <span>{view.entity.id.label}</span>
          {view.entity.status && <small>{view.entity.status.replaceAll("_", " ")}</small>}
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
        <small>{entityDetail(entity)}</small>
      </button>)}
      {view.entities.length > 4 && <div className="attachment-overflow">+{view.entities.length - 4} more</div>}
    </div>
  </div>;
}

const nodeTypes = { structure: StructureNode, occupant: OccupantNode, attachments: AttachmentNode };
const RELATION_SIDES: HandleSide[] = ["top", "right", "bottom", "left"];

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
  const structureById = new Map(scene.entities.filter((entity) => entity.role === "structure").map((entity) => [entity.id.key, entity]));
  const statesByNode = groupBy(scene.entities.filter((entity) => entity.role === "state" && entity.anchor.kind === "graph_node"), (entity) => entity.anchor.kind === "graph_node" ? entity.anchor.node : "");
  const occupied: GraphRect[] = surface.nodes.map((node) => ({ x: node.x ?? 0, y: node.y ?? 0, width: GRAPH_NODE_WIDTH, height: GRAPH_NODE_HEIGHT }));
  const relationEdges: Edge[] = [];
  const relationHandles = new Map<string, GraphHandle[]>();
  const nodes: Node[] = [];

  const attachments = groupBy(scene.entities.filter((entity) => entity.role === "attachment"), (entity) => anchorKey(entity.anchor));
  for (const [key, entities] of [...attachments.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    const anchor = entities[0]?.anchor;
    if (!anchor) continue;
    const height = attachmentHeight(entities.length);
    const point = semanticAttachmentPosition(anchor, surface, positions, occupied, ATTACHMENT_WIDTH, height);
    if (!point) continue;
    occupied.push({ ...point, width: ATTACHMENT_WIDTH, height });
    const nodeId = `attachments:${key}`;
    const label = attachmentLabel(anchor, surface, entities.length);
    nodes.push({ id: nodeId, type: "attachments", position: point, data: { kind: "attachments", label, entities, effects: Object.fromEntries(entities.map((entity) => [entity.id.key, composedEntityEffect(entity, previousById.get(entity.id.key), frame)])), focusAccount: entities.find((entity) => entity.account)?.account ?? undefined, onAccount } satisfies AttachmentNodeData, draggable: false, selectable: true, className: `attachment-group motion-${motion}` });
    if (anchor.kind === "graph_node") addRelation(anchor.node, nodeId, point, "attachment", relationHandles, relationEdges, positions);
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
  const occupantSlots = new Map<string, number>();
  for (const view of occupants) {
    const baseKey = anchorKey(view.entity.anchor);
    const slot = occupantSlots.get(baseKey) ?? 0;
    occupantSlots.set(baseKey, slot + 1);
    const point = semanticOccupantPosition(view.entity, slot, surface, positions);
    if (!point) continue;
    const nodeId = `entity:${view.entity.id.key}`;
    nodes.push({ id: nodeId, type: "occupant", position: point, data: { kind: "occupant", entity: view.entity, effect: composedEntityEffect(view.entity, previousById.get(view.entity.id.key), frame), entering: view.entering, exiting: view.exiting, moving: view.moving, focusAccount: view.entity.account ?? undefined } satisfies OccupantNodeData, draggable: false, selectable: true, className: `occupant-overlay motion-${motion}` });
    if (view.entity.anchor.kind === "graph_node") addRelation(view.entity.anchor.node, nodeId, point, "occupant", relationHandles, relationEdges, positions);
  }

  for (const node of surface.nodes) {
    const structure = structureById.get(node.id.key);
    const states = statesByNode.get(node.id.key) ?? [];
    const effects = [structure, ...states].filter((entity): entity is SceneEntity => Boolean(entity)).map((entity) => composedEntityEffect(entity, previousById.get(entity.id.key), frame));
    nodes.push({
      id: node.id.key,
      type: "structure",
      position: { x: node.x ?? 0, y: node.y ?? 0 },
      data: { kind: "structure", label: node.id.label, entity: structure, states, effect: strongestEffect(effects), handles: [...(handles.get(node.id.key) ?? []), ...(relationHandles.get(node.id.key) ?? [])], focusAccount: states.find((entity) => entity.account)?.account ?? structure?.account ?? undefined } satisfies StructureNodeData,
      className: ["structure-node", ...node.classes].join(" "),
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
      type: "smoothstep",
      sourceHandle: route?.sourceHandle,
      targetHandle: route?.targetHandle,
      label: edge.label,
      className: [...edge.classes, semantic ? `path-${semantic.status}` : ""].filter(Boolean).join(" "),
      animated: semantic?.status === "current" || semantic?.status === "candidate" || edge.classes.includes("selected") || edge.classes.includes("completed"),
      markerEnd: { type: MarkerType.ArrowClosed },
    };
  });
  const context = scene.entities.filter((entity) => entity.role === "context");
  return <div className={`graph-scene motion-${motion}`} data-motion={motion} ref={host} aria-label={scene.title}>
    <ReactFlow nodes={nodes} edges={[...edges, ...relationEdges]} nodeTypes={nodeTypes} fitView fitViewOptions={{ padding: 0.18 }} minZoom={0.25} maxZoom={2} nodesConnectable={false} nodesDraggable={false} elementsSelectable onInit={setFlow} onNodeClick={(_, node) => { const account = (node.data as SceneNodeData).focusAccount; if (account) onAccount?.(account); }}>
      <Background gap={22} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
    {context.length > 0 && <aside className="scene-context" aria-label="Scenario context"><header>Scenario context</header>{context.map((entity) => <button type="button" key={entity.id.key} className={`tone-${entity.tone}`} disabled={!entity.account} onClick={() => entity.account && onAccount?.(entity.account)}><SceneIcon glyph={entity.glyph} size={15} /><span><b>{entity.id.label}</b><small>{contextRelation(entity, surface)} · {entityDetail(entity)}</small></span></button>)}</aside>}
  </div>;
}

type GraphPoint = { x: number; y: number };
type GraphRect = GraphPoint & { width: number; height: number };
type GraphEdgeRoute = { source: string; target: string; sourceHandle: string; targetHandle: string; sourceSide: HandleSide; targetSide: HandleSide; offset: number };

const GRAPH_NODE_WIDTH = 88;
const GRAPH_NODE_HEIGHT = 72;
const OCCUPANT_WIDTH = 116;
const OCCUPANT_HEIGHT = 42;
const ATTACHMENT_WIDTH = 168;

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

function groupBy<T>(items: T[], key: (item: T) => string): Map<string, T[]> {
  const groups = new Map<string, T[]>();
  for (const item of items) groups.set(key(item), [...(groups.get(key(item)) ?? []), item]);
  return groups;
}

function attachmentHeight(count: number): number {
  return 30 + Math.min(count, 4) * 29 + (count > 4 ? 22 : 0);
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

function semanticOccupantPosition(entity: SceneEntity, slot: number, surface: GraphSurface, positions: Map<string, GraphPoint>): GraphPoint | undefined {
  const point = graphAnchorPoint(entity.anchor, surface, positions);
  if (!point) return undefined;
  if (entity.anchor.kind === "graph_edge") {
    const edgeId = entity.anchor.edge;
    const edge = surface.edges.find((candidate) => candidate.id === edgeId);
    const source = edge ? positions.get(edge.source) : undefined;
    const target = edge ? positions.get(edge.target) : undefined;
    const angle = source && target ? Math.atan2(target.y - source.y, target.x - source.x) - Math.PI / 2 : -Math.PI / 2;
    return { x: Math.round(point.x + Math.cos(angle) * 34 - OCCUPANT_WIDTH / 2), y: Math.round(point.y + Math.sin(angle) * 34 - OCCUPANT_HEIGHT / 2) };
  }
  const angle = -Math.PI / 2 + slot * Math.PI / 2;
  const radius = slot < 4 ? 100 : 135;
  return { x: Math.round(point.x + Math.cos(angle) * radius - OCCUPANT_WIDTH / 2), y: Math.round(point.y + Math.sin(angle) * radius - OCCUPANT_HEIGHT / 2) };
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
