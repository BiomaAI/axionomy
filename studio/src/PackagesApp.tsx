import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { keepPreviousData, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { IconChevronDown, IconChevronRight, IconMaximize } from "@tabler/icons-react";
import { ActivityStrip, type Activity, type Said } from "./Activity";
import type { AccountData, ModelData, ViewDocument } from "./api";
import { BrowserPackages } from "./browserPackages";
import { Compare } from "./Compare";
import { connectionLabel, nativeEngineAnswers } from "./engine";
import { ExchangeEditor } from "./ExchangeEditor";
import { ExchangeView, Holding } from "./ExchangeView";
import { HistoryTree, pathOf, type TreeBranch } from "./HistoryTree";
import { IdLink, InspectionProvider, InspectorDock, PanelHeading, subjectKey, useSnapshot, type Subject } from "./Inspectors";
import { inspector } from "./inspect";
import { StateList } from "./Ledger";
import { ExperimentView, endedWords } from "./PackageViews";
import {
  EngineRefusal,
  loadedPackages,
  nativePackages,
  samplePackages,
  type Exchange,
  type ExperimentProgress,
  type ExperimentView as ExperimentData,
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
import { Formatted, QuantityFormats } from "./Quantity";
import { RateTable } from "./RateView";
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

const done = (message: string): Said => ({ tone: "done", message });
const caution = (message: string): Said => ({ tone: "caution", message });
const failed = (cause: unknown): Said => ({ tone: "error", message: described(cause) });

/** `count` of a thing, its word in the singular or the plural. */
function counted(count: number, one: string, many = `${one}s`): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
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

/** How often, at most, the page draws a run's progress, in ms. */
const PROGRESS_SHOWN_EVERY = 200;

/**
 * `show` called with the latest value pushed, at most once every
 * {@link PROGRESS_SHOWN_EVERY} ms. A run reports progress as fast as it
 * makes it; drawing each report would keep the page's thread so busy that
 * reports queue up behind it, the run's answer last among them.
 */
export function coalesced<T>(show: (value: T) => void): { push: (value: T) => void; stop: () => void } {
  let latest: { value: T } | undefined;
  let timer: number | undefined;
  return {
    push: (value) => {
      latest = { value };
      timer ??= window.setTimeout(() => {
        timer = undefined;
        if (latest) show(latest.value);
        latest = undefined;
      }, PROGRESS_SHOWN_EVERY);
    },
    stop: () => {
      window.clearTimeout(timer);
      timer = undefined;
      latest = undefined;
    },
  };
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
  const [said, setSaid] = useState<Said[]>([]);
  const [copied, setCopied] = useState(false);
  /** What made branches in this tab, by run. */
  const [notes, setNotes] = useState<ReadonlyMap<string, Note>>(new Map());
  const note = (point: PointId, label: string, from: PointId) => setNotes((current) => new Map(current).set(parsePoint(point).run, { label, words: `${label}, from step ${stepOf(from)}` }));
  /** Says what the latest action came to, in place of what the one before it said. */
  const report = (...words: Said[]) => setSaid(words);
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
  /** A strategy playing from a point, and how far it is. */
  const [finding, setFinding] = useState<{ cancel(): void; strategy: string; from: PointId; progress?: SearchProgress }>();
  const findingStopped = useRef(false);
  /** The experiments playing now, by path, each with how far it is. */
  const [experimenting, setExperimenting] = useState<ReadonlyMap<string, ExperimentRun>>(new Map());
  /** What each experiment's last play came to, by path. */
  const experimentsStopped = useRef(new Set<string>());
  /** The account last picked in the picture. */
  const [focusedAccount, setFocusedAccount] = useState<string>();
  /** The history the one on screen is set against: picked (a point), closed (null), or the one it left (undefined). */
  const [compareWith, setCompareWith] = useState<PointId | null>();
  const worldPanel = useRef<HTMLDivElement>(null);
  /** The inspectors pinned open, latest last. */
  const [pinned, setPinned] = useState<Subject[]>([]);
  const pin = useCallback((subject: Subject) => setPinned((current) => [...current.filter((each) => subjectKey(each) !== subjectKey(subject)), subject]), []);
  const unpin = useCallback((subject: Subject) => setPinned((current) => current.filter((each) => subjectKey(each) !== subjectKey(subject))), []);
  useEffect(() => setPinned([]), [packageName]);
  const [rulesOpen, setRulesOpen] = useState(false);
  const rulesPanel = useRef<HTMLElement>(null);
  useEffect(() => { if (rulesOpen) rulesPanel.current?.scrollIntoView({ behavior: "smooth", block: "start" }); }, [rulesOpen]);

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
    } catch (cause) { report(failed(cause)); }
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
    report();
    try {
      const from = await loading();
      const names = loadedPackages(from);
      if (from.got === "current") report(done(`Opened ${names.join(", ")}, unchanged.`));
      else if (from.got === "kept") report(caution(`${what} unreachable (${from.message}): opened the stored ${names.join(", ")}.`));
      else report(
        done(`Loaded ${names.join(", ")} from ${what}.`),
        ...(from.loaded.replaced ?? []).map((release) => caution(`Replaced ${release.name} ${release.version}.`)),
        ...from.loaded.kept.refs.map((ref) => caution(`Kept the store's ref ${ref}.`)),
        ...(from.loaded.kept.definitions.length > 0 ? [caution(`Kept the store's ${counted(from.loaded.kept.definitions.length, "definition")}.`)] : []),
      );
      const list = await relist();
      if (open && names[0]) selectPackage(names[0], list);
      return names;
    } catch (cause) {
      report(failed(cause));
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
      // A link naming what the store does not hold opens the first package
      // there is, and says so.
      const stored = (name?: string) => list.packages.find((candidate) => candidate.name === name);
      const named = stored(link.name) ?? stored(names[0]) ?? list.packages[0];
      if (!named) return;
      const missing = (words: string) => setSaid((current) => [...current, caution(`${words}: opened ${named.name}.`)]);
      if (link.name && link.name !== named.name) missing(`No package ${link.name} is stored here`);
      setPackageName(named.name);
      if (link.ref && named.refs.some((ref) => ref.name === link.ref)) void openRef(link.ref, link.step ?? "end");
      else if (link.ref) {
        if (link.name === named.name) missing(`${named.name} has no ref ${link.ref}`);
        if (named.refs[0]) void openRef(named.refs[0].name);
      } else if (link.point) {
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
          report(caution(`${label}: a branch, as ${ref} had moved to ${shortPoint(cause.moved.current)}.`));
          openPoint(applied.point);
        }
      } else {
        report(done(`${label}: a branch from step ${stepOf(at)}.`));
        openPoint(applied.point);
      }
    } catch (cause) { report(failed(cause)); }
  };

  /** Stops the strategy playing, which then says nothing of it. */
  const stopFinding = () => {
    if (!finding) return;
    findingStopped.current = true;
    finding.cancel();
  };
  /**
   * Lets `strategy` play from the point on screen toward `asked.goal`; the
   * branch it makes opens at the step it started from and plays. Asked again
   * while it searches, it stops.
   */
  const letPlay = async (strategy: string, asked: Asked) => {
    if (finding) { stopFinding(); return; }
    if (!engine || !screenPoint || !asked.goal) return;
    report();
    const from = screenPoint;
    const shown = coalesced((progress: SearchProgress) => setFinding((current) => current && { ...current, progress }));
    const work = engine.runStrategy(from, { strategy, goal: asked.goal, seed: asked.seed, budget: asked.budget, turns: asked.turns }, shown.push);
    setFinding({ cancel: work.cancel, strategy, from });
    try {
      const ran = await work.result;
      const found = stepOf(ran.end) - stepOf(ran.start);
      if (found === 0) { report(ran.outcome.kind === "reached" ? done(`${asked.goal} holds here already.`) : caution(`${strategy}: no steps toward ${asked.goal}; ${outcomeWords(ran.outcome).toLowerCase()}.`)); return; }
      note(ran.end, lastPart(strategy), ran.start);
      report(done(`${strategy}: ${outcomeWords(ran.outcome).toLowerCase()} in ${counted(found, "step")}${ran.thinking ? `, ${counted(ran.thinking.expanded, "state")} looked at` : ""}.`));
      openPoint(ran.end, stepOf(ran.start), true);
    } catch (cause) {
      if (!findingStopped.current) report(failed(cause));
    } finally {
      shown.stop();
      setFinding(undefined);
      findingStopped.current = false;
    }
  };
  /** Play where nothing has happened yet: the first strategy that plays this history. */
  const playAhead = () => {
    const first = runs.strategies[0];
    if (!first) { report(caution(`No strategy of ${pkg?.name ?? "this package"} runs on this history.`)); return; }
    void letPlay(first.strategy, first.asked);
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

  /** Plays every arm of `experiment`; each trial it plays is a new branch. */
  const playExperiment = async (experiment: ExperimentData) => {
    const path = experiment.experiment;
    if (!engine || experimenting.has(path)) return;
    const before = results.find((result) => result.experiment === path);
    const shown = coalesced((progress: ExperimentProgress) => setExperimenting((current) => {
      const run = current.get(path);
      return run ? changed(current, path, { ...run, progress }) : current;
    }));
    const work = engine.runExperiment(path, shown.push);
    setExperimenting((current) => changed(current, path, { cancel: work.cancel, experiment }));
    try {
      const next = await work.result;
      queryClient.setQueryData(["results", engine.kind, path], next);
      const ends = (played: ResultsView) => JSON.stringify(played.arms.map((arm) => arm.trials.map((trial) => [trial.ended, trial.values])));
      report(done(!before ? `Ran ${path}.` : ends(before) === ends(next) ? `Ran ${path} again: same results.` : `Ran ${path} again: results changed.`));
    } catch (cause) {
      report(experimentsStopped.current.has(path) ? caution(`Stopped ${path}: no results kept.`) : failed(cause));
    } finally {
      shown.stop();
      experimentsStopped.current.delete(path);
      setExperimenting((current) => changed(current, path));
    }
  };
  const stopExperiment = (path: string) => {
    experimentsStopped.current.add(path);
    experimenting.get(path)?.cancel();
  };

  /** Watches a trial: its branch opens at the step it started from, and plays. */
  const watch = (trial: { end: PointId; start: PointId }) => openPoint(trial.end, stepOf(trial.start), true);

  /** Deletes the ref on screen from the point it is on; the history stays on screen. */
  const deleteRef = async () => {
    if (!engine || !opened?.ref) return;
    const ref = opened.ref;
    try {
      await engine.deleteRef(ref, opened.point);
      report(done(`Deleted ${ref}.`));
      setOpened({ point: opened.point });
    } catch (cause) { report(failed(cause)); }
  };

  const keepAsRef = async (name: string) => {
    if (!engine || !screenPoint) return;
    try {
      await engine.createRef(name, screenPoint);
      report(done(`${name} names step ${stepOf(screenPoint)}.`));
      pendingStep.current = "end";
      setOpened({ point: screenPoint, ref: name });
    } catch (cause) { report(failed(cause)); }
  };

  const copyLink = async () => {
    await navigator.clipboard.writeText(window.location.href).catch(() => undefined);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  };

  const label = engine ? connectionLabel(engine.kind, "connected") : connectionLabel("native", "checking");
  const storeWords = !engine ? "Looking for an engine…" : engine.kind === "native" ? "Saved on the server" : kept.data?.persistent ? "Saved in this browser" : `Not saved${kept.data?.reason ? `: ${kept.data.reason}` : ""}`;
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
  const activities: Activity[] = [
    ...(busy ? [{ key: "loading", words: busy }] : []),
    ...(finding ? [{
      key: "strategy",
      words: <>Running <strong>{finding.strategy}</strong> from step {stepOf(finding.from)}{finding.progress && ` · ${counted(finding.progress.expanded, "state")} looked at`}</>,
      stop: { label: "Stop", onStop: stopFinding },
    }] : []),
    ...[...experimenting].map(([path, run]) => ({ key: `experiment:${path}`, words: experimentWords(run), stop: { label: "Stop", onStop: () => stopExperiment(path) } })),
    ...(follow.following && opened?.ref ? [{
      key: "following",
      words: <>Following <strong>{opened.ref}</strong>{follow.delivered.length > 0 && ` · ${counted(follow.delivered.length, "step")} arrived`}</>,
      stop: { label: "Stop following", onStop: () => follow.setFollowing(false) },
    }] : []),
    // The transport, just below, pauses it.
    ...(playing && document ? [{ key: "playback", words: <>Playing back <strong>{historyName}</strong>: step {position} of {steps}</> }] : []),
  ];
  const rootOf = (id: string) => {
    let branch = tree.branches.find((candidate) => candidate.id === id);
    while (branch?.parent) branch = tree.branches.find((candidate) => candidate.id === branch?.parent?.id) ?? undefined;
    return branch?.id;
  };
  const screenRoot = opened ? rootOf(parsePoint(opened.point).run) : undefined;
  const inTree = (ref: string) => {
    const point = pkg?.refs.find((candidate) => candidate.name === ref)?.point;
    return point !== undefined && screenRoot !== undefined && rootOf(parsePoint(point).run) === screenRoot;
  };
  const runs = playable(pkg, document?.source.key, inTree);
  const stepWords = position === 0 ? "Initial state" : frame?.cues[0]?.label ?? frame?.exchange.rate.label ?? "Exchange";
  const model = document?.model ?? undefined;
  const inspect = inspector({ document, position, snapshot, previous, changing, pkg, results, onOpenRef: (name) => void openRef(name), onWatch: watch });
  /** An account the picture picked: marked in the state list, and its inspector pinned. */
  const pickAccount = (key: string) => {
    setFocusedAccount(key);
    pin({ kind: "account", id: snapshot?.accounts.find((account) => account.account.key === key)?.account ?? { key, label: key } });
  };

  return <InspectionProvider value={{ inspect, pin }}>
  <QuantityFormats.Provider value={document?.formats ?? {}}>
  <div className={`app-shell packages-workspace${pinned.length > 0 ? " docked" : ""}`}>
    <header className="topbar">
      <Brand />
      <div className="connection" title={engine?.kind === "native" ? "Verified by a live health check" : "Rust/WASM engine isolated in a Web Worker"}><span className={`status-dot ${engine ? "online" : "static"}`} />{label}</div>
    </header>
    <ActivityStrip activities={activities} said={said} store={storeWords} onReplaceStore={engine instanceof BrowserPackages && kept.data?.replaceable ? () => void engine.replaceStore().then(() => window.location.reload(), (cause: unknown) => report(failed(cause))) : undefined} />

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
      <button type="button" aria-expanded={rulesOpen} onClick={() => setRulesOpen(!rulesOpen)} disabled={!model}>Rules</button>
    </section>


    {!engine || !listing.data ? <div className="loading" role="status"><strong>{engine ? "Opening the engine's store…" : "Looking for an engine…"}</strong><div className="indeterminate-progress" /></div>
      : listing.data.packages.length === 0 ? <EmptyCatalog samples={samples.data ?? []} busy={Boolean(busy)} onOpen={openSample} />
      : <main className="point-cockpit" aria-busy={stale}>
        <section className="panel tree-panel" aria-label="History tree">
          <PanelHeading kicker={<><Term name="history">History</Term> tree</>} title={document?.title ?? pkg?.name ?? ""} aside={counted(tree.branches.length, "branch", "branches")} />
          {histories.isError ? <div className="assessment-issue">{described(histories.error)}</div>
            : !histories.data ? <div className="empty-state">Loading…</div>
            : <HistoryTree branches={tree.branches} selection={selection} onSelect={pick} label="History tree" describe={(branch, step) => <BranchCard branch={branch} step={step} info={tree.info.get(branch.id)} branches={tree.branches}
              onCompare={opened && selection && branch.id !== selection.id && rootOf(branch.id) === rootOf(selection.id) ? () => setCompareWith(`${branch.id}:${branch.last}`) : undefined} />} />}
        </section>
        {!opened ? <div className="empty-state">No refs</div>
          : !document ? <div className="loading" role="status"><strong>{drawn.isError ? described(drawn.error) : "Loading…"}</strong>{!drawn.isError && <div className="indeterminate-progress" />}</div>
          : snapshot && <>
            <PlaybackControls position={position} count={steps} playing={playing} delay={delay} onDelay={setDelay} onPosition={(next, how) => { setMotion(how); setPosition(next); }} onPlaying={(next) => { if (next && position >= steps) { setMotion("seek"); setPosition(0); } setPlaying(next); }} onPlayAhead={playAhead} finding={Boolean(finding)} label={stepWords} />
            <section className="stage point-stage">
              <div className="panel world-panel" ref={worldPanel}>
                <PanelHeading kicker="Picture" title={snapshot.scene?.title ?? document.title} aside={snapshot.scene?.surface.kind} action={<button type="button" className="theater-btn" onClick={toggleTheater} title="Theater mode" aria-label="Toggle theater mode"><IconMaximize size={16} stroke={1.8} aria-hidden="true" /></button>} />
                <SceneView scene={snapshot.scene} previousScene={previous?.scene} history={[document.initial, ...document.frames.slice(0, position).map((candidate) => candidate.after)].flatMap((candidate) => candidate.scene ? [candidate.scene] : [])} frame={frame} motion={motion} onAccount={pickAccount} />
              </div>
              <div className="panel action-panel">
                <PanelHeading kicker="From here" title={`Step ${position} of ${historyName}`} aside={onRefPoint ? <Term name="ref">your ref</Term> : opened.shipped && atEnd ? <Term name="fixed">fixed</Term> : undefined} />
                <FromHere engine={engine} pkg={pkg} document={document} accounts={snapshot.accounts} runs={runs} runsKey={`${pkg?.name}:${screenRoot}:${document?.source.key}`} at={playing ? undefined : screenPoint} playing={playing} opened={opened} onRefPoint={onRefPoint} position={position}
                  onApply={act} finding={Boolean(finding)} onLetPlay={letPlay} results={results} onWatch={watch} onKeep={keepAsRef} onDelete={deleteRef} follow={follow} objectives={objectives}
                  experimenting={experimenting} onPlayExperiment={(experiment) => void playExperiment(experiment)} onStopExperiment={stopExperiment} />
              </div>
              <div className="truth-rail">
                <div className="panel ticker-panel">
                  <PanelHeading kicker="This step" title={stepWords} aside={position === 0 ? `0 / ${steps}` : <IdLink subject={{ kind: "step", step: position }} label={`${position} / ${steps}`} />} />
                  <div className="step-now">{frame ? <ExchangeView exchange={frame.exchange} assessment={frame.assessment} recorded /> : <div className="empty-state">{steps === 0 ? "No steps" : "Initial state"}</div>}</div>
                </div>
                <div className="panel accounts-panel">
                  <PanelHeading kicker="State" title="Accounts" aside={counted(snapshot.accounts.length, "account")} />
                  <StateList accounts={snapshot.accounts} before={position > 0 ? previous?.accounts : undefined} focus={focusedAccount} />
                </div>
              </div>
            </section>
            {rulesOpen && model && <section className="panel rules-panel" aria-label="Rules" ref={rulesPanel}>
              <PanelHeading kicker="Rules" title={document.title} aside={counted(model.rates.length, "rate")} action={<button type="button" onClick={() => setRulesOpen(false)}>Close</button>} />
              <div className="rules-body">
                <RateTable rates={model.rates} />
                {((model.constants ?? []).length > 0 || model.goal.length > 0 || model.invariants.length > 0) && <div className="rules-side">
                  {(model.constants ?? []).length > 0 && <section className="view-section"><h4><Term name="constant">Constants</Term></h4><dl className="facts">{model.constants?.map((constant) => <Fragment key={constant.name}><dt>{constant.name}</dt><dd><Formatted atoms={constant.value} format={constant.format ?? undefined} /></dd></Fragment>)}</dl></section>}
                  {model.goal.length > 0 && <section className="view-section"><h4><Term name="goal">Goal</Term></h4>{model.goal.map((goal) => <div className="effect" key={goal.account.key}><IdLink subject={{ kind: "account", id: goal.account }} /><div className="holdings">{goal.required.map((required) => <Holding key={required.asset.key} value={required} />)}</div></div>)}</section>}
                  {model.invariants.length > 0 && <section className="view-section"><h4>Invariants</h4><dl className="facts">{model.invariants.map((invariant) => <InvariantRow key={invariant.name} invariant={invariant} />)}</dl></section>}
                </div>}
              </div>
            </section>}
            {against && against !== opened.point && !stale && <Compare engine={engine} left={opened.point} right={against} leftName={historyName} rightName={againstBranch?.title ?? "the history it left"}
              leftDocument={document} objectives={objectives} onStep={(step) => { setPlaying(false); setMotion("seek"); setPosition(step); }} onClose={() => setCompareWith(null)} />}
          </>}
      </main>}
    <InspectorDock pinned={pinned} inspect={inspect} onClose={unpin} />
  </div>
  </QuantityFormats.Provider>
  </InspectionProvider>;
}

