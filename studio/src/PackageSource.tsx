import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { PackageEngine, PackageView, SourceTree } from "./packageEngine";
import { RhaiEditor } from "./RhaiEditor";

export function PackageSource({ engine, pkg }: { engine: PackageEngine; pkg: PackageView }) {
  const source = useQuery({
    queryKey: ["package-source", engine.kind, pkg.name, pkg.version],
    queryFn: () => engine.sourceTree(pkg.name),
  });
  const [file, setFile] = useState("package.rhai");
  const [draft, setDraft] = useState<SourceTree["files"]>();
  const [newFile, setNewFile] = useState("");
  const files = draft ?? source.data?.files;
  const build = useMutation({ mutationFn: (files: SourceTree["files"]) => engine.buildSource({ files }) });
  function edit(files: SourceTree["files"]) {
    build.reset();
    setDraft(files);
  }
  const [experiment, setExperiment] = useState(pkg.experiments[0]?.experiment ?? "");
  const comparison = useMutation({
    mutationFn: (other: File) => engine.compareBuild(pkg.name, experiment, other),
  });
  return <section className="panel package-source" aria-label="Package source">
    <h2>{pkg.name} <small>{pkg.version}</small></h2>
    {source.error && <p role="alert">{source.error.message}</p>}
    {source.isPending && <p role="status">Reading source…</p>}
    {files && <>
      <label className="field"><span>Source file</span><select value={file} onChange={e => setFile(e.target.value)}>
        {Object.keys(files).sort().map(name => <option key={name}>{name}</option>)}
      </select></label>
      <RhaiEditor key={file} label="Rhai source" value={files[file] ?? ""} formatOnOpen={files[file] === source.data?.files[file]} disabled={build.isPending}
        onChange={text => edit({ ...files, [file]: text })} />
      <details><summary>Manage source files</summary>
        <label className="field"><span>New source path</span><input value={newFile} placeholder="lib/example/helpers.rhai"
          onChange={e => setNewFile(e.target.value)} /></label>
        <button disabled={build.isPending || !newFile.endsWith(".rhai") || Object.hasOwn(files, newFile)}
          onClick={() => { edit({ ...files, [newFile]: "" }); setFile(newFile); setNewFile(""); }}>Add file</button>
        <button disabled={build.isPending || file === "package.rhai"} onClick={() => {
          const remaining = { ...files }; delete remaining[file]; edit(remaining); setFile("package.rhai");
        }}>Remove selected file</button>
      </details>
      <div className="actions">
        <button className="primary" disabled={build.isPending} onClick={() => build.mutate(files)}>Build package</button>
        {draft && <button disabled={build.isPending} onClick={() => { setDraft(undefined); setFile("package.rhai"); build.reset(); }}>Discard edits</button>}
      </div>
      {build.isPending && <p role="status">Compiling and running package tests…</p>}
      {build.error && <p role="alert">{build.error.message}</p>}
      {build.data && <p role="status">{build.data.name} {build.data.version} built · {build.data.tests.length} package tests passed.{" "}
        <a download={build.data.name.replaceAll("/", "-") + ".zip"} href={"data:application/zip;base64," + build.data.archive}>Download package</a>
      </p>}
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
