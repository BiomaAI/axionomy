import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ViewDocument } from "./api";
import { PanelHeading } from "./Inspectors";
import type { DivergenceView, PackageEngine, PointId } from "./packageEngine";
import { lastPart } from "./packageTree";
import { Term } from "./Term";

/** The most steps after the split the comparison lists at once. */
const STEPS = 100;

/** The labels the views of both histories give their accounts and assets, found by the paths exchanges bind. */
function labelsOf(documents: (ViewDocument | undefined)[]) {
  const accounts = new Map<string, string>();
  const assets = new Map<string, string>();
  const snapshots = documents.flatMap((document) => document ? [document.initial, ...document.frames.map((frame) => frame.after)] : []);
  for (const snapshot of snapshots) {
    for (const account of snapshot.accounts) {
      accounts.set(account.account.key, account.account.label);
      for (const balance of account.balances) assets.set(balance.asset.key, balance.asset.label);
    }
  }
  const find = (known: Map<string, string>, kind: string, path: string) => {
    for (const [key, label] of known) if (key === path || key.endsWith(`/${kind}/${path}`)) return label;
    return path;
  };
  return { account: (path: string) => find(accounts, "account", path), asset: (path: string) => find(assets, "asset", path) };
}

/** What a step of a history did, in its view's words. */
function stepWords(document: ViewDocument | undefined, step: number, rate?: string): string {
  const frame = document?.frames[step - 1];
  return frame?.cues[0]?.label ?? frame?.exchange.rate.label ?? (rate ? lastPart(rate) : "—");
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
  const labels = useMemo(() => labelsOf([leftDocument, rightDocument]), [leftDocument, rightDocument]);
  const view: DivergenceView | undefined = compared.data;
  const split = view ? Number(view.shared.split(":")[1]) : undefined;
  return <section className="panel compare-panel" aria-label="Comparison">
    <PanelHeading kicker={<><Term name="branch">Branch</Term> set against another history</>} title={<>{leftName} <span className="versus">vs</span> {rightName}</>}
      action={<button type="button" onClick={onClose}>Close</button>} />
    {compared.isError ? <div className="assessment-issue">{(compared.error as Error).message}</div>
      : !view ? <p className="compare-note muted">Comparing…</p>
      : <div className="compare-body">
        <p className="compare-note">
          The same up to step {split}{view.steps.length === 0 ? ", and no different after it." : ", then they part."}
          {view.converged != null ? ` Equal again from step ${view.converged}.` : view.steps.length > 0 ? " Never equal again." : ""}
        </p>
        {view.objectives.length > 0 && <dl className="compare-ends">{view.objectives.map((objective, index) => <div key={objective.objective}>
          <dt><Term name="objective">{lastPart(objective.objective)}</Term> {objective.direction === "minimize" ? "↓" : "↑"}</dt>
          <dd><span>{view.left.values[index]}</span> vs <span>{view.right.values[index]}</span></dd>
        </div>)}</dl>}
        {view.steps.length > 0 && <div className="compare-scroll"><table>
          <thead><tr><th>Step</th><th>{leftName}</th><th>{rightName}</th><th>Differs after it</th></tr></thead>
          <tbody>{view.steps.map((aligned) => {
            const differs = [
              ...aligned.balances.map((difference) => `${labels.account(difference.account)} · ${labels.asset(difference.asset)}: ${difference.left} vs ${difference.right}`),
              ...aligned.accounts.map((difference) => `${labels.account(difference.account)} only on ${difference.on === "left" ? leftName : rightName}`),
            ];
            return <tr key={aligned.step} onClick={() => aligned.left && onStep(aligned.step)} className={aligned.left ? "clickable" : undefined}>
              <td><code>{aligned.step}</code></td>
              <td>{aligned.left ? stepWords(leftDocument, aligned.step, aligned.left.rate) : <span className="muted">ended</span>}</td>
              <td>{aligned.right ? stepWords(rightDocument, aligned.step, aligned.right.rate) : <span className="muted">ended</span>}</td>
              <td>{differs.length === 0 ? <span className="muted">nothing</span> : <>{differs.slice(0, 4).join("; ")}{differs.length > 4 && <span className="muted"> and {differs.length - 4} more</span>}</>}</td>
            </tr>;
          })}</tbody>
        </table></div>}
        {view.next != null && <p className="compare-note muted">The first {STEPS} steps after the split are shown.</p>}
      </div>}
  </section>;
}
