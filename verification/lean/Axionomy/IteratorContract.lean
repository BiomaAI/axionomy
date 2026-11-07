import Axionomy.CommitRefinement

namespace Axionomy.IteratorContract
open Aeneas Aeneas.Std

/-- The primitive `next` calls up to the first `None`. No fused-iterator law is
required: the caller does not inspect the state after that first `None`. -/
inductive Yields {I T : Type} (next : I → Result (Option T × I)) : I → List T → Prop
  | done {state after} (called : next state = .ok (none, after)) : Yields next state []
  | step {state after value rest} (called : next state = .ok (some value, after))
      (tail : Yields next after rest) : Yields next state (value :: rest)

/-- A finite successful prefix followed by an iterator panic. This makes no
claim about state observable after Rust unwinding. -/
inductive FailsAfter {I T : Type} (next : I → Result (Option T × I))
    (error : Error) : I → List T → Prop
  | stopped {state} (called : next state = .fail error) : FailsAfter next error state []
  | step {state after value rest} (called : next state = .ok (some value, after))
      (tail : FailsAfter next error after rest) : FailsAfter next error state (value :: rest)

/-- A finite observed prefix, with no assumption about what the iterator does
after it. This permits early rejection before a panic or infinite unused tail. -/
inductive Prefix {I T : Type} (next : I → Result (Option T × I)) : I → List T → I → Prop
  | nil (state : I) : Prefix next state [] state
  | step {state after value rest finalState} (called : next state = .ok (some value, after))
      (tail : Prefix next after rest finalState) : Prefix next state (value :: rest) finalState

theorem yields_unique {I T : Type} {next : I → Result (Option T × I)}
    {state : I} {left right : List T} (first : Yields next state left)
    (second : Yields next state right) : left = right := by
  induction first generalizing right with
  | done called =>
    cases second with
    | done => rfl
    | step other => simp [called] at other
  | step called tail ih =>
    cases second with
    | done other => simp [called] at other
    | step other following =>
      have same := called.symm.trans other
      simp only [Result.ok.injEq, Prod.mk.injEq, Option.some.injEq] at same
      rcases same with ⟨rfl, rfl⟩
      exact congrArg (List.cons _) (ih following)

/-- A concrete, inhabited witness for the protocol, derived from Aeneas' Vec
iterator semantics rather than assumed as a consumer postcondition. -/
theorem vector_yields {T : Type} (iter : alloc.vec.into_iter.IntoIter T) :
    Yields alloc.vec.into_iter.IteratorIntoIter.next iter iter.val := by
  generalize contents : iter.val = values
  induction values generalizing iter with
  | nil => exact .done (CommitRefinement.owning_next_none iter contents)
  | cons value rest ih =>
    obtain ⟨after, called, remaining⟩ := CommitRefinement.owning_next_cons iter value rest contents
    exact .step called (ih after remaining)

/-- A testable non-fused iterator: a `Some` may appear after a `None`. -/
def scriptedNext {T : Type} : List (Option T) → Result (Option T × List (Option T))
  | [] => .ok (none, [])
  | value :: rest => .ok (value, rest)

def takeUntilNone {T : Type} : List (Option T) → List T
  | [] => []
  | none :: _ => []
  | some value :: rest => value :: takeUntilNone rest

theorem scripted_yields {T : Type} (script : List (Option T)) :
    Yields scriptedNext script (takeUntilNone script) := by
  induction script with
  | nil => exact .done rfl
  | cons value rest ih =>
    cases value with
    | none => exact .done rfl
    | some value => exact .step rfl ih

end Axionomy.IteratorContract
