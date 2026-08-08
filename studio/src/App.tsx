import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import "@xyflow/react/dist/style.css";
import logoDark from "../../assets/axionomy-logo-dark.webp";
import logoLight from "../../assets/axionomy-logo-light.webp";
import {
  type ExchangeFrame,
  type RunArtifact,
  type RunSummary,
  type SearchObservation,
  type StudioEvent,
  type ViewDocument,
  type ViewSnapshot,
} from "./api";
import { browserEngine } from "./browserEngine";
import { connectionLabel, nativeEngine, staticEngine, type EngineConnectivity, type EngineRunSubscription } from "./engine";
import { SceneIcon } from "./SceneIcon";
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
type CompletionNotice = {
  artifactId: string;
  problem: string;
  instance: string;
  strategy: string;
  seed: number;
  budget: number;
  durationMs: number;
  documents: number;
};

export function App() {
  const nativeProbeEnabled = import.meta.env.VITE_AXIONOMY_ENGINE !== "browser" && !window.location.hostname.endsWith(".github.io");
  const nativeHealth = useQuery({
    queryKey: ["native-engine-health"],
    queryFn: () => nativeEngine.health(),
    refetchInterval: 3_000,
    refetchIntervalInBackground: true,
    retry: false,
    enabled: nativeProbeEnabled,
  });
  const nativeConnected = nativeHealth.data === true;
  const browserHealth = useQuery({
    queryKey: ["browser-engine-health"],
    queryFn: () => browserEngine.health(),
    retry: false,
    enabled: !nativeConnected,
  });
  const browserConnected = browserHealth.data === true;
  const engine = nativeConnected ? nativeEngine : browserConnected ? browserEngine : staticEngine;
  const connectivity: EngineConnectivity = engine.kind === "static"
    ? nativeHealth.isPending || browserHealth.isPending ? "checking" : "static"
    : "connected";
  const catalog = useQuery({
    queryKey: ["problems", engine.kind],
    retry: false,
    queryFn: () => engine.catalog(),
  });
  const [problemKey, setProblemKey] = useState("maze");
  const [instanceKey, setInstanceKey] = useState("showcase");
  const [strategyKey, setStrategyKey] = useState("a_star");
  const [seed, setSeed] = useState(17);
  const [budget, setBudget] = useState(128);
  const [artifact, setArtifact] = useState<RunArtifact>();
  const [documentId, setDocumentId] = useState<string>();
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [playbackDelay, setPlaybackDelay] = useState(650);
  const [viewMode, setViewMode] = useState<"solve" | "replay">("replay");
  const [focusedAccount, setFocusedAccount] = useState<string>();
  const [run, setRun] = useState<RunSummary>();
  const [launching, setLaunching] = useState(false);
  const [events, setEvents] = useState<StudioEvent[]>([]);
  const [error, setError] = useState<string>();
  const [elapsedMs, setElapsedMs] = useState(0);
  const [completion, setCompletion] = useState<CompletionNotice>();
  const [freshArtifactId, setFreshArtifactId] = useState<string>();
  const eventSource = useRef<EngineRunSubscription | null>(null);
  const runStartedAt = useRef<number | undefined>(undefined);

  const problem = catalog.data?.find((candidate) => candidate.key === problemKey);
  const instance = problem?.instances.find((candidate) => candidate.key === instanceKey);
  const document = artifact?.documents.find((candidate) => candidate.id === documentId) ?? artifact?.documents.find((candidate) => candidate.id === artifact.selected_document_id) ?? artifact?.documents[0];
  const snapshot = useSnapshot(document, position);
  const previous = useSnapshot(document, Math.max(0, position - 1));
  const frame = position > 0 ? document?.frames[position - 1] : undefined;

  useEffect(() => () => eventSource.current?.close(), []);

  useEffect(() => {
    if (!launching && !run) return;
    const update = () => setElapsedMs(runStartedAt.current === undefined ? 0 : performance.now() - runStartedAt.current);
    update();
    const timer = window.setInterval(update, 100);
    return () => window.clearInterval(timer);
  }, [launching, run]);

  useEffect(() => {
    if (!problem) return;
    setInstanceKey(problem.default_instance);
    setStrategyKey(problem.default_strategy);
    setError(undefined);
    setFreshArtifactId(undefined);
    engine.defaultArtifact(problem.key).then((next) => {
      setArtifact(next);
      setDocumentId(next.selected_document_id);
    }).catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, [problem?.key, engine.kind]);

  useEffect(() => { setPosition(0); setPlaying(false); setFocusedAccount(undefined); }, [document?.id]);

  useEffect(() => {
    if (!playing || !document) return;
    const timer = window.setInterval(() => setPosition((current) => {
      if (current >= document.frames.length) { setPlaying(false); return current; }
      return current + 1;
    }), playbackDelay);
    return () => window.clearInterval(timer);
  }, [playing, document, playbackDelay]);

  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (!document || (event.target instanceof HTMLElement && event.target.matches("input, select, textarea, button"))) return;
      if (event.key === "ArrowLeft") setPosition((current) => Math.max(0, current - 1));
      else if (event.key === "ArrowRight") setPosition((current) => Math.min(document.frames.length, current + 1));
      else if (event.key === " ") { event.preventDefault(); setPlaying((current) => !current); }
      else return;
    };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, [document]);

  const start = async () => {
    const submittedProblem = problem?.title ?? problemKey;
    const submittedInstance = problem?.instances.find((candidate) => candidate.key === instanceKey)?.label ?? instanceKey;
    const submittedStrategy = problem?.strategies.find((candidate) => candidate.key === strategyKey)?.label ?? strategyKey;
    setError(undefined); setEvents([]); setCompletion(undefined); eventSource.current?.close();
    setViewMode("solve");
    runStartedAt.current = performance.now(); setElapsedMs(0); setLaunching(true);
    try {
      const subscription = await engine.start({ problem: problemKey, instance: instanceKey, strategy: strategyKey, seed, budget }, async (event) => {
        setEvents((current) => current.some((known) => known.sequence === event.sequence) ? current : [...current, event]);
        if (event.kind === "run_completed") {
          eventSource.current?.close();
          const next = await engine.artifact(event.artifact_id);
          const durationMs = runStartedAt.current === undefined ? 0 : performance.now() - runStartedAt.current;
          setArtifact(next); setDocumentId(event.document_id); setFreshArtifactId(event.artifact_id);
          setCompletion({ artifactId: event.artifact_id, problem: submittedProblem, instance: submittedInstance, strategy: submittedStrategy, seed, budget, durationMs, documents: next.documents.length });
          setElapsedMs(durationMs); setRun(undefined);
        } else if (event.kind === "run_cancelled" || event.kind === "run_failed") {
          eventSource.current?.close(); setRun(undefined);
          if (event.kind === "run_failed") setError(event.message);
        }
      }, setError);
      eventSource.current = subscription;
      setRun(subscription.summary); setLaunching(false);
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); setRun(undefined); setLaunching(false); }
  };

  const pause = async () => { if (run) setRun(await engine.pause(run.id)); };
  const resume = async () => { if (run) setRun(await engine.resume(run.id)); };
  const cancel = async () => { if (run) await engine.cancel(run.id); };

  const loadFile = async (file?: File) => {
    if (!file) return;
    try {
      const next = JSON.parse(await file.text()) as RunArtifact;
      if (!Array.isArray(next.documents) || !next.selected_document_id) throw new Error("missing artifact documents");
      setArtifact(next); setProblemKey(next.problem.key); setInstanceKey(next.instance.key); setStrategyKey(next.request.strategy ?? next.problem.default_strategy); setDocumentId(next.selected_document_id); setFreshArtifactId(undefined); setError(undefined);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "the selected file is not a RunArtifact"); }
  };

  const selectPareto = (values: string[]) => {
    const match = artifact?.documents.find((candidate) => candidate.objectives.slice(0, values.length).every((objective, index) => objective.value === values[index]));
    if (match) setDocumentId(match.id);
  };
  const active = launching || Boolean(run);

  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><picture><source media="(prefers-color-scheme: light)" srcSet={logoLight} /><img src={logoDark} alt="Axionomy" /></picture><div><div className="brand-name">Studio</div><div className="brand-subtitle">Economic reasoning workbench</div></div></div>
      <div className="connection" title={engine.kind === "native" ? "Verified by a live health check" : engine.kind === "browser" ? "Rust/WASM engine isolated in a Web Worker" : "No executable engine is available"}><span className={`status-dot ${engine.canRun ? "online" : "static"}`} />{connectionLabel(engine.kind, connectivity)}</div>
    </header>

    <section className="command-bar" aria-label="Problem controls">
      <label className="field problem-field"><span>Canonical problem</span><select value={problemKey} onChange={(event) => setProblemKey(event.target.value)} disabled={active}>{catalog.data?.map((candidate) => <option value={candidate.key} key={candidate.key}>{candidate.title}</option>)}</select></label>
      <label className="field instance-field"><span>Instance</span><select value={instanceKey} onChange={(event) => { setInstanceKey(event.target.value); setCompletion(undefined); }} disabled={active}>{problem?.instances.map((candidate) => <option value={candidate.key} key={candidate.key}>{candidate.label}</option>)}</select></label>
      <label className="field strategy-field"><span>Strategy</span><select value={strategyKey} onChange={(event) => { const key = event.target.value; setStrategyKey(key); const match = artifact?.documents.find((candidate) => candidate.id === `${problemKey}:${key}`); if (match) setDocumentId(match.id); }} disabled={active}>{problem?.strategies.map((strategy) => <option value={strategy.key} key={strategy.key}>{strategy.label}</option>)}</select></label>
      <label className="field numeric-field"><span>Seed</span><input type="number" min="0" value={seed} disabled={active} onChange={(event) => setSeed(Number(event.target.value))} /></label>
      <label className="field numeric-field"><span>Budget</span><input type="number" min="1" value={budget} disabled={active} onChange={(event) => setBudget(Math.max(1, Number(event.target.value)))} /></label>
      <button className="primary" onClick={start} disabled={active || !engine.canRun}>{launching ? "Starting…" : run?.status === "paused" ? "Paused" : run ? "Running…" : "Run"}</button>
      {engine.canPause && run?.status === "running" && <button onClick={pause}>Pause</button>}
      {engine.canPause && run?.status === "paused" && <button onClick={resume}>Resume</button>}
      {run && <button className="danger" onClick={cancel}>Cancel</button>}
      <label className="file-button">Load artifact<input type="file" accept="application/json,.json" onChange={(event) => loadFile(event.target.files?.[0])} /></label>
      <div className="run-message">{active ? "A new replay-verified artifact will replace the current view." : engine.canRun ? "CLI, HTTP, MCP, and Studio share this artifact contract." : "Native engine unavailable; deterministic Rust-generated artifacts remain playable."}</div>
    </section>

    {active && <RunActivity launching={launching} run={run} events={events} elapsedMs={elapsedMs} />}
    {completion && <CompletionBanner notice={completion} onDismiss={() => setCompletion(undefined)} />}

    {problem && <section className="problem-context"><div><span>{problem.family.replaceAll("_", " ")}</span><p>{problem.summary}<small>{instance?.label ?? artifact?.instance.label}: {instance?.description ?? artifact?.instance.description}</small></p></div><div>{problem.capabilities.map((capability) => <span key={capability}>{capability.replaceAll("_", " ")}</span>)}</div></section>}
    {error && <div className="error-banner" role="alert">{error}</div>}

    {artifact && document && snapshot ? <main className={freshArtifactId === artifact.id ? "fresh-artifact" : undefined}>
      <section className="alternative-bar"><div><span>Artifact alternatives</span><strong>{artifact.documents.length} replayable outcomes</strong></div><div role="tablist">{artifact.documents.map((candidate) => <button key={candidate.id} role="tab" aria-selected={candidate.id === document.id} onClick={() => setDocumentId(candidate.id)}>{candidate.title.replace(`${artifact.problem.title} · `, "")}</button>)}</div></section>
      <StrategyComparison artifact={artifact} selected={document.id} onSelect={setDocumentId} />
      <section className="document-heading"><div><span className="eyebrow">{artifact.instance.label} · {document.source.label} · {document.id}{freshArtifactId === artifact.id ? " · newly computed" : ""}</span><h1>{document.title}</h1><p>{document.description}</p></div><div className="objective-pills">{document.objectives.map((objective) => <div className="objective" key={objective.key}><span>{objective.label}</span><strong>{objective.value}</strong><small>{objective.direction}</small></div>)}</div></section>
      <div className="view-tabs" role="tablist" aria-label="Studio evidence mode"><button type="button" role="tab" aria-selected={viewMode === "solve"} onClick={() => setViewMode("solve")}>Solve evidence <small>{active ? "live" : document.solve_observations.length}</small></button><button type="button" role="tab" aria-selected={viewMode === "replay"} onClick={() => setViewMode("replay")}>Verified replay <small>{document.frames.length}</small></button></div>
      {viewMode === "solve" ? <SolveWorkspace observations={active ? events.flatMap((event) => event.kind === "search_observation" ? [event.observation] : []) : document.solve_observations} active={active} /> : <>
      <PlaybackControls position={position} count={document.frames.length} playing={playing} delay={playbackDelay} onDelay={setPlaybackDelay} onPosition={setPosition} onPlaying={(next) => { if (next && position >= document.frames.length) setPosition(0); setPlaying(next); }} frame={frame} />

      <section className="workspace-grid">
        <div className="panel world-panel"><PanelHeading kicker="Derived projection" title={snapshot.scene?.title ?? "Economic world"} aside={snapshot.scene?.surface.kind} /><SceneView scene={snapshot.scene} onAccount={setFocusedAccount} /></div>
        <div className="panel accounts-panel"><PanelHeading kicker="Authoritative state" title="Accounts & assets" aside={focusedAccount ? "linked from scene" : `${snapshot.accounts.length} accounts`} /><Accounts snapshot={snapshot} previous={previous} focus={focusedAccount} /></div>
      </section>

      <section className="evidence-grid">
        <div className="panel transition-panel"><PanelHeading kicker="Atomic transition" title={frame?.exchange.rate.label ?? "Initial snapshot"} /><Transition frame={frame} /></div>
        <div className="panel proposal-panel"><PanelHeading kicker="Constraint probes" title="Expected rejection proofs" aside={`${document.proposals.length} ${document.proposals.length === 1 ? "proof" : "proofs"}`} /><ProposalInspector proposals={document.proposals} /></div>
        <div className="panel analysis-panel"><PanelHeading kicker="Decision surface" title={document.pareto_fronts[0]?.title ?? "Search evidence"} /><Suspense fallback={<div className="empty-state">Loading analysis…</div>}><ParetoChart document={document} onSelect={selectPareto} /></Suspense><Telemetry document={document} /></div>
      </section>

      <section className="definition-grid">
        <div className="panel model-panel"><PanelHeading kicker="Closed model" title="Rates, roles, goals & invariants" aside={`${document.model?.rates.length ?? 0} rates`} /><ModelExplorer document={document} /></div>
        <div className="panel observation-panel"><PanelHeading kicker="Information boundary" title="Actor-relative observations" aside={`${document.observations.length} views`} /><Observations document={document} /></div>
      </section>
      </>}
    </main> : <div className="loading">Loading replay-derived problem artifact…</div>}

    <footer>Scenes explain. Accounts, assets, rates, bindings, assessment, and replay prove.</footer>
  </div>;
}

