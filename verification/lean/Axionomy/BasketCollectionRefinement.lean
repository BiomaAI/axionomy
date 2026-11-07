import Axionomy.BasketCollection
import Axionomy.IteratorContract
import Axionomy.TypedDataRefinement

namespace Axionomy.BasketCollectionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement IteratorContract

variable {A N M Source Iter : Type}

def encode : Except A (List (A × N)) → core.result.Result (basket.Basket A N) (basket.BasketError A)
  | .ok entries => .Ok ⟨entries⟩
  | .error asset => .Err (.DuplicateAsset asset)

def collected : Except A (List (A × N)) → Result (basket.Basket A N)
  | .ok entries => .ok ⟨entries⟩
  | .error _ => .fail .panic

theorem collected_panics_iff [DecidableEq A] (isZero : N → Bool) (values : List (A × N)) :
    collected (BasketCollection.construct isZero [] values) = .fail .panic ↔
      ¬ (values.map Prod.fst).Nodup := by
  simp only [BasketCollection.construct, List.map_nil, List.nil_append]
  cases found : Basket.firstDuplicate [] (values.map Prod.fst) with
  | none =>
    have unique := (Basket.firstDuplicate_none_iff (values.map Prod.fst) []).mp found
    simp [collected, unique.1]
  | some asset =>
    have duplicated : ¬ (values.map Prod.fst).Nodup := by
      intro unique
      have absent := (Basket.firstDuplicate_none_iff (values.map Prod.fst) []).mpr (by simpa using unique)
      simp [found] at absent
    simp [collected, duplicated]

theorem found_isSome [DecidableEq A] (asset : A) (entries : List (A × N)) :
    (Account.find? asset entries).isSome = decide (asset ∈ entries.map Prod.fst) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases same : asset = key <;> simp [Account.find?, same, ih]

