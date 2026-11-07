import Std.Tactic

namespace Axionomy.Basket

-- Asset identities stay typed and independent of quantities. Zero quantities
-- participate in validation, even though successful output omits them.
def firstDuplicate {A : Type} [DecidableEq A] (seen : List A) : List A → Option A
  | [] => none
  | asset :: rest =>
    if asset ∈ seen then some asset else firstDuplicate (seen ++ [asset]) rest

def canonical {A : Type} (entries : List (A × Nat)) : List (A × Nat) :=
  entries.filter (fun entry => entry.2 != 0)

def construct {A : Type} [DecidableEq A] (entries : List (A × Nat)) :
    Except A (List (A × Nat)) :=
  match firstDuplicate [] (entries.map Prod.fst) with
  | some asset => .error asset
  | none => .ok (canonical entries)

theorem firstDuplicate_none_iff {A : Type} [DecidableEq A]
    (keys seen : List A) :
    firstDuplicate seen keys = none ↔
      keys.Nodup ∧ ∀ asset ∈ keys, asset ∉ seen := by
  induction keys generalizing seen with
  | nil => simp [firstDuplicate]
  | cons asset rest ih =>
    by_cases h : asset ∈ seen
    · simp [firstDuplicate, h]
    · simp [firstDuplicate, h, ih, List.nodup_cons]
      grind

theorem construction_succeeds_iff {A : Type} [DecidableEq A]
    (entries : List (A × Nat)) :
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

theorem zero_cannot_hide_duplicate {A : Type} [DecidableEq A] (asset : A) (n : Nat) :
    construct [(asset, 0), (asset, n)] = .error asset := by
  simp [construct, firstDuplicate]

theorem canonical_has_no_zero {A : Type} (entries : List (A × Nat))
    (entry : A × Nat) (h : entry ∈ canonical entries) : entry.2 ≠ 0 := by
  simpa [canonical] using (List.mem_filter.mp h).2

theorem canonical_preserves_entries {A : Type} (entries : List (A × Nat))
    (entry : A × Nat) :
    entry ∈ canonical entries ↔ entry ∈ entries ∧ entry.2 ≠ 0 := by
  simp [canonical]

theorem canonical_preserves_order {A : Type} (entries : List (A × Nat)) :
    (canonical entries).Sublist entries := List.filter_sublist

theorem canonical_unique {A : Type} (entries : List (A × Nat))
    (unique : (entries.map Prod.fst).Nodup) :
    ((canonical entries).map Prod.fst).Nodup :=
  List.Nodup.sublist ((canonical_preserves_order entries).map Prod.fst) unique

end Axionomy.Basket
