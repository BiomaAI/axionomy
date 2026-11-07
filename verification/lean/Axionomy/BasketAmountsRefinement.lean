import Axionomy.BasketCollectionRefinement

namespace Axionomy.BasketAmountsRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement IteratorContract BasketCollectionRefinement
variable {A N M Source Iter : Type}

def parts (value : amount.AssetAmount A N) : A × N := (value.asset, value.quantity)

abbrev entriesIterator (iter : core.iter.traits.iterator.Iterator Iter (amount.AssetAmount A N)) :=
  basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity iter

theorem yields_parts (iterInst : core.iter.traits.iterator.Iterator Iter (amount.AssetAmount A N))
    (iter : Iter) (values : List (amount.AssetAmount A N)) (yields : Yields iterInst.next iter values) :
    Yields (entriesIterator iterInst).next iter (values.map parts) := by
  induction yields with
  | @done state after called =>
    exact .done (after := after) (by simp [
      basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next, called])
  | step called tail ih =>
    exact .step (by simp [
      basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next, called,
      amount.AssetAmount.into_parts, parts]) ih

theorem prefix_parts (iterInst : core.iter.traits.iterator.Iterator Iter (amount.AssetAmount A N))
    (iter after : Iter) (values : List (amount.AssetAmount A N))
    (observed : Prefix iterInst.next iter values after) :
    Prefix (entriesIterator iterInst).next iter (values.map parts) after := by
  induction observed with
  | nil => exact .nil _
  | step called tail ih =>
    exact .step (by simp [
      basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next, called,
      amount.AssetAmount.into_parts, parts]) ih

theorem failure_parts (iterInst : core.iter.traits.iterator.Iterator Iter (amount.AssetAmount A N))
    (error : Error) (iter : Iter) (values : List (amount.AssetAmount A N))
    (fails : FailsAfter iterInst.next error iter values) :
    FailsAfter (entriesIterator iterInst).next error iter (values.map parts) := by
  induction fails with
  | stopped called =>
    exact .stopped (by simp [
      basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next, called])
  | step called tail ih =>
    exact .step (by simp [
      basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next, called,
      amount.AssetAmount.into_parts, parts]) ih

section Public
variable [DecidableEq A]
variable (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (lawful : LawfulEquality equal)
variable (numbers : quantity.QuantityScalar N M) (isZero : N → Bool)
variable (zeroLaw : ∀ value, numbers.num_traitsidentitiesZeroInst.is_zero value = .ok (isZero value))
variable (into : core.iter.traits.collect.IntoIterator Source (amount.AssetAmount A N) Iter)

include lawful zeroLaw in
theorem amounts_refines (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (amount.AssetAmount A N))
    (yields : Yields into.iteratorInst.next iter values) :
    basket.Basket.try_from_amounts equal hash numbers into source =
      .ok (encode (BasketCollection.construct isZero [] (values.map parts))) := by
  have called := entries_refines equal hash lawful numbers isZero zeroLaw
    (core.iter.traits.collect.IntoIterator.Blanket (entriesIterator into.iteratorInst))
    iter iter rfl (values.map parts) (yields_parts into.iteratorInst iter values yields)
  simpa [basket.Basket.try_from_amounts, started] using called

include lawful in
theorem amounts_reject_prefix (source : Source) (iter after : Iter)
    (started : into.into_iter source = .ok iter) (values : List (amount.AssetAmount A N))
    (observed : Prefix into.iteratorInst.next iter values after) (asset : A)
    (duplicate : Basket.firstDuplicate [] ((values.map parts).map Prod.fst) = some asset) :
    basket.Basket.try_from_amounts equal hash numbers into source = .ok (.Err (.DuplicateAsset asset)) := by
  have called := entries_reject_prefix equal hash lawful numbers
    (core.iter.traits.collect.IntoIterator.Blanket (entriesIterator into.iteratorInst))
    iter iter after rfl (values.map parts) (prefix_parts into.iteratorInst iter after values observed) asset duplicate
  simpa [basket.Basket.try_from_amounts, started] using called

include lawful in
theorem amounts_failure (error : Error) (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List (amount.AssetAmount A N))
    (fails : FailsAfter into.iteratorInst.next error iter values)
    (unique : ((values.map parts).map Prod.fst).Nodup) :
    basket.Basket.try_from_amounts equal hash numbers into source = .fail error := by
  have called := entries_failure equal hash lawful numbers
    (core.iter.traits.collect.IntoIterator.Blanket (entriesIterator into.iteratorInst))
    error iter iter rfl (values.map parts) (failure_parts into.iteratorInst error iter values fails) unique
  simpa [basket.Basket.try_from_amounts, started] using called

omit [DecidableEq A] in
theorem amounts_start_failure (error : Error) (source : Source)
    (failed : into.into_iter source = .fail error) :
    basket.Basket.try_from_amounts equal hash numbers into source = .fail error := by
  simp [basket.Basket.try_from_amounts, failed]

end Public
end Axionomy.BasketAmountsRefinement