/** A conservation law: its name, and the weighted sum of assets it keeps. */
function InvariantRow({ invariant }: { invariant: ModelData["invariants"][number] }) {
  return <><dt>{invariant.name}</dt><dd>{invariant.terms.map((term, index) => <span key={term.asset.key}>{index > 0 && " + "}{term.weight !== 1 && `${term.weight}·`}<IdLink subject={{ kind: "asset", id: term.asset }} /></span>)}</dd></>;
}

/** An experiment playing: what it is, how to stop it, and how far it is. */
type ExperimentRun = { cancel(): void; experiment: ExperimentData; progress?: ExperimentProgress };

/** `map` with `key` set to `value`, or without `key` when there is none. */
function changed<V>(map: ReadonlyMap<string, V>, key: string, value?: V): ReadonlyMap<string, V> {
  const next = new Map(map);
  if (value === undefined) next.delete(key);
  else next.set(key, value);
  return next;
}

/** An experiment playing, in words: the arm and the trial it is on, and how far that trial is. */
function experimentWords({ experiment, progress }: ExperimentRun) {
  const trials = Number(experiment.trials);
  const arm = progress && experiment.arms[progress.arm];
  return <>Running <strong>{experiment.experiment}</strong>{arm && progress ? <>: <strong>{arm.strategy}</strong>, arm {progress.arm + 1} of {experiment.arms.length}{trials > 1 && `, trial ${progress.trial + 1} of ${trials}`} · {counted(progress.progress.expanded, "state")} looked at</> : "…"}</>;
}

