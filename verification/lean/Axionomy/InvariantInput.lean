import Axionomy.Basket
import Axionomy.Inspection

namespace Axionomy.InvariantInput

-- Input weights participate in duplicate validation before zero normalization.
-- The result retains signed coefficients and input order.
open Basket (firstDuplicate firstDuplicate_none_iff)

def canonical {A : Type} (entries : List (A × Int)) : List (A × Int) :=
  entries.filter (fun entry => entry.2 != 0)

def construct {A : Type} [DecidableEq A] (entries : List (A × Int)) :
    Except A (List (A × Int)) :=
  match firstDuplicate [] (entries.map Prod.fst) with
  | some asset => .error asset
  | none => .ok (canonical entries)

theorem construction_succeeds_iff {A : Type} [DecidableEq A]
    (entries : List (A × Int)) :
    construct entries = .ok (canonical entries) ↔
      (entries.map Prod.fst).Nodup := by
  simp only [construct]
  cases h : firstDuplicate [] (entries.map Prod.fst) with
  | none => simpa using (firstDuplicate_none_iff (entries.map Prod.fst) []).mp h
  | some asset =>
    have hdup : ¬ (entries.map Prod.fst).Nodup := by
      intro hnodup
      have := (firstDuplicate_none_iff (entries.map Prod.fst) []).mpr (by simpa using hnodup)
      simp [h] at this
    simp [hdup]

theorem zero_cannot_hide_duplicate {A : Type} [DecidableEq A] (asset : A) (n : Int) :
    construct [(asset, 0), (asset, n)] = .error asset := by
  simp [construct, firstDuplicate]

theorem canonical_has_no_zero {A : Type} (entries : List (A × Int))
    (entry : A × Int) (h : entry ∈ canonical entries) : entry.2 ≠ 0 := by
  simpa [canonical] using (List.mem_filter.mp h).2

theorem canonical_preserves_entries {A : Type} (entries : List (A × Int))
    (entry : A × Int) :
    entry ∈ canonical entries ↔ entry ∈ entries ∧ entry.2 ≠ 0 := by
  simp [canonical]

theorem canonical_preserves_order {A : Type} (entries : List (A × Int)) :
    (canonical entries).Sublist entries := List.filter_sublist

theorem canonical_unique {A : Type} (entries : List (A × Int))
    (unique : (entries.map Prod.fst).Nodup) :
    ((canonical entries).map Prod.fst).Nodup :=
  List.Nodup.sublist ((canonical_preserves_order entries).map Prod.fst) unique

theorem successful_weights_valid {A : Type} [DecidableEq A]
    (entries output : List (A × Int)) (success : construct entries = .ok output) :
    Inspection.ValidWeights output := by
  cases duplicate : firstDuplicate [] (entries.map Prod.fst) with
  | some asset => simp [construct, duplicate] at success
  | none =>
    have same : canonical entries = output := by simpa [construct, duplicate] using success
    subst output
    refine ⟨canonical_unique entries ?_, canonical_has_no_zero entries⟩
    exact ((firstDuplicate_none_iff _ _).mp duplicate).1

end Axionomy.InvariantInput
