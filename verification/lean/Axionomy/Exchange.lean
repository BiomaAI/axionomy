import Axionomy.Quantity

/-!
Prepared exchanges. Each Cell represents one distinct account/asset entry
after role binding, static scaling and computed-effect evaluation and
aggregation.

Nat models exact nonnegative quantities and `limit` bounds the resulting
balance (`Quantity.limit`, 2^256 - 1, for the kernel). Conservation is
measured in mathematical Int.
-/

namespace Axionomy.Exchange

structure Cell where
  before : Nat
  consumed : Nat
  produced : Nat
  preserved : Nat
  weight : Int
  deriving Repr

def after (c : Cell) : Nat := c.before - c.consumed + c.produced

def available (c : Cell) : Prop := c.consumed + c.preserved ≤ c.before

instance (c : Cell) : Decidable (available c) := inferInstanceAs
  (Decidable (c.consumed + c.preserved ≤ c.before))

def admissible (limit : Nat) (c : Cell) : Prop :=
  available c ∧ after c ≤ limit

instance (limit : Nat) (c : Cell) : Decidable (admissible limit c) :=
  inferInstanceAs (Decidable (available c ∧ after c ≤ limit))

theorem preserved_survives (c : Cell) (h : available c) :
    c.preserved ≤ after c := by
  unfold available at h
  unfold after
  omega

theorem exact_balance_delta (c : Cell) (h : available c) :
    (after c : Int) = (c.before : Int) - (c.consumed : Int) + (c.produced : Int) := by
  unfold available at h
  unfold after
  omega

theorem shortfall_zero_iff_available (c : Cell) :
    c.consumed + c.preserved - c.before = 0 ↔ available c := by
  unfold available
  omega

def weightedTotal (quantity : Cell → Nat) : List Cell → Int
  | [] => 0
  | c :: cs => c.weight * (quantity c : Int) + weightedTotal quantity cs

theorem weighted_delta (cells : List Cell)
    (h : ∀ c ∈ cells, available c) :
    weightedTotal after cells =
      weightedTotal Cell.before cells - weightedTotal Cell.consumed cells +
        weightedTotal Cell.produced cells := by
  induction cells with
  | nil => simp [weightedTotal]
  | cons c cs ih =>
    have hc : available c := h c (by simp)
    have ht : ∀ x ∈ cs, available x := by
      intro x hx
      exact h x (by simp [hx])
    have hd := exact_balance_delta c hc
    have hw : c.weight * (after c : Int) =
        c.weight * (c.before : Int) - c.weight * (c.consumed : Int) +
          c.weight * (c.produced : Int) := by
      rw [hd, Int.mul_add, Int.mul_sub]
    simp only [weightedTotal]
    rw [hw, ih ht]
    omega

-- This is the algebra behind Rust LinearInvariant::conserves: compare only
-- consumed/produced deltas instead of rescanning the whole world.
theorem conservation_from_deltas (cells : List Cell)
    (h : ∀ c ∈ cells, available c)
    (conserved : weightedTotal Cell.consumed cells = weightedTotal Cell.produced cells) :
    weightedTotal after cells = weightedTotal Cell.before cells := by
  have hd := weighted_delta cells h
  omega

def accepted (limit : Nat) (cells : List Cell) : Prop :=
  (∀ c ∈ cells, admissible limit c) ∧
    weightedTotal Cell.consumed cells = weightedTotal Cell.produced cells

instance (limit : Nat) (cells : List Cell) : Decidable (accepted limit cells) :=
  inferInstanceAs (Decidable ((∀ c ∈ cells, admissible limit c) ∧
    weightedTotal Cell.consumed cells = weightedTotal Cell.produced cells))

-- A specification of a prepared atomic exchange: rejected proposals return
-- their original balances, accepted proposals return all new balances together.
def applyPrepared (limit : Nat) (cells : List Cell) : Bool × List Nat :=
  if accepted limit cells then (true, cells.map after)
  else (false, cells.map Cell.before)

theorem assessment_agrees_with_apply (limit : Nat) (cells : List Cell) :
    (applyPrepared limit cells).1 = true ↔ accepted limit cells := by
  by_cases h : accepted limit cells <;> simp [applyPrepared, h]

theorem rejection_is_atomic (limit : Nat) (cells : List Cell)
    (h : (applyPrepared limit cells).1 = false) :
    (applyPrepared limit cells).2 = cells.map Cell.before := by
  unfold applyPrepared at *
  split at h <;> simp_all

theorem acceptance_applies_every_delta (limit : Nat) (cells : List Cell)
    (h : (applyPrepared limit cells).1 = true) :
    (applyPrepared limit cells).2 = cells.map after := by
  unfold applyPrepared at *
  split at h <;> simp_all

theorem accepted_balances_bounded (limit : Nat) (cells : List Cell)
    (h : (applyPrepared limit cells).1 = true) :
    ∀ q ∈ (applyPrepared limit cells).2, q ≤ limit := by
  have ha := (assessment_agrees_with_apply limit cells).mp h
  rw [acceptance_applies_every_delta limit cells h]
  intro q hq
  obtain ⟨c, hc, rfl⟩ := List.mem_map.mp hq
  exact (ha.1 c hc).2

theorem accepted_exchange_conserves (limit : Nat) (cells : List Cell)
    (h : (applyPrepared limit cells).1 = true) :
    weightedTotal after cells = weightedTotal Cell.before cells := by
  have ha := (assessment_agrees_with_apply limit cells).mp h
  exact conservation_from_deltas cells (fun c hc => (ha.1 c hc).1) ha.2

-- A reusable consume-one/produce-one rule can fire twice sequentially from
-- one token, but a units=2 batch requires two tokens up front. Universal
-- equivalence of batch applicability and sequential applicability is false.
def reusable : Cell := ⟨1, 1, 1, 0, 1⟩
def reusableBatch : Cell := ⟨1, 2, 2, 0, 1⟩

theorem batching_requires_a_qualified_statement :
    available reusable ∧ after reusable = reusable.before ∧ ¬available reusableBatch := by
  decide

-- Executable u64 preflight for the static single-cell conformance domain.
-- It checks scaled intermediates even when a later subtraction could cancel
-- an overflow. Runtime invariant error classification is outside this model.
def prepareStatic (before consumed produced preserved units : Nat) (weight : Int) : Option Cell := do
  if units = 0 then none else do
    let c ← Quantity.mul Quantity.limit consumed units
    let p ← Quantity.mul Quantity.limit produced units
    let _ ← Quantity.add Quantity.limit c preserved
    some ⟨before, c, p, preserved, weight⟩

theorem static_zero_units_rejected (before consumed produced preserved : Nat) (weight : Int) :
    prepareStatic before consumed produced preserved 0 weight = none := by
  simp [prepareStatic]

def staticOutcome (before consumed produced preserved units : Nat) (conserve : Bool) : Bool × Nat :=
  match prepareStatic before consumed produced preserved units (if conserve then 1 else 0) with
  | none => (false, before)
  | some cell =>
    let result := applyPrepared Quantity.limit [cell]
    (result.1, result.2.headD before)

end Axionomy.Exchange
