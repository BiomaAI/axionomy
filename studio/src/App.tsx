import { lazy, Suspense, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { IconChevronLeft, IconChevronRight, IconMaximize, IconPlayerPauseFilled, IconPlayerPlayFilled, IconPlayerSkipBack, IconPlayerSkipForward } from "@tabler/icons-react";
import "@xyflow/react/dist/style.css";
import logoDark from "../../assets/axionomy-logo-dark.png";
import logoLight from "../../assets/axionomy-logo-light.png";
import {
  type ExchangeFrame,
  type LeaderboardView,
  type ProblemDescriptor,
  type Scene,
  type SearchObservation,
  type StudioEvent,
  type Study,
  type StudyDownload,
  type StudySummary,
  type ViewDocument,
  type ViewSnapshot,
} from "./api";
import { browserEngine } from "./browserEngine";
import { connectionLabel, nativeEngine, staticEngine, type EngineConnectivity, type EngineStudySubscription } from "./engine";
import { SceneIcon } from "./SceneIcon";
import { SceneView, type ReplayMotionMode } from "./SceneView";
import {
  Accounts,
  ModelExplorer,
  Observations,
  PanelHeading,
  ProposalInspector,
  StepTicker,
  Telemetry,
  Transition,
  useSnapshot,
} from "./Inspectors";
import { formatQuantity, QuantityFormats } from "./Quantity";

const ParetoChart = lazy(() => import("./ParetoChart"));
type StudioViewMode = "thinking" | "replay";
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

/** The service's `StudyRequest` defaults; a link that omits them asks for these. */
export const DEFAULT_SEED = 17;
export const DEFAULT_BUDGET = 128;

/** A parameter the link wrote but Studio could not read, quoted as written. */
type UnreadParameter = { key: "seed" | "budget" | "step" | "view"; written: string };

function readUrlState(): StudioUrlState & { unread: UnreadParameter[] } {
  const query = new URLSearchParams(window.location.search);
  const unread: UnreadParameter[] = [];
  // An absent parameter means "the default"; only a written one can be wrong.
  const number = (key: "seed" | "budget" | "step", minimum = 0) => {
    const written = query.get(key);
    if (written === null || written.trim() === "") return undefined;
    const value = Number(written);
    if (/^\d+$/.test(written.trim()) && Number.isSafeInteger(value) && value >= minimum) return value;
    unread.push({ key, written });
    return undefined;
  };
  const view = query.get("view");
  if (view !== null && view !== "thinking" && view !== "replay") unread.push({ key: "view", written: view });
  return {
    problem: query.get("problem") ?? undefined,
    instance: query.get("instance") ?? undefined,
    strategy: query.get("strategy") ?? undefined,
    document: query.get("document") ?? undefined,
    view: view === "thinking" || view === "replay" ? view : undefined,
    step: number("step"),
    leaderboard: query.get("leaderboard") ?? undefined,
    seed: number("seed"),
    budget: number("budget", 1),
    unread,
  };
}

function unreadNotice({ key, written }: UnreadParameter): string {
  switch (key) {
    case "seed": return `The link’s seed “${written}” is not a whole number; Studio used seed ${DEFAULT_SEED}.`;
    case "budget": return `The link’s budget “${written}” is not a whole number of at least 1; Studio used budget ${DEFAULT_BUDGET}.`;
    case "step": return `The link’s step “${written}” is not a whole number; Studio opened the initial state.`;
    case "view": return `The link’s view “${written}” is not Thinking or Replay; Studio opened the replay.`;
  }
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
  studyId: string;
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
    // Every engine serves the same Rust catalog; keeping it across an engine
    // change keeps the open problem, and its saved study, in place.
    placeholderData: keepPreviousData,
    retry: false,
    queryFn: () => engine.catalog(),
  });
  const [problemKey, setProblemKey] = useState(initialUrl.problem ?? "maze");
  // An omitted option means the problem's default, which is only known once
  // the catalog loads. The empty key resolves to it without a warning.
  const [instanceKey, setInstanceKey] = useState(initialUrl.instance ?? "");
  const [strategyKey, setStrategyKey] = useState(initialUrl.strategy ?? "");
  const [seed, setSeed] = useState(initialUrl.seed ?? DEFAULT_SEED);
  const [budget, setBudget] = useState(initialUrl.budget ?? DEFAULT_BUDGET);
  const [study, setStudy] = useState<Study>();
  // Where the study on screen came from: the saved one for its problem, one
  // computed in this session, or a file the visitor loaded.
  const [studyOrigin, setStudyOrigin] = useState<StudyOrigin>("saved");
  const [documentId, setDocumentId] = useState<string | undefined>(initialUrl.document);
  const [position, setPosition] = useState(initialUrl.step ?? 0);
  const [replayMotion, setReplayMotion] = useState<ReplayMotionMode>("seek");
  const [playing, setPlaying] = useState(false);
  const [playbackDelay, setPlaybackDelay] = useState(650);
  const [viewMode, setViewMode] = useState<StudioViewMode>(initialUrl.view ?? "replay");
  const [leaderboardKey, setLeaderboardKey] = useState<string | undefined>(initialUrl.leaderboard);
  const [focusedAccount, setFocusedAccount] = useState<string>();
  const [ongoing, setOngoing] = useState<StudySummary>();
  const [launching, setLaunching] = useState(false);
  const [events, setEvents] = useState<StudioEvent[]>([]);
  const [error, setError] = useState<string>();
  const [elapsedMs, setElapsedMs] = useState(0);
  const [completion, setCompletion] = useState<CompletionNotice>();
  const [freshStudy, setFreshStudy] = useState<Study>();
  // Each notice says what a link asked for that Studio could not open, and
  // what it opened instead. Nothing a link names is replaced silently.
  const [linkNotices, setLinkNotices] = useState<string[]>(() => initialUrl.unread.map(unreadNotice));
  const notice = (message: string) => setLinkNotices((current) => current.includes(message) ? current : [...current, message]);
  // A step a link names is checked once its replay has loaded.
  const linkedStep = useRef(initialUrl.step);
  const [copied, setCopied] = useState(false);
  const eventSource = useRef<EngineStudySubscription | null>(null);
  const studyStartedAt = useRef<number | undefined>(undefined);
  const previewGeneration = useRef(0);
  // The saved study being downloaded, and how much of it has arrived.
  const [preview, setPreview] = useState<{ problem: string; download: StudyDownload }>();
  const previewAbort = useRef<AbortController | undefined>(undefined);
  const stopPreview = () => { previewGeneration.current += 1; previewAbort.current?.abort(); setPreview(undefined); };

  const problem = catalog.data?.find((candidate) => candidate.key === problemKey);
  const instance = problem?.instances.find((candidate) => candidate.profile === instanceKey);
  const document = study?.documents.find((candidate) => candidate.id === documentId) ?? study?.documents.find((candidate) => candidate.id === study.selected_document) ?? study?.documents[0];
  const snapshot = useSnapshot(document, position);
  const previous = useSnapshot(document, Math.max(0, position - 1));
  const frame = position > 0 ? document?.frames[position - 1] : undefined;
  const worldPanel = useRef<HTMLDivElement>(null);

  // `${account}|${asset}` pairs that change somewhere in this document's trace.
  // This is only a display hint: lack of change does not redefine an asset's
  // economic meaning or make it non-authoritative.
  const changingBalanceKeys = useMemo(() => {
    if (!document || document.frames.length === 0) return null;
    const flatten = (snap: ViewSnapshot) => {
      const map = new Map<string, string>();
      for (const account of snap.accounts) for (const balance of account.balances) map.set(`${account.account.key}|${balance.asset.key}`, balance.quantity);
      return map;
    };
    const keys = new Set<string>();
    let prior = flatten(document.initial);
    for (const step of document.frames) {
      const next = flatten(step.after);
      for (const [key, value] of next) if (prior.get(key) !== value) keys.add(key);
      for (const key of prior.keys()) if (!next.has(key)) keys.add(key);
      prior = next;
    }
    return keys;
  }, [document]);

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
    if (!launching && !ongoing) return;
    const update = () => setElapsedMs(studyStartedAt.current === undefined ? 0 : performance.now() - studyStartedAt.current);
    update();
    const timer = window.setInterval(update, 100);
    return () => window.clearInterval(timer);
  }, [launching, ongoing]);

  useEffect(() => {
    if (!catalog.data || problem) return;
    const fallback = catalog.data.find((candidate) => candidate.key === "maze") ?? catalog.data[0];
    if (!fallback) return;
    notice(`The linked problem “${problemKey}” does not exist; opened ${fallback.title} instead.`);
    setProblemKey(fallback.key);
    setInstanceKey(fallback.default_instance);
    setStrategyKey(fallback.default_strategy);
    setDocumentId(undefined);
  }, [catalog.data, problem, problemKey]);

  useEffect(() => {
    if (!problem) return;
    const requestedInstance = problem.instances.some((candidate) => candidate.profile === instanceKey) ? instanceKey : problem.default_instance;
    const requestedStrategy = problem.strategies.some((candidate) => candidate.key === strategyKey) ? strategyKey : problem.default_strategy;
    const invalidInstance = instanceKey !== "" && requestedInstance !== instanceKey;
    const invalidStrategy = strategyKey !== "" && requestedStrategy !== strategyKey;
    if (invalidInstance || invalidStrategy) {
      notice(`Some linked options were not valid for ${problem.title}; Studio selected its defaults.`);
    }
    setInstanceKey(requestedInstance);
    setStrategyKey(requestedStrategy);
    setError(undefined);
    setFreshStudy(undefined);
    const generation = ++previewGeneration.current;
    let disposed = false;
    const abort = new AbortController();
    previewAbort.current = abort;
    const current = () => !disposed && generation === previewGeneration.current;
    setPreview({ problem: problem.title, download: { received: 0 } });
    engine.defaultStudy(problem.key, {
      signal: abort.signal,
      onProgress: (download) => { if (current()) setPreview({ problem: problem.title, download }); },
    }).then((next) => {
      if (!current()) return;
      setPreview(undefined);
      const requestedDocument = next.documents.find((candidate) => candidate.id === documentId)?.id
        ?? next.documents.find((candidate) => candidate.id === requestedStrategy)?.id
        ?? next.selected_document;
      if (documentId && requestedDocument !== documentId) {
        const opened = next.documents.find((candidate) => candidate.id === requestedDocument);
        notice(`The linked outcome “${documentId}” is not in the saved ${problem.title} study; opened ${opened ? documentName(next, opened) : "its default outcome"} instead.`);
      }
      setStudy(next);
      setStudyOrigin("saved");
      setDocumentId(requestedDocument);
    }).catch((cause: unknown) => {
      if (!current()) return;
      setPreview(undefined);
      setError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => { disposed = true; abort.abort(); };
  }, [problem?.key]);

  useEffect(() => {
    if (!document || study?.problem.key !== problemKey) return;
    const steps = document.frames.length;
    const linked = linkedStep.current;
    linkedStep.current = undefined;
    if (linked !== undefined && linked > steps) {
      notice(`The link’s step ${linked} is past the end of this replay, which has ${steps} ${steps === 1 ? "step" : "steps"}; Studio opened its final state.`);
    }
    setPosition((current) => Math.min(current, steps));
    setPlaying(false);
    setFocusedAccount(undefined);
  }, [document?.id, study?.problem.key]);

  useEffect(() => {
    if (!study || study.problem.key !== problemKey || !documentId) return;
    if (!study.documents.some((candidate) => candidate.id === documentId)) {
      const opened = study.documents.find((candidate) => candidate.id === study.selected_document);
      notice(`The linked outcome “${documentId}” is not in this ${study.problem.title} study; opened ${opened ? documentName(study, opened) : "its default outcome"} instead.`);
      setDocumentId(study.selected_document);
    }
  }, [study, documentId, problemKey]);

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
    if (!problem || !problem.instances.some((candidate) => candidate.profile === instanceKey) || !problem.strategies.some((candidate) => candidate.key === strategyKey)) return;
    writeUrl(urlState(), "replace");
  }, [problemKey, instanceKey, strategyKey, documentId, viewMode, position, leaderboardKey, seed, budget]);

  useEffect(() => {
    const restore = () => {
      const linked = readUrlState();
      if (linked.problem) setProblemKey(linked.problem);
      if (linked.instance) setInstanceKey(linked.instance);
      if (linked.strategy) setStrategyKey(linked.strategy);
      setDocumentId(linked.document);
      if (linked.view) setViewMode(linked.view);
      setLeaderboardKey(linked.leaderboard);
      setSeed(linked.seed ?? DEFAULT_SEED);
      setBudget(linked.budget ?? DEFAULT_BUDGET);
      for (const unread of linked.unread) notice(unreadNotice(unread));
      linkedStep.current = linked.step;
      setReplayMotion("seek");
      setPosition(linked.step ?? 0);
    };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);

  useEffect(() => {
    if (!playing || !document) return;
    const timer = window.setInterval(() => { setReplayMotion("step"); setPosition((current) => {
      if (current >= document.frames.length) { setPlaying(false); return current; }
      return current + 1;
    }); }, playbackDelay);
    return () => window.clearInterval(timer);
  }, [playing, document, playbackDelay]);

  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      const target = event.target;
      const interactive = target instanceof Element
        && target.closest("input, select, textarea, button, a, summary, [contenteditable='true']");
      if (viewMode !== "replay" || !document || document.frames.length === 0 || interactive) return;
      if (event.key === "ArrowLeft") { setPlaying(false); setReplayMotion("step"); setPosition((current) => Math.max(0, current - 1)); }
      else if (event.key === "ArrowRight") { setPlaying(false); setReplayMotion("step"); setPosition((current) => Math.min(document.frames.length, current + 1)); }
      else if (event.key === "Home") { setPlaying(false); setReplayMotion("seek"); setPosition(0); }
      else if (event.key === "End") { setPlaying(false); setReplayMotion("seek"); setPosition(document.frames.length); }
      else if (event.key === " ") {
        event.preventDefault();
        setPlaying((current) => {
          if (!current) setPosition((step) => step >= document.frames.length ? 0 : step);
          return !current;
        });
      }
      else return;
    };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, [document, viewMode]);

  const start = async () => {
    stopPreview();
    const submittedProblem = problem?.title ?? problemKey;
    const submittedInstance = problem?.instances.find((candidate) => candidate.profile === instanceKey)?.label ?? instanceKey;
    const submittedStrategy = problem?.strategies.find((candidate) => candidate.key === strategyKey)?.label ?? strategyKey;
    setError(undefined); setEvents([]); setCompletion(undefined); eventSource.current?.close();
    setViewMode("thinking");
    studyStartedAt.current = performance.now(); setElapsedMs(0); setLaunching(true);
    try {
      const profile = problem?.instances.find((candidate) => candidate.profile === instanceKey)?.profile;
      const subscription = await engine.start({ problem: problemKey, instance: profile, strategy: strategyKey, seed, budget }, async (event) => {
        setEvents((current) => current.some((known) => known.sequence === event.sequence) ? current : [...current, event]);
        if (event.kind === "study_completed") {
          eventSource.current?.close();
          const next = await engine.study(event.study_id);
          const durationMs = studyStartedAt.current === undefined ? 0 : performance.now() - studyStartedAt.current;
          previewGeneration.current += 1;
          setStudy(next); setStudyOrigin("computed"); setDocumentId(event.document); setFreshStudy(next);
          setCompletion({ studyId: event.study_id, problem: submittedProblem, instance: submittedInstance, strategy: submittedStrategy, seed, budget, durationMs, documents: next.documents.length });
          setElapsedMs(durationMs); setOngoing(undefined);
        } else if (event.kind === "study_cancelled" || event.kind === "study_failed") {
          eventSource.current?.close(); setOngoing(undefined);
          if (event.kind === "study_failed") setError(event.message);
        }
      }, setError);
      eventSource.current = subscription;
      setOngoing(subscription.summary); setLaunching(false);
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); setOngoing(undefined); setLaunching(false); }
  };

  const pause = async () => { if (ongoing) setOngoing(await engine.pause(ongoing.id)); };
  const resume = async () => { if (ongoing) setOngoing(await engine.resume(ongoing.id)); };
  const cancel = async () => { if (ongoing) await engine.cancel(ongoing.id); };

  const loadFile = async (file?: File) => {
    if (!file) return;
    stopPreview();
    try {
      const next = JSON.parse(await file.text()) as Study;
      if (!Array.isArray(next.documents) || !next.selected_document) throw new Error("missing study documents");
      setStudy(next); setStudyOrigin("file"); setProblemKey(next.problem.key); setInstanceKey(next.instance.profile); setStrategyKey(next.key.strategy); setSeed(next.key.seed); setBudget(next.key.budget); setDocumentId(next.selected_document); setFreshStudy(undefined); setError(undefined);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "the selected file is not a study"); }
  };

  const selectPareto = (values: string[]) => {
    const match = study?.documents.find((candidate) => candidate.objectives.slice(0, values.length).every((objective, index) => objective.value === values[index]));
    if (match) setDocumentId(match.id);
  };
  const active = launching || Boolean(ongoing);
  // A link or Study during a download would name or replace a study not yet on screen.
  const loading = preview !== undefined;
  const modelSize = document?.telemetry.find((entry) => entry.subject === "model");
  const modelCounts = modelSize?.points
    .filter((point) => point.kind === "accounts" || point.kind === "rates")
    .map((point) => `${point.value} ${point.label}`)
    .join(" · ");
  const selectProblem = (key: string) => {
    const next = catalog.data?.find((candidate) => candidate.key === key);
    if (!next) return;
    const nextState = urlState({ problem: key, instance: next.default_instance, strategy: next.default_strategy, document: undefined, step: 0, leaderboard: undefined });
    writeUrl(nextState, "push");
    setProblemKey(key);
    setCompletion(undefined);
    setInstanceKey(next.default_instance);
    setStrategyKey(next.default_strategy);
    setDocumentId(undefined);
    setLeaderboardKey(undefined);
    setLinkNotices([]);
  };
  const selectInstance = (key: string) => {
    writeUrl(urlState({ instance: key, step: 0 }), "push");
    setInstanceKey(key); setPosition(0); setCompletion(undefined);
  };
  const selectStrategy = (key: string) => {
    const match = study?.documents.find((candidate) => candidate.id === key);
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
      <div className="brand"><picture><source media="(prefers-color-scheme: light)" srcSet={logoLight} /><img src={logoDark} alt="Axionomy" /></picture><div><div className="brand-name">Studio</div><div className="brand-subtitle">Economic reasoning workbench</div></div></div>
      <div className="connection" title={engine.kind === "native" ? "Verified by a live health check" : engine.kind === "browser" ? "Rust/WASM engine isolated in a Web Worker" : "No executable engine is available"}><span className={`status-dot ${engine.canStudy ? "online" : "static"}`} />{connectionLabel(engine.kind, connectivity)}</div>
    </header>

    <section className="command-bar" aria-label="Problem controls">
      <label className="field problem-field"><span>Problem</span><select name="problem" value={problemKey} onChange={(event) => selectProblem(event.target.value)} disabled={active}>{catalog.data?.map((candidate) => <option value={candidate.key} key={candidate.key}>{candidate.title}</option>)}</select></label>
      <label className="field instance-field"><span>Instance (size)</span><select name="instance" value={instanceKey} onChange={(event) => selectInstance(event.target.value)} disabled={active}>{problem?.instances.map((candidate) => <option value={candidate.profile} key={candidate.profile}>{candidate.label}</option>)}</select></label>
      <label className="field strategy-field"><span>Strategy</span><select name="strategy" value={strategyKey} onChange={(event) => selectStrategy(event.target.value)} disabled={active}>{problem?.strategies.map((strategy) => <option value={strategy.key} key={strategy.key}>{strategy.label}</option>)}</select></label>
      <label className="field numeric-field"><span>Seed</span><input name="seed" type="number" min="0" value={seed} disabled={active} onChange={(event) => setSeed(Number(event.target.value))} /></label>
      <label className="field numeric-field"><span>Budget</span><input name="budget" type="number" min="1" value={budget} disabled={active} onChange={(event) => setBudget(Math.max(1, Number(event.target.value)))} /></label>
      <button className="primary" onClick={start} disabled={active || loading || !engine.canStudy}>{launching ? "Starting…" : ongoing?.status === "paused" ? "Paused" : ongoing ? "Studying…" : "Study"}</button>
      {engine.canPause && ongoing?.status === "studying" && <button onClick={pause}>Pause</button>}
      {engine.canPause && ongoing?.status === "paused" && <button onClick={resume}>Resume</button>}
      {ongoing && <button className="danger" onClick={cancel}>Cancel</button>}
      <label className="file-button">Load study<input name="study" type="file" accept="application/json,.json" onChange={(event) => loadFile(event.target.files?.[0])} /></label>
      <button type="button" className="share-link" onClick={copyLink} disabled={loading}>{copied ? "Link copied ✓" : "Copy link"}</button>
      <div className="study-message">{active ? "A newly computed study will replace the current view." : engine.canStudy ? "The CLI, HTTP API, MCP server, and Studio all study this same problem the same way." : "No engine running — you can still replay the saved studies."}</div>
    </section>

    {(active || completion || linkNotices.length > 0) && <div className="status-layer">
      {active && <StudyActivity launching={launching} ongoing={ongoing} events={events} elapsedMs={elapsedMs} />}
      {completion && <><CompletionBanner notice={completion} onDismiss={() => setCompletion(undefined)} /><VerifiedStandings events={events} live={false} /></>}
      {linkNotices.map((message) => <div className="link-notice" role="status" key={message}>{message}<button type="button" onClick={() => setLinkNotices((current) => current.filter((candidate) => candidate !== message))}>Dismiss</button></div>)}
    </div>}

    {problem && <section className="problem-context"><div><span>{familyLabel(problem.family)}</span><div className="problem-copy"><p>{problem.summary}</p><small><strong>{instance?.label ?? study?.instance.label}</strong>{instance?.description ?? study?.instance.description}</small></div></div><ul className="capabilities" aria-label="What this problem exercises">{problem.capabilities.map((capability) => <li key={capability}>{capabilityLabel(capability)}</li>)}</ul></section>}
    {error && <div className="error-banner" role="alert">{error}</div>}

    {!loading && study && document && snapshot ? <QuantityFormats.Provider value={document.formats ?? {}}><main className={freshStudy === study ? "fresh-study" : undefined}>
      <OutcomeTabs study={study} selected={document.id} onSelect={selectDocument} />
      <StrategyComparison study={study} selected={document.id} onSelect={selectDocument} />
      <section className="document-heading"><div><span className="eyebrow">{study.problem.title} · {isStrategyDocument(study, document) ? documentName(study, document) : "Computed for comparison"}{freshStudy === study ? " · just computed" : ""}</span><h1>{document.title}</h1><p>{document.description}</p><StudyKeyLine study={study} origin={studyOrigin} model={modelCounts} /></div><div className="objective-pills">{document.objectives.map((objective) => <div className="objective" key={objective.key}><span>{objective.label}</span><strong>{formatQuantity(objective.value, objective.format ?? undefined)}</strong><small>{objective.direction}</small></div>)}</div></section>
      {problem && <StudyMismatch problem={problem} study={study} origin={studyOrigin} requested={{ instance: instanceKey || problem.default_instance, strategy: strategyKey || problem.default_strategy, seed, budget }} canStudy={engine.canStudy} />}
      <div className="view-tabs" role="tablist" aria-label="Studio view"><button type="button" role="tab" aria-selected={viewMode === "thinking"} onClick={() => selectView("thinking")}>Thinking <small>{active ? "live" : document.thinking.filter(isSearchObservation).length}</small></button><button type="button" role="tab" aria-selected={viewMode === "replay"} onClick={() => selectView("replay")}>Step-by-step replay <small>{document.frames.length}</small></button></div>
      {viewMode === "thinking" ? <ThinkingWorkspace key={document.id} scene={document.initial.scene} observations={active ? events.flatMap((event) => event.kind === "thinking" ? [event.observation] : []) : document.thinking} active={active} /> : (() => {
        const surface = snapshot.scene?.surface;
        const sparseScene = surface?.kind === "graph" && surface.nodes.length <= 6;
        const hasBoards = snapshot.leaderboards.length > 0;
        const hasAnalysis = document.pareto_fronts.length > 0 || document.telemetry.length > 0;
        const hasObservations = document.observations.length > 0;
        const toggleTheater = () => {
          const node = worldPanel.current;
          if (!node) return;
          if (window.document.fullscreenElement) void window.document.exitFullscreen();
          else void node.requestFullscreen().catch(() => undefined);
        };
        const showProof = () => window.document.querySelector(".transition-panel")?.scrollIntoView({ behavior: "smooth", block: "start" });
        return <>
      <PlaybackControls position={position} count={document.frames.length} playing={playing} delay={playbackDelay} onDelay={setPlaybackDelay} onPosition={(next, motion) => { setReplayMotion(motion); setPosition(next); }} onPlaying={(next) => { if (next && position >= document.frames.length) { setReplayMotion("seek"); setPosition(0); } setPlaying(next); }} frame={frame} onShare={copyLink} shared={copied} />

      <section className={`stage${hasBoards ? "" : " no-boards"}${sparseScene ? " scene-sparse" : ""}`}>
        <div className="panel world-panel" ref={worldPanel}>
          <PanelHeading kicker="Encoded state projection" title={snapshot.scene?.title ?? "Problem state"} aside={snapshot.scene?.surface.kind} action={<button type="button" className="theater-btn" onClick={toggleTheater} title="Theater mode" aria-label="Toggle theater mode"><IconMaximize size={16} stroke={1.8} aria-hidden="true" /></button>} />
          <SceneView scene={snapshot.scene} previousScene={previous?.scene} history={[document.initial, ...document.frames.slice(0, position).map((candidate) => candidate.after)].flatMap((candidate) => candidate.scene ? [candidate.scene] : [])} frame={frame} motion={replayMotion} onAccount={setFocusedAccount} />
        </div>
        {hasBoards && <LeaderboardDock document={document} snapshot={snapshot} previous={previous} position={position} selectedKey={leaderboardKey} onSelect={(key) => { writeUrl(urlState({ leaderboard: key }), "push"); setLeaderboardKey(key); }} onParticipant={setFocusedAccount} />}
        <div className="truth-rail">
          <div className="panel ticker-panel">
            <PanelHeading kicker="This step" title={position === 0 ? "Initial state" : frame?.cues[0]?.label ?? frame?.exchange.rate.label ?? "Exchange"} aside={`${position} / ${document.frames.length}`} />
            <StepTicker frame={frame} steps={document.frames.length} onProof={showProof} />
          </div>
          <div className="panel accounts-panel">
            <PanelHeading kicker="Source of truth" title="Accounts & assets" aside={focusedAccount ? "linked from picture" : `${snapshot.accounts.length} accounts`} />
            <Accounts snapshot={snapshot} previous={previous} focus={focusedAccount} changingBalanceKeys={changingBalanceKeys} />
          </div>
        </div>
      </section>

      <section className={`evidence-grid${hasAnalysis ? "" : " no-analysis"}`}>
        <div className="panel transition-panel"><PanelHeading kicker="One step" title={frame?.exchange.rate.label ?? "Initial state"} /><Transition frame={frame} steps={document.frames.length} /></div>
        <div className="panel proposal-panel"><PanelHeading kicker="Rule checks" title="Moves that should be refused" aside={`${document.proposals.length} ${document.proposals.length === 1 ? "check" : "checks"}`} /><ProposalInspector proposals={document.proposals} /></div>
        {hasAnalysis && <div className="panel analysis-panel"><PanelHeading kicker="Tradeoffs" title={document.pareto_fronts[0]?.title ?? "Search telemetry"} /><Suspense fallback={<div className="empty-state">Loading analysis…</div>}><ParetoChart document={document} onSelect={selectPareto} /></Suspense><Telemetry document={document} /></div>}
      </section>

      <section className={`definition-grid${hasObservations ? "" : " no-observations"}`}>
        <div className="panel model-panel"><PanelHeading kicker="The rules" title="Rates, roles, goals & invariants" aside={`${document.model?.rates.length ?? 0} rates`} /><ModelExplorer document={document} /></div>
        {hasObservations && <div className="panel observation-panel"><PanelHeading kicker="Who can see what" title="Actor-relative observations" aside={`Step ${position} · ${document.observations.length} views`} /><Observations document={document} frame={frame} /></div>}
      </section>
      </>;
      })()}
    </main></QuantityFormats.Provider> : <StudyLoading problem={preview?.problem ?? problem?.title} download={preview?.download} />}

    <footer>The picture explains. The accounts, rules, and replay prove.</footer>
  </div>;
}

