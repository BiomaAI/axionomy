import { useEffect, useMemo, useRef, useState } from "react";
import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { IconMaximize } from "@tabler/icons-react";
import type { ViewDocument } from "./api";
import { BrowserPackages } from "./browserPackages";
import { connectionLabel, nativeEngineAnswers } from "./engine";
import { Accounts, Assessment, ModelExplorer, PanelHeading, StepTicker, Transition, useSnapshot } from "./Inspectors";
import {
  EngineRefusal,
  loadedPackages,
  nativePackages,
  samplePackages,
  type Candidate,
  type Exchange,
  type ExperimentView,
  type LoadedFrom,
  type ObjectiveView,
  type PackageEngine,
  type PackageView,
  type PointId,
  type RanView,
  type RefFrames,
  type ResultsView,
  type SearchProgress,
} from "./packageEngine";
import { Brand, PlaybackControls, changingBalances } from "./Playback";
import { QuantityFormats } from "./Quantity";
import { SceneView, type ReplayMotionMode } from "./SceneView";

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

type PackagesView = "point" | "experiments";
type ActionTab = "act" | "continue" | "follow";

/** What a link asks the packages workspace to open. */
type PackagesLink = {
  /** A package's URL to load first. */
  package?: string;
  /** A loaded package, by name. */
  name?: string;
  ref?: string;
  point?: PointId;
  step?: number;
  view?: PackagesView;
};

function readLink(): PackagesLink {
  const query = new URLSearchParams(window.location.search);
  const step = query.get("step");
  const view = query.get("view");
  return {
    package: query.get("package") ?? undefined,
    name: query.get("name") ?? undefined,
    ref: query.get("ref") ?? undefined,
    point: query.get("point") ?? undefined,
    step: step !== null && /^\d+$/.test(step) ? Number(step) : undefined,
    view: view === "experiments" || view === "point" ? view : undefined,
  };
}

function writeLink(link: PackagesLink) {
  const query = new URLSearchParams();
  if (link.package) query.set("package", link.package);
  if (link.name) query.set("name", link.name);
  if (link.ref) query.set("ref", link.ref);
  else if (link.point) query.set("point", link.point);
  if (link.step !== undefined) query.set("step", String(link.step));
  if (link.view) query.set("view", link.view);
  const written = query.toString();
  window.history.replaceState({}, "", `${window.location.pathname}${written ? `?${written}` : ""}${window.location.hash}`);
}

/**
 * The point on screen: where a ref is, or a point reached some other way
 * (a branch, where a strategy ended, a trial's end).
 */