function RunActivity({ launching, run, events, elapsedMs }: { launching: boolean; run?: RunSummary; events: StudioEvent[]; elapsedMs: number }) {
  const latest = events.at(-1);
  const progress = [...events].reverse().find((event): event is Extract<StudioEvent, { kind: "progress" }> => event.kind === "progress");
  const completed = progress?.completed ?? run?.completed ?? 0;
  const total = progress?.total ?? run?.total ?? undefined;
  const status = launching ? "Starting engine run" : run?.status === "paused" ? "Run paused" : "Running computation";
  const message = latest ? eventMessage(latest) : "Submitting reproducible run request…";
  return <section className={`run-activity ${run?.status === "paused" ? "paused" : ""}`} role="status" aria-live="polite">
    <div className="run-activity-heading"><span className="spinner" aria-hidden="true" /><div><strong>{status}</strong><span>{message}</span></div></div>
    <div className="run-activity-progress">
      {total !== undefined && total > 0 ? <progress max={total} value={Math.min(completed, total)} aria-label="Run phase progress" /> : <div className="indeterminate-progress" />}
      <span>{total !== undefined ? `${completed} / ${total}` : "waiting for first checkpoint"}</span>
    </div>
    <div className="run-activity-meta"><span>{formatDuration(elapsedMs)} elapsed</span>{run && <><span>{run.request.instance ?? "default"} instance</span><span>seed {run.request.seed}</span><span>budget {run.request.budget}</span><code>{run.id}</code></>}</div>
  </section>;
}