function StudyActivity({ launching, ongoing, events, elapsedMs }: { launching: boolean; ongoing?: StudySummary; events: StudioEvent[]; elapsedMs: number }) {
  const latest = events.at(-1);
  const progress = [...events].reverse().find((event): event is Extract<StudioEvent, { kind: "progress" }> => event.kind === "progress");
  const completed = progress?.completed ?? ongoing?.completed ?? 0;
  const total = progress?.total ?? ongoing?.total ?? undefined;
  const status = launching ? "Starting study" : ongoing?.status === "paused" ? "Study paused" : "Study in progress";
  const message = latest ? eventMessage(latest) : "Submitting reproducible study request…";
  return <section className={`study-activity ${ongoing?.status === "paused" ? "paused" : ""}`} role="status" aria-live="polite">
    <div className="study-activity-heading"><span className="spinner" aria-hidden="true" /><div><strong>{status}</strong><span>{message}</span></div></div>
    <div className="study-activity-progress">
      {total !== undefined && total > 0 ? <progress max={total} value={Math.min(completed, total)} aria-label="Study phase progress" /> : <div className="indeterminate-progress" />}
      <span>{total === 0 ? `${completed} states expanded` : total !== undefined ? `${completed} / ${total}` : "waiting for first checkpoint"}</span>
    </div>
    <div className="study-activity-meta"><span>{formatDuration(elapsedMs)} elapsed</span>{ongoing && <><span>{ongoing.request.instance ?? "default"} instance</span><span>seed {ongoing.request.seed}</span><span>budget {ongoing.request.budget}</span><code>{ongoing.id}</code></>}</div>
    <VerifiedStandings events={events} live />
  </section>;
}

