import { useEffect, useMemo, useRef, useState } from "react";
import { keepPreviousData, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { IconMaximize } from "@tabler/icons-react";
import { BrowserPackages } from "./browserPackages";
import { Compare } from "./Compare";
import { connectionLabel, nativeEngineAnswers } from "./engine";
import { HistoryTree, pathOf, type TreeBranch } from "./HistoryTree";
import { Accounts, Assessment, ModelExplorer, PanelHeading, StepTicker, Transition, useSnapshot } from "./Inspectors";
import {
  EngineRefusal,
  loadedPackages,
  nativePackages,
  samplePackages,
  type Candidate,
  type Exchange,
  type ExperimentView,
  type HistoriesView,
  type LoadedFrom,
  type PackageEngine,
  type PackageView,
  type PointId,
  type RanView,
  type RefFrames,
  type ResultsView,
  type SearchProgress,
} from "./packageEngine";
import { branchesOf, lastPart, parsePoint, type BranchInfo, type Note } from "./packageTree";
import { Brand, PlaybackControls, changingBalances } from "./Playback";
import { QuantityFormats } from "./Quantity";
import { SceneView, type ReplayMotionMode } from "./SceneView";
import { Term } from "./Term";

/** The one browser engine of the page: one Worker holds the saved store. */
let browser: BrowserPackages | undefined;
function browserPackages(): BrowserPackages {
  browser ??= new BrowserPackages();
  return browser;
}

/**
 * The engine packages are served by, chosen once: the native server when it
 * answers, else the engine in the browser's Worker. A store is the engine's
 * own, so Studio does not move between them while it is open.
 */
function useEngine(): PackageEngine | undefined {
  const probe = import.meta.env.VITE_AXIONOMY_ENGINE !== "browser" && !window.location.hostname.endsWith(".github.io");
  const chosen = useQuery({
    queryKey: ["package-engine"],
    queryFn: async () => (probe && await nativeEngineAnswers() ? "native" : "browser") as PackageEngine["kind"],
    staleTime: Infinity,
    retry: false,
  });
  if (!chosen.data) return undefined;
  return chosen.data === "native" ? nativePackages : browserPackages();
}

/** What a link asks Studio to open. */
type PackagesLink = {
  /** A package's URL to load first. */
  package?: string;
  /** A loaded package, by name. */
  name?: string;
  ref?: string;
  point?: PointId;
  step?: number;
};

function readLink(): PackagesLink {
  const query = new URLSearchParams(window.location.search);
  const step = query.get("step");
  return {
    package: query.get("package") ?? undefined,
    name: query.get("name") ?? undefined,
    ref: query.get("ref") ?? undefined,
    point: query.get("point") ?? undefined,
    step: step !== null && /^\d+$/.test(step) ? Number(step) : undefined,
  };
}

function writeLink(link: PackagesLink) {
  const query = new URLSearchParams();
  if (link.package) query.set("package", link.package);
  if (link.name) query.set("name", link.name);
  if (link.ref) query.set("ref", link.ref);
  else if (link.point) query.set("point", link.point);
  if (link.step !== undefined) query.set("step", String(link.step));
  const written = query.toString();
  window.history.replaceState({}, "", `${window.location.pathname}${written ? `?${written}` : ""}${window.location.hash}`);
}

/**
 * The history on screen: up to a ref's point, or to the tip of a branch
 * picked in the tree or reached some other way.
 */
type Opened = {
  point: PointId;
  /** The ref naming the point, when it was opened as a ref's. */
  ref?: string;
  /** The package that ships the ref, which fixes it: it never moves. */
  shipped?: string;
};

/** A point written briefly: its run's last digits and its step. */
export function shortPoint(point: PointId): string {
  const [run, step] = point.split(":");
  return `${run.slice(0, 4)}…${run.slice(-6)}:${step}`;
}

/** What Studio says an action did: a confirmation, or a caution worth reading. */
type Notice = { tone: "done" | "caution"; message: string };
const done = (message: string): Notice => ({ tone: "done", message });
const caution = (message: string): Notice => ({ tone: "caution", message });

/** `count` of a thing, its word in the singular or the plural. */
function counted(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

/**
 * A sample's URL as a link writes it: relative to Studio's own address, so
 * a link names `packages/maze.zip` wherever Studio is served.
 */
function linkPath(url: string): string {
  const base = new URL(import.meta.env.BASE_URL, window.location.origin).href;
  return url.startsWith(base) ? url.slice(base.length) : url;
}

function stepOf(point: PointId): number {
  return Number(point.split(":")[1]);
}

function described(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/** A value as it waits: settled once it has not changed for `delay` ms. */
function useSettled<T>(value: T, delay: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(value), delay);
    return () => window.clearTimeout(timer);
  }, [value, delay]);
  return settled;
}

/** The value of the package select that opens a sample not loaded yet. */
const SAMPLE = "sample:";

