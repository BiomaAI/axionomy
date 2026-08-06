import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import "@xyflow/react/dist/style.css";
import logoDark from "../../assets/axionomy-logo-dark.webp";
import logoLight from "../../assets/axionomy-logo-light.webp";
import {
  cancelRun,
  createRun,
  fetchArtifact,
  fetchProblems,
  fetchStaticArtifact,
  fetchStaticProblems,
  pauseRun,
  resumeRun,
  type ExchangeFrame,
  type RunArtifact,
  type RunSummary,
  type StudioEvent,
  type ViewDocument,
  type ViewSnapshot,
} from "./api";
import { SceneView } from "./SceneView";
import {
  Accounts,
  ModelExplorer,
  Observations,
  PanelHeading,
  ProposalInspector,
  Telemetry,
  Transition,
  useSnapshot,
} from "./Inspectors";

const ParetoChart = lazy(() => import("./ParetoChart"));
const eventKinds: StudioEvent["kind"][] = ["run_started", "progress", "frame_appended", "artifact_published", "run_paused", "run_resumed", "run_completed", "run_cancelled", "run_failed"];

export function App() {
  const catalog = useQuery({
    queryKey: ["problems"],
    retry: false,
    queryFn: async () => {
      try { return { problems: await fetchProblems(), connected: true }; }
      catch { return { problems: await fetchStaticProblems(), connected: false }; }
    },
  });
  const [problemKey, setProblemKey] = useState("maze");
  const [strategyKey, setStrategyKey] = useState("a_star");
  const [seed, setSeed] = useState(17);
  const [budget, setBudget] = useState(128);
  const [artifact, setArtifact] = useState<RunArtifact>();
  const [documentId, setDocumentId] = useState<string>();
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [run, setRun] = useState<RunSummary>();
  const [events, setEvents] = useState<StudioEvent[]>([]);
  const [error, setError] = useState<string>();
  const eventSource = useRef<EventSource | null>(null);

  const problem = catalog.data?.problems.find((candidate) => candidate.key === problemKey);
  const document = artifact?.documents.find((candidate) => candidate.id === documentId) ?? artifact?.documents.find((candidate) => candidate.id === artifact.selected_document_id) ?? artifact?.documents[0];
  const snapshot = useSnapshot(document, position);
  const previous = useSnapshot(document, Math.max(0, position - 1));
  const frame = position > 0 ? document?.frames[position - 1] : undefined;
  const lastEvent = events.at(-1);

  useEffect(() => () => eventSource.current?.close(), []);

  useEffect(() => {
    if (!problem) return;
    setStrategyKey(problem.default_strategy);
    setError(undefined);
    fetchStaticArtifact(problem.key).then((next) => {
      setArtifact(next);
      setDocumentId(next.selected_document_id);
    }).catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, [problem?.key]);

  useEffect(() => { setPosition(0); setPlaying(false); }, [document?.id]);

  useEffect(() => {
    if (!playing || !document) return;
    const timer = window.setInterval(() => setPosition((current) => {
      if (current >= document.frames.length) { setPlaying(false); return current; }
      return current + 1;
    }), 650);
    return () => window.clearInterval(timer);
  }, [playing, document]);

  const start = async () => {
    setError(undefined); setEvents([]); eventSource.current?.close();
    try {
      const created = await createRun({ problem: problemKey, strategy: strategyKey, seed, budget });
      setRun(created);
      const source = new EventSource(`/api/runs/${created.id}/events`);
      eventSource.current = source;
      for (const kind of eventKinds) source.addEventListener(kind, async (message) => {
        const event = JSON.parse((message as MessageEvent<string>).data) as StudioEvent;
        setEvents((current) => current.some((known) => known.sequence === event.sequence) ? current : [...current, event]);
        if (event.kind === "run_completed") {
          source.close();
          const next = await fetchArtifact(event.artifact_id);
          setArtifact(next); setDocumentId(event.document_id); setRun(undefined);
        } else if (event.kind === "run_cancelled" || event.kind === "run_failed") {
          source.close(); setRun(undefined);
          if (event.kind === "run_failed") setError(event.message);
        }
      });
      source.onerror = () => { if (source.readyState !== EventSource.CLOSED) setError("event stream disconnected; reconnecting remains safe by sequence"); };
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); setRun(undefined); }
  };

  const pause = async () => { if (run) setRun(await pauseRun(run.id)); };
  const resume = async () => { if (run) setRun(await resumeRun(run.id)); };
  const cancel = async () => { if (run) { await cancelRun(run.id); setRun(undefined); } };

  const loadFile = async (file?: File) => {
    if (!file) return;
    try {
      const next = JSON.parse(await file.text()) as RunArtifact;
      if (!Array.isArray(next.documents) || !next.selected_document_id) throw new Error("missing artifact documents");
      setArtifact(next); setProblemKey(next.problem.key); setStrategyKey(next.request.strategy ?? next.problem.default_strategy); setDocumentId(next.selected_document_id); setError(undefined);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "the selected file is not a RunArtifact"); }
  };

  const selectPareto = (values: string[]) => {
    const match = artifact?.documents.find((candidate) => candidate.objectives.slice(0, values.length).every((objective, index) => objective.value === values[index]));
    if (match) setDocumentId(match.id);
  };

  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><picture><source media="(prefers-color-scheme: light)" srcSet={logoLight} /><img src={logoDark} alt="Axionomy" /></picture><div><div className="brand-name">Studio</div><div className="brand-subtitle">Economic reasoning workbench</div></div></div>
      <div className="connection"><span className={`status-dot ${catalog.data?.connected ? "online" : "static"}`} />{catalog.data?.connected ? "engine connected" : "static artifacts"}</div>
    </header>

    <section className="command-bar" aria-label="Problem controls">
      <label className="field problem-field"><span>Canonical problem</span><select value={problemKey} onChange={(event) => setProblemKey(event.target.value)} disabled={Boolean(run)}>{catalog.data?.problems.map((candidate) => <option value={candidate.key} key={candidate.key}>{candidate.title}</option>)}</select></label>
      <label className="field strategy-field"><span>Strategy</span><select value={strategyKey} onChange={(event) => { const key = event.target.value; setStrategyKey(key); const match = artifact?.documents.find((candidate) => candidate.id === `${problemKey}:${key}`); if (match) setDocumentId(match.id); }} disabled={Boolean(run)}>{problem?.strategies.map((strategy) => <option value={strategy.key} key={strategy.key}>{strategy.label}</option>)}</select></label>
      <label className="field numeric-field"><span>Seed</span><input type="number" min="0" value={seed} onChange={(event) => setSeed(Number(event.target.value))} /></label>
      <label className="field numeric-field"><span>Budget</span><input type="number" min="1" value={budget} onChange={(event) => setBudget(Math.max(1, Number(event.target.value)))} /></label>
      <button className="primary" onClick={start} disabled={Boolean(run) || !catalog.data?.connected}>Run</button>
      {run?.status === "running" && <button onClick={pause}>Pause</button>}
      {run?.status === "paused" && <button onClick={resume}>Resume</button>}
      {run && <button className="danger" onClick={cancel}>Cancel</button>}
      <label className="file-button">Load artifact<input type="file" accept="application/json,.json" onChange={(event) => loadFile(event.target.files?.[0])} /></label>
      <div className="run-message">{lastEvent ? eventMessage(lastEvent) : catalog.data?.connected ? "CLI, HTTP, MCP, and Studio share this artifact contract." : "Browsing deterministic Rust-generated artifacts."}</div>
    </section>

    {problem && <section className="problem-context"><div><span>{problem.family.replaceAll("_", " ")}</span><p>{problem.summary}</p></div><div>{problem.capabilities.map((capability) => <span key={capability}>{capability.replaceAll("_", " ")}</span>)}</div></section>}
    {error && <div className="error-banner" role="alert">{error}</div>}

    {artifact && document && snapshot ? <main>
      <section className="alternative-bar"><div><span>Artifact alternatives</span><strong>{artifact.documents.length} replayable outcomes</strong></div><div role="tablist">{artifact.documents.map((candidate) => <button key={candidate.id} role="tab" aria-selected={candidate.id === document.id} onClick={() => setDocumentId(candidate.id)}>{candidate.title.replace(`${artifact.problem.title} · `, "")}</button>)}</div></section>
      <section className="document-heading"><div><span className="eyebrow">{document.source.label} · {document.id}</span><h1>{document.title}</h1><p>{document.description}</p></div><div className="objective-pills">{document.objectives.map((objective) => <div className="objective" key={objective.key}><span>{objective.label}</span><strong>{objective.value}</strong><small>{objective.direction}</small></div>)}</div></section>
      <PlaybackControls position={position} count={document.frames.length} playing={playing} onPosition={setPosition} onPlaying={setPlaying} frame={frame} />

      <section className="workspace-grid">
        <div className="panel world-panel"><PanelHeading kicker="Derived projection" title={snapshot.scene?.title ?? "Economic world"} aside={snapshot.scene?.kind} /><SceneView scene={snapshot.scene} /></div>
        <div className="panel accounts-panel"><PanelHeading kicker="Authoritative state" title="Accounts & assets" aside={`${snapshot.accounts.length} accounts`} /><Accounts snapshot={snapshot} previous={previous} /></div>
      </section>

      <section className="evidence-grid">
        <div className="panel transition-panel"><PanelHeading kicker="Atomic transition" title={frame?.exchange.rate.label ?? "Initial snapshot"} /><Transition frame={frame} /></div>
        <div className="panel proposal-panel"><PanelHeading kicker="Distance to feasibility" title="Assessed rejected proposals" aside={`${document.proposals.length} candidates`} /><ProposalInspector proposals={document.proposals} /></div>
        <div className="panel analysis-panel"><PanelHeading kicker="Decision surface" title={document.pareto_fronts[0]?.title ?? "Search evidence"} /><Suspense fallback={<div className="empty-state">Loading analysis…</div>}><ParetoChart document={document} onSelect={selectPareto} /></Suspense><Telemetry document={document} /></div>
      </section>

      <section className="definition-grid">
        <div className="panel model-panel"><PanelHeading kicker="Closed model" title="Rates, roles, goals & invariants" aside={`${document.model?.rates.length ?? 0} rates`} /><ModelExplorer document={document} /></div>
        <div className="panel observation-panel"><PanelHeading kicker="Information boundary" title="Actor-relative observations" aside={`${document.observations.length} views`} /><Observations document={document} /></div>
      </section>
    </main> : <div className="loading">Loading replay-derived problem artifact…</div>}

    <footer>Scenes explain. Accounts, assets, rates, bindings, assessment, and replay prove.</footer>
  </div>;
}

