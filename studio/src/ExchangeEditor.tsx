import { useEffect, useMemo, useState } from "react";
import type { RateData, ViewId } from "./api";
import { ExchangeView, Written } from "./ExchangeView";
import { exchangeSource, type Candidate, type Exchange } from "./packageEngine";
import { Term } from "./Term";

type Pieces = NonNullable<RateData["pattern"]>;

/**
 * The identifier a pattern's pieces write with values for its variables:
 * `step/{from}/{to}` with from 1 and to 2 writes `step/1/2`. Undefined
 * while a variable has no value.
 */
export function written(pieces: Pieces, values: Readonly<Record<string, string>>): string | undefined {
  const segments: string[] = [];
  for (const piece of pieces) {
    const segment = piece.kind === "segment" ? piece.value : values[piece.name];
    if (!segment) return undefined;
    segments.push(segment);
  }
  return segments.join("/");
}

/** A path as identifiers are written: lowercase segments of letters, digits and inner hyphens. */
const PATH = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\/[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$/;

/**
 * The path a view's key names in its ontology: a view keys a value
 * `<ontology>/<category>/<path>`, so the rate `maze/rate/finish` of the
 * ontology `maze` is the rate `finish`. Undefined for a key of another
 * ontology or category.
 */
export function pathUnder(ontology: string, category: "rate" | "role" | "account", key: string): string | undefined {
  const prefix = `${ontology}/${category}/`;
  return key.startsWith(prefix) && key.length > prefix.length ? key.slice(prefix.length) : undefined;
}

/** The most values a variable is chosen from in a list; more are typed. */
const LISTED = 500;

type Variables = NonNullable<RateData["variables"]>;

/** Every value of a domain, or undefined where there are too many to list. */
function domainValues(domain: Variables[number]["domain"]): string[] | undefined {
  if (domain.kind === "values") return domain.values;
  if (domain.last - domain.first + 1 > LISTED) return undefined;
  return Array.from({ length: domain.last - domain.first + 1 }, (_, index) => String(domain.first + index));
}

/**
 * The values the variable at `index` may take, given the values of the
 * variables before it: its domain, kept to the rows of each relation it is
 * in that agree with those values, without the value of any earlier
 * variable it must differ from. Undefined where its domain is too large to
 * list.
 */
export function variableChoices(rate: RateData, values: Readonly<Record<string, string>>, index: number): string[] | undefined {
  const variables = rate.variables ?? [];
  const variable = variables[index];
  const earlier = new Set(variables.slice(0, index).map((each) => each.name));
  let choices = domainValues(variable.domain);
  if (!choices) return undefined;
  for (const relation of rate.relations ?? []) {
    const column = relation.variables.indexOf(variable.name);
    if (column < 0) continue;
    const allowed = new Set(relation.rows.filter((row) => relation.variables.every((name, at) => !earlier.has(name) || values[name] === undefined || row[at] === values[name])).map((row) => row[column]));
    choices = choices.filter((value) => allowed.has(value));
  }
  for (const [left, right] of rate.distinct_variables ?? []) {
    const other = left === variable.name ? right : right === variable.name ? left : undefined;
    if (other && earlier.has(other)) choices = choices.filter((value) => value !== values[other]);
  }
  return choices;
}

/**
 * Values for every variable in order, each kept where it is still allowed
 * after the ones before it, else the first value allowed.
 */
export function settleValues(rate: RateData, wanted: Readonly<Record<string, string>>): Record<string, string> {
  const values: Record<string, string> = {};
  (rate.variables ?? []).forEach((variable, index) => {
    const choices = variableChoices(rate, values, index);
    const asked = wanted[variable.name];
    if (!choices) { if (asked) values[variable.name] = asked; return; }
    const value = asked !== undefined && choices.includes(asked) ? asked : choices[0];
    if (value !== undefined) values[variable.name] = value;
  });
  return values;
}

/** An exchange being written: its rate's key, its variables' values, each free role's account, its units and parameters. */
export type Draft = {
  rate: string;
  values: Record<string, string>;
  /** Free roles' accounts, by role key and account key. */
  bindings: Record<string, string>;
  units: string;
  parameters: Record<string, string>;
};

/** The account a rate fixes for a role, written with the variables' values: its key under `ontology`. */
export function fixedAccount(rate: RateData, role: string, ontology: string, values: Readonly<Record<string, string>>): string | undefined {
  const fixed = (rate.accounts ?? []).find((each) => each.role.key === role);
  if (!fixed) return undefined;
  if (!fixed.pattern?.length) return fixed.account.key;
  const path = written(fixed.pattern, values);
  return path ? `${ontology}/account/${path}` : undefined;
}

/** A whole number written in decimal. */
function natural(text: string): boolean {
  return /^(0|[1-9]\d*)$/.test(text);
}

/** The exchange a draft writes, or what it still lacks. */
export function writeExchange(rate: RateData, draft: Draft, ontology: string): { exchange: Exchange } | { lacks: string } {
  const pattern = rate.pattern ?? [];
  const id = pattern.length > 0 ? written(pattern, draft.values) : pathUnder(ontology, "rate", rate.rate.key);
  if (!id) return { lacks: pattern.length > 0 ? "a value for each variable" : `a rate of ${ontology}` };
  const bindings: Record<string, string> = {};
  for (const role of rate.roles) {
    const rolePath = pathUnder(ontology, "role", role.role.key);
    const account = fixedAccount(rate, role.role.key, ontology, draft.values) ?? draft.bindings[role.role.key];
    const accountPath = account ? pathUnder(ontology, "account", account) : undefined;
    if (!rolePath || !accountPath || !PATH.test(accountPath)) return { lacks: `an account for ${role.role.label}` };
    bindings[rolePath] = accountPath;
  }
  if (!natural(draft.units) || BigInt(draft.units) === 0n) return { lacks: "units, a whole number from 1" };
  const parameters: Record<string, string> = {};
  for (const name of rate.parameters ?? []) {
    const value = draft.parameters[name]?.trim();
    if (!value) continue;
    if (!natural(value)) return { lacks: `${name}, a whole number` };
    parameters[name] = value;
  }
  return { exchange: { rate: id, bindings, units: draft.units, parameters } };
}

/** A fresh draft of `rate`: its variables' first allowed values, its free roles unbound, one unit. */
function draftOf(rate: RateData | undefined): Draft {
  return { rate: rate?.rate.key ?? "", values: rate ? settleValues(rate, {}) : {}, bindings: {}, units: "1", parameters: {} };
}

/** The most rates the picker lists at once; a filter finds the rest. */
const PICKED = 200;

/**
 * An exchange written from the model's rates: a rate picked, its variables
 * filled from their domains, which writes the rate's identifier, the
 * accounts it fixes filled in and its free roles chosen from the accounts,
 * its units and parameters set. It is checked before it can be applied.
 */
export function ExchangeEditor({ rates, accounts, ontology, point, onCheck, onApply }: {
  rates: RateData[];
  /** The accounts at the point, for the free roles. */
  accounts: ViewId[];
  /** The ontology the model's keys are under. */
  ontology: string;
  /** Where the exchange is checked, once there is a point on screen: a check made elsewhere does not hold here. */
  point?: string;
  onCheck: (program: string, point: string) => Promise<Candidate>;
  onApply: (program: string, label: string) => Promise<void>;
}) {
  const [filter, setFilter] = useState("");
  const [draft, setDraft] = useState<Draft>(() => draftOf(rates[0]));
  const [assessed, setAssessed] = useState<{ written: string; candidate: Candidate }>();
  const [problem, setProblem] = useState<string>();
  const [applying, setApplying] = useState(false);
  const rate = rates.find((candidate) => candidate.rate.key === draft.rate) ?? rates[0];
  // Another model (another timeline's) starts a fresh draft.
  useEffect(() => { if (!rates.some((candidate) => candidate.rate.key === draft.rate)) setDraft(draftOf(rates[0])); }, [rates]);
  const listed = useMemo(() => {
    const wanted = filter.trim().toLowerCase();
    const matching = wanted ? rates.filter((candidate) => `${candidate.rate.label} ${candidate.rate.key}`.toLowerCase().includes(wanted)) : rates;
    const shown = matching.slice(0, PICKED);
    return rate && !shown.includes(rate) ? [rate, ...shown] : shown;
  }, [rates, filter, rate]);
  if (!rate) return <section className="from-section exchange-editor" aria-label="Exchange"><h3><Term name="exchange">Exchange</Term></h3><p className="muted">No rates</p></section>;
  const written = writeExchange(rate, draft, ontology);
  const exchange = "exchange" in written ? written.exchange : undefined;
  const key = exchange && point ? `${point} ${JSON.stringify(exchange)}` : undefined;
  const current = assessed && assessed.written === key ? assessed.candidate : undefined;
  const ready = current?.proposal.assessment.status === "applicable";
  const change = (next: Partial<Draft>) => { setDraft((was) => ({ ...was, ...next })); setProblem(undefined); };
  const variables = rate.variables ?? [];
  const check = async () => {
    if (!exchange || !key || !point) return;
    setProblem(undefined);
    try { setAssessed({ written: key, candidate: await onCheck(exchangeSource(exchange), point) }); } catch (cause) { setProblem(cause instanceof Error ? cause.message : String(cause)); }
  };
  return <section className="from-section exchange-editor" aria-label="Exchange">
    <h3><Term name="exchange">Exchange</Term></h3>
    {rates.length > 12 && <input type="search" name="exchange-rate-filter" aria-label="Filter rates" placeholder={`Filter ${rates.length.toLocaleString()} rates`} value={filter} onChange={(event) => setFilter(event.target.value)} />}
    <label className="field"><span>Rate</span><select name="exchange-rate" value={rate.rate.key} onChange={(event) => { const picked = rates.find((candidate) => candidate.rate.key === event.target.value); setDraft(draftOf(picked)); setProblem(undefined); }}>
      {listed.map((candidate) => <option key={candidate.rate.key} value={candidate.rate.key}>{candidate.rate.label}</option>)}
    </select></label>
    {variables.length > 0 && <>
      <div className="settings-fields">{variables.map((variable, index) => {
        const choices = variableChoices(rate, draft.values, index);
        const value = draft.values[variable.name] ?? "";
        const set = (next: string) => change({ values: settleValues(rate, { ...draft.values, [variable.name]: next }) });
        return <label className="field" key={variable.name}><span>{`{${variable.name}}`}</span>{choices
          ? <select name={`variable-${variable.name}`} value={value} onChange={(event) => set(event.target.value)}>{choices.length === 0 && <option value="">none</option>}{choices.map((choice) => <option key={choice} value={choice}>{choice}</option>)}</select>
          : <input name={`variable-${variable.name}`} inputMode="numeric" value={value} onChange={(event) => set(event.target.value.trim())} />}</label>;
      })}</div>
      <Written pattern={rate.rate.label} values={Object.entries(draft.values).map(([variable, value]) => ({ variable, value }))} />
    </>}
    <div className="settings-fields">
      {rate.roles.map((role) => {
        const fixed = fixedAccount(rate, role.role.key, ontology, draft.values);
        const opens = (rate.opens ?? []).some((opened) => opened.key === role.role.key);
        const bound = draft.bindings[role.role.key];
        return <label className="field" key={role.role.key}><span>{role.role.label}</span>{fixed !== undefined
          ? <output name={`role-${role.role.key}`}>{accounts.find((account) => account.key === fixed)?.label ?? pathUnder(ontology, "account", fixed) ?? fixed}</output>
          : opens
          ? <input name={`role-${role.role.key}`} placeholder="new account" spellCheck={false} value={bound ? pathUnder(ontology, "account", bound) ?? "" : ""} onChange={(event) => change({ bindings: { ...draft.bindings, [role.role.key]: `${ontology}/account/${event.target.value.trim()}` } })} />
          : <select name={`role-${role.role.key}`} value={draft.bindings[role.role.key] ?? ""} onChange={(event) => change({ bindings: { ...draft.bindings, [role.role.key]: event.target.value } })}>
            <option value="">—</option>
            {accounts.map((account) => <option key={account.key} value={account.key}>{account.label}</option>)}
          </select>}</label>;
      })}
      <label className="field"><span>Units</span><input name="exchange-units" inputMode="numeric" value={draft.units} onChange={(event) => change({ units: event.target.value.trim() })} /></label>
      {(rate.parameters ?? []).map((name) => <label className="field" key={name}><span>{name}</span><input name={`parameter-${name}`} inputMode="numeric" value={draft.parameters[name] ?? ""} onChange={(event) => change({ parameters: { ...draft.parameters, [name]: event.target.value } })} /></label>)}
    </div>
    <div className="let-play">
      <button type="button" onClick={() => void check()} disabled={!key}>Check</button>
      <button type="button" className="primary" disabled={!ready || applying} onClick={async () => {
        if (!current) return;
        setApplying(true);
        try { await onApply(current.program, current.proposal.label); } finally { setApplying(false); }
      }}>Apply</button>
    </div>
    {"lacks" in written && <p className="muted">Needs {written.lacks}</p>}
    {problem && <div className="assessment-issue" role="alert">{problem}</div>}
    {current && <ExchangeView form="full" exchange={current.proposal.exchange} assessment={current.proposal.assessment} />}
  </section>;
}