/** Packages, their history trees, and what to do from any point of them. */
export function PackagesApp() {
  const engine = useEngine();
  const queryClient = useQueryClient();
  const link = useRef(readLink()).current;
  const kind = engine?.kind;
  const kept = useQuery({ queryKey: ["store", kind], queryFn: () => engine!.store(), enabled: Boolean(engine), staleTime: Infinity });
  const listing = useQuery({ queryKey: ["packages", kind], queryFn: () => engine!.packages(), enabled: Boolean(engine) && kept.isSuccess, placeholderData: keepPreviousData });
  const samples = useQuery({ queryKey: ["sample-packages"], queryFn: samplePackages, staleTime: Infinity, retry: false });
  const [packageName, setPackageName] = useState<string>();
  const [opened, setOpened] = useState<Opened>();
  const [loadedUrl, setLoadedUrl] = useState<string | undefined>(link.package);
  const [busy, setBusy] = useState<string>();
  const [notices, setNotices] = useState<Notice[]>([]);
  const [error, setError] = useState<string>();
  const [copied, setCopied] = useState(false);
  /** What made branches in this tab, by run. */
  const [notes, setNotes] = useState<ReadonlyMap<string, Note>>(new Map());
  const note = (point: PointId, label: string, from: PointId) => setNotes((current) => new Map(current).set(parsePoint(point).run, { label, words: `${label}, from step ${stepOf(from)}` }));
  /** Says what the latest action did, in place of what the one before it said. */
  const report = (...said: Notice[]) => setNotices(said);
  // A confirmation goes by itself; a caution stays until it is dismissed.
  useEffect(() => {
    if (!notices.some((said) => said.tone === "done")) return;
    const timer = window.setTimeout(() => setNotices((current) => current.filter((said) => said.tone !== "done")), 5_000);
    return () => window.clearTimeout(timer);
  }, [notices]);
  const pkg = listing.data?.packages.find((candidate) => candidate.name === packageName);

  // The history tree stays the store's: each change arrives as it is made.
  const histories = useQuery({ queryKey: ["histories", kind, packageName], queryFn: () => engine!.histories(packageName!), enabled: Boolean(engine && packageName && pkg), placeholderData: keepPreviousData });
  useEffect(() => {
    if (!engine) return;
    return engine.onChange((change) => {
      if (change.kind === "run") {
        queryClient.setQueriesData<HistoriesView>({ queryKey: ["histories", engine.kind] }, (current) => {
          const from = change.segment.from;
          if (!current || !from) return current;
          const runs = new Set(current.segments.map((segment) => parsePoint(segment.tip).run));
          // A branch of another package's tree is not this one's.
          if (!runs.has(parsePoint(from).run) || current.segments.some((segment) => segment.tip === change.segment.tip)) return current;
          return { segments: [...current.segments, change.segment] };
        });
      } else if (change.kind === "ref") {
        void queryClient.invalidateQueries({ queryKey: ["packages", engine.kind] });
      } else {
        for (const key of ["packages", "histories", "results"]) void queryClient.invalidateQueries({ queryKey: [key, engine.kind] });
      }
    });
  }, [engine, queryClient]);
  const recorded = useQueries({
    queries: (pkg?.experiments ?? []).filter((experiment) => experiment.recorded).map((experiment) => ({
      queryKey: ["results", kind, experiment.experiment],
      queryFn: () => engine!.results(experiment.experiment),
      enabled: Boolean(engine),
    })),
  });
  const results = recorded.flatMap((query) => query.data ? [query.data] : []);

  // The step to show once the history is drawn: the end, unless a link or a pick names one.
  const pendingStep = useRef<"end" | number>(link.step ?? "end");
  const [position, setPosition] = useState(0);
  const [motion, setMotion] = useState<ReplayMotionMode>("seek");
  const [playing, setPlaying] = useState(false);
  const [delay, setDelay] = useState(650);
  // Play where nothing has happened yet finds the moves, then plays them as
  // soon as their history is drawn.
  const playOnOpen = useRef(false);
  const [finding, setFinding] = useState<{ cancel(): void }>();
  const findingStopped = useRef(false);
  const [focusedAccount, setFocusedAccount] = useState<string>();
  /** The history the one on screen is set against: picked (a point), closed (null), or the one it left (undefined). */
  const [compareWith, setCompareWith] = useState<PointId | null>();
  const worldPanel = useRef<HTMLDivElement>(null);
  const proofPanel = useRef<HTMLDivElement>(null);

  const drawn = useQuery({
    queryKey: ["draw", kind, opened?.point],
    queryFn: () => engine!.draw(opened!.point),
    enabled: Boolean(engine && opened),
    // A point never changes, so neither does its history.
    staleTime: Infinity,
    // The last history stays on screen while the next is drawn.
    placeholderData: keepPreviousData,
  });
  const document = drawn.data;
  // A history drawn for another point than the one opened, shown until the next is drawn.
  const stale = !document || document.id !== `point/${opened?.point.replace(":", "/")}`;
  const steps = document?.frames.length ?? 0;
  const snapshot = useSnapshot(document, position);
  const previous = useSnapshot(document, Math.max(0, position - 1));
  const frame = position > 0 ? document?.frames[position - 1] : undefined;
  const changing = useMemo(() => changingBalances(document), [document]);

  useEffect(() => setCompareWith(undefined), [opened?.point]);

  useEffect(() => {
    if (!document) return;
    const wanted = pendingStep.current;
    pendingStep.current = "end";
    setPosition(wanted === "end" ? document.frames.length : Math.min(wanted, document.frames.length));
    setMotion("step");
    setPlaying(playOnOpen.current);
    playOnOpen.current = false;
    setFocusedAccount(undefined);
  }, [document?.id]);

  useEffect(() => {
    if (!playing || !document) return;
    const timer = window.setInterval(() => {
      setMotion("step");
      setPosition((current) => {
        if (current >= document.frames.length) { setPlaying(false); return current; }
        return current + 1;
      });
    }, delay);
    return () => window.clearInterval(timer);
  }, [playing, document, delay]);

  // The point at the cursor: the opened point at the end of its history,
  // else the point at that step of it, named once the cursor settles.
  const settled = useSettled(position, 250);
  const atEnd = position === steps;
  const earlier = useQuery({
    queryKey: ["ancestor", kind, opened?.point, settled],
    queryFn: () => engine!.ancestor(opened!.point, settled),
    enabled: Boolean(engine && opened && !stale && settled < steps && settled === position),
    staleTime: Infinity,
  });
  const screenPoint: PointId | undefined = !opened || stale ? undefined : atEnd ? opened.point : settled === position ? earlier.data : undefined;
  // At a ref's point acting moves the ref, unless a package ships it.
  const onRefPoint = Boolean(opened?.ref) && !opened?.shipped && atEnd;
  const objectives = useMemo(() => pkg?.objectives.map((objective) => objective.objective) ?? [], [pkg]);
  const follow = useFollowing(engine, opened?.shipped ? undefined : opened?.ref, opened?.point, objectives, (point) => {
    pendingStep.current = "end";
    setOpened((current) => current?.ref ? { point, ref: current.ref, shipped: current.shipped } : current);
  });

  /** The packages as the store holds them now, never a listing cached before a change. */
  const relist = () => queryClient.fetchQuery({ queryKey: ["packages", kind], queryFn: () => engine!.packages(), staleTime: 0 });

  /** Opens `name`'s point as it is now. */
  const openRef = async (name: string, step: "end" | number = "end") => {
    if (!engine) return;
    try {
      const ref = await engine.readRef(name);
      pendingStep.current = step;
      setOpened({ point: ref.point, ref: name, shipped: ref.shipped_by ?? undefined });
    } catch (cause) { setError(described(cause)); }
  };

  /** Opens the history up to `point`, at `step` of it (its end by default), playing it when asked. */
  const openPoint = (point: PointId, step: "end" | number = "end", play = false) => {
    const ref = pkg?.refs.find((candidate) => candidate.point === point);
    pendingStep.current = step;
    playOnOpen.current = play;
    setOpened({ point, ref: ref?.name, shipped: ref?.shipped_by ?? undefined });
  };

  const tree = useMemo(() => branchesOf(histories.data?.segments ?? [], pkg, results, notes, (name) => void openRef(name)),
    // Results are compared by what they hold, so a refetch that changed nothing redraws nothing.
    [histories.data, pkg, JSON.stringify(results), notes]);
  const selection = opened && !stale ? { id: parsePoint(opened.point).run, end: stepOf(opened.point), cursor: position } : undefined;
  /** A step of the tree picked: on the history on screen the cursor moves there; elsewhere that branch opens, there. */
  const pick = (id: string, step: number) => {
    setPlaying(false);
    const onScreen = pathOf(tree.branches, selection).get(id);
    if (onScreen && step <= (selection?.end ?? 0) && (step >= onScreen.from || step === 0)) {
      setMotion("seek");
      setPosition(step);
      return;
    }
    const branch = tree.branches.find((candidate) => candidate.id === id);
    if (branch) openPoint(`${id}:${branch.last}`, step);
  };

  const selectPackage = (name: string, refs = listing.data) => {
    setPackageName(name);
    const first = refs?.packages.find((candidate) => candidate.name === name)?.refs[0];
    if (first) void openRef(first.name);
    else setOpened(undefined);
  };

  /** Loads a package, says what it did, and opens the first package it held. */
  const load = async (what: string, loading: () => Promise<LoadedFrom>, open = true) => {
    setBusy(`Loading ${what}…`);
    setError(undefined);
    report();
    try {
      const from = await loading();
      const names = loadedPackages(from);
      if (from.got === "current") report(done(`Opened ${names.join(", ")}: the store already holds what ${what} serves.`));
      else if (from.got === "kept") report(caution(`${what} could not be reached (${from.message}); the store's copy of ${names.join(", ")} is shown.`));
      else report(
        done(`Loaded ${names.join(", ")} from ${what}.`),
        ...(from.loaded.replaced ?? []).map((release) => caution(`Replaced ${release.name} ${release.version}: what it shipped and this version does not carry is gone; refs of your own stay.`)),
        ...from.loaded.kept.refs.map((ref) => caution(`Kept ref ${ref}: the store already names another point with it.`)),
        ...(from.loaded.kept.definitions.length > 0 ? [caution(`Kept ${counted(from.loaded.kept.definitions.length, "definition")} the store already defines otherwise.`)] : []),
      );
      const list = await relist();
      if (open && names[0]) selectPackage(names[0], list);
      return names;
    } catch (cause) {
      setError(described(cause));
      return [];
    } finally {
      setBusy(undefined);
    }
  };

  /** Loads the sample at `url` and opens it; a link names it relative to Studio. */
  const openSample = (url: string) => {
    if (!url || !engine) return;
    const path = linkPath(url);
    setLoadedUrl(path);
    void load(path, () => engine.loadUrl(url));
  };

  // Once the engine's store is open: a linked package's URL first, then
  // what the link names, else the first package the store holds.
  const started = useRef(false);
  useEffect(() => {
    if (!engine || !kept.isSuccess || !listing.data || started.current) return;
    started.current = true;
    void (async () => {
      let list = listing.data;
      let names: string[] = [];
      if (link.package) {
        const url = new URL(link.package, window.location.href).href;
        names = await load(link.package, () => engine.loadUrl(url), false);
        list = await relist();
      }
      const name = link.name ?? names[0] ?? list.packages[0]?.name;
      if (!name) return;
      setPackageName(name);
      const named = list.packages.find((candidate) => candidate.name === name);
      if (link.ref && named?.refs.some((ref) => ref.name === link.ref)) void openRef(link.ref, link.step ?? "end");
      else if (link.point) {
        pendingStep.current = link.step ?? "end";
        setOpened({ point: link.point });
      } else if (named?.refs[0]) void openRef(named.refs[0].name);
    })();
  }, [engine, kept.isSuccess, listing.data]);

  useEffect(() => {
    if (!started.current) return;
    writeLink({ package: loadedUrl, name: packageName, ref: opened?.ref, point: opened?.point, step: document ? position : undefined });
  }, [loadedUrl, packageName, opened, position, document?.id]);

  /** Applies `exchange` at the point on screen: at a ref of one's own the ref moves over it; anywhere else it makes a branch. */
  const act = async (exchange: Exchange, label: string) => {
    if (!engine || !opened || !screenPoint) return;
    const at = screenPoint;
    const ref = onRefPoint ? opened.ref : undefined;
    try {
      const applied = await engine.apply(at, [exchange]);
      note(applied.point, label, at);
      if (ref) {
        try {
          await engine.advanceRef(ref, at, applied.point);
          report(done(`${label}: ${ref} moved to step ${stepOf(applied.point)}.`));
          pendingStep.current = "end";
          setOpened({ point: applied.point, ref });
        } catch (cause) {
          if (!(cause instanceof EngineRefusal) || !cause.moved) throw cause;
          report(caution(`${ref} moved to ${shortPoint(cause.moved.current)} before this step counted, so the step is a branch; look again from where the ref is.`));
          openPoint(applied.point);
        }
      } else {
        report(done(`${label}: a new branch from step ${stepOf(at)}.`));
        openPoint(applied.point);
      }
    } catch (cause) { setError(described(cause)); }
  };

  /**
   * Lets `strategy` play from the point on screen toward `asked.goal`; the
   * branch it makes opens at the step it started from and plays. Asked again
   * while it searches, it stops.
   */
  const letPlay = async (strategy: string, asked: Asked) => {
    if (finding) { findingStopped.current = true; finding.cancel(); return; }
    if (!engine || !screenPoint || !asked.goal) return;
    report();
    const work = engine.runStrategy(screenPoint, { strategy, goal: asked.goal, seed: asked.seed, budget: asked.budget, turns: asked.turns }, () => undefined);
    setFinding({ cancel: work.cancel });
    try {
      const ran = await work.result;
      const found = stepOf(ran.end) - stepOf(ran.start);
      if (found === 0) { report(caution(`${lastPart(strategy)} found no move toward ${asked.goal} from here: ${outcomeWords(ran.outcome).toLowerCase()}.`)); return; }
      note(ran.end, lastPart(strategy), ran.start);
      report(done(`${lastPart(strategy)} ${outcomeWords(ran.outcome).toLowerCase()} in ${counted(found, "step")}${ran.thinking ? `, after looking at ${ran.thinking.expanded.toLocaleString()} states` : ""}.`));
      openPoint(ran.end, stepOf(ran.start), true);
    } catch (cause) {
      if (!findingStopped.current) setError(described(cause));
    } finally {
      setFinding(undefined);
      findingStopped.current = false;
    }
  };
  /** Play where nothing has happened yet: the strategy the package's first experiment runs, else its first. */
  const firstStrategy = pkg?.experiments[0]?.arms[0]?.strategy ?? pkg?.strategies[0];
  const playAhead = () => {
    if (!firstStrategy) { report(caution(`${pkg?.name ?? "This package"} defines no strategy to find moves with; apply a move to make some.`)); return; }
    void letPlay(firstStrategy, askedOf(pkg, firstStrategy));
  };

  // The keys the transport names: Space plays or pauses (or finds moves to
  // play), the arrows step. Keys typed into a field or a control stay theirs.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target instanceof Element ? event.target : undefined;
      if (!document || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || target?.closest("input, textarea, select, button, [contenteditable], [role=button]")) return;
      if (event.key === " ") {
        event.preventDefault();
        if (steps === 0) { playAhead(); return; }
        if (!playing && position >= steps) { setMotion("seek"); setPosition(0); }
        setPlaying(!playing);
      } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        setPlaying(false);
        setMotion("step");
        setPosition((current) => Math.min(steps, Math.max(0, current + (event.key === "ArrowLeft" ? -1 : 1))));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  /** Watches a trial: its branch opens at the step it started from, and plays. */
  const watch = (trial: { end: PointId; start: PointId }) => openPoint(trial.end, stepOf(trial.start), true);

  /** Deletes the ref on screen from the point it is on; the history stays on screen. */
  const deleteRef = async () => {
    if (!engine || !opened?.ref) return;
    const ref = opened.ref;
    try {
      await engine.deleteRef(ref, opened.point);
      report(done(`${ref} is deleted; its history stays in the tree.`));
      setOpened({ point: opened.point });
    } catch (cause) { setError(described(cause)); }
  };

  const keepAsRef = async (name: string) => {
    if (!engine || !screenPoint) return;
    try {
      await engine.createRef(name, screenPoint);
      report(done(`${name} names step ${stepOf(screenPoint)} now.`));
      pendingStep.current = "end";
      setOpened({ point: screenPoint, ref: name });
    } catch (cause) { setError(described(cause)); }
  };

  const copyLink = async () => {
    await navigator.clipboard.writeText(window.location.href).catch(() => undefined);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  };

  const label = engine ? connectionLabel(engine.kind, "connected") : connectionLabel("native", "checking");
  const storeWords = !engine ? "" : engine.kind === "native" ? "Store on the server" : kept.data?.persistent ? "Store saved in this browser" : `Store in memory${kept.data?.reason ? `: ${kept.data.reason}` : ""}`;
  const toggleTheater = () => {
    const node = worldPanel.current;
    if (!node) return;
    if (window.document.fullscreenElement) void window.document.exitFullscreen();
    else void node.requestFullscreen().catch(() => undefined);
  };
  const loaded = new Set(listing.data?.packages.map((candidate) => candidate.name));
  const unloaded = (samples.data ?? []).filter((sample) => !loaded.has(sample.name));
  const onScreen = selection ? tree.branches.find((branch) => branch.id === selection.id) : undefined;
  const historyName = opened?.ref ?? onScreen?.title ?? "this history";
  // A branch is set against the history it left, unless another is picked, or the comparison is closed.
  const left = onScreen?.parent ? tree.branches.find((branch) => branch.id === onScreen.parent?.id) : undefined;
  const against = compareWith === null ? undefined : compareWith ?? (left ? `${left.id}:${left.last}` : undefined);
  const againstBranch = against ? tree.branches.find((branch) => branch.id === parsePoint(against).run) : undefined;
  const rootOf = (id: string) => {
    let branch = tree.branches.find((candidate) => candidate.id === id);
    while (branch?.parent) branch = tree.branches.find((candidate) => candidate.id === branch?.parent?.id) ?? undefined;
    return branch?.id;
  };

  return <div className="app-shell packages-workspace">
    <header className="topbar">
      <Brand />
      <div className="connection" title={engine?.kind === "native" ? "Verified by a live health check" : "Rust/WASM engine isolated in a Web Worker"}><span className={`status-dot ${engine ? "online" : "static"}`} />{label}</div>
    </header>

    <section className="command-bar" aria-label="Package controls">
      <label className="field package-field"><span><Term name="package">Package</Term></span><select name="package" value={packageName ?? ""} disabled={!engine || Boolean(busy)} onChange={(event) => {
        const value = event.target.value;
        if (value.startsWith(SAMPLE)) openSample(value.slice(SAMPLE.length));
        else if (value) selectPackage(value);
      }}>
        {!packageName && <option value="">Open a package…</option>}
        {(listing.data?.packages.length ?? 0) > 0 && <optgroup label="In the store">{listing.data?.packages.map((candidate) => <option key={candidate.name} value={candidate.name}>{candidate.name} {candidate.version}</option>)}</optgroup>}
        {unloaded.length > 0 && <optgroup label="Samples">{unloaded.map((sample) => <option key={sample.url} value={`${SAMPLE}${sample.url}`}>{sample.name}</option>)}</optgroup>}
      </select></label>
      <label className="file-button">Load a file<input name="package-file" type="file" accept=".zip,.redb,application/zip" disabled={!engine || Boolean(busy)} onChange={(event) => {
        const file = event.target.files?.[0];
        event.target.value = "";
        if (!file || !engine) return;
        setLoadedUrl(undefined);
        void load(file.name, () => engine.loadFile(file));
      }} /></label>
      <button type="button" onClick={copyLink} disabled={!pkg}>{copied ? "Link copied ✓" : "Copy link"}</button>
      <div className="engine-message">{storeWords}</div>
    </section>

    {(busy || notices.length > 0) && <div className="status-layer">
      {busy && <div className="activity" role="status"><div className="activity-heading"><span className="spinner" aria-hidden="true" /><div><strong>{busy}</strong><span>Its manifests are checked before it is merged into the store</span></div></div></div>}
      {notices.map((said) => <div className={`notice notice-${said.tone}`} role="status" key={said.message}>{said.message}{said.tone === "caution" && <button type="button" onClick={() => setNotices((current) => current.filter((candidate) => candidate !== said))}>Dismiss</button>}</div>)}
    </div>}
    {error && <div className="error-banner" role="alert">{error}<button type="button" onClick={() => setError(undefined)}>Dismiss</button></div>}

    {!engine || !listing.data ? <div className="loading" role="status"><strong>{engine ? "Opening the engine's store…" : "Looking for an engine…"}</strong><div className="indeterminate-progress" /></div>
      : listing.data.packages.length === 0 ? <EmptyCatalog samples={samples.data ?? []} busy={Boolean(busy)} onOpen={openSample} />
      : <main className="point-cockpit" aria-busy={stale}>
        <section className="panel tree-panel" aria-label="History tree">
          <PanelHeading kicker={<>Every <Term name="history">history</Term> of {pkg?.name ?? "the package"}, and its <Term name="branch">branches</Term></>} title={document?.title ?? pkg?.name ?? ""} aside={counted(tree.branches.length, "branch", "branches")} />
          {histories.isError ? <div className="assessment-issue">{described(histories.error)}</div>
            : !histories.data ? <div className="empty-state">Drawing the tree…</div>
            : <HistoryTree branches={tree.branches} selection={selection} onSelect={pick} label="History tree" describe={(branch, step) => <BranchCard branch={branch} step={step} info={tree.info.get(branch.id)} branches={tree.branches}
              onCompare={opened && selection && branch.id !== selection.id && rootOf(branch.id) === rootOf(selection.id) ? () => setCompareWith(`${branch.id}:${branch.last}`) : undefined} />} />}
        </section>
        {!opened ? <div className="empty-state">This package names no point. Load another.</div>
          : !document ? <div className="loading" role="status"><strong>{drawn.isError ? described(drawn.error) : "Drawing the history…"}</strong>{!drawn.isError && <div className="indeterminate-progress" />}</div>
          : snapshot && <QuantityFormats.Provider value={document.formats ?? {}}>
            <PlaybackControls position={position} count={steps} playing={playing} delay={delay} onDelay={setDelay} onPosition={(next, how) => { setMotion(how); setPosition(next); }} onPlaying={(next) => { if (next && position >= steps) { setMotion("seek"); setPosition(0); } setPlaying(next); }} onPlayAhead={playAhead} finding={Boolean(finding)} frame={frame} />
            <section className="stage point-stage">
              <div className="panel world-panel" ref={worldPanel}>
                <PanelHeading kicker="The picture" title={snapshot.scene?.title ?? document.title} aside={snapshot.scene?.surface.kind} action={<button type="button" className="theater-btn" onClick={toggleTheater} title="Theater mode" aria-label="Toggle theater mode"><IconMaximize size={16} stroke={1.8} aria-hidden="true" /></button>} />
                <SceneView scene={snapshot.scene} previousScene={previous?.scene} history={[document.initial, ...document.frames.slice(0, position).map((candidate) => candidate.after)].flatMap((candidate) => candidate.scene ? [candidate.scene] : [])} frame={frame} motion={motion} onAccount={setFocusedAccount} />
              </div>
              <div className="panel action-panel">
                <PanelHeading kicker="From here" title={`Step ${position} of ${historyName}`} />
                <FromHere engine={engine} pkg={pkg} at={playing ? undefined : screenPoint} playing={playing} opened={opened} atEnd={atEnd} onRefPoint={onRefPoint} position={position}
                  onApply={act} finding={Boolean(finding)} onLetPlay={letPlay} results={results} onWatch={watch} onKeep={keepAsRef} onDelete={deleteRef} follow={follow} objectives={objectives} />
              </div>
              <div className="truth-rail">
                <div className="panel ticker-panel">
                  <PanelHeading kicker="This step" title={position === 0 ? "Initial state" : frame?.cues[0]?.label ?? frame?.exchange.rate.label ?? "Exchange"} aside={`${position} / ${steps}`} />
                  <StepTicker frame={frame} steps={steps} onProof={() => proofPanel.current?.scrollIntoView({ behavior: "smooth", block: "start" })} />
                </div>
                <div className="panel accounts-panel">
                  <PanelHeading kicker="Source of truth" title="Accounts & assets" aside={focusedAccount ? "linked from picture" : counted(snapshot.accounts.length, "account")} />
                  <Accounts snapshot={snapshot} previous={previous} focus={focusedAccount} changingBalanceKeys={changing} />
                </div>
              </div>
            </section>
            {against && against !== opened.point && !stale && <Compare engine={engine} left={opened.point} right={against} leftName={historyName} rightName={againstBranch?.title ?? "the history it left"}
              leftDocument={document} objectives={objectives} onStep={(step) => { setPlaying(false); setMotion("seek"); setPosition(step); }} onClose={() => setCompareWith(null)} />}
            <section className="definition-grid proof-grid">
              <div className="panel transition-panel" ref={proofPanel}><PanelHeading kicker="Proof of this step" title={frame?.exchange.rate.label ?? "Initial state"} /><Transition frame={frame} steps={steps} /></div>
              <div className="panel model-panel"><PanelHeading kicker="The rules" title="Rates, roles, goals & invariants" aside={counted(document.model?.rates.length ?? 0, "rate")} /><ModelExplorer document={document} /></div>
            </section>
          </QuantityFormats.Provider>}
      </main>}

    <footer>The picture explains. The accounts, rules, and replay prove.</footer>
  </div>;
}