type Opened = {
  point: PointId;
  /** The ref the point is where, when it was opened as a ref's. */
  ref?: string;
  /** The package that ships the ref, which fixes it: it never moves. */
  shipped?: string;
  how: "ref" | "branch" | "continued" | "trial" | "link";
  /** What it was reached from, in words. */
  note?: string;
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

/** Packages, points, refs and experiments (P12): what Studio opens. */
export function PackagesApp() {
  const engine = useEngine();
  const queryClient = useQueryClient();
  const link = useRef(readLink()).current;
  const kind = engine?.kind;
  const kept = useQuery({ queryKey: ["store", kind], queryFn: () => engine!.store(), enabled: Boolean(engine), staleTime: Infinity });
  const listing = useQuery({ queryKey: ["packages", kind], queryFn: () => engine!.packages(), enabled: Boolean(engine) && kept.isSuccess });
  const samples = useQuery({ queryKey: ["sample-packages"], queryFn: samplePackages, staleTime: Infinity, retry: false });
  const [packageName, setPackageName] = useState<string>();
  const [opened, setOpened] = useState<Opened>();
  const [view, setView] = useState<PackagesView>(link.view ?? "point");
  const [loadedUrl, setLoadedUrl] = useState<string | undefined>(link.package);
  const [busy, setBusy] = useState<string>();
  const [notices, setNotices] = useState<Notice[]>([]);
  const [error, setError] = useState<string>();
  const [copied, setCopied] = useState(false);
  /** Says what the latest action did, in place of what the one before it said. */
  const report = (...said: Notice[]) => setNotices(said);
  // A confirmation goes by itself; a caution stays until it is dismissed.
  useEffect(() => {
    if (!notices.some((said) => said.tone === "done")) return;
    const timer = window.setTimeout(() => setNotices((current) => current.filter((said) => said.tone !== "done")), 5_000);
    return () => window.clearTimeout(timer);
  }, [notices]);
  const pkg = listing.data?.packages.find((candidate) => candidate.name === packageName);

  // The step to show once the point's history is drawn: the end, unless a link names one.
  const pendingStep = useRef<"end" | number>(link.step ?? "end");
  const [position, setPosition] = useState(0);
  const [motion, setMotion] = useState<ReplayMotionMode>("seek");
  const [playing, setPlaying] = useState(false);
  const [delay, setDelay] = useState(650);
  const [focusedAccount, setFocusedAccount] = useState<string>();
  const [action, setAction] = useState<ActionTab>("act");
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

  useEffect(() => {
    if (!document) return;
    const wanted = pendingStep.current;
    pendingStep.current = "end";
    setPosition(wanted === "end" ? document.frames.length : Math.min(wanted, document.frames.length));
    setPlaying(false);
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

  // The point at the scrubber: the opened point at the end of its history,
  // else the point at that step of it, named once the scrubber settles.
  const settled = useSettled(position, 250);
  const atEnd = position === steps;
  const earlier = useQuery({
    queryKey: ["ancestor", kind, opened?.point, settled],
    queryFn: () => engine!.ancestor(opened!.point, settled),
    enabled: Boolean(engine && opened && !stale && settled < steps && settled === position),
    staleTime: Infinity,
  });
  const screenPoint: PointId | undefined = !opened || stale ? undefined : atEnd ? opened.point : settled === position ? earlier.data : undefined;
  // At a ref's point applying moves the ref, unless a package ships it.
  const onRefPoint = Boolean(opened?.ref) && !opened?.shipped && atEnd;
  const objectives = useMemo(() => pkg?.objectives.map((objective) => objective.objective) ?? [], [pkg]);
  const follow = useFollowing(engine, opened?.ref, opened?.point, objectives, (point) => {
    pendingStep.current = "end";
    setOpened((current) => current?.ref ? { point, ref: current.ref, shipped: current.shipped, how: "ref" } : current);
    void queryClient.invalidateQueries({ queryKey: ["packages", kind] });
  });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ["packages", kind] });
  /** The packages as the store holds them now, never a listing cached before a change. */
  const relist = () => queryClient.fetchQuery({ queryKey: ["packages", kind], queryFn: () => engine!.packages(), staleTime: 0 });

  /** Opens `name`'s point as it is now. */
  const openRef = async (name: string, step: "end" | number = "end", show = true) => {
    if (!engine) return;
    try {
      const ref = await engine.readRef(name);
      pendingStep.current = step;
      setOpened({ point: ref.point, ref: name, shipped: ref.shipped_by ?? undefined, how: "ref" });
      if (show) setView("point");
    } catch (cause) { setError(described(cause)); }
  };

  const openPoint = (point: PointId, how: Opened["how"], note?: string) => {
    pendingStep.current = "end";
    setOpened({ point, how, note });
    setView("point");
  };

  const selectPackage = (name: string, refs = listing.data) => {
    setPackageName(name);
    const first = refs?.packages.find((candidate) => candidate.name === name)?.refs[0];
    if (first) void openRef(first.name, "end", false);
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
      if (link.ref && named?.refs.some((ref) => ref.name === link.ref)) void openRef(link.ref, link.step ?? "end", false);
      else if (link.point) {
        pendingStep.current = link.step ?? "end";
        setOpened({ point: link.point, how: "link" });
      } else if (named?.refs[0]) void openRef(named.refs[0].name, "end", false);
    })();
  }, [engine, kept.isSuccess, listing.data]);

  useEffect(() => {
    if (!started.current) return;
    writeLink({ package: loadedUrl, name: packageName, ref: opened?.ref, point: opened?.point, step: document ? position : undefined, view });
  }, [loadedUrl, packageName, opened, position, view, document?.id]);

  /** Applies `exchange` at the point on screen: at a ref's point the ref moves over it; anywhere else it makes a branch. */
  const act = async (exchange: Exchange) => {
    if (!engine || !opened || !screenPoint) return;
    const at = screenPoint;
    const ref = onRefPoint ? opened.ref : undefined;
    try {
      const applied = await engine.apply(at, [exchange]);
      if (ref) {
        try {
          await engine.advanceRef(ref, at, applied.point);
          report(done(`Applied at ${ref}'s point: ${ref} moved to step ${stepOf(applied.point)}.`));
          pendingStep.current = "end";
          setOpened({ point: applied.point, ref, how: "ref" });
        } catch (cause) {
          if (!(cause instanceof EngineRefusal) || !cause.moved) throw cause;
          report(caution(`${ref} moved to ${shortPoint(cause.moved.current)} before this step counted, so the step is a branch; look again from where the ref is.`));
          openPoint(applied.point, "branch", `from step ${stepOf(at)}`);
        }
      } else {
        report(done(`Applied at step ${stepOf(at)}: a branch, a new point that no ref names.`));
        openPoint(applied.point, "branch", `from step ${stepOf(at)}`);
      }
      void refresh();
    } catch (cause) { setError(described(cause)); }
  };

  const advanceTo = async (from: PointId, to: PointId) => {
    if (!engine || !opened?.ref) return;
    const ref = opened.ref;
    try {
      await engine.advanceRef(ref, from, to);
      report(done(`${ref} moved to step ${stepOf(to)}.`));
      pendingStep.current = "end";
      setOpened({ point: to, ref, how: "ref" });
      void refresh();
    } catch (cause) { setError(described(cause)); }
  };

  /** Deletes the ref on screen from the point it is on; the point stays on screen, a branch now. */
  const deleteRef = async () => {
    if (!engine || !opened?.ref) return;
    const ref = opened.ref;
    try {
      await engine.deleteRef(ref, opened.point);
      report(done(`${ref} is deleted; the points it named stay.`));
      setOpened({ point: opened.point, how: "branch", note: `where ${ref} was` });
      void refresh();
    } catch (cause) { setError(described(cause)); }
  };

  const keepAsRef = async (name: string) => {
    if (!engine || !opened) return;
    try {
      await engine.createRef(name, opened.point);
      report(done(`${name} names this point now.`));
      setOpened({ point: opened.point, ref: name, how: "ref" });
      void refresh();
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

  return <div className="app-shell packages-workspace">
    <header className="topbar">
      <Brand />
      <div className="connection" title={engine?.kind === "native" ? "Verified by a live health check" : "Rust/WASM engine isolated in a Web Worker"}><span className={`status-dot ${engine ? "online" : "static"}`} />{label}</div>
    </header>

    <section className="command-bar" aria-label="Package controls">
      <label className="field package-field"><span>Package</span><select name="package" value={packageName ?? ""} onChange={(event) => selectPackage(event.target.value)} disabled={!listing.data?.packages.length}>
        {!packageName && <option value="">No package loaded</option>}
        {listing.data?.packages.map((candidate) => <option key={candidate.name} value={candidate.name}>{candidate.name} {candidate.version}</option>)}
      </select></label>
      <label className="field ref-field"><span>Ref</span><select name="ref" value={opened?.ref ?? ""} onChange={(event) => void openRef(event.target.value)} disabled={!pkg?.refs.length}>
        {!opened?.ref && <option value="">{opened ? "Not on a ref" : "No ref"}</option>}
        {pkg?.refs.map((ref) => <option key={ref.name} value={ref.name}>{ref.name}</option>)}
      </select></label>
      {(samples.data?.length ?? 0) > 0 && <label className="field sample-field"><span>Sample package</span><select name="sample" value="" disabled={!engine || Boolean(busy)} onChange={(event) => openSample(event.target.value)}>
        <option value="">Open a sample…</option>
        {samples.data?.map((candidate) => <option key={candidate.url} value={candidate.url}>{candidate.name}</option>)}
      </select></label>}
      <label className="file-button">Load package<input name="package-file" type="file" accept=".zip,.redb,application/zip" disabled={!engine || Boolean(busy)} onChange={(event) => {
        const file = event.target.files?.[0];
        event.target.value = "";
        if (!file || !engine) return;
        setLoadedUrl(undefined);
        void load(file.name, () => engine.loadFile(file));
      }} /></label>
      <button type="button" onClick={copyLink} disabled={!pkg}>{copied ? "Link copied ✓" : "Copy link"}</button>
      <div className="engine-message">{engine ? `The ${engine.kind === "native" ? "native" : "browser"} engine serves these packages; every exchange is the kernel's to apply.` : "Looking for an engine…"}</div>
    </section>

    {(busy || notices.length > 0) && <div className="status-layer">
      {busy && <div className="activity" role="status"><div className="activity-heading"><span className="spinner" aria-hidden="true" /><div><strong>{busy}</strong><span>Its manifests are checked before it is merged into the store</span></div></div></div>}
      {notices.map((said) => <div className={`notice notice-${said.tone}`} role="status" key={said.message}>{said.message}{said.tone === "caution" && <button type="button" onClick={() => setNotices((current) => current.filter((candidate) => candidate !== said))}>Dismiss</button>}</div>)}
    </div>}

    {pkg && <section className="problem-context"><div><span>Package</span><div className="problem-copy"><p>{pkg.name} {pkg.version} · store format {pkg.format} · script host API {pkg.host_api}</p><small><strong>{storeWords}</strong>{pkg.refs.length} refs · {pkg.experiments.length} experiments · {pkg.strategies.length} strategies · {pkg.objectives.length} objectives</small></div></div><ul className="capabilities" aria-label="What this package defines">{pkg.goals.map((goal) => <li key={goal}>goal {goal}</li>)}{pkg.candidates.map((set) => <li key={set}>candidates {set}</li>)}</ul></section>}
    {error && <div className="error-banner" role="alert">{error}<button type="button" onClick={() => setError(undefined)}>Dismiss</button></div>}

    {!engine || !listing.data ? <div className="loading" role="status"><strong>{engine ? "Opening the engine's store…" : "Looking for an engine…"}</strong><div className="indeterminate-progress" /></div>
      : listing.data.packages.length === 0 ? <EmptyCatalog samples={samples.data ?? []} busy={Boolean(busy)} onOpen={openSample} />
      : <main>
        <div className="view-tabs" role="tablist" aria-label="Package view">
          <button type="button" role="tab" aria-selected={view === "point"} onClick={() => setView("point")}>Point <small>{opened ? `step ${stepOf(opened.point)}` : "none"}</small></button>
          <button type="button" role="tab" aria-selected={view === "experiments"} onClick={() => setView("experiments")}>Experiments <small>{pkg?.experiments.length ?? 0}</small></button>
        </div>
        {view === "experiments" ? pkg && <ExperimentsPanel engine={engine} pkg={pkg} onOpen={(point, note) => openPoint(point, "trial", note)} onRecorded={refresh} />
          : !opened ? <div className="empty-state">This package names no point. Load another, or open an experiment's trial.</div>
          : !document ? <div className="loading" role="status"><strong>{drawn.isError ? described(drawn.error) : "Drawing the point's history…"}</strong>{!drawn.isError && <div className="indeterminate-progress" />}</div>
          : snapshot && <QuantityFormats.Provider value={document.formats ?? {}}><div className="point-cockpit" aria-busy={stale}>
            <PointHeading pkg={pkg} opened={opened} document={document} position={position} onKeep={keepAsRef} onDelete={deleteRef} />
            {steps === 0
              ? <StartNote experiments={pkg?.experiments.length ?? 0} onExperiments={() => setView("experiments")} />
              : <PlaybackControls position={position} count={steps} playing={playing} delay={delay} onDelay={setDelay} onPosition={(next, how) => { setMotion(how); setPosition(next); }} onPlaying={(next) => { if (next && position >= steps) { setMotion("seek"); setPosition(0); } setPlaying(next); }} frame={frame} />}
            <section className="stage point-stage">
              <div className="panel world-panel" ref={worldPanel}>
                <PanelHeading kicker="The picture" title={snapshot.scene?.title ?? document.title} aside={snapshot.scene?.surface.kind} action={<button type="button" className="theater-btn" onClick={toggleTheater} title="Theater mode" aria-label="Toggle theater mode"><IconMaximize size={16} stroke={1.8} aria-hidden="true" /></button>} />
                <SceneView scene={snapshot.scene} previousScene={previous?.scene} history={[document.initial, ...document.frames.slice(0, position).map((candidate) => candidate.after)].flatMap((candidate) => candidate.scene ? [candidate.scene] : [])} frame={frame} motion={motion} onAccount={setFocusedAccount} />
              </div>
              <div className="panel action-panel">
                <PanelHeading kicker={onRefPoint ? `At ${opened.ref}'s point` : `At step ${position}`} title={action === "follow" && opened.ref ? `Following ${opened.ref}` : onRefPoint ? `Applying moves ${opened.ref}` : opened.shipped && atEnd ? `${opened.ref} is fixed: applying makes a branch` : "Applying makes a branch"} />
                <div className="action-tabs" role="tablist" aria-label="What to do here">
                  <button type="button" role="tab" aria-selected={action === "act"} onClick={() => setAction("act")}>Act</button>
                  <button type="button" role="tab" aria-selected={action === "continue"} onClick={() => setAction("continue")}>Continue from here</button>
                  <button type="button" role="tab" aria-selected={action === "follow"} onClick={() => setAction("follow")} disabled={!opened.ref}>Follow</button>
                </div>
                {/* Every panel stays mounted, so a run or a draft survives a change of tab. */}
                <div className="action-body" hidden={action !== "act"}><ActPanel engine={engine} pkg={pkg} at={screenPoint} onRefPoint={onRefPoint} refName={opened.ref} onApply={act} /></div>
                <div className="action-body" hidden={action !== "continue"}><ContinuePanel key={pkg?.name} engine={engine} pkg={pkg} at={screenPoint} refName={onRefPoint ? opened.ref : undefined} onOpen={(point, note) => openPoint(point, "continued", note)} onAdvance={advanceTo} /></div>
                {opened.ref && <div className="action-body" hidden={action !== "follow"}><FollowPanel refName={opened.ref} objectives={objectives} follow={follow} /></div>}
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
            <section className="definition-grid proof-grid">
              <div className="panel transition-panel" ref={proofPanel}><PanelHeading kicker="Proof of this step" title={frame?.exchange.rate.label ?? "Initial state"} /><Transition frame={frame} steps={steps} /></div>
              <div className="panel model-panel"><PanelHeading kicker="The rules" title="Rates, roles, goals & invariants" aside={counted(document.model?.rates.length ?? 0, "rate")} /><ModelExplorer document={document} /></div>
            </section>
          </div></QuantityFormats.Provider>}
      </main>}

    <footer>The picture explains. The accounts, rules, and replay prove.</footer>
  </div>;
}

/** The page before any package is loaded: the samples, each a click from open. */
function EmptyCatalog({ samples, busy, onOpen }: { samples: { name: string; url: string }[]; busy: boolean; onOpen: (url: string) => void }) {
  return <div className="empty-catalog" role="note">
    <strong>Open a package</strong>
    <p>A package holds economies, the refs that name them, and the goals, scripts, strategies, experiments and views that use them. {samples.length > 0 ? "Open one of the samples, or load a package file of your own." : "Load a package file, a store file or a zip of one."} A link such as <code>?package=packages/maze.zip</code> opens one by its URL.</p>
    {samples.length > 0 && <ul className="sample-list" aria-label="Sample packages">{samples.map((sample) => <li key={sample.url}><button type="button" disabled={busy} onClick={() => onOpen(sample.url)}>{sample.name}</button></li>)}</ul>}
  </div>;
}

/** In place of playback at a point with no history: what makes one. */
function StartNote({ experiments, onExperiments }: { experiments: number; onExperiments: () => void }) {
  return <section className="playback start-note" aria-label="Nothing to play yet">
    <p><strong>Nothing has happened here yet,</strong> so there is nothing to play. Act on it, continue with a strategy, or run {experiments > 0 ? "one of the package's experiments" : "an experiment"}; what it reaches plays here.</p>
    {experiments > 0 && <button type="button" onClick={onExperiments}>Experiments</button>}
  </section>;
}

/** What the point on screen is, and where it came from. */
function PointHeading({ pkg, opened, document, position, onKeep, onDelete }: { pkg?: PackageView; opened: Opened; document: ViewDocument; position: number; onKeep: (name: string) => void; onDelete: () => void }) {
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
  const steps = counted(document.frames.length, "step");
  const how = opened.ref ? `ref ${opened.ref}` : opened.how === "branch" ? "Branch" : opened.how === "continued" ? "Where a strategy ended" : opened.how === "trial" ? "A trial's end" : "A linked point";
  return <section className="document-heading point-heading">
    <div>
      <span className="eyebrow">{pkg?.name ?? "Package"} · {how}{opened.note ? ` · ${opened.note}` : ""}</span>
      <h1>{document.title}</h1>
      <p>{opened.shipped ? `${opened.ref} ships with ${opened.shipped}, after ${steps}. It is fixed, so acting here makes a branch; name a ref of your own to keep going from here.`
        : opened.ref ? `${opened.ref}, after ${steps}. Acting here moves it forward.`
        : `A point no ref names, after ${steps}. Acting here makes another branch; name it to keep it.`}</p>
      <dl className="point-key" aria-label="Point on screen">
        <div><dt>On screen</dt><dd>{opened.shipped ? "On a package's ref" : opened.ref ? "On a ref" : "Not on any ref"}</dd></div>
        <div><dt>Point</dt><dd title={opened.point}>{shortPoint(opened.point)}</dd></div>
        <div><dt>History</dt><dd>{steps}</dd></div>
        <div><dt>Showing</dt><dd>step {position}</dd></div>
      </dl>
    </div>
    {(!opened.ref || opened.shipped) && <form className="keep-ref" onSubmit={(event) => { event.preventDefault(); if (name.trim()) onKeep(name.trim()); }}>
      <label className="field"><span>Name a ref here</span><input name="ref-name" type="text" value={name} onChange={(event) => setName(event.target.value)} /></label>
      <button type="submit" disabled={!name.trim()}>Name it</button>
    </form>}
    {opened.ref && !opened.shipped && <div className="keep-ref">
      <button type="button" onClick={onDelete}>Delete {opened.ref}</button>
    </div>}
  </section>;
}

/** An exchange's roles and accounts, in the view's words. */
function bindingWords(candidate: Candidate): string {
  const exchange = candidate.proposal.exchange;
  return [...exchange.bindings.map((binding) => `${binding.role.label}: ${binding.account.label}`), `×${exchange.units}`].join(" · ");
}

/** Candidates at the point on screen, each assessed, and a written exchange assessed before it is applied. */
function ActPanel({ engine, pkg, at, onRefPoint, refName, onApply }: { engine: PackageEngine; pkg?: PackageView; at?: PointId; onRefPoint: boolean; refName?: string; onApply: (exchange: Exchange) => Promise<void> }) {
  const [set, setSet] = useState(pkg?.candidates[0]);
  useEffect(() => setSet(pkg?.candidates[0]), [pkg?.name]);
  const offered = useQuery({
    queryKey: ["candidates", engine.kind, at, set],
    queryFn: () => engine.candidates(at!, set!),
    enabled: Boolean(at && set),
    staleTime: Infinity,
  });
  const [selected, setSelected] = useState<number>();
  const [written, setWritten] = useState("");
  const [assessed, setAssessed] = useState<{ text: string; candidate: Candidate }>();
  const [problem, setProblem] = useState<string>();
  const [applying, setApplying] = useState(false);
  useEffect(() => { setSelected(undefined); setAssessed(undefined); setProblem(undefined); }, [at]);
  const candidates = offered.data?.candidates ?? [];
  const chosen = selected === undefined ? undefined : candidates[selected];
  const apply = async (exchange: Exchange) => {
    setApplying(true);
    try { await onApply(exchange); } finally { setApplying(false); }
  };
  const assess = async () => {
    setProblem(undefined);
    if (!at) return;
    let exchange: Exchange;
    try { exchange = JSON.parse(written) as Exchange; } catch (cause) { setProblem(`Not an exchange written as JSON: ${described(cause)}`); return; }
    try { setAssessed({ text: written, candidate: await engine.assess(at, exchange) }); } catch (cause) { setProblem(described(cause)); }
  };
  const applies = (candidate?: Candidate) => candidate?.proposal.assessment.status === "applicable";
  const verb = onRefPoint ? `Apply and move ${refName}` : "Apply as a branch";
  if (!at) return <div className="empty-state">Naming the point at this step…</div>;
  return <div className="act">
    {pkg && pkg.candidates.length > 0 ? <>
      <label className="field"><span>Candidates</span><select name="candidates" value={set ?? ""} onChange={(event) => { setSet(event.target.value); setSelected(undefined); }}>{pkg.candidates.map((candidate) => <option key={candidate} value={candidate}>{candidate}</option>)}</select></label>
      {offered.isError ? <div className="assessment-issue">{described(offered.error)}</div>
        : !offered.data ? <div className="empty-state">Asking the candidate set…</div>
        : candidates.length === 0 ? <div className="empty-state">The candidate set offers nothing here.</div>
        : <ul className="candidate-list" aria-label="Candidates here">{candidates.map((candidate, index) => <li key={index}>
          <button type="button" aria-pressed={index === selected} className={`candidate candidate-${candidate.proposal.assessment.status}`} onClick={() => { setSelected(index); setWritten(JSON.stringify(candidate.exchange, null, 2)); setAssessed(undefined); }}>
            <strong>{candidate.proposal.label}</strong><span>{bindingWords(candidate)}</span><small>{candidate.proposal.assessment.status}</small>
          </button>
        </li>)}</ul>}
      {chosen && <div className="candidate-detail">
        <Assessment assessment={chosen.proposal.assessment} />
        <button type="button" className="primary" disabled={!applies(chosen) || applying} onClick={() => void apply(chosen.exchange)}>{verb}</button>
      </div>}
    </> : <div className="empty-state">This package defines no candidate set; write an exchange instead.</div>}
    <details className="exchange-editor" open={!pkg?.candidates.length}>
      <summary>Write an exchange</summary>
      <textarea name="exchange" aria-label="Exchange as JSON" spellCheck={false} rows={7} value={written} placeholder={'{"rate": "…", "bindings": {"role": "account"}, "units": "1"}'} onChange={(event) => { setWritten(event.target.value); setAssessed(undefined); }} />
      <div className="editor-actions">
        <button type="button" onClick={() => void assess()} disabled={!written.trim()}>Assess</button>
        <button type="button" className="primary" disabled={!assessed || assessed.text !== written || !applies(assessed.candidate) || applying} onClick={() => assessed && void apply(assessed.candidate.exchange)}>{verb}</button>
      </div>
      {problem && <div className="assessment-issue" role="alert">{problem}</div>}
      {assessed && assessed.text === written && <Assessment assessment={assessed.candidate.proposal.assessment} />}
    </details>
  </div>;
}

/** A whole number below 2^64, written in decimal. */
function natural(text: string): boolean {
  return /^(0|[1-9]\d*)$/.test(text) && BigInt(text) < 2n ** 64n;
}

/** The effort Studio gives a strategy no experiment runs. */
const DEFAULT_EFFORT = { seed: "17", budget: "1000000", turns: "1000" };

/**
 * What an experiment that runs `strategy` asks of it, when one does: the
 * goal it works toward and the effort it is given. Otherwise the package's
 * first goal and Studio's own effort.
 */
function askedOf(pkg: PackageView | undefined, strategy: string | undefined): { goal?: string; seed: string; budget: string; turns: string } {
  const experiment = pkg?.experiments.find((candidate) => candidate.arms.some((arm) => arm.strategy === strategy));
  if (!experiment) return { goal: pkg?.goals[0], ...DEFAULT_EFFORT };
  return { goal: experiment.goal, seed: experiment.seed, budget: experiment.budget, turns: experiment.turns };
}

/** A strategy run from the point on screen toward a goal: its progress, where it ended and its thinking. */
function ContinuePanel({ engine, pkg, at, refName, onOpen, onAdvance }: { engine: PackageEngine; pkg?: PackageView; at?: PointId; refName?: string; onOpen: (point: PointId, note: string) => void; onAdvance: (from: PointId, to: PointId) => void }) {
  const [strategy, setStrategy] = useState(pkg?.strategies[0]);
  const [goal, setGoal] = useState(() => askedOf(pkg, pkg?.strategies[0]).goal);
  const [seed, setSeed] = useState(() => askedOf(pkg, pkg?.strategies[0]).seed);
  const [budget, setBudget] = useState(() => askedOf(pkg, pkg?.strategies[0]).budget);
  const [turns, setTurns] = useState(() => askedOf(pkg, pkg?.strategies[0]).turns);
  const [running, setRunning] = useState<{ cancel(): void; progress?: SearchProgress }>();
  const [ran, setRan] = useState<RanView>();
  const [problem, setProblem] = useState<string>();
  const cancelled = useRef(false);
  const choose = (next: string) => {
    setStrategy(next);
    const asked = askedOf(pkg, next);
    setGoal(asked.goal);
    setSeed(asked.seed);
    setBudget(asked.budget);
    setTurns(asked.turns);
  };
  const run = async () => {
    if (!at || !strategy || !goal) return;
    setRan(undefined);
    setProblem(undefined);
    const work = engine.runStrategy(at, { strategy, goal, seed, budget, turns }, (progress) => setRunning((current) => current && { ...current, progress }));
    setRunning({ cancel: work.cancel });
    try { setRan(await work.result); } catch (cause) { setProblem(cancelled.current ? "Cancelled: the strategy stopped, and nothing it reached is shown." : described(cause)); } finally { setRunning(undefined); cancelled.current = false; }
  };
  if (!pkg || pkg.strategies.length === 0) return <div className="empty-state">This package defines no strategy.</div>;
  if (pkg.goals.length === 0) return <div className="empty-state">This package defines no goal for a strategy to work toward.</div>;
  const valid = natural(seed) && natural(budget) && BigInt(budget) > 0n && natural(turns);
  return <div className="continue">
    <div className="continue-controls">
      <label className="field"><span>Strategy</span><select name="strategy" value={strategy} onChange={(event) => choose(event.target.value)} disabled={Boolean(running)}>{pkg.strategies.map((candidate) => <option key={candidate} value={candidate}>{candidate}</option>)}</select></label>
      <label className="field"><span>Goal</span><select name="goal" value={goal} onChange={(event) => setGoal(event.target.value)} disabled={Boolean(running)}>{pkg.goals.map((candidate) => <option key={candidate} value={candidate}>{candidate}</option>)}</select></label>
      <label className="field numeric-field"><span>Seed</span><input name="seed" type="text" inputMode="numeric" value={seed} onChange={(event) => setSeed(event.target.value.trim())} disabled={Boolean(running)} /></label>
      <label className="field numeric-field"><span>Budget</span><input name="budget" type="text" inputMode="numeric" value={budget} onChange={(event) => setBudget(event.target.value.trim())} disabled={Boolean(running)} /></label>
      <label className="field numeric-field"><span>Turns</span><input name="turns" type="text" inputMode="numeric" value={turns} onChange={(event) => setTurns(event.target.value.trim())} disabled={Boolean(running)} /></label>
      {running ? <button type="button" className="danger" onClick={() => { cancelled.current = true; running.cancel(); }}>Cancel</button> : <button type="button" className="primary" disabled={!at || !goal || !valid} onClick={() => void run()}>Run strategy</button>}
    </div>
    <p className="muted">A search expands at most the budget's states and a policy or a game plays at most the turns; each strategy reads only its own limit. The goal and the effort start as those of the experiment that runs the strategy, or, where none does, the package's first goal, a budget of {DEFAULT_EFFORT.budget} and {DEFAULT_EFFORT.turns} turns.</p>
    {!valid && <div className="assessment-issue">A seed, a budget and turns are whole numbers below 2^64, the budget at least 1.</div>}
    {running && <div className="activity-progress" role="status"><div className="indeterminate-progress" /><span>{running.progress ? `${running.progress.expanded} expanded · ${running.progress.generated} generated · ${running.progress.frontier} frontier` : "running"}</span></div>}
    {problem && <div className="assessment-issue" role="alert">{problem}</div>}
    {ran && <RanSummary ran={ran} refName={refName} onOpen={() => onOpen(ran.end, `${ran.strategy} toward ${ran.goal} from step ${stepOf(ran.start)}`)} onAdvance={() => onAdvance(ran.start, ran.end)} />}
  </div>;
}

function outcomeWords(outcome: RanView["outcome"]): string {
  switch (outcome.kind) {
    case "reached": return "Reached its goal";
    case "stopped": return "Stopped short of its goal";
    case "horizon": return "Played every turn it was given";
    case "refused": return `Refused on turn ${outcome.turn}`;
    case "exhausted": return "Explored everything without its goal";
    case "spent": return "Spent its budget first";
    case "interrupted": return "Stopped before it ended";
  }
}

function RanSummary({ ran, refName, onOpen, onAdvance }: { ran: RanView; refName?: string; onOpen: () => void; onAdvance: () => void }) {
  const applied = stepOf(ran.end) - stepOf(ran.start);
  const thinking = ran.thinking;
  return <div className="ran-summary" aria-label="Where the strategy ended">
    <div className="thinking-summary">
      <div><span>Outcome</span><strong>{outcomeWords(ran.outcome)}</strong></div>
      <div><span>Steps applied</span><strong>{applied}</strong></div>
      <div><span>Ended at</span><strong title={ran.end}>{shortPoint(ran.end)}</strong></div>
      <div><span>Cost</span><strong>{ran.cost ?? "—"}</strong></div>
    </div>
    {thinking ? <div className="thinking-counters" aria-label="Thinking"><span><small>Expanded</small><b>{thinking.expanded}</b></span><span><small>Generated</small><b>{thinking.generated}</b></span><span><small>Frontier</small><b>{thinking.frontier}</b></span><span><small>Visited</small><b>{thinking.visited}</b></span><span><small>Solution depth</small><b>{thinking.solution_depth ?? "—"}</b></span></div>
      : <p className="muted">A policy or a plan records no search; its steps are its thinking.</p>}
    <div className="editor-actions">
      <button type="button" onClick={onOpen} disabled={applied === 0}>Open where it ended</button>
      {refName && applied > 0 && <button type="button" className="primary" onClick={onAdvance}>Move {refName} there</button>}
    </div>
  </div>;
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

/** The steps a ref is moved over, as they are delivered. */
function FollowPanel({ refName, objectives, follow }: { refName: string; objectives: string[]; follow: Following }) {
  const { following, setFollowing, delivered, problem } = follow;
  return <div className="follow">
    <div className="editor-actions">
      <button type="button" className={following ? "danger" : "primary"} onClick={() => setFollowing(!following)}>{following ? "Stop following" : `Follow ${refName}`}</button>
      <span className="muted">{following ? "Waiting for the ref to move; each step it is moved over arrives here." : "Only the steps a ref is moved over are delivered, never one only tried out."}</span>
    </div>
    {problem && <div className="assessment-issue" role="alert">{problem}</div>}
    {delivered.length > 0 && <ol className="delivered-frames" aria-label="Delivered steps">{delivered.map((frame) => <li key={frame.point}>
      <strong>step {stepOf(frame.point)}</strong><span>{frame.exchange.rate} ×{frame.exchange.units}</span>
      {frame.values.length > 0 && <small>{frame.values.map((value, index) => `${objectives[index]} ${value}`).join(" · ")}</small>}
    </li>)}</ol>}
  </div>;
}

/** The package's experiments: each run, its trials and objective values, compared. */
function ExperimentsPanel({ engine, pkg, onOpen, onRecorded }: { engine: PackageEngine; pkg: PackageView; onOpen: (point: PointId, note: string) => void; onRecorded: () => void }) {
  if (pkg.experiments.length === 0) return <div className="empty-state">This package carries no experiment.</div>;
  return <div className="experiments">{pkg.experiments.map((experiment) => <ExperimentCard key={experiment.experiment} engine={engine} experiment={experiment} onOpen={onOpen} onRecorded={onRecorded} />)}</div>;
}

function direction(objective: ObjectiveView): string {
  return objective.direction === "minimize" ? "↓ less is better" : "↑ more is better";
}

function endedWords(ended: ResultsView["arms"][number]["trials"][number]["ended"]): string {
  return ended.kind === "refused" ? `refused on turn ${ended.turn}` : ended.kind;
}

function ExperimentCard({ engine, experiment, onOpen, onRecorded }: { engine: PackageEngine; experiment: ExperimentView; onOpen: (point: PointId, note: string) => void; onRecorded: () => void }) {
  const queryClient = useQueryClient();
  const path = experiment.experiment;
  const results = useQuery({ queryKey: ["results", engine.kind, path], queryFn: () => engine.results(path), enabled: experiment.recorded });
  const [ran, setRan] = useState<ResultsView>();
  const [running, setRunning] = useState<{ cancel(): void; progress?: SearchProgress }>();
  const [problem, setProblem] = useState<string>();
  const cancelled = useRef(false);
  const [by, setBy] = useState(experiment.objectives[0]?.objective);
  const shown = ran ?? results.data;
  const order = useMemo(() => by ? [by, ...experiment.objectives.map((objective) => objective.objective).filter((objective) => objective !== by)] : [], [by, experiment.objectives]);
  const ends = useMemo(() => shown?.arms.flatMap((arm) => arm.trials.map((trial) => trial.end)) ?? [], [shown]);
  const compared = useQuery({
    queryKey: ["compare", engine.kind, ends, order],
    queryFn: () => engine.compare({ points: ends, objectives: order }),
    enabled: ends.length > 0 && order.length > 0,
    staleTime: Infinity,
  });
  const front = new Set(compared.data?.front ?? []);
  const run = async () => {
    setProblem(undefined);
    const work = engine.runExperiment(path, (progress) => setRunning((current) => current && { ...current, progress }));
    setRunning({ cancel: work.cancel });
    try {
      const next = await work.result;
      setRan(next);
      queryClient.setQueryData(["results", engine.kind, path], next);
      onRecorded();
    } catch (cause) { setProblem(cancelled.current ? "Cancelled: the experiment stopped and kept no results." : described(cause)); } finally { setRunning(undefined); cancelled.current = false; }
  };
  return <details className="strategy-comparison experiment-card" open aria-label={`Experiment ${path}`}>
    <summary><span>Experiment</span><strong>{path}</strong><i aria-hidden="true">▸</i></summary>
    <div className="experiment-body">
      <dl className="point-key">
        <div><dt>Starts at</dt><dd>{experiment.start}</dd></div>
        <div><dt>Goal</dt><dd>{experiment.goal}</dd></div>

        <div><dt>Arms</dt><dd>{experiment.arms.map((arm) => arm.strategy).join(", ")}</dd></div>
        <div><dt>Trials</dt><dd>{experiment.trials} each</dd></div>
        <div><dt>Seed</dt><dd>{experiment.seed}</dd></div>
        <div><dt>Budget</dt><dd>{experiment.budget}</dd></div>
        <div><dt>Turns</dt><dd>{experiment.turns}</dd></div>
      </dl>
      <div className="editor-actions">
        {running ? <button type="button" className="danger" onClick={() => { cancelled.current = true; running.cancel(); }}>Cancel</button> : <button type="button" className="primary" onClick={() => void run()}>Run experiment</button>}
        {running && <span className="muted" role="status">{running.progress ? `Running · ${running.progress.expanded} expanded · ${running.progress.generated} generated` : "Running every arm's trials…"}</span>}
        {!running && !shown && <span className="muted">{experiment.recorded ? "Reading the recorded results…" : "No results recorded yet."}</span>}
        {shown && experiment.objectives.length > 0 && <label className="field pick-field"><span>Pick by</span><select name={`pick-${path}`} value={by} onChange={(event) => setBy(event.target.value)}>{experiment.objectives.map((objective) => <option key={objective.objective} value={objective.objective}>{objective.objective} first</option>)}</select></label>}
      </div>
      {problem && <div className="assessment-issue" role="alert">{problem}</div>}
      {shown && <div className="comparison-scroll"><table aria-label={`Results of ${path}`}>
        <thead><tr><th>Arm</th><th>Trial seed</th><th>Ended</th><th>At step</th>{shown.objectives.map((objective) => <th key={objective.objective} title={direction(objective)}>{objective.objective} {objective.direction === "minimize" ? "↓" : "↑"}</th>)}{shown.objectives.length > 0 && <th>Front</th>}<th></th></tr></thead>
        <tbody>{shown.arms.flatMap((arm) => arm.trials.map((trial, index) => {
          const picked = compared.data?.pick === trial.end;
          return <tr key={`${arm.arm.strategy}:${index}`} className={picked ? "selected" : undefined}>
            <td>{arm.arm.strategy}{arm.arm.start && <small className="outcome-note">from {arm.arm.start}</small>}</td>
            <td><code>{trial.seed}</code></td>
            <td>{endedWords(trial.ended)}</td>
            <td><code>{stepOf(trial.end)}</code></td>
            {trial.values.map((value, column) => <td key={column}><code>{value}</code></td>)}
            {shown.objectives.length > 0 && <td>{picked ? "picked" : front.has(trial.end) ? "on the front" : compared.data ? "dominated" : "…"}</td>}
            <td><button type="button" onClick={() => onOpen(trial.end, `${path} · ${arm.arm.strategy} · trial ${index + 1}`)}>Open</button></td>
          </tr>;
        }))}</tbody>
      </table></div>}
      {compared.data && <p className="muted">{compared.data.front.length} of {ends.length} trial ends are on the front no other dominates; the pick is the best by {order.join(", then ")}.</p>}
    </div>
  </details>;
}