section Construction
variable [DecidableEq A]
variable (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (lawful : LawfulEquality equal)
variable (zero : num_traits.identities.Zero N) (isZero : N → Bool)
variable (zeroLaw : ∀ value, zero.is_zero value = .ok (isZero value))
variable (into : core.iter.traits.collect.IntoIterator Source (A × N) Iter)

omit [DecidableEq A] in
include zeroLaw in
theorem retain_refines (entries : List (A × N)) :
    basket.retain_quantities
      (basket.construct_entries.closure.Insts.CoreOpsFunctionFnMutTupleShared0QuantityBool equal hash zero into)
      entries () = .ok (BasketCollection.canonical isZero entries) := by
  unfold basket.retain_quantities BasketCollection.canonical
  apply BasketRefinement.retain_exact _ (fun value => !isZero value)
  intro state value
  simp [basket.construct_entries.closure.Insts.CoreOpsFunctionFnMutTupleShared0QuantityBool.call_mut,
    quantity.Quantity.scalar_is_zero, zeroLaw value]

include lawful in
theorem body_step (iter after : Iter) (asset : A) (value : N)
    (called : into.iteratorInst.next iter = .ok (some (asset, value), after))
    (seen : List (A × N)) :
    basket.construct_entries_loop.body equal hash zero into iter seen =
      .ok (if asset ∈ seen.map Prod.fst then .done (.Err (.DuplicateAsset asset))
        else .cont (after, seen ++ [(asset, value)])) := by
  unfold basket.construct_entries_loop.body
  simp only [called, bind_ok, indexmap.map.IndexMap.contains_key, indexmap.map.IndexMap.get,
    equivalent_eq, get_exact equal lawful, found_isSome]
  by_cases present : asset ∈ seen.map Prod.fst
  · simp [present]
  · simp [present, indexmap.map.IndexMap.insert, BasketRefinement.insert_fresh equal lawful _ _ _ present]

include lawful zeroLaw in
theorem loop_refines (iter : Iter) (values : List (A × N))
    (yields : Yields into.iteratorInst.next iter values) (seen : List (A × N)) :
    basket.construct_entries_loop equal hash zero into iter seen =
      .ok (encode (BasketCollection.construct isZero seen values)) := by
  induction yields generalizing seen with
  | done called =>
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp [basket.construct_entries_loop.body, called,
      retain_refines equal hash zero isZero zeroLaw into seen,
      BasketCollection.construct, Basket.firstDuplicate, encode]
  | @step state after entry rest called tail ih =>
    rcases entry with ⟨asset, value⟩
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp only [body_step equal hash lawful zero into state after asset value called seen]
    by_cases present : asset ∈ seen.map Prod.fst
    · simp [present, BasketCollection.construct, Basket.firstDuplicate, encode]
    · simp only [present, ↓reduceIte, bind_ok]
      change basket.construct_entries_loop equal hash zero into after (seen ++ [(asset, value)]) = _
      rw [ih]
      simp [BasketCollection.construct, Basket.firstDuplicate, present, List.map_append, List.append_assoc]

/- Duplicate rejection needs only an observed prefix, even if the remaining
iterator would panic or run forever. No zero predicate is evaluated on this path. -/
include lawful in
theorem reject_prefix (iter after : Iter) (values : List (A × N))
    (observed : Prefix into.iteratorInst.next iter values after) (seen : List (A × N))
    (asset : A) (duplicate : Basket.firstDuplicate (seen.map Prod.fst) (values.map Prod.fst) = some asset) :
    basket.construct_entries_loop equal hash zero into iter seen = .ok (.Err (.DuplicateAsset asset)) := by
  induction observed generalizing seen with
  | nil => simp [Basket.firstDuplicate] at duplicate
  | @step state next entry rest finalState called tail ih =>
    rcases entry with ⟨key, value⟩
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp only [body_step equal hash lawful zero into state next key value called seen]
    by_cases present : key ∈ seen.map Prod.fst
    · have same : key = asset := by simpa [Basket.firstDuplicate, present] using duplicate
      simp only [present, ↓reduceIte, bind_ok]
      simp [same]
    · simp only [present, ↓reduceIte, bind_ok]
      apply ih (seen ++ [(key, value)])
      simpa [Basket.firstDuplicate, present, List.map_append] using duplicate

include lawful in
theorem loop_failure (error : Error) (iter : Iter) (values : List (A × N))
    (fails : FailsAfter into.iteratorInst.next error iter values) (seen : List (A × N))
    (unique : Basket.firstDuplicate (seen.map Prod.fst) (values.map Prod.fst) = none) :
    basket.construct_entries_loop equal hash zero into iter seen = .fail error := by
  induction fails generalizing seen with
  | stopped called =>
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp [basket.construct_entries_loop.body, called]
  | @step state next entry rest called tail ih =>
    rcases entry with ⟨key, value⟩
    have fresh : key ∉ seen.map Prod.fst := by
      intro present
      simp [Basket.firstDuplicate, present] at unique
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp only [body_step equal hash lawful zero into state next key value called seen,
      fresh, ↓reduceIte, bind_ok]
    apply ih (seen ++ [(key, value)])
    simpa [Basket.firstDuplicate, fresh, List.map_append] using unique

end Construction

section Public
variable [DecidableEq A]
variable (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (lawful : LawfulEquality equal)
variable (numbers : quantity.QuantityScalar N M) (isZero : N → Bool)
variable (zeroLaw : ∀ value, numbers.num_traitsidentitiesZeroInst.is_zero value = .ok (isZero value))
variable (into : core.iter.traits.collect.IntoIterator Source (A × N) Iter)

include lawful zeroLaw in
theorem entries_refines (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (yields : Yields into.iteratorInst.next iter values) :
    basket.Basket.try_from_entries equal hash numbers into source =
      .ok (encode (BasketCollection.construct isZero [] values)) := by
  simp [basket.Basket.try_from_entries, basket.construct_entries,
    indexmap.map.IndexMapKVRandomState.new, started,
    loop_refines equal hash lawful numbers.num_traitsidentitiesZeroInst isZero zeroLaw into iter values yields []]

include lawful zeroLaw in
theorem entries_contract (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (yields : Yields into.iteratorInst.next iter values) (unique : (values.map Prod.fst).Nodup)
    (capacity : values.length ≤ Usize.max) :
    basket.Basket.try_from_entries equal hash numbers into source =
      .ok (.Ok ⟨BasketCollection.canonical isZero values⟩) ∧
    ((BasketCollection.canonical isZero values).map Prod.fst).Nodup ∧
    (∀ entry ∈ BasketCollection.canonical isZero values, isZero entry.2 = false) ∧
    (BasketCollection.canonical isZero values).length ≤ Usize.max := by
  have noDuplicate := (Basket.firstDuplicate_none_iff (values.map Prod.fst) []).mpr (by simpa using unique)
  refine ⟨?_, BasketCollection.unique_entries isZero values unique,
    BasketCollection.nonzero_entries isZero values, ?_⟩
  · rw [entries_refines equal hash lawful numbers isZero zeroLaw into source iter started values yields]
    simp [BasketCollection.construct, noDuplicate, encode]
  · exact Nat.le_trans (List.length_filter_le _ _) capacity

include lawful in
theorem entries_reject_prefix (source : Source) (iter after : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (observed : Prefix into.iteratorInst.next iter values after) (asset : A)
    (duplicate : Basket.firstDuplicate [] (values.map Prod.fst) = some asset) :
    basket.Basket.try_from_entries equal hash numbers into source = .ok (.Err (.DuplicateAsset asset)) := by
  simp [basket.Basket.try_from_entries, basket.construct_entries,
    indexmap.map.IndexMapKVRandomState.new, started,
    reject_prefix equal hash lawful numbers.num_traitsidentitiesZeroInst into iter after values observed [] asset duplicate]

include lawful in
theorem entries_failure (error : Error) (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (fails : FailsAfter into.iteratorInst.next error iter values) (unique : (values.map Prod.fst).Nodup) :
    basket.Basket.try_from_entries equal hash numbers into source = .fail error := by
  have noDuplicate := (Basket.firstDuplicate_none_iff (values.map Prod.fst) []).mpr (by simpa using unique)
  simp [basket.Basket.try_from_entries, basket.construct_entries,
    indexmap.map.IndexMapKVRandomState.new, started,
    loop_failure equal hash lawful numbers.num_traitsidentitiesZeroInst into error iter values fails [] noDuplicate]

omit [DecidableEq A] in
theorem entries_start_failure (error : Error) (source : Source)
    (failed : into.into_iter source = .fail error) :
    basket.Basket.try_from_entries equal hash numbers into source = .fail error := by
  simp [basket.Basket.try_from_entries, basket.construct_entries, indexmap.map.IndexMapKVRandomState.new, failed]

include lawful zeroLaw in
theorem collect_refines (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (yields : Yields into.iteratorInst.next iter values) :
    basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter equal hash numbers into source =
      collected (BasketCollection.construct isZero [] values) := by
  simp only [basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter,
    entries_refines equal hash lawful numbers isZero zeroLaw into source iter started values yields, bind_ok]
  cases BasketCollection.construct isZero [] values <;> rfl

include lawful in
theorem collect_duplicate_panics (source : Source) (iter after : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (observed : Prefix into.iteratorInst.next iter values after) (asset : A)
    (duplicate : Basket.firstDuplicate [] (values.map Prod.fst) = some asset) :
    basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter equal hash numbers into source = .fail .panic := by
  simp [basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter,
    entries_reject_prefix equal hash lawful numbers into source iter after started values observed asset duplicate]

include lawful zeroLaw in
theorem collect_panics_iff (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (yields : Yields into.iteratorInst.next iter values) :
    basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter equal hash numbers into source = .fail .panic ↔
      ¬ (values.map Prod.fst).Nodup := by
  rw [collect_refines equal hash lawful numbers isZero zeroLaw into source iter started values yields]
  exact collected_panics_iff isZero values

include lawful in
theorem collect_failure (error : Error) (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (A × N))
    (fails : FailsAfter into.iteratorInst.next error iter values) (unique : (values.map Prod.fst).Nodup) :
    basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter equal hash numbers into source = .fail error := by
  simp [basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter,
    entries_failure equal hash lawful numbers into error source iter started values fails unique]

omit [DecidableEq A] in
theorem collect_start_failure (error : Error) (source : Source)
    (failed : into.into_iter source = .fail error) :
    basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter equal hash numbers into source = .fail error := by
  simp [basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter,
    entries_start_failure equal hash numbers into error source failed]

end Public
end Axionomy.BasketCollectionRefinement
