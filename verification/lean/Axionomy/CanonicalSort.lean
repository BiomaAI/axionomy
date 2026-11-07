import Axionomy.Observations
import Axionomy.RateConstructionRefinement
import Mathlib.Data.List.Sort


namespace Axionomy.CanonicalSort
open Aeneas Aeneas.Std AccountExtracted RateConstructionRefinement

variable {T : Type}

def before (cmp : T → T → Ordering) (left right : T) : Prop := cmp left right ≠ .gt
instance (cmp : T → T → Ordering) : DecidableRel (before cmp) := fun left right => inferInstanceAs (Decidable (cmp left right ≠ .gt))

def sort (cmp : T → T → Ordering) (values : List T) : List T := List.insertionSort (before cmp) values

theorem before_iff (cmp : T → T → Ordering) (laws : OrderedEffects.OrderLaws cmp) (left right : T) :
    before cmp left right ↔ cmp left right = .lt ∨ left = right := by
  cases compared : cmp left right with
  | lt => simp [before, compared]
  | eq => simp [before, laws.equal left right compared, laws.reflexive]
  | gt =>
    have different : left ≠ right := by intro same; subst right; simp [laws.reflexive] at compared
    simp [before, compared, different]

theorem before_total (cmp : T → T → Ordering) (laws : OrderedEffects.OrderLaws cmp) (left right : T) :
    before cmp left right ∨ before cmp right left := by
  cases compared : cmp left right with
  | lt => exact Or.inl (by simp [before, compared])
  | eq => exact Or.inl (by simp [before, compared])
  | gt => exact Or.inr (by simp [before, laws.greater left right compared])

theorem before_trans (cmp : T → T → Ordering) (laws : OrderedEffects.OrderLaws cmp) (x y z : T)
    (first : before cmp x y) (second : before cmp y z) : before cmp x z := by
  rw [before_iff cmp laws] at first second ⊢
  rcases first with less | same
  · rcases second with more | same
    · exact Or.inl (laws.transitive x y z less more)
    · subst z; exact Or.inl less
  · subst y; exact second

theorem before_antisymm (cmp : T → T → Ordering) (laws : OrderedEffects.OrderLaws cmp) (left right : T)
    (forward : before cmp left right) (backward : before cmp right left) : left = right := by
  rw [before_iff cmp laws] at forward backward
  rcases forward with less | same
  · rcases backward with more | same
    · have impossible := laws.transitive left right left less more
      simp [laws.reflexive] at impossible
    · exact same.symm
  · exact same

theorem sort_perm (cmp : T → T → Ordering) (values : List T) : (sort cmp values).Perm values :=
  List.perm_insertionSort (before cmp) values

theorem sort_length (cmp : T → T → Ordering) (values : List T) : (sort cmp values).length = values.length :=
  (sort_perm cmp values).length_eq

theorem sort_sorted (cmp : T → T → Ordering) (laws : OrderedEffects.OrderLaws cmp) (values : List T) :
    (sort cmp values).Pairwise (before cmp) := by
  let : Std.Total (before cmp) := ⟨before_total cmp laws⟩
  let : IsTrans T (before cmp) := ⟨before_trans cmp laws⟩
  exact List.pairwise_insertionSort (before cmp) values

/-- Equal multisets have one canonical order, even when inputs contain repeats. -/
theorem sort_canonical (cmp : T → T → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (left right : List T) (same : left.Perm right) : sort cmp left = sort cmp right := by
  let : Std.Antisymm (before cmp) := ⟨before_antisymm cmp laws⟩
  exact List.Perm.eq_of_pairwise' (sort_sorted cmp laws left) (sort_sorted cmp laws right)
    ((sort_perm cmp left).trans (same.trans (sort_perm cmp right).symm))

theorem insertion_refines (ordInst : core.cmp.Ord T) (cmp : T → T → Ordering)
    (lawful : LawfulOrdering ordInst cmp) (value : T) (values : List T) :
    orderedInsertM ordInst.cmp value values = .ok (List.orderedInsert (before cmp) value values) := by
  induction values with
  | nil => rfl
  | cons head rest ih =>
    cases compared : cmp value head <;>
      simp [orderedInsertM, lawful value head, compared, List.orderedInsert, before, ih]

theorem sorting_refines (ordInst : core.cmp.Ord T) (cmp : T → T → Ordering)
    (lawful : LawfulOrdering ordInst cmp) (values : List T) :
    stableSortM ordInst.cmp values = .ok (sort cmp values) := by
  induction values with
  | nil => rfl
  | cons head rest ih =>
    simp [stableSortM, ih, insertion_refines ordInst cmp lawful, sort]

theorem slice_sort (ordInst : core.cmp.Ord T) (cmp : T → T → Ordering)
    (lawful : LawfulOrdering ordInst cmp) (values : Slice T) :
    ∃ result, alloc.slice.Slice.sort ordInst values = .ok result ∧ result.val = sort cmp values.val := by
  have fits : (sort cmp values.val).length ≤ Usize.max := by rw [sort_length]; exact values.property
  refine ⟨Slice.from _ fits, ?_, by simp⟩
  simp [alloc.slice.Slice.sort, sorting_refines ordInst cmp lawful, fits]

end Axionomy.CanonicalSort
