import Axionomy.BasketAmountsRefinement

namespace Axionomy.BasketPublicRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement IteratorContract BasketCollectionRefinement
variable {A N M : Type}

theorem new_empty : basket.Basket.new A N = .ok ⟨[]⟩ := by
  simp [basket.Basket.new, indexmap.map.IndexMapKVRandomState.new]

theorem default_empty : basket.Basket.Insts.CoreDefaultDefault.default A N = .ok ⟨[]⟩ := by
  simp [basket.Basket.Insts.CoreDefaultDefault.default, new_empty]

theorem empty_refines (state : basket.Basket A N) :
    basket.Basket.is_empty state = .ok (decide (state.quantities = [])) := by
  cases state with
  | mk entries => cases entries <;> rfl

theorem length_refines (state : basket.Basket A N)
    (capacity : state.quantities.length < 2^UScalarTy.Usize.numBits) :
    ∃ value, basket.Basket.len state = .ok value ∧ value.val = state.quantities.length := by
  refine ⟨Usize.ofNatCore state.quantities.length capacity, ?_, rfl⟩
  simp only [basket.Basket.len, indexmap.map.IndexMap.len, capacity, ↓reduceDIte]

theorem lookup_refines [DecidableEq A] (equal : core.cmp.Eq A) (hash : core.hash.Hash A)
    (lawful : LawfulEquality equal) (numbers : quantity.QuantityScalar N M)
    (state : basket.Basket A N) (asset : A) :
    basket.Basket.get equal hash numbers state asset = .ok (Account.find? asset state.quantities) := by
  simp [basket.Basket.get, indexmap.map.IndexMap.get, equivalent_eq, get_exact equal lawful]

theorem iteration_refines (state : basket.Basket A N) :
    basket.Basket.iter state = .ok state.quantities ∧
      Yields indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next
        state.quantities state.quantities := by
  refine ⟨rfl, ?_⟩
  generalize state.quantities = entries
  induction entries with
  | nil => exact .done rfl
  | cons entry rest ih => exact .step rfl ih

theorem array_yields {T : Type} (length : Usize) (entries : core.array.iter.IntoIter T length) :
    Yields core.array.iter.IntoIter.Insts.CoreIterTraitsIteratorIterator.next entries entries := by
  induction entries with
  | nil => exact .done rfl
  | cons entry rest ih => exact .step rfl ih

theorem array_refines [DecidableEq A] {length : Usize}
    (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (lawful : LawfulEquality equal)
    (numbers : quantity.QuantityScalar N M) (isZero : N → Bool)
    (zeroLaw : ∀ value, numbers.num_traitsidentitiesZeroInst.is_zero value = .ok (isZero value))
    (entries : Array (A × N) length) :
    basket.Basket.Insts.CoreConvertFromArrayPairAQuantityLEN.from equal hash numbers entries =
      collected (BasketCollection.construct isZero [] entries.val) := by
  exact collect_refines equal hash lawful numbers isZero zeroLaw
    (Array.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter _ length)
    entries entries.val rfl entries.val (array_yields length entries.val)

theorem array_panics_iff [DecidableEq A] {length : Usize}
    (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (lawful : LawfulEquality equal)
    (numbers : quantity.QuantityScalar N M) (isZero : N → Bool)
    (zeroLaw : ∀ value, numbers.num_traitsidentitiesZeroInst.is_zero value = .ok (isZero value))
    (entries : Array (A × N) length) :
    basket.Basket.Insts.CoreConvertFromArrayPairAQuantityLEN.from equal hash numbers entries = .fail .panic ↔
      ¬ (entries.val.map Prod.fst).Nodup := by
  rw [array_refines equal hash lawful numbers isZero zeroLaw]
  exact collected_panics_iff isZero entries.val

theorem u64_zero_law (value : Std.U64) :
    scalar.num_traitsidentitiesZeroInst.is_zero value = .ok (decide (value = 0#u64)) := rfl

def naturalResult : core.result.Result (basket.Basket A Std.U64) (basket.BasketError A) →
    Except A (List (A × Nat))
  | .Ok state => .ok (BasketRefinement.decodeEntries state.quantities)
  | .Err (.DuplicateAsset asset) => .error asset

theorem u64_entries_refines [DecidableEq A] {Source Iter : Type}
    (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (lawful : LawfulEquality equal)
    (into : core.iter.traits.collect.IntoIterator Source (A × Std.U64) Iter)
    (source : Source) (iter : Iter) (started : into.into_iter source = .ok iter)
    (values : List (A × Std.U64)) (yields : Yields into.iteratorInst.next iter values) :
    ∃ result, basket.Basket.try_from_entries equal hash scalar into source = .ok result ∧
      naturalResult result = Basket.construct (BasketRefinement.decodeEntries values) := by
  refine ⟨encode (BasketCollection.construct (fun value => decide (value = 0#u64)) [] values),
    entries_refines equal hash lawful scalar _ u64_zero_law into source iter started values yields, ?_⟩
  have keys : (BasketRefinement.decodeEntries values).map Prod.fst = values.map Prod.fst := by
    simp [BasketRefinement.decodeEntries, List.map_map]
  simp only [BasketCollection.construct, Basket.construct, List.map_nil, List.nil_append, keys]
  cases Basket.firstDuplicate [] (values.map Prod.fst) <;>
    simp [naturalResult, encode, BasketCollection.canonical, BasketRefinement.decode_canonical]

end Axionomy.BasketPublicRefinement
