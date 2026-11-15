import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ViewDocument, ViewId } from "./api";
import { ExchangeView } from "./ExchangeView";
import { IdLink, PanelHeading } from "./Inspectors";
import type { DivergenceView, PackageEngine, PointId } from "./packageEngine";
import { lastPart } from "./packageTree";

/** The most steps after the split the comparison lists at once. */
const STEPS = 100;

/**
 * The ids the views of both histories give their accounts and assets, by
 * the paths a divergence names them by: a view keys a value under its
 * ontology and category, `<ontology>/account/<path>`.
 */
function idsOf(documents: (ViewDocument | undefined)[]) {
  const known = new Map<string, ViewId>();
  const snapshots = documents.flatMap((document) => document ? [document.initial, ...document.frames.map((frame) => frame.after)] : []);
  for (const snapshot of snapshots) {
    for (const account of snapshot.accounts) {
      known.set(account.account.key, account.account);
      for (const balance of account.balances) known.set(balance.asset.key, balance.asset);
    }
  }
  const ontology = documents.find(Boolean)?.source.key;
  const id = (category: string, path: string): ViewId => {
    const key = `${ontology}/${category}/${path}`;
    return known.get(key) ?? { key, label: path };
  };
  return { account: (path: string) => id("account", path), asset: (path: string) => id("asset", path) };
}

/** What a step of a history did: its exchange, as the view of that history shows it. */
function StepDone({ document, step, rate }: { document: ViewDocument | undefined; step: number; rate: string }) {
  const frame = document?.frames[step - 1];
  return frame ? <ExchangeView exchange={frame.exchange} /> : <code>{rate}</code>;
}

/**
 * The history on screen set against another: where they split, then step by
 * step what each did and what differs after it, whether they are ever equal
 * again, and how each ends on the package's objectives. A branch set against
 * the history it left is a counterfactual.
 */
export function Compare({ engine, left, right, leftName, rightName, leftDocument, objectives, onStep, onClose }: {
  engine: PackageEngine;
  left: PointId;
  right: PointId;
  leftName: string;
  rightName: string;
  leftDocument?: ViewDocument;
  objectives: string[];
  onStep: (step: number) => void;
  onClose: () => void;
}) {
  const asked = objectives.join(",");
  const compared = useQuery({
    queryKey: ["divergence", engine.kind, left, right, asked],
    queryFn: () => engine.divergence({ left, right, limit: STEPS, objectives: asked || undefined }),
    staleTime: Infinity,
  });
  const rightDocument = useQuery({ queryKey: ["draw", engine.kind, right], queryFn: () => engine.draw(right), staleTime: Infinity }).data;
  const ids = useMemo(() => idsOf([leftDocument, rightDocument]), [leftDocument, rightDocument]);
  const view: DivergenceView | undefined = compared.data;
  const split = view ? Number(view.shared.split(":")[1]) : undefined;
  return <section className="panel compare-panel" aria-label="Comparison">
    <PanelHeading kicker="Compare" title={<>{leftName} <span className="versus">vs</span> {rightName}</>}
      action={<button type="button" onClick={onClose}>Close</button>} />
    {compared.isError ? <div className="assessment-issue">{(compared.error as Error).message}</div>
      : !view ? <p className="compare-note muted">Comparing…</p>
      : <div className="compare-body">
        <p className="compare-note">
          Split at step {split}{view.steps.length === 0 ? " · no difference" : view.converged != null ? ` · equal again at step ${view.converged}` : ""}
        </p>
        {view.objectives.length > 0 && <dl className="compare-ends">{view.objectives.map((objective, index) => <div key={objective.objective}>
          <dt><IdLink subject={{ kind: "objective", path: objective.objective }} label={lastPart(objective.objective)} /> {objective.direction === "minimize" ? "↓" : "↑"}</dt>
          <dd><span>{view.left.values[index]}</span> vs <span>{view.right.values[index]}</span></dd>
        </div>)}</dl>}
        {view.steps.length > 0 && <div className="compare-scroll"><table>
          <thead><tr><th>Step</th><th>{leftName}</th><th>{rightName}</th><th>Differs after it</th></tr></thead>
          <tbody>{view.steps.map((aligned) => {
            const differs = [
              ...aligned.balances.map((difference) => <span key={`${difference.account}|${difference.asset}`}><IdLink subject={{ kind: "account", id: ids.account(difference.account) }} /> · <IdLink subject={{ kind: "asset", id: ids.asset(difference.asset) }} /> {difference.left} vs {difference.right}</span>),
              ...aligned.accounts.map((difference) => <span key={difference.account}><IdLink subject={{ kind: "account", id: ids.account(difference.account) }} /> only on {difference.on === "left" ? leftName : rightName}</span>),
            ];
            return <tr key={aligned.step} onClick={() => aligned.left && onStep(aligned.step)} className={aligned.left ? "clickable" : undefined}>
              <td>{aligned.left ? <IdLink subject={{ kind: "step", step: aligned.step }} label={String(aligned.step)} /> : <code>{aligned.step}</code>}</td>
              <td>{aligned.left ? <StepDone document={leftDocument} step={aligned.step} rate={aligned.left.rate} /> : <span className="muted">ended</span>}</td>
              <td>{aligned.right ? <StepDone document={rightDocument} step={aligned.step} rate={aligned.right.rate} /> : <span className="muted">ended</span>}</td>
              <td>{differs.length === 0 ? <span className="muted">nothing</span> : <div className="differs">{differs.slice(0, 4)}{differs.length > 4 && <span className="muted">and {differs.length - 4} more</span>}</div>}</td>
            </tr>;
          })}</tbody>
        </table></div>}
        {view.next != null && <p className="compare-note muted">First {STEPS} steps</p>}
      </div>}
  </section>;
}
