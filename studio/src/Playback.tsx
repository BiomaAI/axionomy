import { IconChevronLeft, IconChevronRight, IconPlayerPauseFilled, IconPlayerPlayFilled, IconPlayerSkipBack, IconPlayerSkipForward } from "@tabler/icons-react";
import logoDark from "../../assets/axionomy-logo-dark.png";
import logoLight from "../../assets/axionomy-logo-light.png";
import type { ExchangeFrame, ViewDocument, ViewSnapshot } from "./api";
import type { ReplayMotionMode } from "./SceneView";

/** The masthead's mark. */
export function Brand() {
  return <div className="brand"><picture><source media="(prefers-color-scheme: light)" srcSet={logoLight} /><img src={logoDark} alt="Axionomy" /></picture><div><div className="brand-name">Studio</div><div className="brand-subtitle">Economic reasoning workbench</div></div></div>;
}

/**
 * `${account}|${asset}` pairs that change somewhere in a document's trace.
 * This is only a display hint: lack of change does not redefine an asset's
 * economic meaning or make it non-authoritative.
 */
export function changingBalances(document: ViewDocument | undefined): Set<string> | null {
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
}

/** Playback of a point's history: to its start or end, a step at a time, at a chosen speed. */
/**
 * The transport. Where nothing has happened yet, Play asks for moves
 * (`onPlayAhead`) and plays them once they are found; while they are being
 * found (`finding`), Play stops the search.
 */
export function PlaybackControls({ position, count, playing, delay, onDelay, onPosition, onPlaying, onPlayAhead, finding = false, frame }: { position: number; count: number; playing: boolean; delay: number; onDelay: (delay: number) => void; onPosition: (position: number, motion: ReplayMotionMode) => void; onPlaying: (playing: boolean) => void; onPlayAhead?: () => void; finding?: boolean; frame?: ExchangeFrame }) {
  const ahead = count === 0 && onPlayAhead !== undefined;
  return <section className="playback"><div className="transport"><button type="button" aria-label="Go to initial state" disabled={position === 0} onClick={() => { onPlaying(false); onPosition(0, "seek"); }}><IconPlayerSkipBack size={19} stroke={2} aria-hidden="true" /></button><button type="button" aria-label="Previous exchange" disabled={position === 0} onClick={() => { onPlaying(false); onPosition(Math.max(0, position - 1), "step"); }}><IconChevronLeft size={20} stroke={2.1} aria-hidden="true" /></button><button type="button" className="play" aria-label={finding ? "Stop finding moves" : playing ? "Pause" : "Play"} disabled={count === 0 && !ahead} onClick={() => (ahead ? onPlayAhead() : onPlaying(!playing))}>{playing || finding ? <IconPlayerPauseFilled size={18} aria-hidden="true" /> : <IconPlayerPlayFilled size={18} aria-hidden="true" />}</button><button type="button" aria-label="Next exchange" disabled={position === count} onClick={() => { onPlaying(false); onPosition(Math.min(count, position + 1), "step"); }}><IconChevronRight size={20} stroke={2.1} aria-hidden="true" /></button><button type="button" aria-label="Go to final state" disabled={position === count} onClick={() => { onPlaying(false); onPosition(count, "seek"); }}><IconPlayerSkipForward size={19} stroke={2} aria-hidden="true" /></button></div><div className="scrubber"><div className="scrubber-label"><span>{finding ? "Finding the moves to play…" : position === 0 ? count === 0 && ahead ? "Initial state · Play finds the moves and plays them" : "Initial state" : frame?.cues[0]?.label ?? frame?.exchange.rate.label}</span><strong>{position} / {count}</strong></div><input name="trace-position" aria-label="Trace position" type="range" min="0" max={count} value={position} onChange={(event) => onPosition(Number(event.target.value), "seek")} /></div><label className="playback-speed"><span>Speed</span><select name="playback-speed" aria-label="Playback speed" value={delay} onChange={(event) => onDelay(Number(event.target.value))}><option value={1600}>0.5×</option><option value={900}>0.75×</option><option value={650}>1×</option><option value={325}>2×</option><option value={160}>4×</option></select></label><div className="kbd-hint" aria-hidden="true"><b>Space</b> play · <b>←</b><b>→</b> step</div><div className="replay-proof"><span>✓</span> replay verified</div></section>;
}