function CompletionBanner({ notice, onDismiss }: { notice: CompletionNotice; onDismiss: () => void }) {
  return <section className="completion-banner" role="status">
    <div className="completion-mark" aria-hidden="true">✓</div>
    <div><strong>New artifact computed and loaded</strong><span>{notice.problem} · {notice.instance} · {notice.strategy} · seed {notice.seed} · budget {notice.budget}</span><small>{notice.documents} replayable {notice.documents === 1 ? "outcome" : "outcomes"} · completed in {formatDuration(notice.durationMs)} · <code>{notice.artifactId}</code></small></div>
    <button type="button" onClick={onDismiss}>Dismiss</button>
  </section>;
}

function SolveWorkspace({ observations, active }: { observations: SearchObservation[]; active: boolean }) {
  const latest = observations.at(-1);
  const kinds = [...new Set(observations.map((observation) => observation.kind))];
  return <section className={`solve-workspace ${active ? "live" : "retained"}`} aria-label="Solver evidence">
    <header><div><span className={active ? "spinner" : "solve-complete"} aria-hidden="true">{active ? "" : "✓"}</span><div><strong>{active ? "Live solver observations" : "Retained solver observations"}</strong><small>{active ? "Bounded updates from cooperative search checkpoints" : "Saved in the portable artifact for offline inspection"}</small></div></div><span>{observations.length} observations</span></header>
    {latest && <div className="solve-summary"><div><span>Current phase</span><strong>{latest.phase.replaceAll("_", " ")}</strong></div><div><span>Algorithm</span><strong>{latest.algorithm.replaceAll("_", " ")}</strong></div><div><span>Progress</span><strong>{latest.completed} / {latest.total}</strong></div><div><span>Evidence</span><strong>{kinds.join(" · ").replaceAll("_", " ")}</strong></div></div>}
    {latest && latest.total > 0 && <progress max={latest.total} value={Math.min(latest.completed, latest.total)} aria-label="Solver observation progress" />}
    <div className="solve-stream">{observations.length === 0 ? <div className="empty-state">Waiting for the first solver checkpoint…</div> : observations.map((observation) => <article key={`${observation.sequence}:${observation.phase}`} className={`observation-${observation.kind}`}><div className="solve-observation-icon"><SceneIcon glyph={observationGlyph(observation.kind)} size={20} /></div><div><strong>{observation.label}</strong><span>{observation.phase.replaceAll("_", " ")} · {observation.completed} / {observation.total}</span></div><div>{observation.metrics.map((metric) => <span key={metric.key}><small>{metric.label}</small><b>{metric.value}</b></span>)}</div></article>)}</div>
  </section>;
}