/** The page before any package is loaded: the samples, each a click from open. */
function EmptyCatalog({ samples, busy, onOpen }: { samples: { name: string; url: string }[]; busy: boolean; onOpen: (url: string) => void }) {
  return <div className="empty-catalog" role="note">
    <strong>Open a <Term name="package">package</Term></strong>
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
    {info?.refs.map((ref) => <span key={ref.name}><IdLink subject={{ kind: "ref", path: ref.name }} /> names step {ref.step}{ref.fixed ? <>, <Term name="fixed">fixed</Term></> : ""}</span>)}
    {trial && <span><Term name="trial">Trial</Term> {trial.index + 1} of <IdLink subject={{ kind: "experiment", path: trial.experiment }} />, <IdLink subject={{ kind: "strategy", path: trial.strategy }} label={lastPart(trial.strategy)} />: {endedWords(trial.trial.ended)}</span>}
    {trial && trial.objectives.length > 0 && <span className="branch-values">{trial.objectives.map((objective, index) => <span key={objective.objective}><IdLink subject={{ kind: "objective", path: objective.objective }} label={lastPart(objective.objective)} /> {trial.trial.values[index]}</span>)}</span>}
    {info?.note && !trial && <span>Made in this tab: {info.note.words}</span>}
    {onCompare && <button type="button" onClick={onCompare}>Compare with the history on screen</button>}
  </div>;
}

