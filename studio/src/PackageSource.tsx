import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { PackageEngine, PackageView } from "./packageEngine";

export function PackageSource({ engine, pkg }: { engine: PackageEngine; pkg: PackageView }) {
  const source = useQuery({
    queryKey: ["package-source", engine.kind, pkg.name, pkg.version],
    queryFn: () => engine.sourceTree(pkg.name),
  });
  const [file, setFile] = useState("package.rhai");
  const [experiment, setExperiment] = useState(pkg.experiments[0]?.experiment ?? "");
  const comparison = useMutation({
    mutationFn: (other: File) => engine.compareBuild(pkg.name, experiment, other),
  });
  return <section className="panel package-source" aria-label="Package source">
    <h2>{pkg.name} <small>{pkg.version}</small></h2>
    {source.error && <p role="alert">{source.error.message}</p>}
    {source.isPending && <p role="status">Reading source…</p>}
    {source.data && <>
      <label className="field"><span>Source file</span><select value={file} onChange={e => setFile(e.target.value)}>
        {Object.keys(source.data.files).sort().map(name => <option key={name}>{name}</option>)}
      </select></label>
      <pre tabIndex={0}>{source.data.files[file]}</pre>
    </>}
    {pkg.experiments.length > 0 && <div className="source-comparison">
      <label className="field"><span>Experiment</span><select aria-label="Experiment" value={experiment} disabled={comparison.isPending}
        onChange={e => { setExperiment(e.target.value); comparison.reset(); }}>
        {pkg.experiments.map(e => <option key={e.experiment}>{e.experiment}</option>)}
      </select></label>
      <label className="file-button">Compare build<input type="file" accept=".redb,.zip" disabled={comparison.isPending}
        onChange={e => { const other = e.target.files?.[0]; e.target.value = ""; if (other) comparison.mutate(other); }} /></label>
      {comparison.isPending && <p role="status">Running both builds…</p>}
      {comparison.error && <p role="alert">{comparison.error.message}</p>}
      {comparison.data && <div className="source-results" role="status">
        <h3>{comparison.data.equal ? "Matching trials" : "Trials differ"}</h3>
        {comparison.data.trials.map((trial, i) => <details key={i}>
          <summary>{trial.strategy} · seed {trial.seed} · {trial.first_difference ? `first difference at step ${trial.first_difference.step}` : "matching exchanges and balances"}</summary>
          <pre>{JSON.stringify({ installed: trial.left, uploaded: trial.right, first_difference: trial.first_difference }, null, 2)}</pre>
        </details>)}
      </div>}
    </div>}
  </section>;
}
