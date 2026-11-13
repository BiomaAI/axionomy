import { useContext, useEffect, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import type { ExchangeFrame, ViewDocument, ViewSnapshot } from "./api";
import { Amount, QuantityFormats, useQuantity } from "./Quantity";

/** A name written as a path, such as `time-accounting`, read as words. */
function words(path: string): string {
  return path.replaceAll("-", " ").replaceAll("/", " · ");
}

export function PanelHeading({ kicker, title, aside, action }: { kicker: string; title: string; aside?: string; action?: ReactNode }) {
  return <div className="panel-heading"><div><span>{kicker}</span><h2>{title}</h2></div><div className="panel-heading-side">{aside && <small>{aside}</small>}{action}</div></div>;
}

type QuantityValue = { asset: { key: string; label: string }; quantity: string };
type BalanceRow = { balance: QuantityValue; prior?: string; changed: boolean; added: boolean; live: boolean };

// changingBalanceKeys contains `${account}|${asset}` pairs that change somewhere
// in the trace. It controls disclosure only; unchanged assets remain equally
// authoritative and must not be reclassified as configuration by the viewer.
export function Accounts({ snapshot, previous, focus, changingBalanceKeys }: { snapshot: ViewSnapshot; previous?: ViewSnapshot; focus?: string; changingBalanceKeys?: Set<string> | null }) {
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
          live: !changingBalanceKeys || changingBalanceKeys.has(`${account.account.key}|${balance.asset.key}`),
        };
      });
      const ghosts = (prevAccount?.balances ?? []).filter((prev) => !account.balances.some((candidate) => candidate.asset.key === prev.asset.key));
      return { account: account.account, rows, ghosts };
    });
    if (!changingBalanceKeys) return mapped;
    const weight = (card: (typeof mapped)[number]) => (card.rows.some((row) => row.live) || card.ghosts.length > 0 ? 0 : 1);
    return [...mapped].sort((a, b) => weight(a) - weight(b));
  }, [snapshot, previous, changingBalanceKeys]);
  const row = (entry: BalanceRow) => <div className={`balance ${entry.changed || entry.added ? "changed" : ""}`} key={entry.balance.asset.key}>
    <span title={entry.balance.asset.key}>{entry.balance.asset.label}</span>
    <strong><Amount asset={entry.balance.asset.key} atoms={entry.balance.quantity} /></strong>
    {entry.changed && entry.prior !== undefined && <small><Amount asset={entry.balance.asset.key} atoms={entry.prior} /> →</small>}
    {entry.added && <small>new</small>}
  </div>;
  return <div className="accounts-list">
    {cards.map(({ account, rows, ghosts }) => {
      const live = rows.filter((entry) => entry.live);
      const unchanged = rows.filter((entry) => !entry.live);
      const unchangedOnly = live.length === 0 && ghosts.length === 0 && unchanged.length > 0;
      return <article id={`account-${encodeURIComponent(account.key)}`} className={`account-card ${focus === account.key ? "focused" : ""} ${unchangedOnly ? "unchanged-only" : ""}`} key={account.key}>
        <h3><span className="account-icon">{account.label.slice(0, 1)}</span>{account.label}</h3>
        <div className="balances">
          {live.map(row)}
          {ghosts.map((ghost) => <div className="balance ghost" key={`ghost:${ghost.asset.key}`}>
            <span title={ghost.asset.key}>{ghost.asset.label}</span>
            <strong><Amount asset={ghost.asset.key} atoms="0" /></strong>
            <small><Amount asset={ghost.asset.key} atoms={ghost.quantity} /> →</small>
          </div>)}
          {unchanged.length > 0 && <details className="unchanged-assets">
            <summary>{unchanged.length} unchanged {unchanged.length === 1 ? "asset" : "assets"} in this replay</summary>
            {unchanged.map(row)}
          </details>}
          {rows.length === 0 && ghosts.length === 0 && <div className="muted">No balances</div>}
        </div>
      </article>;
    })}
  </div>;
}

export function StepTicker({ frame, steps, onProof }: { frame?: ExchangeFrame; steps: number; onProof?: () => void }) {
  if (steps === 0) return <div className="empty-state">This outcome has no exchanges, so there is nothing to play: its initial state is the whole result.</div>;
  if (!frame) return <div className="empty-state">This is the initial state. Press play and every exchange narrates here as it applies.</div>;
  return <div className="step-ticker">
    <div className="ticker-feed" key={frame.index}>
      {frame.cues.map((cue, index) => <article key={`${cue.kind}:${index}`} className={`cue-${cue.kind}`} style={{ "--cue-i": index } as CSSProperties}>
        <strong>{cue.label}</strong>
        {cue.details.map((detail, detailIndex) => <span key={`${detail}:${detailIndex}`}>{detail}</span>)}
      </article>)}
    </div>
    <div className="ticker-foot">
      <span>{frame.exchange.rate.label} · {frame.exchange.units} {String(frame.exchange.units) === "1" ? "unit" : "units"}</span>
      {onProof && <button type="button" onClick={onProof}>Full proof ↓</button>}
    </div>
  </div>;
}