function observationGlyph(kind: SearchObservation["kind"]): Parameters<typeof SceneIcon>[0]["glyph"] {
  switch (kind) {
    case "tree": return "move";
    case "rollout": return "weather";
    case "frontier": return "constraint";
    case "belief": return "information";
    case "candidate": return "token";
    case "incumbent": return "goal";
    case "prune": return "hazard";
    case "artifact": return "package";
    case "phase": return "clock";
  }
}

function StrategyComparison({ artifact, selected, onSelect }: { artifact: RunArtifact; selected: string; onSelect: (id: string) => void }) {
  return <section className="strategy-comparison" aria-label="Outcome comparison">
    <div className="comparison-heading"><span>Outcome comparison</span><strong>Strategies and tradeoffs on one evidence surface</strong></div>
    <div className="comparison-scroll"><table><thead><tr><th>Outcome</th><th>Result</th><th>Trace</th><th>Search evidence</th></tr></thead><tbody>{artifact.documents.map((candidate) => {
      const series = candidate.telemetry.find((entry) => entry.algorithm !== "Model size");
      const work = series ? [...series.points].reverse().find((point) => ["generated", "expanded", "iteration", "sample"].includes(point.kind)) : undefined;
      return <tr key={candidate.id} className={candidate.id === selected ? "selected" : undefined}>
        <td><button type="button" onClick={() => onSelect(candidate.id)}>{candidate.title.replace(`${artifact.problem.title} · `, "")}</button></td>
        <td>{candidate.objectives.length > 0 ? candidate.objectives.map((objective) => `${objective.label}: ${objective.value}`).join(" · ") : "Feasibility outcome"}</td>
        <td>{candidate.frames.length} atomic {candidate.frames.length === 1 ? "transition" : "transitions"}</td>
        <td>{series ? `${series.algorithm} · ${series.exact ? "exact" : "sampled"}${work ? ` · ${work.value} ${work.kind.replaceAll("_", " ")}` : ""}` : "Replay only"}</td>
      </tr>;
    })}</tbody></table></div>
  </section>;
}