function PlaybackControls({ position, count, playing, onPosition, onPlaying, frame }: { position: number; count: number; playing: boolean; onPosition: (position: number) => void; onPlaying: (playing: boolean) => void; frame?: ExchangeFrame }) {
  return <section className="playback"><div className="transport"><button aria-label="Previous exchange" onClick={() => onPosition(Math.max(0, position - 1))}>←</button><button className="play" aria-label={playing ? "Pause" : "Play"} onClick={() => onPlaying(!playing)}>{playing ? "Ⅱ" : "▶"}</button><button aria-label="Next exchange" onClick={() => onPosition(Math.min(count, position + 1))}>→</button></div><div className="scrubber"><div className="scrubber-label"><span>{position === 0 ? "Initial state" : frame?.exchange.rate.label}</span><strong>{position} / {count}</strong></div><input aria-label="Trace position" type="range" min="0" max={count} value={position} onChange={(event) => onPosition(Number(event.target.value))} /></div><div className="replay-proof"><span>✓</span> replay verified</div></section>;
}

function eventMessage(event: StudioEvent): string {
  switch (event.kind) {
    case "run_started": return `Started ${event.problem} · ${event.strategy}`;
    case "progress": return event.message;
    case "frame_appended": return `Published replay frame ${event.frame_index + 1}`;
    case "artifact_published": return `Published ${event.documents} alternatives`;
    case "run_paused": return "Run paused";
    case "run_resumed": return "Run resumed";
    case "run_completed": return "Artifact completed and replay verified";
    case "run_cancelled": return "Run cancelled by caller";
    case "run_failed": return event.message;
  }
}

export function currentSnapshot(document: ViewDocument | undefined, position: number): ViewSnapshot | undefined {
  if (!document) return undefined;
  return position === 0 ? document.initial : document.frames[position - 1]?.after;
}