function VerifiedStandings({ events, live }: { events: StudioEvent[]; live: boolean }) {
  const last = [...events].reverse().find((event): event is Extract<StudioEvent, { kind: "frame_appended" }> => event.kind === "frame_appended");
  const board = last?.frame.after.leaderboards[0];
  if (!last || !board) return null;
  return <div className="live-standings"><span>{live ? "Live" : "Last"} verified frame {last.frame_index + 1} · {board.label}</span>{board.entries.slice(0, 4).map((entry) => <strong key={entry.participant.key}>{entry.rank ? `#${entry.rank}` : "—"} {entry.participant.label} <b>{entry.value}</b></strong>)}</div>;
}

type StudyOrigin = "saved" | "computed" | "file";
type RequestedStudy = { instance: string; strategy: string; seed: number; budget: number };

/** A document a strategy produced, as opposed to one computed beside them. */
function isStrategyDocument(study: Study, document: ViewDocument): boolean {
  return study.problem.strategies.some((strategy) => strategy.key === document.id);
}

/**
 * How an outcome is named wherever Studio lists a study's outcomes: by its
 * strategy's label, as in the Strategy picker. An outcome no strategy names
 * is named by its own title, without the problem's prefix.
 */
function documentName(study: Study, document: ViewDocument): string {
  const strategy = study.problem.strategies.find((candidate) => candidate.key === document.id);
  if (strategy) return strategy.label;
  const separator = document.title.indexOf(" · ");
  if (separator < 0) return document.title;
  const prefix = document.title.slice(0, separator);
  const name = document.title.slice(separator + 3);
  const capitalized = name.charAt(0).toUpperCase() + name.slice(1);
  return prefix.endsWith(" Pareto") ? `Pareto: ${name}` : capitalized;
}

