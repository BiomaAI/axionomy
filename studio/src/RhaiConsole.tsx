import { lazy, Suspense, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import type { Candidate, PackageEngine, PointId } from "./packageEngine";

const RhaiEditor = lazy(() => import("./RhaiEditor"));

/** Read or propose against the point currently being inspected. Applying
 * submits the concrete proposal that the engine has already assessed. */
export function RhaiConsole({ engine, at, onApply }: {
  engine: PackageEngine;
  at: PointId;
  onApply: (program: string, label: string) => Promise<void>;
}) {
  const [opened, setOpened] = useState(false);
  const [source, setSource] = useState('state.accounts("")');
  const run = useMutation({
    mutationFn: async ({ mode, source }: { mode: "query" | "assess"; source: string }):
      Promise<{ value: unknown; proposal?: Candidate; source: string }> => {
      if (mode === "query") return { value: await engine.query(at, source), source };
      const proposal = await engine.assess(at, source);
      return { value: proposal.proposal, proposal, source };
    },
  });
  const apply = useMutation({ mutationFn: (proposal: Candidate) => onApply(proposal.program, proposal.proposal.label) });
  const current = run.data?.source === source ? run.data : undefined;
  return <details className="from-section" onToggle={e => { if (e.currentTarget.open) setOpened(true); }}>
    <summary>Rhai console</summary>
    {opened && <Suspense fallback={<p role="status">Opening editor…</p>}>
      <RhaiEditor compact label="Program" value={source}
        onChange={text => { setSource(text); run.reset(); apply.reset(); }} />
    </Suspense>}
    <div className="actions">
      <button disabled={!source.trim() || run.isPending} onClick={() => run.mutate({ mode: "query", source })}>Run query</button>
      <button disabled={!source.trim() || run.isPending} onClick={() => run.mutate({ mode: "assess", source })}>Assess exchange</button>
      {current?.proposal && <button className="primary"
        disabled={apply.isPending || current.proposal.proposal.assessment.status !== "applicable"}
        onClick={() => apply.mutate(current.proposal!)}>Apply assessed exchange</button>}
    </div>
    {(run.error || apply.error) && <p role="alert">{(run.error || apply.error)?.message}</p>}
    {current && <pre tabIndex={0}>{JSON.stringify(current.value, null, 2)}</pre>}
  </details>;
}