/** The page before any package is loaded: the samples, each a click from open. */
function EmptyCatalog({ samples, busy, onOpen }: { samples: { name: string; url: string }[]; busy: boolean; onOpen: (url: string) => void }) {
  return <div className="empty-catalog" role="note">
    <strong>Open a <Term name="package">package</Term></strong>
    <p>{samples.length > 0 ? "Pick a sample, or load a package file of your own." : "Load a package file, a store file or a zip of one."}</p>
    {samples.length > 0 && <ul className="sample-list" aria-label="Sample packages">{samples.map((sample) => <li key={sample.url}><button type="button" disabled={busy} onClick={() => onOpen(sample.url)}>{sample.name}</button></li>)}</ul>}
  </div>;
}

/** What a hover over a step of the tree says of its branch. */
function BranchCard({ branch, step, info, branches, onCompare }: { branch: TreeBranch; step: number; info?: BranchInfo; branches: TreeBranch[]; onCompare?: () => void }) {
  const parent = branch.parent ? branches.find((candidate) => candidate.id === branch.parent?.id) : undefined;
  const trial = info?.trial;
  return <div className="branch-card">
    <strong>{branch.title}</strong>
    <span>Step {step}{step === branch.last ? ", its last" : ` of ${branch.last}`}</span>
    {branch.parent && <span>A <Term name="branch">branch</Term> from step {branch.parent.step} of {parent?.title ?? "another history"}</span>}
    {info?.refs.map((ref) => <span key={ref.name}><Term name="ref">{ref.name}</Term> names step {ref.step}{ref.fixed ? <>, <Term name="fixed">fixed</Term></> : ""}</span>)}
    {trial && <span><Term name="trial">Trial</Term> {trial.index + 1} of <Term name="experiment">{trial.experiment}</Term>, {lastPart(trial.strategy)}: {endedWords(trial.trial.ended)}{trial.objectives.length > 0 && ` · ${trial.objectives.map((objective, index) => `${lastPart(objective.objective)} ${trial.trial.values[index]}`).join(" · ")}`}</span>}
    {info?.note && !trial && <span>Made in this tab: {info.note.words}</span>}
    <small>Click to open it at this step.</small>
    {onCompare && <button type="button" onClick={onCompare}>Compare with the history on screen</button>}
  </div>;
}