/** The study's outcomes: one per strategy, then any it computed for comparison. */
function OutcomeTabs({ study, selected, onSelect }: { study: Study; selected: string; onSelect: (id: string) => void }) {
  const strategies = study.documents.filter((candidate) => isStrategyDocument(study, candidate));
  const others = study.documents.filter((candidate) => !isStrategyDocument(study, candidate));
  const tab = (candidate: ViewDocument) => <button key={candidate.id} role="tab" aria-selected={candidate.id === selected} onClick={() => onSelect(candidate.id)}>{documentName(study, candidate)}</button>;
  return <section className="alternative-bar">
    <div><span>Outcomes</span><strong>{study.documents.length} replayable {study.documents.length === 1 ? "result" : "results"}</strong></div>
    <div role="tablist" aria-label="Outcomes">
      {strategies.map(tab)}
      {others.length > 0 && <span className="outcome-group" title="The study computed these beside its strategies; none of them is a strategy you can pick.">Also computed for comparison</span>}
      {others.map(tab)}
    </div>
  </section>;
}

function studyKeyWords(problem: ProblemDescriptor | Study["problem"], key: RequestedStudy): string {
  const instance = problem.instances.find((candidate) => candidate.profile === key.instance)?.label ?? key.instance;
  const strategy = problem.strategies.find((candidate) => candidate.key === key.strategy)?.label ?? key.strategy;
  return `${instance} · ${strategy} · seed ${key.seed} · budget ${key.budget}`;
}