export function Transition({ frame, steps }: { frame?: ExchangeFrame; steps: number }) {
  if (steps === 0) return <div className="empty-state">This outcome has no exchanges to inspect.</div>;
  if (!frame) return <div className="empty-state">Move the scrubber to see what a step checked, predicted, and actually changed.</div>;
  return <div className="transition">
    <div className="frame-cues" aria-label="Transition cues">
      {frame.cues.map((cue, index) => <article key={`${cue.kind}:${index}`} className={`cue-${cue.kind}`}><strong>{cue.label}</strong>{cue.details.map((detail) => <span key={detail}>{detail}</span>)}</article>)}
    </div>
    <div className="binding-row">
      {frame.exchange.bindings.map((binding) => <span key={binding.role.key}><small>{binding.role.label}</small>{binding.account.label}</span>)}
      <span><small>Units</small>{frame.exchange.units}</span>
      {frame.exchange.parameters?.map((parameter) => <span key={parameter.name}><small>{words(parameter.name)}</small>{parameter.value}</span>)}
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

export function Assessment({ assessment }: { assessment: ExchangeFrame["assessment"] }) {
  const quantity = useQuantity();
  return <div className={`assessment assessment-${assessment.status}`}>
    <strong>{assessment.status}</strong>
    {assessment.shortfalls.map((shortfall) => <div key={shortfall.account.key}>
      {shortfall.account.label} lacks {shortfall.missing.map((missing) => `${quantity(missing.asset.key, missing.quantity)} ${missing.asset.label}`).join(", ")}
    </div>)}
    {assessment.issues.map((issue, index) => <div className="assessment-issue" key={`${issue.kind}:${index}`}><span>{issue.message}</span><small>{issue.kind.replaceAll("_", " ")}</small></div>)}
    {assessment.status === "applicable" && <div>{assessment.projected_deltas.length} accounts would change — predicted, nothing changed yet</div>}
  </div>;
}

function DeltaGroup({ label, kind, values }: { label: string; kind: string; values: QuantityValue[] }) {
  if (values.length === 0) return null;
  return <div className={`delta-group ${kind}`}><span>{label}</span><div>{values.map((value) => <BasketQuantity key={value.asset.key} value={value} />)}</div></div>;
}

/**
 * An asset and how much of it. A bare count reads as a multiple ("Key ×1");
 * a quantity with a unit already says what it counts ("Time left 1 ticks").
 */
function BasketQuantity({ value }: { value: QuantityValue }) {
  const format = useContext(QuantityFormats)[value.asset.key];
  return <b>{value.asset.label} <em>{format?.unit ? "" : "×"}<Amount asset={value.asset.key} atoms={value.quantity} /></em></b>;
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
      <div className="rate-controls"><label><span>Filter {model.rates.length} rates</span><input name="rate-filter" type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="Rate, action, route, operation…" /></label><label><span>Rate</span><select name="rate" value={rate?.rate.key ?? ""} onChange={(event) => setSelectedRate(event.target.value)}>{visibleRates.map((candidate) => <option key={candidate.rate.key} value={candidate.rate.key}>{candidate.rate.label}</option>)}</select></label></div>
      {matchingRates.length > visibleRates.length && <p className="muted">Showing the first 200 matches; refine the filter to inspect the rest.</p>}
      {rate && <div className="rate-contract">
        {rate.roles.map((role) => <article key={role.role.key}><h3>{role.role.label}</h3><RateBasket label="Consume" values={role.consumed} /><RateBasket label="Produce" values={role.produced} /><RateBasket label="Preserve" values={role.preserved} /></article>)}
        {rate.computed_consumed?.map((amount, index) => <ComputedRateAmount key={`consume:${amount.role.key}:${amount.asset.key}:${index}`} label="Computed consume" amount={amount} />)}
        {rate.computed_produced?.map((amount, index) => <ComputedRateAmount key={`produce:${amount.role.key}:${amount.asset.key}:${index}`} label="Computed produce" amount={amount} />)}
        {rate.computed_preserved?.map((amount, index) => <ComputedRateAmount key={`preserve:${amount.role.key}:${amount.asset.key}:${index}`} label="Computed preserve" amount={amount} />)}
        {rate.conditions?.map((condition) => <article className="rate-condition" key={condition.name}><h3>{words(condition.name)}</h3><code>{condition.left} {condition.comparison} {condition.right}</code></article>)}
        {rate.distinct_roles.length > 0 && <p>Distinct accounts: {rate.distinct_roles.map((pair) => pair.map((role) => role.label).join(" ≠ ")).join(", ")}</p>}
      </div>}
    </section>
    <section className="model-rules">
      <h3>Goal</h3>
      {model.goal.map((goal) => <div key={goal.account.key}><strong>{goal.account.label}</strong>{goal.required.map((required) => <BasketQuantity key={required.asset.key} value={required} />)}</div>)}
      <h3>Conservation laws</h3>
      {model.invariants.map((invariant) => <details key={invariant.name}><summary>{words(invariant.name)}</summary><p>{invariant.terms.map((term) => `${term.weight}·${term.asset.label}`).join(" + ")}</p></details>)}
    </section>
  </div>;
}

function RateBasket({ label, values }: { label: string; values: QuantityValue[] }) {
  if (values.length === 0) return null;
  return <div className="rate-basket"><span>{label}</span><div>{values.map((value) => <BasketQuantity key={value.asset.key} value={value} />)}</div></div>;
}

function ComputedRateAmount({ label, amount }: { label: string; amount: { role: { key: string; label: string }; asset: { key: string; label: string }; quantity: string } }) {
  return <article className="computed-rate-amount"><h3>{amount.role.label}</h3><div className="rate-basket"><span>{label}</span><b>{amount.asset.label}</b><code>{amount.quantity}</code></div></article>;
}

export function useSnapshot(document: ViewDocument | undefined, position: number) {
  return useMemo(() => {
    if (!document) return undefined;
    return position === 0 ? document.initial : document.frames[position - 1]?.after;
  }, [document, position]);
}