/** An exchange's roles and accounts, in the view's words. */
function bindingWords(candidate: Candidate): string {
  const exchange = candidate.proposal.exchange;
  return [...exchange.bindings.map((binding) => `${binding.role.label}: ${binding.account.label}`), `×${exchange.units}`].join(" · ");
}

/** A whole number below 2^64, written in decimal. */
function natural(text: string): boolean {
  return /^(0|[1-9]\d*)$/.test(text) && BigInt(text) < 2n ** 64n;
}

/** The effort Studio gives a strategy no experiment runs. */
const DEFAULT_EFFORT = { seed: "17", budget: "1000000", turns: "1000" };

/** What a strategy is asked: the goal it plays toward and the effort it is given. */
type Asked = { goal?: string; seed: string; budget: string; turns: string };

/**
 * What an experiment that runs `strategy` asks of it, when one does: the
 * goal it works toward and the effort it is given. Otherwise the package's
 * first goal and Studio's own effort.
 */
function askedOf(pkg: PackageView | undefined, strategy: string | undefined): Asked {
  const experiment = pkg?.experiments.find((candidate) => candidate.arms.some((arm) => arm.strategy === strategy));
  if (!experiment) return { goal: pkg?.goals[0], ...DEFAULT_EFFORT };
  return { goal: experiment.goal, seed: experiment.seed, budget: experiment.budget, turns: experiment.turns };
}

