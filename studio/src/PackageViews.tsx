import type { ReactNode } from "react";
import { IdLink } from "./Inspectors";
import type { ExperimentView as ExperimentData, ObjectiveView as ObjectiveData, PointId, RefView as RefData, ResultsView as ResultsData } from "./packageEngine";
import { Term } from "./Term";

type Trial = ResultsData["arms"][number]["trials"][number];

/** The step a point is at: `run-…:3` is step 3. */
function stepOf(point: PointId): number {
  return Number(point.split(":")[1]);
}

/** How a trial ended, in a few words. */
export function endedWords(ended: Trial["ended"]): string {
  switch (ended.kind) {
    case "reached": return "reached the goal";
    case "stopped": return "stopped short of the goal";
    case "horizon": return "played every turn";
    case "refused": return `refused on turn ${ended.turn}`;
    case "exhausted": return "explored everything";
    case "spent": return "spent its budget";
    case "interrupted": return "stopped before it ended";
  }
}

const ARROW = { minimize: "↓", maximize: "↑" } as const;

/** A ref: the step it names, whether a package ships it, and, where it can be, how to open it. */
export function RefView({ ref, onOpen }: { ref?: RefData; onOpen?: () => void }) {
  if (!ref) return <p className="muted">Not a ref of this package</p>;
  return <div className="package-view">
    <dl className="facts">
      <dt>step</dt><dd>{stepOf(ref.point)}</dd>
      <dt>point</dt><dd><code>{ref.point}</code></dd>
      <dt>{ref.shipped_by ? <Term name="fixed" /> : "yours"}</dt><dd>{ref.shipped_by ?? ""}</dd>
    </dl>
    {onOpen && <button type="button" onClick={onOpen}>Open</button>}
  </div>;
}

/** A strategy and the experiments it plays in, each with its start, goal and effort. */
export function StrategyView({ arms }: { arms: { experiment: ExperimentData; start: string }[] }) {
  if (arms.length === 0) return <p className="muted">No experiment runs it</p>;
  return <div className="package-view">
    {arms.map(({ experiment, start }) => <dl className="facts" key={`${experiment.experiment}:${start}`}>
      <dt><Term name="experiment" /></dt><dd><IdLink subject={{ kind: "experiment", path: experiment.experiment }} /></dd>
      <dt><Term name="ref">start</Term></dt><dd><IdLink subject={{ kind: "ref", path: start }} /></dd>
      <dt><Term name="goal" /></dt><dd>{experiment.goal}</dd>
      <dt>effort</dt><dd><Effort experiment={experiment} /></dd>
    </dl>)}
  </div>;
}

function Effort({ experiment }: { experiment: ExperimentData }) {
  return <><span><Term name="seed" /> {experiment.seed}</span><span><Term name="budget" /> {experiment.budget}</span><span><Term name="turns" /> {experiment.turns}</span></>;
}

/** An objective: the direction that improves it, and the experiments that measure it. */
export function ObjectiveView({ objective, measuredBy }: { objective?: ObjectiveData; measuredBy: string[] }) {
  if (!objective) return <p className="muted">Not an objective of this package</p>;
  return <dl className="facts">
    <dt>better</dt><dd>{objective.direction === "minimize" ? "less ↓" : "more ↑"}</dd>
    {measuredBy.length > 0 && <><dt><Term name="experiment">experiments</Term></dt><dd>{measuredBy.map((path) => <IdLink key={path} subject={{ kind: "experiment", path }} />)}</dd></>}
  </dl>;
}

/**
 * An experiment: what it compares from where, toward what goal, with what
 * effort, and what its trials came to. `actions` runs or stops it where it
 * can be run.
 */
export function ExperimentView({ experiment, results, onWatch, actions }: {
  experiment: ExperimentData;
  results?: ResultsData;
  onWatch?: (trial: { end: PointId; start: PointId }) => void;
  actions?: ReactNode;
}) {
  return <div className="experiment-view">
    <div className="experiment-line">
      <IdLink subject={{ kind: "experiment", path: experiment.experiment }} />
      {actions}
    </div>
    <dl className="facts">
      <dt><Term name="ref">start</Term></dt><dd><IdLink subject={{ kind: "ref", path: experiment.start }} /></dd>
      <dt><Term name="goal" /></dt><dd>{experiment.goal}</dd>
      <dt><Term name="arm">arms</Term></dt><dd>{experiment.arms.map((arm) => <span key={`${arm.strategy}:${arm.start ?? ""}`}><IdLink subject={{ kind: "strategy", path: arm.strategy }} />{arm.start && <> from <IdLink subject={{ kind: "ref", path: arm.start }} /></>}</span>)}</dd>
      <dt><Term name="trial">trials</Term></dt><dd>{experiment.trials}</dd>
      <dt>effort</dt><dd><Effort experiment={experiment} /></dd>
      {experiment.objectives.length > 0 && <><dt><Term name="objective">objectives</Term></dt><dd>{experiment.objectives.map((objective) => <span key={objective.objective}><IdLink subject={{ kind: "objective", path: objective.objective }} /> {ARROW[objective.direction]}</span>)}</dd></>}
    </dl>
    {results && <ResultsTable results={results} onWatch={onWatch} />}
  </div>;
}

/** What an experiment's trials came to: one row per arm and trial, a column per objective. */
export function ResultsTable({ results, onWatch }: { results: ResultsData; onWatch?: (trial: { end: PointId; start: PointId }) => void }) {
  return <div className="table-scroll"><table className="compact-table results-table">
    <thead><tr>
      <th><Term name="arm">Arm</Term></th>
      <th><Term name="trial">Trial</Term></th>
      <th>Ended</th>
      <th className="number">Steps</th>
      {results.objectives.map((objective) => <th key={objective.objective} className="number"><IdLink subject={{ kind: "objective", path: objective.objective }} label={objective.objective.split("/").at(-1)} /> {ARROW[objective.direction]}</th>)}
      {onWatch && <th />}
    </tr></thead>
    <tbody>{results.arms.flatMap((arm) => arm.trials.map((trial, index) => <tr key={`${arm.arm.strategy}:${arm.arm.start ?? ""}:${index}`}>
      <td><IdLink subject={{ kind: "strategy", path: arm.arm.strategy }} label={arm.arm.strategy.split("/").at(-1)} /></td>
      <td className="number">{index + 1}</td>
      <td>{endedWords(trial.ended)}</td>
      <td className="number">{stepOf(trial.end) - stepOf(arm.start)}</td>
      {trial.values.map((value, column) => <td key={column} className="number">{value}</td>)}
      {onWatch && <td><button type="button" disabled={trial.end === arm.start} onClick={() => onWatch({ end: trial.end, start: arm.start })}>Watch</button></td>}
    </tr>))}</tbody>
  </table></div>;
}
