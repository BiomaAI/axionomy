import Axionomy.Execution
import Axionomy.Conservation

/-!
# Divergence

Two traces replayed from one state reach two states, and what those hold of
each asset differs by exactly what the two traces' exchanges made of it:
what their receipts produced of it less what they consumed. No difference
arises that an exchange did not make, and steps the two traces share make
none of it: comparing two histories from the last point both pass through
accounts for every difference there is.

This holds for any transition that accounts for its receipts, stated here
over ordered execution (`Execution.run`), the trace semantics replay has,
with receipts' deltas measured as `Conservation` measures them, weighting
the one asset. That the kernel's exchanges account for their receipts is
proved in `DivergenceKernel`.
-/

namespace Axionomy.Divergence

variable {State Action Error Receipt AccountId A : Type} [DecidableEq A]

/-- An entry's quantity when its asset is `asset`, and nothing otherwise:
`asset` weighted one, as an invariant weighting it alone weights it. -/
def unit (asset : A) : A → Nat → Int :=
  Conservation.contribution [(asset, 1)]

/-- What `deltas` made of `asset`: what they produced of it less what they
consumed. -/
def made (asset : A) (deltas : List (Conservation.Delta AccountId A Nat)) : Int :=
  (deltas.map fun delta => Conservation.measure (unit asset) delta.produced).sum -
    (deltas.map fun delta => Conservation.measure (unit asset) delta.consumed).sum

/-- What a trace's receipts made of `asset`, each by its deltas. -/
def madeBy (deltas : Receipt → List (Conservation.Delta AccountId A Nat)) (asset : A)
    (receipts : List Receipt) : Int :=
  (receipts.map fun receipt => made asset (deltas receipt)).sum

/-- A transition accounts for `total`, what a state holds of each asset, by
its receipts' `deltas`: from a state `sound` holds of, it reaches only such
states, and changes what is held of every asset by exactly what the
receipt's deltas made of it. -/
structure Accounts (step : State → Action → Option (Except Error Receipt × State))
    (sound : State → Prop) (total : State → A → Int)
    (deltas : Receipt → List (Conservation.Delta AccountId A Nat)) : Prop where
  preserves : ∀ state action receipt next, sound state →
    step state action = some (.ok receipt, next) → sound next
  exact : ∀ state action receipt next, sound state →
    step state action = some (.ok receipt, next) →
    ∀ asset, total next asset = total state asset + made asset (deltas receipt)

section Traces

variable (step : State → Action → Option (Except Error Receipt × State))
variable (sound : State → Prop) (total : State → A → Int)
variable (deltas : Receipt → List (Conservation.Delta AccountId A Nat))

theorem accepted_total (accounts : Accounts step sound total deltas)
    {state final : State} {actions : List Action} {receipts : List Receipt}
    (history : Execution.Accepted step state actions receipts final) :
    sound state → ∀ asset, total final asset = total state asset + madeBy deltas asset receipts := by
  induction history with
  | nil => intro _ asset; simp [madeBy]
  | cons executed following ih =>
    intro initial asset
    rw [ih (accounts.preserves _ _ _ _ initial executed) asset,
      accounts.exact _ _ _ _ initial executed asset]
    simp only [madeBy, List.map_cons, List.sum_cons]
    omega

/-- A trace replayed from a sound state holds of each asset what the state
held, plus exactly what its exchanges made of it. -/
theorem run_total (accounts : Accounts step sound total deltas)
    (state final : State) (actions : List Action) (receipts : List Receipt)
    (initial : sound state)
    (success : Execution.run step state actions [] = some (.ok receipts, final)) (asset : A) :
    total final asset = total state asset + madeBy deltas asset receipts := by
  obtain ⟨produced, same, history⟩ :=
    Execution.success_history step actions state final [] receipts success
  simp only [List.nil_append] at same
  subst same
  exact accepted_total step sound total deltas accounts history initial asset

/-- Two traces replayed from one sound state reach states whose holdings of
each asset differ by exactly what the two traces' exchanges made of it: no
difference arises that an exchange did not make. -/
theorem difference_made (accounts : Accounts step sound total deltas)
    (state left right : State) (lefts rights : List Action)
    (leftReceipts rightReceipts : List Receipt) (initial : sound state)
    (leftRun : Execution.run step state lefts [] = some (.ok leftReceipts, left))
    (rightRun : Execution.run step state rights [] = some (.ok rightReceipts, right)) (asset : A) :
    total left asset - total right asset =
      madeBy deltas asset leftReceipts - madeBy deltas asset rightReceipts := by
  rw [run_total step sound total deltas accounts state left lefts leftReceipts initial leftRun asset,
    run_total step sound total deltas accounts state right rights rightReceipts initial rightRun asset]
  omega

/-- Two traces from one sound state whose exchanges made the same of an
asset hold the same of it. -/
theorem same_made_same_total (accounts : Accounts step sound total deltas)
    (state left right : State) (lefts rights : List Action)
    (leftReceipts rightReceipts : List Receipt) (initial : sound state)
    (leftRun : Execution.run step state lefts [] = some (.ok leftReceipts, left))
    (rightRun : Execution.run step state rights [] = some (.ok rightReceipts, right)) (asset : A)
    (same : madeBy deltas asset leftReceipts = madeBy deltas asset rightReceipts) :
    total left asset = total right asset := by
  have difference := difference_made step sound total deltas accounts state left right lefts rights
    leftReceipts rightReceipts initial leftRun rightRun asset
  omega

/-- Two traces from one sound state that share their first steps, as two
histories do up to the last point both pass through: the shared steps give
both the same receipts, and the difference between what the two hold of
each asset is exactly what the steps after them made, each side's. -/
theorem shared_steps_make_no_difference (accounts : Accounts step sound total deltas)
    (state left right : State) (shared lefts rights : List Action)
    (leftReceipts rightReceipts : List Receipt) (initial : sound state)
    (leftRun : Execution.run step state (shared ++ lefts) [] = some (.ok leftReceipts, left))
    (rightRun : Execution.run step state (shared ++ rights) [] = some (.ok rightReceipts, right)) :
    ∃ common leftAfter rightAfter,
      leftReceipts = common ++ leftAfter ∧ rightReceipts = common ++ rightAfter ∧
      ∀ asset, total left asset - total right asset =
        madeBy deltas asset leftAfter - madeBy deltas asset rightAfter := by
  rw [Execution.append] at leftRun rightRun
  cases sharedRun : Execution.run step state shared [] with
  | none => simp [sharedRun] at leftRun
  | some value =>
    rcases value with ⟨result, middle⟩
    cases result with
    | error error => simp [sharedRun] at leftRun
    | ok common =>
      simp only [sharedRun, Option.bind_eq_bind, Option.bind_some] at leftRun rightRun
      obtain ⟨leftAfter, leftSplit, leftHistory⟩ :=
        Execution.success_history step lefts middle left common leftReceipts leftRun
      obtain ⟨rightAfter, rightSplit, rightHistory⟩ :=
        Execution.success_history step rights middle right common rightReceipts rightRun
      obtain ⟨produced, same, sharedHistory⟩ :=
        Execution.success_history step shared state middle [] common sharedRun
      have reached : sound middle :=
        sharedHistory.invariant step sound accounts.preserves initial
      refine ⟨common, leftAfter, rightAfter, leftSplit, rightSplit, fun asset => ?_⟩
      rw [accepted_total step sound total deltas accounts leftHistory reached asset,
        accepted_total step sound total deltas accounts rightHistory reached asset]
      omega

end Traces

end Axionomy.Divergence
