import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import type { Candidate, PackageEngine, PointId } from "./packageEngine";
import { RhaiEditor } from "./RhaiEditor";

type Run = { mode: "query" | "assess"; source: string; at: PointId };

/**
 * Reads or proposes against the point on screen, and waits while there is
 * none. Applying submits the concrete proposal the engine has already
 * assessed. The program stays from point to point; what it gave is of the
 * point it was run at.
 */
export function RhaiConsole({ engine, at, onApply }: {
  engine: PackageEngine;
  at?: PointId;
  onApply: (program: string, label: string) => Promise<void>;
}) {
  const [source, setSource] = useState('state.accounts("")');
  const run = useMutation({
    mutationFn: async ({ mode, source, at }: Run): Promise<{ value: unknown; proposal?: Candidate }> => {
      if (mode === "query") return { value: await engine.query(at, source) };
      const proposal = await engine.assess(at, source);
      return { value: proposal.proposal, proposal };
    },
  });
  const apply = useMutation({ mutationFn: (proposal: Candidate) => onApply(proposal.program, proposal.proposal.label) });
  const here = run.variables !== undefined && run.variables.at === at && run.variables.source === source;
  const current = here ? run.data : undefined;
  const problem = here ? run.error ?? apply.error : undefined;
  const start = (mode: Run["mode"]) => { if (at) { apply.reset(); run.mutate({ mode, source, at }); } };
  return <section className="rhai-console" aria-label="Rhai console">
    <RhaiEditor compact label="Program" value={source} onChange={setSource} />
    <div className="actions">
      <button disabled={!at || !source.trim() || run.isPending} onClick={() => start("query")}>Run query</button>
      <button disabled={!at || !source.trim() || run.isPending} onClick={() => start("assess")}>Assess exchange</button>
      {current?.proposal && <button className="primary"
        disabled={apply.isPending || current.proposal.proposal.assessment.status !== "applicable"}
        onClick={() => apply.mutate(current.proposal!)}>Apply assessed exchange</button>}
    </div>
    {problem && <p role="alert">{problem.message}</p>}
    {current && <pre tabIndex={0}>{JSON.stringify(current.value, null, 2)}</pre>}
  </section>;
}
