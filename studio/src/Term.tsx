import { useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

/**
 * The words Studio uses, each said once. A word is explained where it is
 * met, on a hover or a focus, and nowhere else: the screen stays the work.
 */
export const GLOSSARY = {
  package: "A file holding economies with their rules, the histories that ship with them, and the goals, strategies and experiments that use them.",
  history: "Everything that happened, step by step, from an economy's first state to a point. Play walks it.",
  step: "One exchange applied: the kernel checked it against the rules, and every account it touched changed at once.",
  point: "The economy after some step of a history. A point never changes; anything done at it starts a new history from it.",
  branch: "A history that splits off another at some step: the same past, then something else. Comparing a branch with the history it left is a counterfactual.",
  ref: "A name for a point, such as maze/micro. A ref of your own moves forward as you act at it; one a package ships is fixed.",
  fixed: "A ref the package ships. It always names what the package shipped, so acting at it makes a branch; name a ref of your own to move forward.",
  move: "An exchange the package's rules offer at this point, checked against the economy before you apply it.",
  strategy: "A way of choosing moves: a search such as A*, or a policy, or a game player. It plays from a point toward a goal.",
  goal: "What a strategy plays toward: a state the economy can reach, such as the maze solved.",
  experiment: "Several strategies played from the same point toward the same goal, with the same effort and the same luck, and measured alike.",
  arm: "One of the strategies an experiment compares, with the point it starts from.",
  trial: "One play of an arm, with its own seed. An experiment with more trials plays each arm more times.",
  objective: "What an experiment measures where each trial ends, such as energy used or time taken, and whether less or more is better.",
  seed: "The number every chance is drawn from. The same seed gives the same luck.",
  budget: "How many states a search may look at before it gives up.",
  turns: "How many turns a policy or a game may play.",
  follow: "Watch a ref live: each step it is moved over arrives as it happens, from this tab, another, or an agent.",
} as const;

export type TermName = keyof typeof GLOSSARY;

/**
 * A word of Studio's, explained on hover or focus. The explanation is drawn
 * over the page, so no panel it sits in clips it.
 */
export function Term({ name, children }: { name: TermName; children?: ReactNode }) {
  const [at, setAt] = useState<{ left: number; top: number }>();
  const id = useId();
  const word = useRef<HTMLSpanElement>(null);
  const show = () => {
    const box = word.current?.getBoundingClientRect();
    if (box) setAt({ left: Math.min(box.left, window.innerWidth - 300), top: box.bottom + 6 });
  };
  return <span ref={word} className="term" tabIndex={0} aria-describedby={id} onMouseEnter={show} onMouseLeave={() => setAt(undefined)} onFocus={show} onBlur={() => setAt(undefined)}>
    {children ?? name}
    {at && createPortal(<span id={id} role="tooltip" className="term-tip" style={{ left: Math.max(8, at.left), top: at.top }}>{GLOSSARY[name]}</span>, document.body)}
  </span>;
}