function outcomeWords(outcome: RanView["outcome"]): string {
  switch (outcome.kind) {
    case "reached": return "Reached the goal";
    case "stopped": return "Stopped short of the goal";
    case "horizon": return "Played every turn it was given";
    case "refused": return `Was refused on turn ${outcome.turn}`;
    case "exhausted": return "Explored everything without reaching the goal";
    case "spent": return "Spent its budget first";
    case "interrupted": return "Stopped before it ended";
  }
}

function endedWords(ended: ResultsView["arms"][number]["trials"][number]["ended"]): string {
  switch (ended.kind) {
    case "reached": return "reached the goal";
    case "stopped": return "stopped short of the goal";
    case "horizon": return "played every turn";
    case "refused": return `refused on turn ${ended.turn}`;
    case "exhausted": return "explored everything";
    case "spent": return "spent its budget";
    case "interrupted": return "stopped before it ended";
  }
}

/** Everything one can do from the point on screen, each a click from done. */
function FromHere({ engine, pkg, at, playing, opened, atEnd, onRefPoint, position, onApply, finding, onLetPlay, results, onWatch, onKeep, onDelete, follow, objectives }: {
  engine: PackageEngine;
  pkg?: PackageView;
  at?: PointId;
  playing: boolean;
  opened: Opened;
  atEnd: boolean;
  onRefPoint: boolean;
  position: number;
  onApply: (exchange: Exchange, label: string) => Promise<void>;
  finding: boolean;
  onLetPlay: (strategy: string, asked: Asked) => Promise<void>;
  results: ResultsView[];
  onWatch: (trial: { end: PointId; start: PointId }) => void;
  onKeep: (name: string) => void;
  onDelete: () => void;
  follow: Following;
  objectives: string[];
}) {
  const experiments = (pkg?.experiments ?? []).filter((experiment) => at && pkg?.refs.find((ref) => ref.name === experiment.start)?.point === at);
  const what = onRefPoint ? <>What you do here moves <Term name="ref">{opened.ref}</Term> forward.</>
    : opened.shipped && atEnd ? <><Term name="fixed">{opened.ref} is fixed</Term>: what you do here makes a <Term name="branch">branch</Term>.</>
    : <>What you do here makes a <Term name="branch">branch</Term> from step {position}.</>;
  if (!at) return <div className="from-here"><p className="from-what">{playing ? "Playing. Pause at any step to act from it." : `Finding step ${position}…`}</p></div>;
  return <div className="from-here">
    <p className="from-what">{what}</p>
    <Moves engine={engine} pkg={pkg} at={at} onApply={onApply} />
    <LetPlay pkg={pkg} finding={finding} onLetPlay={onLetPlay} />
    {experiments.length > 0 && <section className="from-section" aria-label="Experiments from here">
      <h3><Term name="experiment">Experiments</Term> from here</h3>
      {experiments.map((experiment) => <ExperimentHere key={experiment.experiment} engine={engine} experiment={experiment} recorded={results.find((result) => result.experiment === experiment.experiment)} onWatch={onWatch} />)}
    </section>}
    <NameHere pkg={pkg} opened={opened} onRefPoint={onRefPoint} onKeep={onKeep} onDelete={onDelete} follow={follow} objectives={objectives} />
    <HandWritten engine={engine} at={at} onApply={onApply} />
  </div>;
}