function originWords(origin: StudyOrigin): string {
  return origin === "saved" ? "Saved study" : origin === "computed" ? "Computed study" : "Study from file";
}

/** The key of the study on screen, which the controls above may not match. */
function StudyKeyLine({ study, origin, model }: { study: Study; origin: StudyOrigin; model?: string }) {
  const strategy = study.problem.strategies.find((candidate) => candidate.key === study.key.strategy)?.label ?? study.key.strategy;
  return <dl className="study-key" aria-label="Study on screen">
    <div><dt>On screen</dt><dd>{originWords(origin)}</dd></div>
    <div><dt>Instance</dt><dd>{study.instance.label}</dd></div>
    <div><dt>Computed for</dt><dd>{strategy}</dd></div>
    <div><dt>Seed</dt><dd>{study.key.seed}</dd></div>
    <div><dt>Budget</dt><dd>{study.key.budget}</dd></div>
    {model && <div><dt>Model</dt><dd>{model}</dd></div>}
  </dl>;
}

/**
 * The controls ask for a study; the screen shows one. When they differ, say
 * which was asked for, which is shown, and how to get the other.
 */
function StudyMismatch({ problem, study, origin, requested, canStudy }: { problem: ProblemDescriptor; study: Study; origin: StudyOrigin; requested: RequestedStudy; canStudy: boolean }) {
  if (study.problem.key !== problem.key) return null;
  const differs = requested.instance !== study.key.instance
    || requested.seed !== study.key.seed
    || requested.budget !== study.key.budget
    || !study.documents.some((candidate) => candidate.id === requested.strategy);
  if (!differs) return null;
  const shown = studyKeyWords(study.problem, { instance: study.key.instance, strategy: study.key.strategy, seed: study.key.seed, budget: study.key.budget });
  const missing = origin === "saved" ? "There is no saved study for it" : "It is not the study on screen";
  return <div className="study-mismatch" role="status">
    <strong>The controls ask for a study that is not loaded</strong>
    <span>You asked for {problem.title} · {studyKeyWords(problem, requested)}. {missing}, so this is the {originWords(origin).toLowerCase()} {shown}. {canStudy ? "Press Study to compute the one you asked for." : "Start an engine to compute the one you asked for."}</span>
  </div>;
}