/** A whole number below 2^64, written in decimal. */
function natural(text: string): boolean {
  return /^(0|[1-9]\d*)$/.test(text) && BigInt(text) < 2n ** 64n;
}

/** The effort Studio gives a strategy no experiment runs. */
const DEFAULT_EFFORT = { seed: "17", budget: "1000000", turns: "1000" };

/** What a strategy is asked: the goal it plays toward and the effort it is given. */
type Asked = { goal?: string; seed: string; budget: string; turns: string };

/** What can play a history: its strategies, each with what it is asked by default, and the goals it may be asked toward. */
type Playable = { strategies: { strategy: string; asked: Asked }[]; goals: string[] };

/**
 * What runs on a history: the strategies of the arms that start in its tree
 * (an arm is a strategy and the ref it starts from, its own or its
 * experiment's), or, where none does, of the arms of experiments on its
 * ontology; in their order, each asked what its experiment asks, the goal
 * it works toward and the effort it is given. Then the strategies no
 * experiment compares, which may run on any, asked with Studio's own
 * effort. Its goals likewise: those of its arms' experiments, then those no
 * experiment works toward. `inTree` says whether a ref names a point of the
 * history's tree.
 */
function playable(pkg: PackageView | undefined, ontology: string | undefined, inTree: (ref: string) => boolean): Playable {
  if (!pkg) return { strategies: [], goals: [] };
  const arms = pkg.experiments.flatMap((experiment) => experiment.arms.map((arm) => ({ experiment, strategy: arm.strategy, start: arm.start ?? experiment.start })));
  const inThisTree = arms.filter((arm) => inTree(arm.start));
  const here = inThisTree.length > 0 ? inThisTree : arms.filter((arm) => arm.experiment.ontology === ontology);
  const compared = new Set(arms.map((arm) => arm.strategy));
  const aimed = new Set(pkg.experiments.map((experiment) => experiment.goal));
  const goals = [...new Set(here.map((arm) => arm.experiment.goal)), ...pkg.goals.filter((goal) => !aimed.has(goal))];
  const strategies = new Map<string, Asked>();
  for (const { experiment, strategy } of here) {
    if (!strategies.has(strategy)) strategies.set(strategy, { goal: experiment.goal, seed: experiment.seed, budget: experiment.budget, turns: experiment.turns });
  }
  for (const strategy of pkg.strategies) {
    if (!compared.has(strategy)) strategies.set(strategy, { goal: goals[0], ...DEFAULT_EFFORT });
  }
  return { strategies: [...strategies].map(([strategy, asked]) => ({ strategy, asked })), goals };
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

/** Everything one can do from the point on screen, each a click from done. */
function FromHere({ engine, pkg, document, accounts, runs, runsKey, at, playing, opened, onRefPoint, position, onApply, finding, onLetPlay, results, onWatch, onKeep, onDelete, follow, objectives, experimenting, onPlayExperiment, onStopExperiment }: {
  engine: PackageEngine;
  pkg?: PackageView;
  /** The history on screen. */
  document: ViewDocument;
  /** The accounts at the step on screen. */
  accounts: AccountData[];
  /** What runs on the history on screen, and what changes when another history's tree is opened. */
  runs: Playable;
  runsKey: string;
  at?: PointId;
  playing: boolean;
  opened: Opened;
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
  experimenting: ReadonlyMap<string, ExperimentRun>;
  onPlayExperiment: (experiment: ExperimentData) => void;
  onStopExperiment: (path: string) => void;
}) {
  const ids = useMemo(() => accounts.map((account) => account.account), [accounts]);
  if (!at) return <div className="from-here"><p className="from-what">{playing ? "Playing…" : `Finding step ${position}…`}</p></div>;
  return <div className="from-here">
    <Moves engine={engine} pkg={pkg} at={at} onApply={onApply} />
    <StrategyHere key={runsKey} offered={runs} finding={finding} onLetPlay={onLetPlay} />
    <section className="from-section" aria-label="Experiments">
      <h3><Term name="experiment">Experiments</Term></h3>
      {(pkg?.experiments.length ?? 0) === 0 ? <p className="muted">No experiments</p>
        : pkg?.experiments.map((experiment) => {
          const running = experimenting.has(experiment.experiment);
          const recorded = results.find((result) => result.experiment === experiment.experiment);
          return <ExperimentView key={experiment.experiment} experiment={experiment} results={recorded} onWatch={onWatch}
            actions={running ? <button type="button" className="danger" onClick={() => onStopExperiment(experiment.experiment)}>Stop</button>
              : <button type="button" className="primary" onClick={() => onPlayExperiment(experiment)}>{recorded ? "Run again" : "Run"}</button>} />;
        })}
    </section>
    <RefHere pkg={pkg} document={document} at={at} opened={opened} onRefPoint={onRefPoint} onKeep={onKeep} onDelete={onDelete} follow={follow} objectives={objectives} />
    <ExchangeEditor rates={document.model?.rates ?? []} accounts={ids} ontology={document.source.key} point={at}
      onCheck={(exchange) => engine.assess(at, exchange)} onApply={onApply} />
  </div>;
}

/** The moves the package's rules offer here, each checked, each one click from applied:
 * those of the candidate set picked, or of the first that offers any here. */
function Moves({ engine, pkg, at, onApply }: { engine: PackageEngine; pkg?: PackageView; at: PointId; onApply: (exchange: Exchange, label: string) => Promise<void> }) {
  const sets = useMemo(() => pkg?.candidates ?? [], [pkg]);
  const [picked, setPicked] = useState<string>();
  useEffect(() => setPicked(undefined), [pkg?.name]);
  const offers = useQueries({
    queries: sets.map((set) => ({
      queryKey: ["candidates", engine.kind, at, set],
      queryFn: () => engine.candidates(at, set),
      staleTime: Infinity,
    })),
  });
  const settled = offers.every((offer) => !offer.isPending);
  const set = picked ?? (settled ? sets.find((_, index) => (offers[index].data?.candidates.length ?? 0) > 0) ?? sets[0] : undefined);
  const offered = set === undefined ? undefined : offers[sets.indexOf(set)];
  const [open, setOpen] = useState<number>();
  const [applying, setApplying] = useState(false);
  useEffect(() => setOpen(undefined), [at]);
  const candidates = offered?.data?.candidates ?? [];
  return <section className="from-section" aria-label="Moves">
    <h3><Term name="move">Moves</Term></h3>
    {sets.length === 0 ? <p className="muted">No candidate sets</p> : <>
    <label className="field"><span>Candidates</span><select name="candidates" value={set ?? sets[0]} onChange={(event) => setPicked(event.target.value)}>{sets.map((candidate) => <option key={candidate} value={candidate}>{candidate}</option>)}</select></label>
    {offered?.isError ? <div className="assessment-issue">{described(offered.error)}</div>
      : !offered?.data ? <p className="muted">Loading…</p>
      : candidates.length === 0 ? <p className="muted">No moves</p>
      : <ul className="move-list">{candidates.map((candidate, index) => {
        const { exchange, assessment } = candidate.proposal;
        const applies = assessment.status === "applicable";
        const shown = open === index;
        return <li key={index} className={`move move-${assessment.status}`}>
          <ExchangeView exchange={exchange} assessment={assessment} />
          <div className="move-actions">
            <button type="button" className="move-details" aria-expanded={shown} aria-label="Details" title="Details" onClick={() => setOpen(shown ? undefined : index)}>{shown ? <IconChevronDown size={15} aria-hidden="true" /> : <IconChevronRight size={15} aria-hidden="true" />}</button>
            <button type="button" className="primary" disabled={!applies || applying} onClick={async () => { setApplying(true); try { await onApply(candidate.exchange, candidate.proposal.label); } finally { setApplying(false); } }}>Apply</button>
          </div>
          {shown && <div className="move-detail"><ExchangeView form="full" exchange={exchange} assessment={assessment} /></div>}
        </li>;
      })}</ul>}
    </>}
  </section>;
}

/** A strategy run from here: the branch it makes opens and plays. */
function StrategyHere({ offered, finding, onLetPlay }: { offered: Playable; finding: boolean; onLetPlay: (strategy: string, asked: Asked) => Promise<void> }) {
  const first = offered.strategies[0];
  const [strategy, setStrategy] = useState(first?.strategy);
  const [asked, setAsked] = useState<Asked | undefined>(first?.asked);
  const valid = asked !== undefined && natural(asked.seed) && natural(asked.budget) && BigInt(asked.budget) > 0n && natural(asked.turns);
  return <section className="from-section" aria-label="Strategy">
    <h3><Term name="strategy">Strategy</Term></h3>
    {!strategy || !asked ? <p className="muted">No strategy runs on this history</p> : <>
      <div className="let-play">
        <select name="strategy" aria-label="Strategy" value={strategy} disabled={finding} onChange={(event) => { setStrategy(event.target.value); setAsked(offered.strategies.find((candidate) => candidate.strategy === event.target.value)?.asked); }}>{offered.strategies.map((candidate) => <option key={candidate.strategy} value={candidate.strategy}>{candidate.strategy}</option>)}</select>
        <button type="button" className={finding ? "danger" : "primary"} disabled={!finding && (!asked.goal || !valid)} onClick={() => void onLetPlay(strategy, asked)}>{finding ? "Stop" : "Run"}</button>
      </div>
      <div className="settings-fields">
        <label className="field"><span><Term name="goal">Goal</Term></span><select name="goal" value={asked.goal} disabled={finding} onChange={(event) => setAsked({ ...asked, goal: event.target.value })}>{offered.goals.map((goal) => <option key={goal} value={goal}>{goal}</option>)}</select></label>
        <label className="field"><span><Term name="seed">Seed</Term></span><input name="seed" inputMode="numeric" value={asked.seed} disabled={finding} onChange={(event) => setAsked({ ...asked, seed: event.target.value.trim() })} /></label>
        <label className="field"><span><Term name="budget">Budget</Term></span><input name="budget" inputMode="numeric" value={asked.budget} disabled={finding} onChange={(event) => setAsked({ ...asked, budget: event.target.value.trim() })} /></label>
        <label className="field"><span><Term name="turns">Turns</Term></span><input name="turns" inputMode="numeric" value={asked.turns} disabled={finding} onChange={(event) => setAsked({ ...asked, turns: event.target.value.trim() })} /></label>
      </div>
      {!valid && <div className="assessment-issue">Seed, budget and turns are whole numbers; budget ≥ 1.</div>}
    </>}
  </section>;
}

/** The refs naming the point on screen, a new one, or following and deleting the ref of one's own on it. */
function RefHere({ pkg, document, at, opened, onRefPoint, onKeep, onDelete, follow, objectives }: { pkg?: PackageView; document: ViewDocument; at: PointId; opened: Opened; onRefPoint: boolean; onKeep: (name: string) => void; onDelete: () => void; follow: Following; objectives: string[] }) {
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
  const here = pkg?.refs.filter((ref) => ref.point === at) ?? [];
  const named = here.length > 0 && <ul className="refs-here">{here.map((ref) => <li key={ref.name}><IdLink subject={{ kind: "ref", path: ref.name }} />{ref.shipped_by ? <> <Term name="fixed">fixed</Term></> : ""}</li>)}</ul>;
  if (onRefPoint && opened.ref) {
    const { following, setFollowing, delivered, problem } = follow;
    return <section className="from-section" aria-label="Ref">
      <h3><Term name="ref">Ref</Term></h3>
      {named}
      <div className="let-play">
        <button type="button" className={following ? "danger" : ""} onClick={() => setFollowing(!following)}>{following ? "Stop following" : <>Follow it</>}</button>
        <button type="button" onClick={onDelete}>Delete it</button>
      </div>
      {problem && <div className="assessment-issue" role="alert">{problem}</div>}
      {delivered.length > 0 && <ol className="delivered-frames" aria-label="Delivered steps">{delivered.map((frame) => {
        const step = stepOf(frame.point);
        // The step as the history on screen draws it, once it is drawn there.
        const drawnFrame = document.frames[step - 1];
        return <li key={frame.point}>
          <IdLink subject={{ kind: "step", step }} />
          {drawnFrame ? <ExchangeView exchange={drawnFrame.exchange} /> : <code>{frame.exchange.rate} ×{frame.exchange.units}</code>}
          {frame.values.length > 0 && <small>{frame.values.map((value, index) => <span key={index}><IdLink subject={{ kind: "objective", path: objectives[index] ?? "" }} label={lastPart(objectives[index] ?? "")} /> {value}</span>)}</small>}
        </li>;
      })}</ol>}
    </section>;
  }
  return <section className="from-section" aria-label="Ref">
    <h3><Term name="ref">Ref</Term></h3>
    {named}
    <form className="let-play" onSubmit={(event) => { event.preventDefault(); if (name.trim()) onKeep(name.trim()); }}>
      <input name="ref-name" aria-label="Ref name" value={name} onChange={(event) => setName(event.target.value)} />
      <button type="submit" disabled={!name.trim()}>Create</button>
    </form>
  </section>;
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