/** The moves the package's rules offer here, each checked, each one click from applied. */
function Moves({ engine, pkg, at, onApply }: { engine: PackageEngine; pkg?: PackageView; at: PointId; onApply: (exchange: Exchange, label: string) => Promise<void> }) {
  const [set, setSet] = useState(pkg?.candidates[0]);
  useEffect(() => setSet(pkg?.candidates[0]), [pkg?.name]);
  const offered = useQuery({
    queryKey: ["candidates", engine.kind, at, set],
    queryFn: () => engine.candidates(at, set!),
    enabled: Boolean(set),
    staleTime: Infinity,
  });
  const [open, setOpen] = useState<number>();
  const [applying, setApplying] = useState(false);
  useEffect(() => setOpen(undefined), [at]);
  if (!pkg || pkg.candidates.length === 0) return null;
  const candidates = offered.data?.candidates ?? [];
  return <section className="from-section" aria-label="Moves here">
    <h3><Term name="move">Moves</Term> here</h3>
    {pkg.candidates.length > 1 && <label className="field"><span>Offered by</span><select name="candidates" value={set ?? ""} onChange={(event) => setSet(event.target.value)}>{pkg.candidates.map((candidate) => <option key={candidate} value={candidate}>{lastPart(candidate)}</option>)}</select></label>}
    {offered.isError ? <div className="assessment-issue">{described(offered.error)}</div>
      : !offered.data ? <p className="muted">Asking the rules…</p>
      : candidates.length === 0 ? <p className="muted">No move is offered here.</p>
      : <ul className="move-list">{candidates.map((candidate, index) => {
        const applies = candidate.proposal.assessment.status === "applicable";
        return <li key={index} className={`move move-${candidate.proposal.assessment.status}`}>
          <button type="button" className="move-name" aria-expanded={open === index} onClick={() => setOpen(open === index ? undefined : index)} title={bindingWords(candidate)}>
            <strong>{candidate.proposal.label}</strong>
            <small>{applies ? "applies" : candidate.proposal.assessment.status.replaceAll("_", " ")}</small>
          </button>
          <button type="button" className="primary" disabled={!applies || applying} onClick={async () => { setApplying(true); try { await onApply(candidate.exchange, candidate.proposal.label); } finally { setApplying(false); } }}>Apply</button>
          {open === index && <div className="move-detail"><span className="muted">{bindingWords(candidate)}</span><Assessment assessment={candidate.proposal.assessment} /></div>}
        </li>;
      })}</ul>}
  </section>;
}