function CompletionBanner({ notice, onDismiss }: { notice: CompletionNotice; onDismiss: () => void }) {
  return <section className="completion-banner" role="status">
    <div className="completion-mark" aria-hidden="true">✓</div>
    <div><strong>New study computed and loaded</strong><span>{notice.problem} · {notice.instance} · {notice.strategy} · seed {notice.seed} · budget {notice.budget}</span><small>{notice.documents} replayable {notice.documents === 1 ? "outcome" : "outcomes"} · completed in {formatDuration(notice.durationMs)} · <code>{notice.studyId}</code></small></div>
    <button type="button" onClick={onDismiss}>Dismiss</button>
  </section>;
}

/**
 * Search checkpoints are thinking; preparing the study and recording its
 * documents are bookkeeping around it.
 */
export function isSearchObservation(observation: SearchObservation): boolean {
  return observation.kind !== "phase" && observation.kind !== "document";
}

/** Where a saved review starts: the last thing the search considered. */
function reviewCursor(observations: SearchObservation[]): number {
  for (let index = observations.length - 1; index >= 0; index -= 1) {
    if (isSearchObservation(observations[index])) return index;
  }
  return Math.max(0, observations.length - 1);
}

/** Thinking is what a policy does when it simulates before it chooses: the search's observations, live or saved with the study. */
export function ThinkingWorkspace({ scene, observations, active }: { scene?: Scene | null; observations: SearchObservation[]; active: boolean }) {
  const [cursor, setCursor] = useState(() => active ? Math.max(0, observations.length - 1) : reviewCursor(observations));
  // Live, follow the newest checkpoint; saved, start on the last search one.
  useEffect(() => {
    setCursor(active ? Math.max(0, observations.length - 1) : reviewCursor(observations));
  }, [active, observations]);
  const visibleObservations = observations.slice(0, Math.min(cursor + 1, observations.length));
  const latest = visibleObservations.at(-1);
  const searches = observations.filter(isSearchObservation);
  const kinds = [...new Set(searches.map((observation) => observation.kind))];
  const searchScene = useMemo(() => thinkingScene(scene, visibleObservations), [scene, visibleObservations]);
  const header = <header><div><span className={active ? "spinner" : "thinking-complete"} aria-hidden="true">{active ? "" : "✓"}</span><div><strong>{active ? "Live thinking" : "Saved thinking"}</strong><small>{active ? "What the search considers as it works" : "Saved with the study, so you can review it later"}</small></div></div><span>{searches.length} search {searches.length === 1 ? "checkpoint" : "checkpoints"}</span></header>;
  if (!active && searches.length === 0) {
    return <section className="thinking-workspace retained" aria-label="Thinking">
      {header}
      <div className="thinking-empty" role="note">
        <strong>This strategy records no search</strong>
        <p>The study saved only its preparation and the documents it produced: there are no frontier, rollout or search-tree checkpoints to step through. The step-by-step replay shows every exchange this outcome chose, and Compare outcomes lists any search totals its algorithm reported.</p>
      </div>
    </section>;
  }
  return <section className={`thinking-workspace ${active ? "live" : "retained"}`} aria-label="Thinking">
    {header}
    {latest && <div className="thinking-summary"><div><span>Current phase</span><strong>{phaseLabel(latest.phase)}</strong></div><div><span>Algorithm</span><strong>{algorithmLabel(latest.algorithm)}</strong></div><div><span>Progress</span><strong>{latest.total === 0 ? `${latest.completed} expanded` : `${latest.completed} / ${latest.total}`}</strong></div><div><span>Checkpoint kinds</span><strong>{kinds.length > 0 ? kinds.map(kindLabel).join(" · ") : "None yet"}</strong></div></div>}
    {latest && latest.total > 0 && <progress max={latest.total} value={Math.min(latest.completed, latest.total)} aria-label="Thinking progress" />}
    {observations.length > 1 && <div className="thinking-scrubber"><label><span>Exploration checkpoint</span><strong>{Math.min(cursor + 1, observations.length)} / {observations.length}</strong></label><input name="thinking-position" type="range" min="0" max={observations.length - 1} value={Math.min(cursor, observations.length - 1)} onChange={(event) => setCursor(Number(event.target.value))} aria-label="Thinking position" /></div>}
    <div className={`thinking-body ${searchScene ? "with-map" : "stream-only"}`}>
      {searchScene && <section className="thinking-map" aria-label="Search expansion map"><header><span>Search through encoded economic states</span><strong>{latest?.label ?? "Waiting for an expansion"}</strong></header><SceneView scene={searchScene} motion="seek" /></section>}
      <div className="thinking-stream">{observations.length === 0 ? <div className="empty-state">Waiting for the search's first checkpoint…</div> : observations.map((observation, index) => <button type="button" key={`${observation.sequence}:${observation.phase}`} onClick={() => setCursor(index)} aria-current={index === cursor ? "step" : undefined} className={`thinking-observation-card observation-${observation.kind}`}><div className="thinking-observation-icon"><SceneIcon glyph={observationGlyph(observation.kind)} size={20} /></div><div><strong>{observation.label}</strong><span>{phaseLabel(observation.phase)} · {observation.total === 0 ? `${observation.completed} expanded` : `${observation.completed} / ${observation.total}`}</span></div><div>{observation.metrics.map((metric) => <span key={metric.key}><small>{metric.label}</small><b>{metric.value}</b></span>)}</div></button>)}</div>
    </div>
  </section>;
}

function algorithmLabel(algorithm: string): string {
  const labels: Record<string, string> = {
    "a-star": "A*",
    "breadth-first": "Breadth-first search",
    dijkstra: "Dijkstra",
    "exact-pareto-search": "Exact Pareto search",
  };
  return labels[algorithm] ?? algorithm.replaceAll("-", " ");
}

function thinkingScene(scene: Scene | null | undefined, observations: SearchObservation[]): Scene | undefined {
  if (!scene || scene.surface.kind !== "graph") return undefined;
  const withSubjects = observations.filter((observation) => (observation.subjects?.length ?? 0) > 0);
  if (withSubjects.length === 0) return undefined;
  const explored = new Set(withSubjects.flatMap((observation) => observation.subjects?.map((subject) => subject.key) ?? []));
  const current = new Set(withSubjects.at(-1)?.subjects?.map((subject) => subject.key) ?? []);
  const entities = scene.entities.filter((entity) => entity.role !== "occupant" && !(entity.anchor.kind === "entity"));
  return {
    ...scene,
    title: "Search expansion over the encoded maze",
    metrics: [],
    annotations: [],
    legend: scene.legend.filter((entry) => !["Planned next", "Traversed route", "Current move"].includes(entry.label)),
    entities,
    paths: scene.paths.map((path) => ({
      ...path,
      status: path.status === "blocked" ? "blocked" : "available",
    })),
    surface: {
      ...scene.surface,
      nodes: scene.surface.nodes.map((node) => ({
        ...node,
        classes: [
          ...node.classes.filter((value) => value !== "current" && value !== "search-explored" && value !== "search-current"),
          ...(explored.has(node.id.key) ? ["search-explored"] : []),
          ...(current.has(node.id.key) ? ["search-current"] : []),
        ],
      })),
      edges: scene.surface.edges.map((edge) => ({
        ...edge,
        classes: edge.classes.filter((value) => value !== "current" && value !== "traversed" && value !== "candidate"),
      })),
    },
  };
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
    case "document": return "package";
    case "phase": return "clock";
  }
}

