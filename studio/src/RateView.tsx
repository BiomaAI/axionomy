import { useMemo, useState, type ReactNode } from "react";
import type { QuantityData, RateData } from "./api";
import { Holding } from "./ExchangeView";
import { IdLink } from "./Inspectors";
import { Term } from "./Term";

type Domain = NonNullable<RateData["variables"]>[number]["domain"];

/** A domain in brief: `1..9`, or its values. */
export function domainWords(domain: Domain): string {
  return domain.kind === "range" ? `${domain.first}..${domain.last}` : domain.values.join(", ");
}

/**
 * A rate as written: its variables and the values each may take, the pairs
 * that must differ, its relations and derived values, the accounts it fixes,
 * what each role gives, keeps and gets, and its conditions.
 */
export function RateView({ rate }: { rate: RateData }) {
  const variables = rate.variables ?? [];
  const fixed = new Map((rate.accounts ?? []).map((fixedAccount) => [fixedAccount.role.key, fixedAccount.account]));
  const computed = [
    ...(rate.computed_consumed ?? []).map((amount) => ({ ...amount, word: "gives" as const })),
    ...(rate.computed_preserved ?? []).map((amount) => ({ ...amount, word: "keeps" as const })),
    ...(rate.computed_produced ?? []).map((amount) => ({ ...amount, word: "gets" as const })),
  ];
  return <div className="rate-view">
    {(variables.length > 0 || rate.instances != null) && <Section title="Variables" aside={rate.instances != null ? `${rate.instances.toLocaleString()} ${rate.instances === 1 ? "instance" : "instances"}` : undefined}>
      <dl className="facts">{variables.map((variable) => <Fact key={variable.name} term={<code>{`{${variable.name}}`}</code>}>{domainWords(variable.domain)}</Fact>)}</dl>
      {(rate.distinct_variables ?? []).length > 0 && <p className="rate-note">{(rate.distinct_variables ?? []).map((pair) => pair.map((name) => `{${name}}`).join(" ≠ ")).join(", ")}</p>}
    </Section>}
    {(rate.relations ?? []).map((relation, index) => <Section key={`relation:${index}`} title="Relation" aside={`${relation.rows.length} rows`}>
      <div className="table-scroll"><table className="compact-table">
        <thead><tr>{relation.variables.map((name) => <th key={name}>{`{${name}}`}</th>)}</tr></thead>
        <tbody>{relation.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((value, column) => <td key={column}>{value}</td>)}</tr>)}</tbody>
      </table></div>
    </Section>)}
    {(rate.derived ?? []).length > 0 && <Section title="Derived">
      <dl className="facts">{(rate.derived ?? []).map((derived) => <Fact key={derived.name} term={<code>{`{${derived.name}}`}</code>}><code>{derived.value}</code></Fact>)}</dl>
    </Section>}
    <Section title="Roles">
      {rate.roles.map((role) => <div className="rate-role" key={role.role.key}>
        <div className="rate-role-name"><strong>{role.role.label}</strong>{fixed.has(role.role.key) && <> <IdLink subject={{ kind: "account", id: fixed.get(role.role.key)! }} /></>}</div>
        <Basket word="gives" values={role.consumed} />
        <Basket word="keeps" values={role.preserved} />
        <Basket word="gets" values={role.produced} />
        {computed.filter((amount) => amount.role.key === role.role.key).map((amount, index) => <div className={`effect-line effect-${amount.word}`} key={`${amount.word}:${amount.asset.key}:${index}`}>
          <Term name={amount.word} /><div><span className="holding"><IdLink subject={{ kind: "asset", id: amount.asset }} /> <code>{amount.quantity}</code></span></div>
        </div>)}
      </div>)}
      {rate.distinct_roles.length > 0 && <p className="rate-note">{rate.distinct_roles.map((pair) => pair.map((role) => role.label).join(" ≠ ")).join(", ")}</p>}
    </Section>
    {(rate.conditions ?? []).length > 0 && <Section title="Conditions">
      <dl className="facts">{(rate.conditions ?? []).map((condition) => <Fact key={condition.name} term={condition.name}><code>{condition.left} {condition.comparison} {condition.right}</code></Fact>)}</dl>
    </Section>}
    {(rate.parameters ?? []).length > 0 && <Section title="Parameters"><p className="rate-note">{(rate.parameters ?? []).join(", ")}</p></Section>}
  </div>;
}

function Section({ title, aside, children }: { title: string; aside?: string; children: ReactNode }) {
  return <section className="view-section"><h4>{title}{aside && <small>{aside}</small>}</h4>{children}</section>;
}

function Fact({ term, children }: { term: ReactNode; children: ReactNode }) {
  return <><dt>{term}</dt><dd>{children}</dd></>;
}

function Basket({ word, values }: { word: "gives" | "keeps" | "gets"; values: QuantityData[] }) {
  if (values.length === 0) return null;
  return <div className={`effect-line effect-${word}`}><Term name={word} /><div>{values.map((value) => <Holding key={value.asset.key} value={value} />)}</div></div>;
}

/** The most rates the table lists at once; a filter finds the rest. */
const SHOWN = 200;

/**
 * Every rate of a model as written, one row each: its variables, how many
 * instances it answers to, and its roles. A rate opens on its name.
 */
export function RateTable({ rates }: { rates: RateData[] }) {
  const [filter, setFilter] = useState("");
  const matching = useMemo(() => {
    const wanted = filter.trim().toLowerCase();
    return wanted ? rates.filter((rate) => `${rate.rate.label} ${rate.rate.key}`.toLowerCase().includes(wanted)) : rates;
  }, [rates, filter]);
  return <div className="rate-table">
    <input type="search" name="rate-filter" aria-label="Filter rates" placeholder={`Filter ${rates.length.toLocaleString()} rates`} value={filter} onChange={(event) => setFilter(event.target.value)} />
    <div className="table-scroll"><table className="compact-table">
      <thead><tr><th>Rate</th><th>Variables</th><th>Instances</th><th>Roles</th></tr></thead>
      <tbody>{matching.slice(0, SHOWN).map((rate) => <tr key={rate.rate.key}>
        <td><IdLink subject={{ kind: "rate", id: rate.rate }} /></td>
        <td>{(rate.variables ?? []).map((variable) => <span key={variable.name} className="variable" title={domainWords(variable.domain)}>{`{${variable.name}}`}</span>)}</td>
        <td className="number">{(rate.instances ?? 1).toLocaleString()}</td>
        <td>{rate.roles.map((role) => role.role.label).join(", ")}</td>
      </tr>)}</tbody>
    </table></div>
    {matching.length > SHOWN && <p className="muted">First {SHOWN} of {matching.length.toLocaleString()}</p>}
  </div>;
}
