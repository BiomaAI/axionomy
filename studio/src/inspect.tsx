import type { ReactNode } from "react";
import type { ViewDocument, ViewSnapshot } from "./api";
import { ExchangeView } from "./ExchangeView";
import type { Subject } from "./Inspectors";
import { AccountView, AssetView } from "./Ledger";
import { ExperimentView, ObjectiveView, RefView, StrategyView } from "./PackageViews";
import type { PackageView, PointId, ResultsView } from "./packageEngine";
import { RateView } from "./RateView";

/** What the page has on screen: the history, the cursor on it, and the package it is of. */
export type World = {
  document?: ViewDocument;
  /** The step at the cursor. */
  position: number;
  snapshot?: ViewSnapshot;
  /** The state before the step at the cursor; none at the start. */
  previous?: ViewSnapshot;
  /** `${account}|${asset}` pairs that change somewhere in the history. */
  changing: ReadonlySet<string> | null;
  pkg?: PackageView;
  results: ResultsView[];
  onOpenRef: (name: string) => void;
  onWatch: (trial: { end: PointId; start: PointId }) => void;
};

/**
 * Each subject as it is at the cursor, drawn by the view for its kind. An
 * account is its ledger there, against the step before; an asset, who
 * holds it; a rate, as the model writes it; a step, its exchange; a ref, a
 * strategy, an experiment or an objective, as the package defines it.
 */
export function inspector(world: World): (subject: Subject) => ReactNode {
  const { document, position, snapshot, previous, changing, pkg } = world;
  const before = position > 0 ? previous : undefined;
  return (subject) => {
    switch (subject.kind) {
      case "account": {
        const key = subject.id.key;
        const assets = changing && new Set([...changing].flatMap((pair) => pair.startsWith(`${key}|`) ? [pair.slice(key.length + 1)] : []));
        return <AccountView account={snapshot?.accounts.find((account) => account.account.key === key)} before={before?.accounts.find((account) => account.account.key === key)} changing={assets} />;
      }
      case "asset": {
        const key = subject.id.key;
        const holding = (state: ViewSnapshot | undefined) => new Map((state?.accounts ?? []).flatMap((account) => account.balances.filter((balance) => balance.asset.key === key).map((balance) => [account.account.key, { account: account.account, quantity: balance.quantity }] as const)));
        const now = holding(snapshot);
        const was = holding(before);
        const keys = [...new Set([...now.keys(), ...was.keys()])];
        return <AssetView asset={subject.id} holders={keys.map((account) => ({ account: (now.get(account) ?? was.get(account))!.account, now: now.get(account)?.quantity, before: before ? was.get(account)?.quantity ?? "0" : undefined }))} />;
      }
      case "rate": {
        const rate = document?.model?.rates.find((candidate) => candidate.rate.key === subject.id.key);
        return rate ? <RateView rate={rate} /> : <p className="muted">Not a rate of this model</p>;
      }
      case "step": {
        const frame = subject.step > 0 ? document?.frames[subject.step - 1] : undefined;
        return frame ? <ExchangeView form="full" exchange={frame.exchange} assessment={frame.assessment} deltas={frame.receipt.deltas} recorded /> : <p className="muted">Not a step of this history</p>;
      }
      case "ref":
        return <RefView ref={pkg?.refs.find((ref) => ref.name === subject.path)} onOpen={() => world.onOpenRef(subject.path)} />;
      case "strategy":
        return <StrategyView arms={(pkg?.experiments ?? []).flatMap((experiment) => experiment.arms.filter((arm) => arm.strategy === subject.path).map((arm) => ({ experiment, start: arm.start ?? experiment.start })))} />;
      case "experiment": {
        const experiment = pkg?.experiments.find((candidate) => candidate.experiment === subject.path);
        return experiment ? <ExperimentView experiment={experiment} results={world.results.find((result) => result.experiment === subject.path)} onWatch={world.onWatch} /> : <p className="muted">Not an experiment of this package</p>;
      }
      case "objective":
        return <ObjectiveView objective={pkg?.objectives.find((objective) => objective.objective === subject.path)} measuredBy={(pkg?.experiments ?? []).filter((experiment) => experiment.objectives.some((objective) => objective.objective === subject.path)).map((experiment) => experiment.experiment)} />;
    }
  };
}
