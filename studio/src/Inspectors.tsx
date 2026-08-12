import { useEffect, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import type { ExchangeFrame, ProposalView, ViewDocument, ViewSnapshot } from "./api";

export function PanelHeading({ kicker, title, aside, action }: { kicker: string; title: string; aside?: string; action?: ReactNode }) {
  return <div className="panel-heading"><div><span>{kicker}</span><h2>{title}</h2></div><div className="panel-heading-side">{aside && <small>{aside}</small>}{action}</div></div>;
}

type QuantityValue = { asset: { key: string; label: string }; quantity: string };
type BalanceRow = { balance: QuantityValue; prior?: string; changed: boolean; added: boolean; live: boolean };

// liveKeys: `${account}|${asset}` pairs that ever change across the whole trace.
// null means the document has no trace, so no live/config split applies.
export function Accounts({ snapshot, previous, focus, liveKeys }: { snapshot: ViewSnapshot; previous?: ViewSnapshot; focus?: string; liveKeys?: Set<string> | null }) {
  useEffect(() => {
    if (!focus) return;
    document.getElementById(`account-${encodeURIComponent(focus)}`)?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, [focus, snapshot.index]);
  const cards = useMemo(() => {
    const mapped = snapshot.accounts.map((account) => {
      const prevAccount = previous?.accounts.find((candidate) => candidate.account.key === account.account.key);
      const rows: BalanceRow[] = account.balances.map((balance) => {
        const prior = prevAccount?.balances.find((candidate) => candidate.asset.key === balance.asset.key)?.quantity;
        return {
          balance,
          prior,
          changed: prior !== undefined && prior !== balance.quantity,
          added: prior === undefined && snapshot.index > 0,
          live: !liveKeys || liveKeys.has(`${account.account.key}|${balance.asset.key}`),
        };
      });
      const ghosts = (prevAccount?.balances ?? []).filter((prev) => !account.balances.some((candidate) => candidate.asset.key === prev.asset.key));
      return { account: account.account, rows, ghosts };
    });
    if (!liveKeys) return mapped;
    const weight = (card: (typeof mapped)[number]) => (card.rows.some((row) => row.live) || card.ghosts.length > 0 ? 0 : 1);
    return [...mapped].sort((a, b) => weight(a) - weight(b));
  }, [snapshot, previous, liveKeys]);
  const row = (entry: BalanceRow) => <div className={`balance ${entry.changed || entry.added ? "changed" : ""}`} key={entry.balance.asset.key}>
    <span title={entry.balance.asset.key}>{entry.balance.asset.label}</span>
    <strong>{entry.balance.quantity}</strong>
    {entry.changed && <small>{entry.prior} →</small>}
    {entry.added && <small>new</small>}
  </div>;
  return <div className="accounts-list">
    {cards.map(({ account, rows, ghosts }) => {
      const live = rows.filter((entry) => entry.live);
      const config = rows.filter((entry) => !entry.live);
      const configOnly = live.length === 0 && ghosts.length === 0 && config.length > 0;
      return <article id={`account-${encodeURIComponent(account.key)}`} className={`account-card ${focus === account.key ? "focused" : ""} ${configOnly ? "config-only" : ""}`} key={account.key}>
        <h3><span className="account-icon">{account.label.slice(0, 1)}</span>{account.label}</h3>
        <div className="balances">
          {live.map(row)}
          {ghosts.map((ghost) => <div className="balance ghost" key={`ghost:${ghost.asset.key}`}>
            <span title={ghost.asset.key}>{ghost.asset.label}</span>
            <strong>0</strong>
            <small>{ghost.quantity} →</small>
          </div>)}
          {config.length > 0 && <details className="config-assets">
            <summary>{config.length} configuration {config.length === 1 ? "asset" : "assets"}</summary>
            {config.map(row)}
          </details>}
          {rows.length === 0 && ghosts.length === 0 && <div className="muted">No balances</div>}
        </div>
      </article>;
    })}
  </div>;
}

export function StepTicker({ frame, onProof }: { frame?: ExchangeFrame; onProof?: () => void }) {
  if (!frame) return <div className="empty-state">This is the initial state. Press play and every exchange narrates here as it applies.</div>;
  return <div className="step-ticker">
    <div className="ticker-feed" key={frame.index}>
      {frame.cues.map((cue, index) => <article key={`${cue.kind}:${index}`} className={`cue-${cue.kind}`} style={{ "--cue-i": index } as CSSProperties}>
        <strong>{cue.label}</strong>
        {cue.details.map((detail) => <span key={detail}>{detail}</span>)}
      </article>)}
    </div>
    <div className="ticker-foot">
      <span>{frame.exchange.rate.label} · {frame.exchange.units} {String(frame.exchange.units) === "1" ? "unit" : "units"}</span>
      {onProof && <button type="button" onClick={onProof}>Full proof ↓</button>}
    </div>
  </div>;
}

export function Transition({ frame }: { frame?: ExchangeFrame }) {
  if (!frame) return <div className="empty-state">Move the scrubber to see what a step checked, predicted, and actually changed.</div>;
  return <div className="transition">
    <div className="frame-cues" aria-label="Transition cues">
      {frame.cues.map((cue, index) => <article key={`${cue.kind}:${index}`} className={`cue-${cue.kind}`}><strong>{cue.label}</strong>{cue.details.map((detail) => <span key={detail}>{detail}</span>)}</article>)}
    </div>
    <div className="binding-row">
      {frame.exchange.bindings.map((binding) => <span key={binding.role.key}><small>{binding.role.label}</small>{binding.account.label}</span>)}
      <span><small>Units</small>{frame.exchange.units}</span>
    </div>
    <Assessment assessment={frame.assessment} />
    <div className="proof-line">Projected deltas {JSON.stringify(frame.assessment.projected_deltas) === JSON.stringify(frame.receipt.deltas) ? "match" : "do not match"} the receipt</div>
    {frame.receipt.deltas.map((delta) => <div className="delta" key={delta.account.key}>
      <h3>{delta.account.label}</h3>
      <DeltaGroup label="Consumed" kind="consumed" values={delta.consumed} />
      <DeltaGroup label="Produced" kind="produced" values={delta.produced} />
      <DeltaGroup label="Preserved" kind="preserved" values={delta.preserved} />
    </div>)}
  </div>;
}

export function ProposalInspector({ proposals }: { proposals: ProposalView[] }) {
  const [selectedId, setSelectedId] = useState(proposals[0]?.id);
  useEffect(() => setSelectedId(proposals[0]?.id), [proposals]);
  if (proposals.length === 0) return <div className="empty-state">This problem has no rule-check probe.</div>;
  const selected = proposals.find((proposal) => proposal.id === selectedId) ?? proposals[0];
  return <div className="proposal-inspector">
    <div className="counterexample-note"><strong>Expected rejection</strong><span>These probes are deliberately invalid or impossible. Their rejection proves the rules are doing their job; they are not failed runs.</span></div>
    <div className="proposal-tabs" role="tablist">
      {proposals.map((proposal) => <button type="button" role="tab" aria-selected={proposal.id === selected.id} key={proposal.id} onClick={() => setSelectedId(proposal.id)}>{proposal.label}</button>)}
    </div>
    <p>{selected.description}</p>
    <div className="binding-row">
      <span><small>Rate</small>{selected.exchange.rate.label}</span>
      {selected.exchange.bindings.map((binding) => <span key={binding.role.key}><small>{binding.role.label}</small>{binding.account.label}</span>)}
    </div>
    <Assessment assessment={selected.assessment} expectedRejection />
  </div>;
}

function Assessment({ assessment, expectedRejection = false }: { assessment: ExchangeFrame["assessment"]; expectedRejection?: boolean }) {
  return <div className={`assessment assessment-${assessment.status} ${expectedRejection ? "expected-rejection" : ""}`}>
    <strong>{expectedRejection ? `expected rejection · ${assessment.status}` : assessment.status}</strong>
    {assessment.shortfalls.map((shortfall) => <div key={shortfall.account.key}>
      {shortfall.account.label} lacks {shortfall.missing.map((missing) => `${missing.quantity} ${missing.asset.label}`).join(", ")}
    </div>)}
    {assessment.issues.map((issue, index) => <div className="assessment-issue" key={`${issue.kind}:${index}`}><span>{issue.message}</span><small>{issue.kind.replaceAll("_", " ")}</small></div>)}
    {assessment.status === "applicable" && <div>{assessment.projected_deltas.length} accounts would change — predicted, nothing changed yet</div>}
  </div>;
}

function DeltaGroup({ label, kind, values }: { label: string; kind: string; values: QuantityValue[] }) {
  if (values.length === 0) return null;
  return <div className={`delta-group ${kind}`}><span>{label}</span><div>{values.map((value) => <b key={value.asset.key}>{value.asset.label} <em>×{value.quantity}</em></b>)}</div></div>;
}

export function Telemetry({ document }: { document: ViewDocument }) {
  if (document.telemetry.length === 0) return <div className="empty-state">No search telemetry was published.</div>;
  return <div className="telemetry-list">
    {document.telemetry.map((series) => <section key={series.algorithm}>
      <h3>{series.algorithm}<span>{series.exact ? "exact" : "sampled"}</span></h3>
      <div>{series.points.map((point) => <article key={`${point.sequence}:${point.kind}`}><strong>{point.value}</strong><span>{point.label}</span><small>{point.kind.replaceAll("_", " ")}</small></article>)}</div>
    </section>)}
  </div>;
}

export function ModelExplorer({ document }: { document: ViewDocument }) {
  const model = document.model;
  const [selectedRate, setSelectedRate] = useState(model?.rates[0]?.rate.key);
  const [filter, setFilter] = useState("");
  useEffect(() => { setSelectedRate(model?.rates[0]?.rate.key); setFilter(""); }, [document.id, model]);
  const rate = model?.rates.find((candidate) => candidate.rate.key === selectedRate) ?? model?.rates[0];
  if (!model) return <div className="empty-state">This result was produced before rule inspection existed.</div>;
  const matchingRates = model.rates.filter((candidate) => `${candidate.rate.label} ${candidate.rate.key}`.toLowerCase().includes(filter.toLowerCase()));
  const visibleRates = matchingRates.slice(0, 200);
  if (rate && !visibleRates.some((candidate) => candidate.rate.key === rate.rate.key)) visibleRates.unshift(rate);
  return <div className="model-explorer">
    <section className="rate-browser">
      <div className="rate-controls"><label><span>Filter {model.rates.length} rates</span><input type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="Rate, action, route, operation…" /></label><label><span>Rate</span><select value={rate?.rate.key ?? ""} onChange={(event) => setSelectedRate(event.target.value)}>{visibleRates.map((candidate) => <option key={candidate.rate.key} value={candidate.rate.key}>{candidate.rate.label}</option>)}</select></label></div>
      {matchingRates.length > visibleRates.length && <p className="muted">Showing the first 200 matches; refine the filter to inspect the rest.</p>}
      {rate && <div className="rate-contract">
        {rate.roles.map((role) => <article key={role.role.key}><h3>{role.role.label}</h3><RateBasket label="Consume" values={role.consumed} /><RateBasket label="Produce" values={role.produced} /><RateBasket label="Preserve" values={role.preserved} /></article>)}
        {rate.distinct_roles.length > 0 && <p>Distinct accounts: {rate.distinct_roles.map((pair) => pair.map((role) => role.label).join(" ≠ ")).join(", ")}</p>}
      </div>}
    </section>
    <section className="model-rules">
      <h3>Goal</h3>
      {model.goal.map((goal) => <div key={goal.account.key}><strong>{goal.account.label}</strong>{goal.required.map((required) => <span key={required.asset.key}>{required.asset.label} ×{required.quantity}</span>)}</div>)}
      <h3>Conservation laws</h3>
      {model.invariants.map((invariant) => <details key={invariant.name}><summary>{invariant.name}</summary><p>{invariant.terms.map((term) => `${term.weight}·${term.asset.label}`).join(" + ")}</p></details>)}
    </section>
  </div>;
}

function RateBasket({ label, values }: { label: string; values: QuantityValue[] }) {
  if (values.length === 0) return null;
  return <div className="rate-basket"><span>{label}</span>{values.map((value) => <b key={value.asset.key}>{value.asset.label} ×{value.quantity}</b>)}</div>;
}

export function Observations({ document }: { document: ViewDocument }) {
  if (document.observations.length === 0) return <div className="empty-state">This problem has no actor-relative observation boundary.</div>;
  return <div className="observations">
    {document.observations.map((observation) => <article key={observation.actor.key}>
      <h3>{observation.actor.label}</h3><p>{observation.label}</p>
      {observation.visible_accounts.map((account) => <div key={account.account.key}><strong>{account.account.label}</strong><span>{account.balances.length} visible balances</span></div>)}
    </article>)}
  </div>;
}

export function useSnapshot(document: ViewDocument | undefined, position: number) {
  return useMemo(() => {
    if (!document) return undefined;
    return position === 0 ? document.initial : document.frames[position - 1]?.after;
  }, [document, position]);
}
