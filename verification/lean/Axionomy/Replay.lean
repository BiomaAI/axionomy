import Std

/-! Trace composition for any deterministic transition function. The step
invariant is an explicit premise, to be discharged by the exchange proof.
This pure result models `replayed`; Rust's mutating `replay` keeps its accepted
prefix on failure and must not be claimed to provide whole-trace rollback. -/
namespace Axionomy.Replay

def run {State Action : Type} (step : State → Action → Option State) :
    State → List Action → Option State
  | state, [] => some state
  | state, action :: actions => (step state action).bind (fun next => run step next actions)

theorem append {State Action : Type} (step : State → Action → Option State)
    (state : State) (left right : List Action) :
    run step state (left ++ right) =
      (run step state left).bind (fun middle => run step middle right) := by
  induction left generalizing state with
  | nil => rfl
  | cons action actions ih =>
    simp only [List.cons_append, run]
    cases h : step state action <;> simp [ih]

theorem invariant_preserved {State Action : Type} (step : State → Action → Option State)
    (invariant : State → Prop)
    (step_preserves : ∀ state action next, invariant state → step state action = some next →
      invariant next)
    (state final : State) (actions : List Action) (initial : invariant state)
    (success : run step state actions = some final) : invariant final := by
  induction actions generalizing state with
  | nil => simp only [run, Option.some.injEq] at success; exact success ▸ initial
  | cons action actions ih =>
    simp only [run, Option.bind_eq_some_iff] at success
    obtain ⟨next, hs, hr⟩ := success
    exact ih next (step_preserves state action next initial hs) hr

theorem deterministic {State Action : Type} (step : State → Action → Option State)
    (state left right : State) (actions : List Action)
    (hl : run step state actions = some left) (hr : run step state actions = some right) :
    left = right := by
  rw [hl] at hr
  exact Option.some.inj hr

end Axionomy.Replay