/** A strategy played from here: the branch it makes opens and plays. */
function LetPlay({ pkg, finding, onLetPlay }: { pkg?: PackageView; finding: boolean; onLetPlay: (strategy: string, asked: Asked) => Promise<void> }) {
  const first = pkg?.experiments[0]?.arms[0]?.strategy ?? pkg?.strategies[0];
  const [strategy, setStrategy] = useState(first);
  const [asked, setAsked] = useState<Asked>(() => askedOf(pkg, first));
  useEffect(() => { setStrategy(first); setAsked(askedOf(pkg, first)); }, [pkg?.name]);
  if (!pkg || pkg.strategies.length === 0 || pkg.goals.length === 0) return null;
  const valid = natural(asked.seed) && natural(asked.budget) && BigInt(asked.budget) > 0n && natural(asked.turns);
  return <section className="from-section" aria-label="Let a strategy play">
    <h3>Let a <Term name="strategy">strategy</Term> play</h3>
    <div className="let-play">
      <select name="strategy" aria-label="Strategy" value={strategy} disabled={finding} onChange={(event) => { setStrategy(event.target.value); setAsked(askedOf(pkg, event.target.value)); }}>{pkg.strategies.map((candidate) => <option key={candidate} value={candidate}>{lastPart(candidate)}</option>)}</select>
      <button type="button" className={finding ? "danger" : "primary"} disabled={!strategy || !asked.goal || !valid} onClick={() => strategy && void onLetPlay(strategy, asked)}>{finding ? "Stop" : "Play it"}</button>
    </div>
    <details className="settings">
      <summary>Toward {lastPart(asked.goal ?? "")}, seed {asked.seed}</summary>
      <div className="settings-fields">
        <label className="field"><span><Term name="goal">Goal</Term></span><select name="goal" value={asked.goal} disabled={finding} onChange={(event) => setAsked({ ...asked, goal: event.target.value })}>{pkg.goals.map((goal) => <option key={goal} value={goal}>{lastPart(goal)}</option>)}</select></label>
        <label className="field"><span><Term name="seed">Seed</Term></span><input name="seed" inputMode="numeric" value={asked.seed} disabled={finding} onChange={(event) => setAsked({ ...asked, seed: event.target.value.trim() })} /></label>
        <label className="field"><span><Term name="budget">Budget</Term></span><input name="budget" inputMode="numeric" value={asked.budget} disabled={finding} onChange={(event) => setAsked({ ...asked, budget: event.target.value.trim() })} /></label>
        <label className="field"><span><Term name="turns">Turns</Term></span><input name="turns" inputMode="numeric" value={asked.turns} disabled={finding} onChange={(event) => setAsked({ ...asked, turns: event.target.value.trim() })} /></label>
      </div>
      {!valid && <div className="assessment-issue">A seed, a budget and turns are whole numbers, the budget at least 1.</div>}
    </details>
  </section>;
}

/** An experiment that starts here: run it, and watch each trial it played. */
function ExperimentHere({ engine, experiment, recorded, onWatch }: { engine: PackageEngine; experiment: ExperimentView; recorded?: ResultsView; onWatch: (trial: { end: PointId; start: PointId }) => void }) {
  const queryClient = useQueryClient();
  const [running, setRunning] = useState<{ cancel(): void; progress?: SearchProgress }>();
  const [said, setSaid] = useState<string>();
  const cancelled = useRef(false);
  const run = async () => {
    setSaid(undefined);
    const before = recorded;
    const work = engine.runExperiment(experiment.experiment, (progress) => setRunning((current) => current && { ...current, progress }));
    setRunning({ cancel: work.cancel });
    try {
      const next = await work.result;
      queryClient.setQueryData(["results", engine.kind, experiment.experiment], next);
      const same = before && JSON.stringify(before.arms.map((arm) => arm.trials.map((trial) => [trial.ended, trial.values]))) === JSON.stringify(next.arms.map((arm) => arm.trials.map((trial) => [trial.ended, trial.values])));
      setSaid(!before ? "Played: each trial is a new branch in the tree." : same ? "Played again: the same seeds gave the same results, as new branches." : "Played again: the results changed.");
    } catch (cause) { setSaid(cancelled.current ? "Stopped: nothing it played is kept." : described(cause)); } finally { setRunning(undefined); cancelled.current = false; }
  };
  const trials = experiment.trials === "1" ? "" : ` × ${experiment.trials} trials`;
  return <div className="experiment-here">
    <div className="experiment-line">
      <span><strong>{lastPart(experiment.experiment)}</strong> <span className="muted">{experiment.arms.length} <Term name="arm">arms</Term>{trials} toward {lastPart(experiment.goal)}</span></span>
      {running ? <button type="button" className="danger" onClick={() => { cancelled.current = true; running.cancel(); }}>Stop</button>
        : <button type="button" className="primary" onClick={() => void run()}>{recorded ? "Play again" : "Play each arm"}</button>}
    </div>
    {running && <p className="muted" role="status">{running.progress ? `Playing · ${running.progress.expanded.toLocaleString()} states looked at` : "Playing every arm…"}</p>}
    {said && !running && <p className="muted" role="status">{said}</p>}
    {recorded && <ul className="trial-list">{recorded.arms.flatMap((arm) => arm.trials.map((trial, index) => <li key={`${arm.arm.strategy}:${index}`}>
      <span><strong>{lastPart(arm.arm.strategy)}</strong>{arm.trials.length > 1 ? ` · trial ${index + 1}` : ""} <span className="muted">{endedWords(trial.ended)}, {counted(stepOf(trial.end) - stepOf(arm.start), "step")}{recorded.objectives.length > 0 && ` · ${recorded.objectives.map((objective, column) => `${lastPart(objective.objective)} ${trial.values[column]}`).join(" · ")}`}</span></span>
      <button type="button" disabled={trial.end === arm.start} onClick={() => onWatch({ end: trial.end, start: arm.start })}>Watch</button>
    </li>))}</ul>}
  </div>;
}

