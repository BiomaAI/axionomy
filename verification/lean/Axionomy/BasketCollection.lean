import Axionomy.Basket

namespace Axionomy.BasketCollection

variable {A N : Type}

/-- Construction tests all asset identities before omitting zero quantities. -/
def canonical (isZero : N → Bool) (entries : List (A × N)) : List (A × N) :=
  entries.filter (fun entry => !isZero entry.2)

def construct [DecidableEq A] (isZero : N → Bool) (seen entries : List (A × N)) :
    Except A (List (A × N)) :=
  match Basket.firstDuplicate (seen.map Prod.fst) (entries.map Prod.fst) with
  | some asset => .error asset
  | none => .ok (canonical isZero (seen ++ entries))

theorem success_iff [DecidableEq A] (isZero : N → Bool) (entries : List (A × N)) :
    construct isZero [] entries = .ok (canonical isZero entries) ↔ (entries.map Prod.fst).Nodup := by
  simp only [construct, List.map_nil, List.nil_append]
  cases found : Basket.firstDuplicate [] (entries.map Prod.fst) with
  | none => simpa using (Basket.firstDuplicate_none_iff (entries.map Prod.fst) []).mp found
  | some asset =>
    have duplicated : ¬ (entries.map Prod.fst).Nodup := by
      intro unique
      have absent := (Basket.firstDuplicate_none_iff (entries.map Prod.fst) []).mpr (by simpa using unique)
      simp [found] at absent
    simp [duplicated]

theorem preserves_order (isZero : N → Bool) (entries : List (A × N)) :
    (canonical isZero entries).Sublist entries := List.filter_sublist

theorem nonzero_entries (isZero : N → Bool) (entries : List (A × N)) (entry : A × N)
    (member : entry ∈ canonical isZero entries) : isZero entry.2 = false := by
  simpa [canonical] using (List.mem_filter.mp member).2

theorem unique_entries (isZero : N → Bool) (entries : List (A × N))
    (unique : (entries.map Prod.fst).Nodup) : ((canonical isZero entries).map Prod.fst).Nodup :=
  unique.sublist ((preserves_order isZero entries).map Prod.fst)

theorem zero_still_duplicates [DecidableEq A] (isZero : N → Bool) (asset : A) (first second : N) :
    construct isZero [] [(asset, first), (asset, second)] = .error asset := by
  simp [construct, Basket.firstDuplicate]

theorem naturals [DecidableEq A] (entries : List (A × Nat)) :
    construct (fun value => decide (value = 0)) [] entries = Basket.construct entries := by
  simp only [construct, Basket.construct, List.map_nil, List.nil_append]
  cases Basket.firstDuplicate [] (entries.map Prod.fst) with
  | some _ => rfl
  | none =>
    congr 1

end Axionomy.BasketCollection