function kindLabel(kind: SearchObservation["kind"]): string {
  const labels: Record<typeof kind, string> = {
    phase: "Progress",
    frontier: "Frontier",
    rollout: "Rollouts",
    tree: "Search tree",
    belief: "Beliefs",
    candidate: "Candidates",
    incumbent: "Solutions",
    prune: "Pruning",
    document: "Documents",
  };
  return labels[kind];
}

function familyLabel(family: NonNullable<Study["problem"]>["family"]): string {
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

function capabilityLabel(capability: Study["problem"]["capabilities"][number]): string {
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
    documents: "Recording documents",
    "fleet-dispatch": "Fleet dispatch",
    "center-opponent": "Game against the center-first opponent",
    "search-opponent": "Game against the depth-five opponent",
    "monte-carlo": "Monte Carlo sampling",
    "pareto-sampling": "Sampling the frontier",
    mcts: "MCTS",
    "mcts-game": "MCTS",
    ismcts: "ISMCTS",
    "multi-agent-match": "Multi-agent match",
    "breadth-first-frontier": "Breadth-first frontier",
    "dijkstra-frontier": "Dijkstra frontier",
    "a-star-frontier": "A* frontier",
    "pareto-frontier": "Exact Pareto frontier",
    "breadth-first-solution": "Breadth-first solution",
    "dijkstra-solution": "Dijkstra solution",
    "a-star-solution": "A* solution",
  };
  return labels[phase] ?? phase.replaceAll("-", " ");
}

function StrategyComparison({ study, selected, onSelect }: { study: Study; selected: string; onSelect: (id: string) => void }) {
  return <details className="strategy-comparison" aria-label="Outcome comparison" open>
    <summary><span>Compare outcomes</span><strong>Every strategy and what it cost, side by side</strong><i aria-hidden="true">▸</i></summary>
    <div className="comparison-scroll"><table><thead><tr><th>Outcome</th><th>Result</th><th>Initial → final state</th><th>Trace</th><th>Algorithm and its work</th></tr></thead><tbody>{study.documents.map((candidate) => {
      const series = candidate.telemetry.find((entry) => entry.subject === "algorithm");
      // The Trace column already counts the exchanges; this one counts the
      // algorithm's own work, each number with the words its algorithm gave it.
      const steps = String(candidate.frames.length);
      const work = series?.points.filter((point) => !(point.value === steps && ["generated", "expanded", "transitions", "message"].includes(point.kind))) ?? [];
      const finalMetrics = (candidate.frames.at(-1)?.after.scene ?? candidate.initial.scene)?.metrics.slice(0, 3) ?? [];
      return <tr key={candidate.id} className={candidate.id === selected ? "selected" : undefined}>
        <td><button type="button" onClick={() => onSelect(candidate.id)}>{documentName(study, candidate)}</button>{!isStrategyDocument(study, candidate) && <small className="outcome-note">for comparison</small>}</td>
        <td>{candidate.objectives.length > 0 ? candidate.objectives.map((objective) => `${objective.label}: ${formatQuantity(objective.value, objective.format ?? undefined)}`).join(" · ") : "Feasibility outcome"}</td>
        <td>{finalMetrics.map((metric) => {
          const initial = candidate.initial.scene?.metrics.find((entry) => entry.key === metric.key);
          return <div key={metric.key}>{metric.label}: {initial && initial.value !== metric.value ? `${initial.value} → ` : ""}{metric.value}{metric.unit ? ` ${metric.unit}` : ""}</div>;
        })}</td>
        <td>{candidate.frames.length} {candidate.frames.length === 1 ? "step" : "steps"}</td>
        <td>{series ? [series.title, series.exact ? "exact" : "sampled", ...(work.length > 0 ? work.map((point) => `${point.value} ${point.label}`) : ["no search counters"])].join(" · ") : "No algorithm telemetry"}</td>
      </tr>;
    })}</tbody></table></div>
  </details>;
}