/** Naming the point on screen, or following and deleting the ref of one's own on it. */
function NameHere({ pkg, opened, onRefPoint, onKeep, onDelete, follow, objectives }: { pkg?: PackageView; opened: Opened; onRefPoint: boolean; onKeep: (name: string) => void; onDelete: () => void; follow: Following; objectives: string[] }) {
  // The first of `<package>/mine`, `<package>/mine-2`, … that no ref of the package has.
  const suggested = useMemo(() => {
    const taken = new Set(pkg?.refs.map((ref) => ref.name));
    const base = `${pkg?.name ?? "studio"}/mine`;
    let name = base;
    for (let next = 2; taken.has(name); next += 1) name = `${base}-${next}`;
    return name;
  }, [pkg]);
  const [name, setName] = useState(suggested);
  useEffect(() => setName(suggested), [suggested, opened.point]);
  if (onRefPoint && opened.ref) {
    const { following, setFollowing, delivered, problem } = follow;
    return <section className="from-section" aria-label="Your ref">
      <h3>Your <Term name="ref">ref</Term> {opened.ref}</h3>
      <div className="let-play">
        <button type="button" className={following ? "danger" : ""} onClick={() => setFollowing(!following)}>{following ? "Stop following" : <>Follow it</>}</button>
        <button type="button" onClick={onDelete}>Delete it</button>
      </div>
      {following && <p className="muted"><Term name="follow">Following</Term>: each step it moves over arrives here.</p>}
      {problem && <div className="assessment-issue" role="alert">{problem}</div>}
      {delivered.length > 0 && <ol className="delivered-frames" aria-label="Delivered steps">{delivered.map((frame) => <li key={frame.point}>
        <strong>step {stepOf(frame.point)}</strong><span>{frame.exchange.rate} ×{frame.exchange.units}</span>
        {frame.values.length > 0 && <small>{frame.values.map((value, index) => `${lastPart(objectives[index] ?? "")} ${value}`).join(" · ")}</small>}
      </li>)}</ol>}
    </section>;
  }
  return <section className="from-section" aria-label="Name this point">
    <h3>Name this point</h3>
    <form className="let-play" onSubmit={(event) => { event.preventDefault(); if (name.trim()) onKeep(name.trim()); }}>
      <input name="ref-name" aria-label="Ref name" value={name} onChange={(event) => setName(event.target.value)} />
      <button type="submit" disabled={!name.trim()}>Name it</button>
    </form>
    <p className="muted">A <Term name="ref">ref</Term> of your own moves forward as you play from it.</p>
  </section>;
}

/** An exchange written by hand, checked before it is applied: out of the way unless wanted. */
function HandWritten({ engine, at, onApply }: { engine: PackageEngine; at: PointId; onApply: (exchange: Exchange, label: string) => Promise<void> }) {
  const [written, setWritten] = useState("");
  const [assessed, setAssessed] = useState<{ text: string; candidate: Candidate }>();
  const [problem, setProblem] = useState<string>();
  useEffect(() => { setAssessed(undefined); setProblem(undefined); }, [at]);
  const check = async () => {
    setProblem(undefined);
    let exchange: Exchange;
    try { exchange = JSON.parse(written) as Exchange; } catch (cause) { setProblem(`Not an exchange: ${described(cause)}`); return; }
    try { setAssessed({ text: written, candidate: await engine.assess(at, exchange) }); } catch (cause) { setProblem(described(cause)); }
  };
  const ready = assessed && assessed.text === written && assessed.candidate.proposal.assessment.status === "applicable";
  return <details className="exchange-editor">
    <summary>Write a move by hand</summary>
    <textarea name="exchange" aria-label="Exchange" spellCheck={false} rows={6} value={written} placeholder={'{"rate": "…", "bindings": {"role": "account"}, "units": "1"}'} onChange={(event) => { setWritten(event.target.value); setAssessed(undefined); }} />
    <div className="let-play">
      <button type="button" onClick={() => void check()} disabled={!written.trim()}>Check it</button>
      <button type="button" className="primary" disabled={!ready} onClick={() => assessed && void onApply(assessed.candidate.exchange, assessed.candidate.proposal.label)}>Apply</button>
    </div>
    {problem && <div className="assessment-issue" role="alert">{problem}</div>}
    {assessed && assessed.text === written && <Assessment assessment={assessed.candidate.proposal.assessment} />}
  </details>;
}

type Following = {
  following: boolean;
  setFollowing: (following: boolean) => void;
  delivered: RefFrames["frames"];
  problem?: string;
};

/**
 * Follows `refName` from `point` while asked to: the steps it is moved over
 * after the last one handled, each as it is delivered, the long poll asked
 * again as soon as it answers. `onMoved` hears where the ref is once every
 * step delivered so far has arrived. Following stops when the ref changes.
 */
function useFollowing(engine: PackageEngine | undefined, refName: string | undefined, point: PointId | undefined, objectives: string[], onMoved: (point: PointId) => void): Following {
  const [following, setFollowing] = useState(false);
  const [delivered, setDelivered] = useState<RefFrames["frames"]>([]);
  const [problem, setProblem] = useState<string>();
  const moved = useRef(onMoved);
  moved.current = onMoved;
  const asked = objectives.join(",");
  useEffect(() => { setFollowing(false); setDelivered([]); setProblem(undefined); }, [refName]);
  useEffect(() => {
    if (!following || !engine || !refName || !point) return;
    const abort = new AbortController();
    let after = point;
    void (async () => {
      while (!abort.signal.aborted) {
        try {
          const answer = await engine.frames(refName, { after, wait_seconds: 20, limit: 100, objectives: asked || undefined }, abort.signal);
          if (abort.signal.aborted) return;
          setProblem(undefined);
          const last = answer.frames.at(-1);
          if (last) {
            setDelivered((current) => [...current, ...answer.frames].slice(-30));
            after = last.point;
            if (last.point === answer.point) moved.current(answer.point);
          }
        } catch (cause) {
          if (abort.signal.aborted) return;
          setProblem(described(cause));
          await new Promise((resolve) => setTimeout(resolve, 1_000));
        }
      }
    })();
    return () => abort.abort();
    // The loop follows from where it started; the points it delivers move the screen, not it.
  }, [following, refName, engine, asked]);
  return { following, setFollowing, delivered, problem };
}
