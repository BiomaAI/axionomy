import { useEffect, useMemo, useState } from "react";
import type { ExchangeFrame, ProposalView, ViewDocument, ViewSnapshot } from "./api";

export function PanelHeading({ kicker, title, aside }: { kicker: string; title: string; aside?: string }) {
  return <div className="panel-heading"><div><span>{kicker}</span><h2>{title}</h2></div>{aside && <small>{aside}</small>}</div>;
}

export function Accounts({ snapshot, previous }: { snapshot: ViewSnapshot; previous?: ViewSnapshot }) {
  return <div className="accounts-list">
    {snapshot.accounts.map((account) => (
      <article className="account-card" key={account.account.key}>
        <h3><span className="account-icon">{account.account.label.slice(0, 1)}</span>{account.account.label}</h3>
        <div className="balances">
          {account.balances.map((balance) => {
            const prior = previous?.accounts.find((candidate) => candidate.account.key === account.account.key)?.balances.find((candidate) => candidate.asset.key === balance.asset.key)?.quantity;
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

export function Transition({ frame }: { frame?: ExchangeFrame }) {
  if (!frame) return <div className="empty-state">Move the scrubber to inspect the assessment, projected deltas, and receipt.</div>;
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
  if (proposals.length === 0) return <div className="empty-state">This adapter does not publish a rejected proposal.</div>;
  const selected = proposals.find((proposal) => proposal.id === selectedId) ?? proposals[0];
  return <div className="proposal-inspector">
    <div className="counterexample-note"><strong>Expected rejection</strong><span>These probes are intentionally malformed or infeasible. Their rejection proves that the encoded constraints are active; they are not run failures.</span></div>
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
    {assessment.status === "applicable" && <div>{assessment.projected_deltas.length} account deltas projected without mutation</div>}
  </div>;
}

type QuantityValue = { asset: { key: string; label: string }; quantity: string };
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
  if (!model) return <div className="empty-state">This document predates model-definition projection.</div>;
  const matchingRates = model.rates.filter((candidate) => `${candidate.rate.label} ${candidate.rate.key}`.toLowerCase().includes(filter.toLowerCase()));
  const visibleRates = matchingRates.slice(0, 200);
  if (rate && !visibleRates.some((candidate) => candidate.rate.key === rate.rate.key)) visibleRates.unshift(rate);
  return <div className="model-explorer">
    <section className="rate-browser">
      <div className="rate-controls"><label><span>Filter {model.rates.length} encoded rates</span><input type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="Rate, action, route, operation…" /></label><label><span>Encoded rate</span><select value={rate?.rate.key ?? ""} onChange={(event) => setSelectedRate(event.target.value)}>{visibleRates.map((candidate) => <option key={candidate.rate.key} value={candidate.rate.key}>{candidate.rate.label}</option>)}</select></label></div>
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