function LeaderboardDock({ document, snapshot, previous, position, selectedKey, onSelect, onParticipant }: { document: ViewDocument; snapshot: ViewSnapshot; previous?: ViewSnapshot; position: number; selectedKey?: string; onSelect: (key: string) => void; onParticipant: (account: string) => void }) {
  const selected = snapshot.leaderboards.find((leaderboard) => leaderboard.key === selectedKey) ?? snapshot.leaderboards[0];
  if (!selected) return null;
  const previousBoard = previous?.leaderboards.find((leaderboard) => leaderboard.key === selected.key);
  const snapshots = [document.initial, ...document.frames.slice(0, position).map((frame) => frame.after)];
  return <section className="leaderboard-dock rail" aria-label="Replay-derived leaderboards">
    <header><div><span>Standings at this replay step</span><strong>Who is winning depends on what you value</strong></div><div className="leaderboard-step">Economic step <b>{snapshot.index}</b></div></header>
    <label className="leaderboard-selector"><span>Ranking dimension</span><select name="leaderboard" aria-label="Leaderboard ranking dimension" value={selected.key} onChange={(event) => onSelect(event.target.value)}>{snapshot.leaderboards.map((leaderboard) => <option key={leaderboard.key} value={leaderboard.key}>{leaderboard.label} · {leaderboard.direction}</option>)}</select></label>
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

/* Ledger participant inks: brass first, then muted register tones that hold
   on both the ink and paper grounds. */
const PARTICIPANT_INKS = ["#b3924e", "#75828e", "#97896f", "#6d977e", "#a07852", "#84895e"];

function participantColor(key: string): string {
  let hash = 0;
  for (const character of key) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return PARTICIPANT_INKS[hash % PARTICIPANT_INKS.length];
}

function formatDuration(milliseconds: number): string {
  if (milliseconds < 1_000) return `${Math.max(0, Math.round(milliseconds))} ms`;
  const seconds = milliseconds / 1_000;
  return seconds < 10 ? `${seconds.toFixed(1)} s` : `${Math.round(seconds)} s`;
}

/** A saved study on its way: how much has arrived, and of how much when the response says. */
function StudyLoading({ problem, download }: { problem?: string; download?: StudyDownload }) {
  const total = download?.total;
  const received = download?.received ?? 0;
  const amount = total !== undefined ? `${megabytes(received)} of ${megabytes(total)}`
    : received > 0 ? `${megabytes(received)} received${download?.compressed ? ` · ${megabytes(download.compressed)} compressed download` : ""}`
    : "Waiting for the first bytes…";
  return <div className="loading" role="status" aria-live="polite">
    <strong>{problem ? `Loading the saved study for ${problem}…` : "Loading the problem catalog…"}</strong>
    {download && (total !== undefined ? <progress max={total} value={Math.min(received, total)} aria-label="Saved study download" /> : <div className="indeterminate-progress" aria-label="Saved study download" />)}
    {download && <span>{amount}</span>}
  </div>;
}

function megabytes(bytes: number): string {
  return `${(bytes / 1_000_000).toFixed(bytes < 10_000_000 ? 1 : 0)} MB`;
}

function PlaybackControls({ position, count, playing, delay, onDelay, onPosition, onPlaying, frame, onShare, shared }: { position: number; count: number; playing: boolean; delay: number; onDelay: (delay: number) => void; onPosition: (position: number, motion: ReplayMotionMode) => void; onPlaying: (playing: boolean) => void; frame?: ExchangeFrame; onShare?: () => void; shared?: boolean }) {
  return <section className="playback"><div className="transport"><button type="button" aria-label="Go to initial state" disabled={position === 0} onClick={() => { onPlaying(false); onPosition(0, "seek"); }}><IconPlayerSkipBack size={19} stroke={2} aria-hidden="true" /></button><button type="button" aria-label="Previous exchange" disabled={position === 0} onClick={() => { onPlaying(false); onPosition(Math.max(0, position - 1), "step"); }}><IconChevronLeft size={20} stroke={2.1} aria-hidden="true" /></button><button type="button" className="play" aria-label={playing ? "Pause" : "Play"} disabled={count === 0} onClick={() => onPlaying(!playing)}>{playing ? <IconPlayerPauseFilled size={18} aria-hidden="true" /> : <IconPlayerPlayFilled size={18} aria-hidden="true" />}</button><button type="button" aria-label="Next exchange" disabled={position === count} onClick={() => { onPlaying(false); onPosition(Math.min(count, position + 1), "step"); }}><IconChevronRight size={20} stroke={2.1} aria-hidden="true" /></button><button type="button" aria-label="Go to final state" disabled={position === count} onClick={() => { onPlaying(false); onPosition(count, "seek"); }}><IconPlayerSkipForward size={19} stroke={2} aria-hidden="true" /></button></div><div className="scrubber"><div className="scrubber-label"><span>{position === 0 ? "Initial state" : frame?.cues[0]?.label ?? frame?.exchange.rate.label}</span><strong>{position} / {count}</strong></div><input name="trace-position" aria-label="Trace position" type="range" min="0" max={count} value={position} onChange={(event) => onPosition(Number(event.target.value), "seek")} /></div><label className="playback-speed"><span>Speed</span><select name="playback-speed" aria-label="Playback speed" value={delay} onChange={(event) => onDelay(Number(event.target.value))}><option value={1600}>0.5×</option><option value={900}>0.75×</option><option value={650}>1×</option><option value={325}>2×</option><option value={160}>4×</option></select></label><div className="kbd-hint" aria-hidden="true"><b>Space</b> play · <b>←</b><b>→</b> step</div>{onShare && <button type="button" className="share-mini" onClick={onShare}>{shared ? "Link copied ✓" : "Copy link"}</button>}<div className="replay-proof"><span>✓</span> replay verified</div></section>;
}

function eventMessage(event: StudioEvent): string {
  switch (event.kind) {
    case "study_started": return `Started ${event.problem} · ${event.strategy}`;
    case "progress": return `${phaseLabel(event.phase)}: ${event.message}`;
    case "thinking": return event.observation.label;
    case "frame_appended": return `Published replay frame ${event.frame_index + 1}`;
    case "study_published": return `Published ${event.documents} alternatives`;
    case "study_paused": return "The study is waiting for resume.";
    case "study_resumed": return "The study resumed from its last checkpoint.";
    case "study_completed": return "Study completed and replay verified";
    case "study_cancelled": return "Study cancelled by caller";
    case "study_failed": return event.message;
  }
}

export function currentSnapshot(document: ViewDocument | undefined, position: number): ViewSnapshot | undefined {
  if (!document) return undefined;
  return position === 0 ? document.initial : document.frames[position - 1]?.after;
}
