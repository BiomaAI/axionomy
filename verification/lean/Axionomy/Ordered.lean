/-!
# Entries in the order of their keys

An economy keeps its accounts and its rates in the order of their
identifiers, never in the order they were written or opened: two economies
holding the same accounts and rates are the same economy, and one read back
from storage, which keys everything by identifier, is the economy that was
written. `ordered` puts entries in that order; for a lawful comparison it is
sorted, a permutation of its input, and the same for every permutation of
entries with distinct keys.
-/

namespace Axionomy.Ordered

variable {K V : Type}

/-- Whether `left` comes no later than `right`, by their keys. -/
def before (cmp : K → K → Ordering) (left right : K × V) : Bool :=
  cmp left.1 right.1 != .gt

/-- `entries` in the order of their keys. -/
def ordered (cmp : K → K → Ordering) (entries : List (K × V)) : List (K × V) :=
  entries.mergeSort (before cmp)

/-- A comparison that orders keys totally. -/
structure Lawful (cmp : K → K → Ordering) : Prop where
  trans : ∀ a b c, cmp a b ≠ .gt → cmp b c ≠ .gt → cmp a c ≠ .gt
  total : ∀ a b, cmp a b ≠ .gt ∨ cmp b a ≠ .gt
  antisymm : ∀ a b, cmp a b ≠ .gt → cmp b a ≠ .gt → a = b

theorem ordered_perm (cmp : K → K → Ordering) (entries : List (K × V)) :
    (ordered cmp entries).Perm entries :=
  List.mergeSort_perm entries _

@[simp] theorem mem_ordered (cmp : K → K → Ordering) (entries : List (K × V)) (entry : K × V) :
    entry ∈ ordered cmp entries ↔ entry ∈ entries :=
  List.mem_mergeSort

/-- Ordered entries are sorted by their keys. -/
theorem ordered_pairwise {cmp : K → K → Ordering} (lawful : Lawful cmp) (entries : List (K × V)) :
    (ordered cmp entries).Pairwise (fun left right => cmp left.1 right.1 ≠ .gt) := by
  have sorted := List.pairwise_mergeSort (le := before cmp)
    (fun a b c ab bc => by
      simp only [before, bne_iff_ne, ne_eq] at *
      exact lawful.trans a.1 b.1 c.1 ab bc)
    (fun a b => by
      simp only [before, Bool.or_eq_true, bne_iff_ne, ne_eq]
      exact lawful.total a.1 b.1)
    entries
  exact sorted.imp fun related => by simpa [before] using related

/-- In entries with distinct keys, two entries with one key are one entry. -/
theorem eq_of_key {entries : List (K × V)} (distinct : (entries.map Prod.fst).Nodup)
    {left right : K × V} (hl : left ∈ entries) (hr : right ∈ entries) (same : left.1 = right.1) :
    left = right := by
  induction entries with
  | nil => simp at hl
  | cons entry rest ih =>
    simp only [List.map_cons, List.nodup_cons, List.mem_map, not_exists, not_and] at distinct
    rcases List.mem_cons.mp hl with first | later <;>
      rcases List.mem_cons.mp hr with first' | later'
    · exact first.trans first'.symm
    · subst first
      exact absurd same.symm (distinct.1 right later')
    · subst first'
      exact absurd same (distinct.1 left later)
    · exact ih distinct.2 later later'

/-- Entries with distinct keys are ordered alike whatever order they come
in: the order of an economy's accounts and rates is a function of which it
holds. -/
theorem ordered_eq_of_perm {cmp : K → K → Ordering} (lawful : Lawful cmp)
    {left right : List (K × V)} (distinct : (left.map Prod.fst).Nodup) (perm : left.Perm right) :
    ordered cmp left = ordered cmp right := by
  apply List.Perm.eq_of_pairwise (le := fun a b => cmp a.1 b.1 ≠ .gt)
  · intro a b ha hb ab ba
    have ha : a ∈ left := (mem_ordered cmp left a).mp ha
    have hb : b ∈ left := perm.mem_iff.mpr ((mem_ordered cmp right b).mp hb)
    exact eq_of_key distinct ha hb (lawful.antisymm a.1 b.1 ab ba)
  · exact ordered_pairwise lawful left
  · exact ordered_pairwise lawful right
  · exact (ordered_perm cmp left).trans (perm.trans (ordered_perm cmp right).symm)

/-- Natural numbers, as the conformance fixtures name accounts, assets,
rates and roles, are ordered lawfully. -/
theorem compare_lawful : Lawful (compare : Nat → Nat → Ordering) where
  trans a b c ab bc := by
    simp only [ne_eq, Nat.compare_eq_gt, Nat.not_lt] at *
    omega
  total a b := by
    simp only [ne_eq, Nat.compare_eq_gt, Nat.not_lt]
    omega
  antisymm a b ab ba := by
    simp only [ne_eq, Nat.compare_eq_gt, Nat.not_lt] at *
    omega

end Axionomy.Ordered
