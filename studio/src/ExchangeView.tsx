import { Fragment, useContext } from "react";
import type { AssessmentData, DeltaData, ExchangeData, QuantityData } from "./api";
import { IdLink } from "./Inspectors";
import { Amount, QuantityFormats, useQuantity } from "./Quantity";
import { Term } from "./Term";

/**
 * A rate written with the values an exchange gives its variables:
 * `step/{from}/{to}` with from 1 and to 2 reads `step/1/2`, each value
 * marked with the variable it fills.
 */
export function Written({ pattern, values }: { pattern: string; values: { variable: string; value: string }[] }) {
  const given = new Map(values.map((argument) => [argument.variable, argument.value]));
  return <code className="written">{pattern.split(/(\{[^}]+\})/).map((piece, index) => {
    const variable = /^\{([^}]+)\}$/.exec(piece)?.[1];
    if (variable === undefined) return <Fragment key={index}>{piece}</Fragment>;
    const value = given.get(variable);
    return value === undefined ? <i key={index}>{piece}</i> : <b key={index} title={variable}>{value}</b>;
  })}</code>;
}

/** An asset and how much of it: a bare count reads as a multiple, a quantity with a unit says what it counts. */
export function Holding({ value }: { value: QuantityData }) {
  const format = useContext(QuantityFormats)[value.asset.key];
  return <span className="holding"><IdLink subject={{ kind: "asset", id: value.asset }} /> <em>{format?.unit ? "" : "×"}<Amount asset={value.asset.key} atoms={value.quantity} /></em></span>;
}

const STATUS: Record<AssessmentData["status"], string> = { applicable: "applies", infeasible: "infeasible", invalid: "invalid" };

/**
 * An exchange: its rate, as written with the values it gives, the accounts
 * its roles bind, its units and parameters. Compact, a line or two, with
 * whether it applies; full, also what each account gives, keeps and gets,
 * and its assessment. `deltas` are what an applied exchange did; without
 * them, what the assessment projects.
 */
export function ExchangeView({ exchange, assessment, deltas, form = "compact", recorded = false }: {
  exchange: ExchangeData;
  assessment?: AssessmentData;
  deltas?: DeltaData[];
  form?: "compact" | "full";
  /** A step taken: checked before it applied, so whether it applies goes without saying. */
  recorded?: boolean;
}) {
  const law = exchange.law ?? undefined;
  return <div className={`exchange exchange-${form}`}>
    <div className="exchange-head">
      <IdLink subject={{ kind: "rate", id: law ?? exchange.rate }} label={exchange.rate.label} className="exchange-rate" />
      {exchange.units !== "1" && <span className="exchange-units">×{exchange.units}</span>}
      {assessment && !recorded && <span className={`exchange-status status-${assessment.status}`}>{STATUS[assessment.status]}</span>}
    </div>
    {law && <Written pattern={law.label} values={exchange.arguments ?? []} />}
    <div className="exchange-bindings">
      {exchange.bindings.map((binding) => <span key={binding.role.key}><small>{binding.role.label}</small> <IdLink subject={{ kind: "account", id: binding.account }} /></span>)}
      {(exchange.parameters ?? []).map((parameter) => <span key={parameter.name}><small>{parameter.name}</small> <code>{parameter.value}</code></span>)}
    </div>
    {form === "full" && <>
      <Effects deltas={deltas ?? effectsOf(assessment)} />
      {assessment && <Assessment assessment={assessment} />}
    </>}
  </div>;
}

/** What an assessment says each account would give, keep and get: projected where it applies, else as checked. */
function effectsOf(assessment: AssessmentData | undefined): DeltaData[] {
  if (!assessment) return [];
  return assessment.projected_deltas.length > 0 ? assessment.projected_deltas : assessment.accounts;
}

/** What each account gave, kept and got. */
export function Effects({ deltas }: { deltas: DeltaData[] }) {
  if (deltas.length === 0) return null;
  return <div className="effects">
    {deltas.map((delta) => <div className="effect" key={delta.account.key}>
      <IdLink subject={{ kind: "account", id: delta.account }} />
      <EffectLine word="gives" values={delta.consumed} />
      <EffectLine word="keeps" values={delta.preserved} />
      <EffectLine word="gets" values={delta.produced} />
    </div>)}
  </div>;
}

function EffectLine({ word, values }: { word: "gives" | "keeps" | "gets"; values: QuantityData[] }) {
  if (values.length === 0) return null;
  return <div className={`effect-line effect-${word}`}><Term name={word} /><div>{values.map((value) => <Holding key={value.asset.key} value={value} />)}</div></div>;
}

/** What the kernel found when it checked an exchange: whether it applies, what an account lacks, what is wrong with it. */
export function Assessment({ assessment }: { assessment: AssessmentData }) {
  const quantity = useQuantity();
  return <div className={`assessment assessment-${assessment.status}`}>
    <strong>{assessment.status}</strong>
    {assessment.shortfalls.map((shortfall) => <div key={shortfall.account.key}>
      <IdLink subject={{ kind: "account", id: shortfall.account }} /> lacks {shortfall.missing.map((missing, index) => <Fragment key={missing.asset.key}>{index > 0 && ", "}{quantity(missing.asset.key, missing.quantity)} <IdLink subject={{ kind: "asset", id: missing.asset }} /></Fragment>)}
    </div>)}
    {assessment.issues.map((issue, index) => <div className="assessment-issue" key={`${issue.kind}:${index}`}><span>{issue.message}</span><small>{issue.kind.replaceAll("_", " ")}</small></div>)}
  </div>;
}
