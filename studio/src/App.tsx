import { lazy, Suspense, useEffect, useRef, useState, type CSSProperties } from "react";
import { useQuery } from "@tanstack/react-query";
import "@xyflow/react/dist/style.css";
import logoDark from "../../assets/axionomy-logo-dark.webp";
import logoLight from "../../assets/axionomy-logo-light.webp";
import {
  type ExchangeFrame,
  type LeaderboardView,
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
type StudioViewMode = "solve" | "replay";
type StudioUrlState = {
  problem?: string;
  instance?: string;
  strategy?: string;
  document?: string;
  view?: StudioViewMode;
  step?: number;
  leaderboard?: string;
  seed?: number;
  budget?: number;
};

function readUrlState(): StudioUrlState {
  const query = new URLSearchParams(window.location.search);
  const number = (key: string, minimum = 0) => {
    const value = Number(query.get(key));
    return Number.isSafeInteger(value) && value >= minimum ? value : undefined;
  };
  const view = query.get("view");
  return {
    problem: query.get("problem") ?? undefined,
    instance: query.get("instance") ?? undefined,
    strategy: query.get("strategy") ?? undefined,
    document: query.get("document") ?? undefined,
    view: view === "solve" || view === "replay" ? view : undefined,
    step: number("step"),
    leaderboard: query.get("leaderboard") ?? undefined,
    seed: number("seed"),
    budget: number("budget", 1),
  };
}

function writeUrl(state: Required<Pick<StudioUrlState, "problem" | "instance" | "strategy" | "view" | "step" | "seed" | "budget">> & Pick<StudioUrlState, "document" | "leaderboard">, mode: "push" | "replace") {
  const query = new URLSearchParams();
  query.set("problem", state.problem);
  query.set("instance", state.instance);
  query.set("strategy", state.strategy);
  if (state.document) query.set("document", state.document);
  query.set("view", state.view);
  query.set("step", String(state.step));
  if (state.leaderboard) query.set("leaderboard", state.leaderboard);
  query.set("seed", String(state.seed));
  query.set("budget", String(state.budget));
  window.history[mode === "push" ? "pushState" : "replaceState"]({}, "", `${window.location.pathname}?${query}${window.location.hash}`);
}

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
  const initialUrl = useRef(readUrlState()).current;
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
  const [problemKey, setProblemKey] = useState(initialUrl.problem ?? "maze");
  const [instanceKey, setInstanceKey] = useState(initialUrl.instance ?? "showcase");
  const [strategyKey, setStrategyKey] = useState(initialUrl.strategy ?? "a_star");
  const [seed, setSeed] = useState(initialUrl.seed ?? 17);
  const [budget, setBudget] = useState(initialUrl.budget ?? 128);
  const [artifact, setArtifact] = useState<RunArtifact>();
  const [documentId, setDocumentId] = useState<string | undefined>(initialUrl.document);
  const [position, setPosition] = useState(initialUrl.step ?? 0);
  const [playing, setPlaying] = useState(false);
  const [playbackDelay, setPlaybackDelay] = useState(650);
  const [viewMode, setViewMode] = useState<StudioViewMode>(initialUrl.view ?? "replay");
  const [leaderboardKey, setLeaderboardKey] = useState<string | undefined>(initialUrl.leaderboard);
  const [focusedAccount, setFocusedAccount] = useState<string>();
  const [run, setRun] = useState<RunSummary>();
  const [launching, setLaunching] = useState(false);
  const [events, setEvents] = useState<StudioEvent[]>([]);
  const [error, setError] = useState<string>();
  const [elapsedMs, setElapsedMs] = useState(0);
  const [completion, setCompletion] = useState<CompletionNotice>();
  const [freshArtifactId, setFreshArtifactId] = useState<string>();
  const [linkNotice, setLinkNotice] = useState<string>();
  const [copied, setCopied] = useState(false);
  const eventSource = useRef<EngineRunSubscription | null>(null);
  const runStartedAt = useRef<number | undefined>(undefined);
  const pendingUrlStep = useRef<number | undefined>(initialUrl.step);

  const problem = catalog.data?.find((candidate) => candidate.key === problemKey);
  const instance = problem?.instances.find((candidate) => candidate.key === instanceKey);
  const document = artifact?.documents.find((candidate) => candidate.id === documentId) ?? artifact?.documents.find((candidate) => candidate.id === artifact.selected_document_id) ?? artifact?.documents[0];
  const snapshot = useSnapshot(document, position);
  const previous = useSnapshot(document, Math.max(0, position - 1));
  const frame = position > 0 ? document?.frames[position - 1] : undefined;

  const urlState = (overrides: StudioUrlState = {}) => ({
    problem: overrides.problem ?? problemKey,
    instance: overrides.instance ?? instanceKey,
    strategy: overrides.strategy ?? strategyKey,
    document: Object.hasOwn(overrides, "document") ? overrides.document : documentId,
    view: overrides.view ?? viewMode,
    step: overrides.step ?? position,
    leaderboard: Object.hasOwn(overrides, "leaderboard") ? overrides.leaderboard : leaderboardKey,
    seed: overrides.seed ?? seed,
    budget: overrides.budget ?? budget,
  });

  useEffect(() => () => eventSource.current?.close(), []);

  useEffect(() => {
    if (!launching && !run) return;
    const update = () => setElapsedMs(runStartedAt.current === undefined ? 0 : performance.now() - runStartedAt.current);
    update();
    const timer = window.setInterval(update, 100);
    return () => window.clearInterval(timer);
  }, [launching, run]);

  useEffect(() => {
    if (!catalog.data || problem) return;
    const fallback = catalog.data.find((candidate) => candidate.key === "maze") ?? catalog.data[0];
    if (!fallback) return;
    setLinkNotice(`The linked problem “${problemKey}” does not exist; opened ${fallback.title} instead.`);
    setProblemKey(fallback.key);
    setInstanceKey(fallback.default_instance);
    setStrategyKey(fallback.default_strategy);
    setDocumentId(undefined);
  }, [catalog.data, problem, problemKey]);

  useEffect(() => {
    if (!problem) return;
    const requestedInstance = problem.instances.some((candidate) => candidate.key === instanceKey) ? instanceKey : problem.default_instance;
    const requestedStrategy = problem.strategies.some((candidate) => candidate.key === strategyKey) ? strategyKey : problem.default_strategy;
    if (requestedInstance !== instanceKey || requestedStrategy !== strategyKey) {
      setLinkNotice(`Some linked options were not valid for ${problem.title}; Studio selected its defaults.`);
    }
    setInstanceKey(requestedInstance);
    setStrategyKey(requestedStrategy);
    setError(undefined);
    setFreshArtifactId(undefined);
    engine.defaultArtifact(problem.key).then((next) => {
      const requestedDocument = next.documents.find((candidate) => candidate.id === documentId)?.id
        ?? next.documents.find((candidate) => candidate.id === `${problem.key}:${requestedStrategy}`)?.id
        ?? next.selected_document_id;
      setArtifact(next);
      setDocumentId(requestedDocument);
    }).catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, [problem?.key, engine.kind]);

  useEffect(() => {
    if (!document) return;
    setPosition(Math.min(pendingUrlStep.current ?? 0, document.frames.length));
    pendingUrlStep.current = undefined;
    setPlaying(false);
    setFocusedAccount(undefined);
  }, [document?.id]);

  useEffect(() => {
    if (!artifact || artifact.problem.key !== problemKey || !documentId) return;
    if (!artifact.documents.some((candidate) => candidate.id === documentId)) {
      setLinkNotice(`The linked outcome “${documentId}” does not exist in ${artifact.problem.title}; opened the default outcome instead.`);
      setDocumentId(artifact.selected_document_id);
    }
  }, [artifact?.id, documentId, problemKey]);

  useEffect(() => {
    if (!snapshot) return;
    if (!snapshot.leaderboards.length) {
      setLeaderboardKey(undefined);
      return;
    }
    if (!snapshot.leaderboards.some((leaderboard) => leaderboard.key === leaderboardKey)) {
      setLeaderboardKey(snapshot.leaderboards[0].key);
    }
  }, [snapshot?.index, document?.id]);

  useEffect(() => {
    if (!problem || !problem.instances.some((candidate) => candidate.key === instanceKey) || !problem.strategies.some((candidate) => candidate.key === strategyKey)) return;
    writeUrl(urlState(), "replace");
  }, [problemKey, instanceKey, strategyKey, documentId, viewMode, position, leaderboardKey, seed, budget]);

  useEffect(() => {
    const restore = () => {
      const linked = readUrlState();
      pendingUrlStep.current = linked.step;
      if (linked.problem) setProblemKey(linked.problem);
      if (linked.instance) setInstanceKey(linked.instance);
      if (linked.strategy) setStrategyKey(linked.strategy);
      setDocumentId(linked.document);
      if (linked.view) setViewMode(linked.view);
      setLeaderboardKey(linked.leaderboard);
      if (linked.seed !== undefined) setSeed(linked.seed);
      if (linked.budget !== undefined) setBudget(linked.budget);
      setPosition(linked.step ?? 0);
    };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);

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
  const modelSize = document?.telemetry.find((entry) => entry.algorithm === "Model size");
  const modelCounts = modelSize?.points
    .filter((point) => point.kind === "accounts" || point.kind === "rates")
    .map((point) => `${point.value} ${point.label}`)
    .join(" · ");
  const selectProblem = (key: string) => {
    const next = catalog.data?.find((candidate) => candidate.key === key);
    if (!next) return;
    const nextState = urlState({ problem: key, instance: next.default_instance, strategy: next.default_strategy, document: undefined, step: 0, leaderboard: undefined });
    writeUrl(nextState, "push");
    pendingUrlStep.current = 0;
    setProblemKey(key);
    setInstanceKey(next.default_instance);
    setStrategyKey(next.default_strategy);
    setDocumentId(undefined);
    setLeaderboardKey(undefined);
    setLinkNotice(undefined);
  };
  const selectInstance = (key: string) => {
    writeUrl(urlState({ instance: key, step: 0 }), "push");
    setInstanceKey(key); setPosition(0); setCompletion(undefined);
  };
  const selectStrategy = (key: string) => {
    const match = artifact?.documents.find((candidate) => candidate.id === `${problemKey}:${key}`);
    writeUrl(urlState({ strategy: key, document: match?.id, step: 0 }), "push");
    setStrategyKey(key); setPosition(0); if (match) setDocumentId(match.id);
  };
  const selectDocument = (id: string) => {
    writeUrl(urlState({ document: id, step: 0 }), "push");
    setDocumentId(id); setPosition(0);
  };
  const selectView = (view: StudioViewMode) => {
    writeUrl(urlState({ view }), "push");
    setViewMode(view);
  };
  const copyLink = async () => {
    writeUrl(urlState(), "replace");
    await navigator.clipboard.writeText(window.location.href);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  };

  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><picture><source media="(prefers-color-scheme: light)" srcSet={logoLight} /><img src={logoDark} alt="Axionomy" /></picture><div><div className="brand-name">Studio</div><div className="brand-subtitle">Inspect and replay how a problem was solved</div></div></div>
      <div className="connection" title={engine.kind === "native" ? "Verified by a live health check" : engine.kind === "browser" ? "Rust/WASM engine isolated in a Web Worker" : "No executable engine is available"}><span className={`status-dot ${engine.canRun ? "online" : "static"}`} />{connectionLabel(engine.kind, connectivity)}</div>
    </header>

    <section className="command-bar" aria-label="Problem controls">
      <label className="field problem-field"><span>Problem</span><select value={problemKey} onChange={(event) => selectProblem(event.target.value)} disabled={active}>{catalog.data?.map((candidate) => <option value={candidate.key} key={candidate.key}>{candidate.title}</option>)}</select></label>
      <label className="field instance-field"><span>Instance (size)</span><select value={instanceKey} onChange={(event) => selectInstance(event.target.value)} disabled={active}>{problem?.instances.map((candidate) => <option value={candidate.key} key={candidate.key}>{candidate.label}</option>)}</select></label>
      <label className="field strategy-field"><span>Strategy</span><select value={strategyKey} onChange={(event) => selectStrategy(event.target.value)} disabled={active}>{problem?.strategies.map((strategy) => <option value={strategy.key} key={strategy.key}>{strategy.label}</option>)}</select></label>
      <label className="field numeric-field"><span>Seed</span><input type="number" min="0" value={seed} disabled={active} onChange={(event) => setSeed(Number(event.target.value))} /></label>
      <label className="field numeric-field"><span>Budget</span><input type="number" min="1" value={budget} disabled={active} onChange={(event) => setBudget(Math.max(1, Number(event.target.value)))} /></label>
      <button className="primary" onClick={start} disabled={active || !engine.canRun}>{launching ? "Starting…" : run?.status === "paused" ? "Paused" : run ? "Running…" : "Run"}</button>
      {engine.canPause && run?.status === "running" && <button onClick={pause}>Pause</button>}
      {engine.canPause && run?.status === "paused" && <button onClick={resume}>Resume</button>}
      {run && <button className="danger" onClick={cancel}>Cancel</button>}
      <label className="file-button">Load artifact<input type="file" accept="application/json,.json" onChange={(event) => loadFile(event.target.files?.[0])} /></label>
      <button type="button" className="share-link" onClick={copyLink}>{copied ? "Link copied ✓" : "Copy link"}</button>
      <div className="run-message">{active ? "A newly computed result will replace the current view." : engine.canRun ? "The CLI, HTTP API, MCP server, and Studio all run this same problem the same way." : "No engine running — you can still replay the saved results."}</div>
    </section>

    {active && <RunActivity launching={launching} run={run} events={events} elapsedMs={elapsedMs} />}
    {completion && <CompletionBanner notice={completion} onDismiss={() => setCompletion(undefined)} />}
    {linkNotice && <div className="link-notice" role="status">{linkNotice}<button type="button" onClick={() => setLinkNotice(undefined)}>Dismiss</button></div>}

    {problem && <section className="problem-context"><div><span>{familyLabel(problem.family)}</span><div className="problem-copy"><p>{problem.summary}</p><small><strong>{instance?.label ?? artifact?.instance.label}</strong>{instance?.description ?? artifact?.instance.description}{modelCounts && <em>{modelCounts}</em>}</small></div></div><div>{problem.capabilities.map((capability) => <span key={capability}>{capabilityLabel(capability)}</span>)}</div></section>}
    {error && <div className="error-banner" role="alert">{error}</div>}

    {artifact && document && snapshot ? <main className={freshArtifactId === artifact.id ? "fresh-artifact" : undefined}>
      <section className="alternative-bar"><div><span>Alternatives</span><strong>{artifact.documents.length} outcomes you can replay</strong></div><div role="tablist">{artifact.documents.map((candidate) => <button key={candidate.id} role="tab" aria-selected={candidate.id === document.id} onClick={() => selectDocument(candidate.id)}>{candidate.title.replace(`${artifact.problem.title} · `, "")}</button>)}</div></section>
      <StrategyComparison artifact={artifact} selected={document.id} onSelect={selectDocument} />
      <section className="document-heading"><div><span className="eyebrow">{artifact.instance.label} · {document.source.label} · {document.id}{freshArtifactId === artifact.id ? " · just computed" : ""}</span><h1>{document.title}</h1><p>{document.description}</p></div><div className="objective-pills">{document.objectives.map((objective) => <div className="objective" key={objective.key}><span>{objective.label}</span><strong>{objective.value}</strong><small>{objective.direction}</small></div>)}</div></section>
      <div className="view-tabs" role="tablist" aria-label="Studio evidence mode"><button type="button" role="tab" aria-selected={viewMode === "solve"} onClick={() => selectView("solve")}>How it was solved <small>{active ? "live" : document.solve_observations.length}</small></button><button type="button" role="tab" aria-selected={viewMode === "replay"} onClick={() => selectView("replay")}>Step-by-step replay <small>{document.frames.length}</small></button></div>
      {viewMode === "solve" ? <SolveWorkspace observations={active ? events.flatMap((event) => event.kind === "search_observation" ? [event.observation] : []) : document.solve_observations} active={active} /> : <>
      <PlaybackControls position={position} count={document.frames.length} playing={playing} delay={playbackDelay} onDelay={setPlaybackDelay} onPosition={setPosition} onPlaying={(next) => { if (next && position >= document.frames.length) setPosition(0); setPlaying(next); }} frame={frame} />

      {snapshot.leaderboards.length > 0 && <LeaderboardDock document={document} snapshot={snapshot} previous={previous} position={position} selectedKey={leaderboardKey} onSelect={(key) => { writeUrl(urlState({ leaderboard: key }), "push"); setLeaderboardKey(key); }} onParticipant={setFocusedAccount} />}

      <section className="workspace-grid">
        <div className="panel world-panel"><PanelHeading kicker="Picture (illustration only)" title={snapshot.scene?.title ?? "Problem picture"} aside={snapshot.scene?.surface.kind} /><SceneView scene={snapshot.scene} onAccount={setFocusedAccount} /></div>
        <div className="panel accounts-panel"><PanelHeading kicker="Source of truth" title="Accounts & assets" aside={focusedAccount ? "linked from picture" : `${snapshot.accounts.length} accounts`} /><Accounts snapshot={snapshot} previous={previous} focus={focusedAccount} /></div>
      </section>

      <section className="evidence-grid">
        <div className="panel transition-panel"><PanelHeading kicker="One step" title={frame?.exchange.rate.label ?? "Initial state"} /><Transition frame={frame} /></div>
        <div className="panel proposal-panel"><PanelHeading kicker="Rule checks" title="Moves that should be refused" aside={`${document.proposals.length} ${document.proposals.length === 1 ? "check" : "checks"}`} /><ProposalInspector proposals={document.proposals} /></div>
        <div className="panel analysis-panel"><PanelHeading kicker="Tradeoffs" title={document.pareto_fronts[0]?.title ?? "Search evidence"} /><Suspense fallback={<div className="empty-state">Loading analysis…</div>}><ParetoChart document={document} onSelect={selectPareto} /></Suspense><Telemetry document={document} /></div>
      </section>

      <section className="definition-grid">
        <div className="panel model-panel"><PanelHeading kicker="The rules" title="Rates, roles, goals & invariants" aside={`${document.model?.rates.length ?? 0} rates`} /><ModelExplorer document={document} /></div>
        <div className="panel observation-panel"><PanelHeading kicker="Who can see what" title="Actor-relative observations" aside={`${document.observations.length} views`} /><Observations document={document} /></div>
      </section>
      </>}
    </main> : <div className="loading">Loading saved problem result…</div>}

    <footer>The picture explains. The accounts, rules, and replay prove.</footer>
  </div>;
}

function RunActivity({ launching, run, events, elapsedMs }: { launching: boolean; run?: RunSummary; events: StudioEvent[]; elapsedMs: number }) {
  const latest = events.at(-1);
  const progress = [...events].reverse().find((event): event is Extract<StudioEvent, { kind: "progress" }> => event.kind === "progress");
  const completed = progress?.completed ?? run?.completed ?? 0;
  const total = progress?.total ?? run?.total ?? undefined;
  const status = launching ? "Starting engine run" : run?.status === "paused" ? "Run paused" : "Running computation";
  const message = latest ? eventMessage(latest) : "Submitting reproducible run request…";
  const liveFrame = [...events].reverse().find((event): event is Extract<StudioEvent, { kind: "frame_appended" }> => event.kind === "frame_appended");
  return <section className={`run-activity ${run?.status === "paused" ? "paused" : ""}`} role="status" aria-live="polite">
    <div className="run-activity-heading"><span className="spinner" aria-hidden="true" /><div><strong>{status}</strong><span>{message}</span></div></div>
    <div className="run-activity-progress">
      {total !== undefined && total > 0 ? <progress max={total} value={Math.min(completed, total)} aria-label="Run phase progress" /> : <div className="indeterminate-progress" />}
      <span>{total !== undefined ? `${completed} / ${total}` : "waiting for first checkpoint"}</span>
    </div>
    <div className="run-activity-meta"><span>{formatDuration(elapsedMs)} elapsed</span>{run && <><span>{run.request.instance ?? "default"} instance</span><span>seed {run.request.seed}</span><span>budget {run.request.budget}</span><code>{run.id}</code></>}</div>
    {liveFrame?.frame.after.leaderboards[0] && <div className="live-standings"><span>Live verified frame {liveFrame.frame_index + 1} · {liveFrame.frame.after.leaderboards[0].label}</span>{liveFrame.frame.after.leaderboards[0].entries.slice(0, 4).map((entry) => <strong key={entry.participant.key}>{entry.rank ? `#${entry.rank}` : "—"} {entry.participant.label} <b>{entry.value}</b></strong>)}</div>}
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
    <header><div><span className={active ? "spinner" : "solve-complete"} aria-hidden="true">{active ? "" : "✓"}</span><div><strong>{active ? "Live solver observations" : "Saved solver observations"}</strong><small>{active ? "Progress reported by the search as it runs" : "Saved with the result, so you can review it later"}</small></div></div><span>{observations.length} observations</span></header>
    {latest && <div className="solve-summary"><div><span>Current phase</span><strong>{phaseLabel(latest.phase)}</strong></div><div><span>Algorithm</span><strong>{latest.algorithm.replaceAll("_", " ")}</strong></div><div><span>Progress</span><strong>{latest.completed} / {latest.total}</strong></div><div><span>Evidence</span><strong>{kinds.join(" · ").replaceAll("_", " ")}</strong></div></div>}
    {latest && latest.total > 0 && <progress max={latest.total} value={Math.min(latest.completed, latest.total)} aria-label="Solver observation progress" />}
    <div className="solve-stream">{observations.length === 0 ? <div className="empty-state">Waiting for the first solver checkpoint…</div> : observations.map((observation) => <article key={`${observation.sequence}:${observation.phase}`} className={`observation-${observation.kind}`}><div className="solve-observation-icon"><SceneIcon glyph={observationGlyph(observation.kind)} size={20} /></div><div><strong>{observation.label}</strong><span>{phaseLabel(observation.phase)} · {observation.completed} / {observation.total}</span></div><div>{observation.metrics.map((metric) => <span key={metric.key}><small>{metric.label}</small><b>{metric.value}</b></span>)}</div></article>)}</div>
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

function familyLabel(family: NonNullable<RunArtifact["problem"]>["family"]): string {
  const labels: Record<typeof family, string> = {
    pathfinding: "Pathfinding",
    constraint: "Constraint satisfaction",
    production: "Production",
    scheduling: "Scheduling",
    allocation: "Allocation",
    market: "Market clearing",
    stochastic_planning: "Planning under uncertainty",
    adversarial_game: "Adversarial game",
    partial_observation: "Hidden information",
    temporal_simulation: "Time and decay",
    multi_agent_competition: "Multi-agent competition",
  };
  return labels[family];
}

function capabilityLabel(capability: RunArtifact["problem"]["capabilities"][number]): string {
  const labels: Record<typeof capability, string> = {
    deterministic_search: "Deterministic search",
    weighted_search: "Cost-guided search",
    specialized_algorithm: "Specialised algorithm",
    exact_pareto: "Exact Pareto frontier",
    approximate_pareto: "Sampled Pareto frontier",
    feasibility_assessment: "Explains why something is impossible",
    multi_account_exchange: "Multi-account atomic changes",
    atomic_settlement: "Atomic settlement",
    branch_optimization: "Branch and bound",
    monte_carlo: "Monte Carlo",
    mcts: "MCTS",
    information_set_search: "Information-set search (ISMCTS)",
    partial_observation: "Partial observation",
    chance: "Random events",
    temporal_effects: "Time-triggered effects",
    fungible_cohorts: "Interchangeable batches",
    non_fungible_facts: "Unique facts",
    rl_projection: "Reinforcement-learning training data",
    replay_derived_leaderboards: "Replay-derived leaderboards",
  };
  return labels[capability];
}

function phaseLabel(phase: string): string {
  const labels: Record<string, string> = {
    prepare: "Preparing",
    artifact: "Building the result",
    monte_carlo: "Monte Carlo sampling",
    pareto_sampling: "Sampling the frontier",
    mcts: "MCTS",
    mcts_game: "MCTS",
    ismcts: "ISMCTS",
    multi_agent_match: "Multi-agent match",
  };
  return labels[phase] ?? phase.replaceAll("_", " ");
}

function StrategyComparison({ artifact, selected, onSelect }: { artifact: RunArtifact; selected: string; onSelect: (id: string) => void }) {
  return <section className="strategy-comparison" aria-label="Outcome comparison">
    <div className="comparison-heading"><span>Compare outcomes</span><strong>Every strategy and what it cost, side by side</strong></div>
    <div className="comparison-scroll"><table><thead><tr><th>Outcome</th><th>Result</th><th>Trace</th><th>Search evidence</th></tr></thead><tbody>{artifact.documents.map((candidate) => {
      const series = candidate.telemetry.find((entry) => entry.algorithm !== "Model size");
      const work = series ? [...series.points].reverse().find((point) => ["generated", "expanded", "iteration", "sample"].includes(point.kind)) : undefined;
      return <tr key={candidate.id} className={candidate.id === selected ? "selected" : undefined}>
        <td><button type="button" onClick={() => onSelect(candidate.id)}>{candidate.title.replace(`${artifact.problem.title} · `, "")}</button></td>
        <td>{candidate.objectives.length > 0 ? candidate.objectives.map((objective) => `${objective.label}: ${objective.value}`).join(" · ") : "Feasibility outcome"}</td>
        <td>{candidate.frames.length} {candidate.frames.length === 1 ? "step" : "steps"}</td>
        <td>{series ? `${series.algorithm} · ${series.exact ? "exact" : "sampled"}${work ? ` · ${work.value} ${work.kind.replaceAll("_", " ")}` : ""}` : "Replay only"}</td>
      </tr>;
    })}</tbody></table></div>
  </section>;
}

function LeaderboardDock({ document, snapshot, previous, position, selectedKey, onSelect, onParticipant }: { document: ViewDocument; snapshot: ViewSnapshot; previous?: ViewSnapshot; position: number; selectedKey?: string; onSelect: (key: string) => void; onParticipant: (account: string) => void }) {
  const selected = snapshot.leaderboards.find((leaderboard) => leaderboard.key === selectedKey) ?? snapshot.leaderboards[0];
  if (!selected) return null;
  const previousBoard = previous?.leaderboards.find((leaderboard) => leaderboard.key === selected.key);
  const snapshots = [document.initial, ...document.frames.slice(0, position).map((frame) => frame.after)];
  return <section className="leaderboard-dock" aria-label="Replay-derived leaderboards">
    <header><div><span>Live comparative outcomes</span><strong>Who is winning depends on what you value</strong></div><div className="leaderboard-step">Economic step <b>{snapshot.index}</b></div></header>
    <div className="leaderboard-tabs" role="tablist">{snapshot.leaderboards.map((leaderboard) => <button type="button" role="tab" aria-selected={leaderboard.key === selected.key} key={leaderboard.key} onClick={() => onSelect(leaderboard.key)}>{leaderboard.label}<small>{leaderboard.direction}</small></button>)}</div>
    <div className="leaderboard-explanation"><strong>{selected.label}</strong><span>{selected.description}</span></div>
    <div className="leaderboard-entries">{selected.entries.map((entry) => {
      const prior = previousBoard?.entries.find((candidate) => candidate.participant.key === entry.participant.key);
      const movement = rankMovement(entry.rank ?? undefined, prior?.rank ?? undefined);
      const tookLead = entry.rank === 1 && prior?.rank != null && prior.rank !== 1;
      const history = snapshots.map((candidate) => candidate.leaderboards.find((board) => board.key === selected.key)?.entries.find((candidateEntry) => candidateEntry.participant.key === entry.participant.key)?.value).filter((value): value is string => value !== undefined);
      return <article key={entry.participant.key} className={`${entry.eligible ? "eligible" : "ineligible"} ${tookLead ? "lead-change" : ""}`} style={{ "--participant-color": participantColor(entry.participant.key) } as CSSProperties}>
        <button type="button" className="leaderboard-participant" onClick={() => onParticipant(entry.participant.key)}>
          <span className="rank">{entry.rank ? `#${entry.rank}` : "—"}</span><span className="participant-dot" /><span><strong>{entry.participant.label}</strong><small>{entry.eligible ? movement : "not yet eligible"}{tookLead ? " · took the lead" : ""}</small></span>
        </button>
        <div className="leaderboard-score"><strong>{entry.value}</strong><small>{entry.unit ?? (entry.eligible ? "standing" : "waiting for work")}</small></div>
        <Sparkline values={history} />
        <details><summary>Why this rank?</summary><div>{entry.components.map((metric) => <span key={metric.key}><small>{metric.label}</small><b>{metric.value}{metric.unit ? ` ${metric.unit}` : ""}</b></span>)}</div></details>
      </article>;
    })}</div>
    <footer>Ranks are disposable projections of this replayed snapshot. The accounts and applied exchanges remain authoritative.</footer>
  </section>;
}

function Sparkline({ values }: { values: string[] }) {
  const numeric = values.map(scoreNumber);
  if (numeric.length < 2 || numeric.some((value) => !Number.isFinite(value))) return <div className="sparkline empty" aria-hidden="true" />;
  const low = Math.min(...numeric);
  const high = Math.max(...numeric);
  const range = high - low || 1;
  const points = numeric.map((value, index) => `${numeric.length === 1 ? 50 : index * 100 / (numeric.length - 1)},${26 - (value - low) * 22 / range}`).join(" ");
  return <svg className="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none" aria-label={`Score history over ${numeric.length} replay states`}><polyline points={points} /></svg>;
}

function scoreNumber(value: string): number {
  if (value === "non-dominated") return 1;
  if (value === "dominated") return 0;
  const [numerator, denominator] = value.split("/").map(Number);
  return denominator === undefined ? numerator : numerator / denominator;
}

function rankMovement(current?: number, previous?: number): string {
  if (current === undefined) return "unranked";
  if (previous === undefined) return "entered ranking";
  if (current < previous) return `↑ up ${previous - current}`;
  if (current > previous) return `↓ down ${current - previous}`;
  return "— held position";
}

function participantColor(key: string): string {
  let hash = 0;
  for (const character of key) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 72% 58%)`;
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