function formatDuration(milliseconds: number): string {
  if (milliseconds < 1_000) return `${Math.max(0, Math.round(milliseconds))} ms`;
  const seconds = milliseconds / 1_000;
  return seconds < 10 ? `${seconds.toFixed(1)} s` : `${Math.round(seconds)} s`;
}

function PlaybackControls({ position, count, playing, delay, onDelay, onPosition, onPlaying, frame }: { position: number; count: number; playing: boolean; delay: number; onDelay: (delay: number) => void; onPosition: (position: number) => void; onPlaying: (playing: boolean) => void; frame?: ExchangeFrame }) {
  return <section className="playback"><div className="transport"><button aria-label="Previous exchange" onClick={() => onPosition(Math.max(0, position - 1))}>←</button><button className="play" aria-label={playing ? "Pause" : "Play"} onClick={() => onPlaying(!playing)}>{playing ? "Ⅱ" : "▶"}</button><button aria-label="Next exchange" onClick={() => onPosition(Math.min(count, position + 1))}>→</button></div><div className="scrubber"><div className="scrubber-label"><span>{position === 0 ? "Initial state" : frame?.cues[0]?.label ?? frame?.exchange.rate.label}</span><strong>{position} / {count}</strong></div><input aria-label="Trace position" type="range" min="0" max={count} value={position} onChange={(event) => onPosition(Number(event.target.value))} /></div><label className="playback-speed"><span>Speed</span><select aria-label="Playback speed" value={delay} onChange={(event) => onDelay(Number(event.target.value))}><option value={1600}>0.5×</option><option value={900}>0.75×</option><option value={650}>1×</option><option value={325}>2×</option><option value={160}>4×</option></select></label><div className="replay-proof"><span>✓</span> replay verified</div></section>;
}

function eventMessage(event: StudioEvent): string {
  switch (event.kind) {
    case "run_started": return `Started ${event.problem} · ${event.strategy}`;
    case "progress": return event.message;
    case "search_observation": return event.observation.label;
    case "frame_appended": return `Published replay frame ${event.frame_index + 1}`;
    case "artifact_published": return `Published ${event.documents} alternatives`;
    case "run_paused": return "Computation is waiting for resume.";
    case "run_resumed": return "Computation resumed from its last checkpoint.";
    case "run_completed": return "Artifact completed and replay verified";
    case "run_cancelled": return "Run cancelled by caller";
    case "run_failed": return event.message;
  }
}

export function currentSnapshot(document: ViewDocument | undefined, position: number): ViewSnapshot | undefined {
  if (!document) return undefined;
  return position === 0 ? document.initial : document.frames[position - 1]?.after;
}
