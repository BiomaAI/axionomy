import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Background,
  Controls,
  MarkerType,
  ReactFlow,
  type Edge,
  type Node,
} from "@xyflow/react";
import {
  cancelRun,
  createRun,
  fetchDocument,
  fetchExamples,
  fetchStaticDocument,
  type ExchangeFrame,
  type Scene,
  type StudioEvent,
  type ViewDocument,
  type ViewSnapshot,
} from "./api";

const eventKinds: StudioEvent["kind"][] = [
  "run_started",
  "progress",
  "frame_appended",
  "run_completed",
  "run_cancelled",
  "run_failed",
];

const ParetoChart = lazy(() => import("./ParetoChart"));

export function App() {
  const examples = useQuery({ queryKey: ["examples"], queryFn: fetchExamples });
  const [document, setDocument] = useState<ViewDocument>();
  const [selectedExample, setSelectedExample] = useState("maze_pareto_energy");
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [runId, setRunId] = useState<string>();
  const [events, setEvents] = useState<StudioEvent[]>([]);
  const [error, setError] = useState<string>();
  const eventSource = useRef<EventSource | null>(null);

  useEffect(() => {
    fetchStaticDocument().then(setDocument).catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => eventSource.current?.close();
  }, []);

  useEffect(() => {
    setPosition(0);
    setPlaying(false);
  }, [document?.id]);

  useEffect(() => {
    if (!playing || !document) return;
    const timer = window.setInterval(() => {
      setPosition((current) => {
        if (current >= document.frames.length) {
          setPlaying(false);
          return current;
        }
        return current + 1;
      });
    }, 850);
    return () => window.clearInterval(timer);
  }, [playing, document]);

  const start = async () => {
    setError(undefined);
    setEvents([]);
    eventSource.current?.close();
    try {
      const run = await createRun(selectedExample);
      setRunId(run.id);
      const source = new EventSource(`/api/runs/${run.id}/events`);
      eventSource.current = source;
      for (const kind of eventKinds) {
        source.addEventListener(kind, async (message) => {
          const event = JSON.parse((message as MessageEvent<string>).data) as StudioEvent;
          setEvents((current) => [...current, event]);
          if (event.kind === "run_completed") {
            source.close();
            setDocument(await fetchDocument(event.document_id));
            setRunId(undefined);
          } else if (event.kind === "run_cancelled" || event.kind === "run_failed") {
            source.close();
            setRunId(undefined);
            if (event.kind === "run_failed") setError(event.message);
          }
        });
      }
      source.onerror = () => {
        if (source.readyState === EventSource.CLOSED) return;
        setError("event stream disconnected");
      };
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setRunId(undefined);
    }
  };

  const cancel = async () => {
    if (!runId) return;
    try {
      await cancelRun(runId);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  const loadFile = async (file?: File) => {
    if (!file) return;
    try {
      setDocument(JSON.parse(await file.text()) as ViewDocument);
      setError(undefined);
    } catch {
      setError("the selected file is not valid JSON");
    }
  };

  const snapshot = useMemo(() => currentSnapshot(document, position), [document, position]);
  const previous = useMemo(
    () => currentSnapshot(document, Math.max(0, position - 1)),
    [document, position],
  );
  const frame = position > 0 ? document?.frames[position - 1] : undefined;
  const lastEvent = events.at(-1);

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true"><span>A</span></div>
          <div>
            <div className="brand-name">Axionomy <em>Studio</em></div>
            <div className="brand-subtitle">Economic trace observatory</div>
          </div>
        </div>
        <div className="connection">
          <span className={`status-dot ${examples.isSuccess ? "online" : "static"}`} />
          {examples.isSuccess ? "engine connected" : "static playback"}
        </div>
      </header>

      <section className="command-bar" aria-label="Run controls">
        <label className="field">
          <span>Encoded problem</span>
          <select
            value={selectedExample}
            onChange={(event) => setSelectedExample(event.target.value)}
            disabled={Boolean(runId)}
          >
            {(examples.data ?? [
              { key: "maze_pareto_energy", title: "Maze Pareto · least energy" },
            ]).map((example) => (
              <option key={example.key} value={example.key}>{example.title}</option>
            ))}
          </select>
        </label>
        <button className="primary" onClick={start} disabled={Boolean(runId) || !examples.isSuccess}>
          {runId ? "Running…" : "Run search"}
        </button>
        {runId && <button className="danger" onClick={cancel}>Cancel</button>}
        <label className="file-button">
          Load ViewDocument
          <input type="file" accept="application/json,.json" onChange={(event) => loadFile(event.target.files?.[0])} />
        </label>
        <div className="run-message">
          {lastEvent ? eventMessage(lastEvent) : "Portable JSON and live SSE use the same Rust-owned contract."}
        </div>
      </section>

      {error && <div className="error-banner" role="alert">{error}</div>}

      {document && snapshot ? (
        <main>
          <section className="document-heading">
            <div>
              <span className="eyebrow">{document.source.label}</span>
              <h1>{document.title}</h1>
              <p>{document.description}</p>
            </div>
            <div className="objective-pills">
              {document.objectives.map((objective) => (
                <div className="objective" key={objective.key}>
                  <span>{objective.label}</span>
                  <strong>{objective.value}</strong>
                  <small>{objective.direction}</small>
                </div>
              ))}
            </div>
          </section>

          <PlaybackControls
            position={position}
            count={document.frames.length}
            playing={playing}
            onPosition={setPosition}
            onPlaying={setPlaying}
            frame={frame}
          />

          <section className="workspace-grid">
            <div className="panel world-panel">
              <PanelHeading kicker="Derived projection" title={snapshot.scene ? sceneTitle(snapshot.scene) : "Economic world"} />
              <SceneView scene={snapshot.scene} />
            </div>
            <div className="panel accounts-panel">
              <PanelHeading kicker="Authoritative state" title="Accounts & assets" />
              <Accounts snapshot={snapshot} previous={previous} />
            </div>
            <div className="panel transition-panel">
              <PanelHeading kicker="Atomic transition" title={frame?.exchange.rate.label ?? "Initial snapshot"} />
              <Transition frame={frame} />
            </div>
            <div className="panel analysis-panel">
              <PanelHeading kicker="Decision surface" title={document.pareto_fronts[0]?.title ?? "Objectives"} />
              <Suspense fallback={<div className="empty-state">Loading analysis…</div>}>
                <ParetoChart document={document} />
              </Suspense>
            </div>
          </section>
        </main>
      ) : (
        <div className="loading">Deriving economic view…</div>
      )}

      <footer>
        Scenes are explanatory projections. Replay through the Axionomy core remains the proof.
      </footer>
    </div>
  );
}

function PlaybackControls({
  position,
  count,
  playing,
  onPosition,
  onPlaying,
  frame,
}: {
  position: number;
  count: number;
  playing: boolean;
  onPosition: (position: number) => void;
  onPlaying: (playing: boolean) => void;
  frame?: ExchangeFrame;
}) {
  return (
    <section className="timeline">
      <div className="transport">
        <button aria-label="Previous exchange" onClick={() => onPosition(Math.max(0, position - 1))}>←</button>
        <button className="play" aria-label={playing ? "Pause" : "Play"} onClick={() => onPlaying(!playing)}>
          {playing ? "Ⅱ" : "▶"}
        </button>
        <button aria-label="Next exchange" onClick={() => onPosition(Math.min(count, position + 1))}>→</button>
      </div>
      <div className="scrubber">
        <div className="scrubber-label">
          <span>{position === 0 ? "Initial state" : frame?.exchange.rate.label}</span>
          <strong>{position} / {count}</strong>
        </div>
        <input
          aria-label="Trace position"
          type="range"
          min="0"
          max={count}
          value={position}
          onChange={(event) => onPosition(Number(event.target.value))}
        />
      </div>
      <div className="replay-proof"><span>✓</span> replay verified</div>
    </section>
  );
}

function PanelHeading({ kicker, title }: { kicker: string; title: string }) {
  return <div className="panel-heading"><span>{kicker}</span><h2>{title}</h2></div>;
}

function currentSnapshot(document: ViewDocument | undefined, position: number): ViewSnapshot | undefined {
  if (!document) return undefined;
  return position === 0 ? document.initial : document.frames[position - 1]?.after;
}

function sceneTitle(scene: Scene): string {
  return scene.title;
}

function SceneView({ scene }: { scene?: Scene | null }) {
  if (!scene) return <div className="empty-state">No domain projection supplied; inspect the accounts directly.</div>;
  if (scene.kind === "grid") {
    return (
      <div className="grid-scene" style={{ gridTemplateColumns: `repeat(${scene.width}, 1fr)` }}>
        {scene.cells.map((cell) => <div key={`${cell.x}:${cell.y}`} className={cell.classes.join(" ")}>{cell.label}</div>)}
      </div>
    );
  }
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
    <div className="graph-scene">
      <ReactFlow nodes={nodes} edges={edges} fitView minZoom={0.5} maxZoom={1.8} nodesConnectable={false} elementsSelectable={false}>
        <Background gap={22} size={1} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}

function Accounts({ snapshot, previous }: { snapshot: ViewSnapshot; previous?: ViewSnapshot }) {
  return <div className="accounts-list">
    {snapshot.accounts.map((account) => (
      <article className="account-card" key={account.account.key}>
        <h3><span className="account-icon">{account.account.label.slice(0, 1)}</span>{account.account.label}</h3>
        <div className="balances">
          {account.balances.map((balance) => {
            const prior = previous?.accounts
              .find((candidate) => candidate.account.key === account.account.key)
              ?.balances.find((candidate) => candidate.asset.key === balance.asset.key)?.quantity;
            const changed = prior !== undefined && prior !== balance.quantity;
            const added = prior === undefined && snapshot.index > 0;
            return <div className={`balance ${changed || added ? "changed" : ""}`} key={balance.asset.key}>
              <span title={balance.asset.key}>{balance.asset.label}</span>
              <strong>{balance.quantity}</strong>
              {changed && <small>{prior} →</small>}
              {added && <small>new</small>}
            </div>;
          })}
          {account.balances.length === 0 && <div className="muted">No balances</div>}
        </div>
      </article>
    ))}
  </div>;
}

function Transition({ frame }: { frame?: ExchangeFrame }) {
  if (!frame) return <div className="empty-state">Move the scrubber to inspect an assessed and applied exchange.</div>;
  return <div className="transition">
    <div className="binding-row">
      {frame.exchange.bindings.map((binding) => (
        <span key={binding.role.key}><small>{binding.role.label}</small>{binding.account.label}</span>
      ))}
      <span><small>Units</small>{frame.exchange.units}</span>
    </div>
    <div className="assessment-ok"><span>✓</span> Assessment: {frame.assessment.status}</div>
    {frame.receipt.deltas.map((delta) => (
      <div className="delta" key={delta.account.key}>
        <h3>{delta.account.label}</h3>
        <DeltaGroup label="Consumed" kind="consumed" values={delta.consumed} />
        <DeltaGroup label="Produced" kind="produced" values={delta.produced} />
        <DeltaGroup label="Preserved" kind="preserved" values={delta.preserved} />
      </div>
    ))}
  </div>;
}

function DeltaGroup({ label, kind, values }: {
  label: string;
  kind: string;
  values: { asset: { key: string; label: string }; quantity: string }[];
}) {
  if (values.length === 0) return null;
  return <div className={`delta-group ${kind}`}><span>{label}</span><div>
    {values.map((value) => <b key={value.asset.key}>{value.asset.label} <em>×{value.quantity}</em></b>)}
  </div></div>;
}

function eventMessage(event: StudioEvent): string {
  switch (event.kind) {
    case "run_started": return `Started ${event.example}`;
    case "progress": return event.message;
    case "frame_appended": return `Replayed exchange ${event.frame_index + 1}`;
    case "run_completed": return "Run completed and replay verified";
    case "run_cancelled": return "Run cancelled by caller";
    case "run_failed": return event.message;
  }
}
