import Axionomy.BasketPublicRefinement
import Axionomy.CanonicalSort

namespace Axionomy.BasketOrdering
open Aeneas.Std
variable {A N : Type}

def compare (cmp : A → A → Ordering) (left right : A × N) : Ordering :=
  cmp left.1 right.1

def sorted (cmp : A → A → Ordering) (entries : List (A × N)) : List (A × N) :=
  CanonicalSort.sort (compare cmp) entries

theorem permutation (cmp : A → A → Ordering) (entries : List (A × N)) :
    (sorted cmp entries).Perm entries := CanonicalSort.sort_perm _ _

theorem ordered (cmp : A → A → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (entries : List (A × N)) :
    (sorted cmp entries).Pairwise (fun left right => cmp left.1 right.1 ≠ .gt) := by
  let relation := CanonicalSort.before (compare (N := N) cmp)
  let : Std.Total relation := ⟨fun x y => CanonicalSort.before_total cmp laws x.1 y.1⟩
  let : IsTrans (A × N) relation := ⟨fun x y z => CanonicalSort.before_trans cmp laws x.1 y.1 z.1⟩
  exact List.pairwise_insertionSort relation entries

theorem unique_keys (cmp : A → A → Ordering) (entries : List (A × N))
    (unique : (entries.map Prod.fst).Nodup) :
    ((sorted cmp entries).map Prod.fst).Nodup :=
  ((permutation cmp entries).map Prod.fst).nodup_iff.mpr unique

theorem positive_contents (cmp : A → A → Ordering) (isZero : N → Bool)
    (entries : List (A × N)) (nonzero : ∀ entry ∈ entries, isZero entry.2 = false) :
    ∀ entry ∈ sorted cmp entries, isZero entry.2 = false := by
  intro entry present
  exact nonzero entry ((permutation cmp entries).mem_iff.mp present)

theorem strictly_ordered (cmp : A → A → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (entries : List (A × N)) (unique : (entries.map Prod.fst).Nodup) :
    OrderedEffects.Sorted cmp (sorted cmp entries) := by
  have distinct : (sorted cmp entries).Pairwise (fun left right => left.1 ≠ right.1) := by
    simpa only [List.nodup_iff_pairwise_ne, List.pairwise_map] using unique_keys cmp entries unique
  have both := (ordered cmp laws entries).and distinct
  unfold OrderedEffects.Sorted
  rw [List.pairwise_map]
  exact both.imp fun {left right} ⟨notGreater, different⟩ => by
    cases compared : cmp left.1 right.1 with
    | lt => rfl
    | eq => exact False.elim (different (laws.equal _ _ compared))
    | gt => exact False.elim (notGreater compared)

theorem ties_stable (cmp : A → A → Ordering) (entries : List (A × N))
    (left right : A × N) (equal : cmp left.1 right.1 = .eq)
    (original : [left, right].Sublist entries) :
    [left, right].Sublist (sorted cmp entries) := by
  exact List.pair_sublist_insertionSort
    (r := CanonicalSort.before (compare cmp)) (by simp [CanonicalSort.before, compare, equal]) original

theorem canonical (cmp : A → A → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (left right : List (A × N)) (same : left.Perm right)
    (unique : (left.map Prod.fst).Nodup) : sorted cmp left = sorted cmp right := by
  let relation := fun x y : A × N => cmp x.1 y.1 = .lt ∨ x = y
  let : Std.Antisymm relation := ⟨by
    intro x y forward backward
    rcases forward with less | equal
    · rcases backward with more | equal
      · have impossible := laws.transitive _ _ _ less more
        simp [laws.reflexive] at impossible
      · exact equal.symm
    · exact equal⟩
  have orderedLeft : (sorted cmp left).Pairwise relation := by
    have h := strictly_ordered cmp laws left unique
    rw [OrderedEffects.Sorted, List.pairwise_map] at h
    exact h.imp fun less => Or.inl less
  have orderedRight : (sorted cmp right).Pairwise relation := by
    have h := strictly_ordered cmp laws right ((same.map Prod.fst).nodup_iff.mp unique)
    rw [OrderedEffects.Sorted, List.pairwise_map] at h
    exact h.imp fun less => Or.inl less
  exact List.Perm.eq_of_pairwise' orderedLeft orderedRight
    ((permutation cmp left).trans (same.trans (permutation cmp right).symm))

end Axionomy.BasketOrdering

namespace Axionomy.BasketSortedRefinement
open Aeneas Aeneas.Std AccountExtracted RateConstructionRefinement IteratorContract
variable {T F I A N : Type}

theorem collect_loop (iterator : core.iter.traits.iterator.Iterator I T)
    (state : I) (values : List T) (yields : Yields iterator.next state values)
    (acc : List T) :
    alloc.vec.FromIteratorVec.iterToList iterator state acc = .ok (acc.reverse ++ values) := by
  induction yields generalizing acc with
  | done called =>
    rw [alloc.vec.FromIteratorVec.iterToList.eq_1]
    simp [called]
  | @step state after value rest called tail ih =>
    rw [alloc.vec.FromIteratorVec.iterToList.eq_1]
    simp [called, ih, List.append_assoc]

theorem insertion_state (callback : F → T × T → Result (Ordering × F))
    (cmp : T → T → Ordering)
    (lawful : ∀ state left right, callback state (left, right) = .ok (cmp left right, state))
    (value : T) (values : List T) (state : F) :
    orderedInsertState callback value values state =
      .ok (List.orderedInsert (CanonicalSort.before cmp) value values, state) := by
  induction values with
  | nil => rfl
  | cons head rest ih =>
    cases compared : cmp value head <;>
      simp [orderedInsertState, lawful, compared, List.orderedInsert, CanonicalSort.before, ih]

theorem sorting_state (callback : F → T × T → Result (Ordering × F))
    (cmp : T → T → Ordering)
    (lawful : ∀ state left right, callback state (left, right) = .ok (cmp left right, state))
    (values : List T) (state : F) :
    stableSortState callback values state = .ok (CanonicalSort.sort cmp values, state) := by
  induction values with
  | nil => rfl
  | cons head rest ih =>
    simp [stableSortState, ih, insertion_state callback cmp lawful, CanonicalSort.sort]

theorem owned_sort (callback : core.ops.function.FnMut F (T × T) Ordering)
    (cmp : T → T → Ordering)
    (lawful : ∀ state left right, callback.call_mut state (left, right) = .ok (cmp left right, state))
    (values : alloc.vec.Vec T) (state : F) :
    ∃ result, basket.sorted_entries callback values state = .ok result ∧
      result.val = CanonicalSort.sort cmp values.val := by
  have fits : (CanonicalSort.sort cmp values.val).length ≤ Usize.max := by
    rw [CanonicalSort.sort_length]; exact values.property
  refine ⟨alloc.vec.Vec.from _ fits, ?_, by simp⟩
  simp [basket.sorted_entries, alloc.vec.Vec.deref_mut, lift, Aeneas.Std.uncurry,
    alloc.slice.Slice.sort_by, sorting_state callback.call_mut cmp lawful,
    CanonicalSort.sort_length, alloc.vec.Vec.from, alloc.vec.Vec.val]

abbrev comparator (order : core.cmp.Ord A) :=
  basket.Basket.iter_sorted.closure.Insts.CoreOpsFunctionFnMutPairShared4PairShared0AShared1QuantityShared5PairShared2AShared3QuantityOrdering
    N order

theorem comparator_law (order : core.cmp.Ord A) (cmp : A → A → Ordering)
    (lawful : LawfulOrdering order cmp) (state : Unit) (left right : A × N) :
    (comparator (N := N) order).call_mut state (left, right) =
      .ok (BasketOrdering.compare cmp left right, state) := by
  cases state
  rcases left with ⟨left, value⟩
  rcases right with ⟨right, other⟩
  simp [
    basket.Basket.iter_sorted.closure.Insts.CoreOpsFunctionFnMutPairShared4PairShared0AShared1QuantityShared5PairShared2AShared3QuantityOrdering.call_mut,
    Shared0A.Insts.CoreCmpOrd.cmp, lawful left right, BasketOrdering.compare]

theorem sorted_refines (order : core.cmp.Ord A) (cmp : A → A → Ordering)
    (lawful : LawfulOrdering order cmp) (_laws : OrderedEffects.OrderLaws cmp)
    (state : basket.Basket A N) (capacity : state.quantities.length ≤ Usize.max) :
    ∃ result, basket.Basket.iter_sorted order state = .ok result ∧
      result.val = BasketOrdering.sorted cmp state.quantities ∧
      Yields alloc.vec.into_iter.IteratorIntoIter.next result (BasketOrdering.sorted cmp state.quantities) := by
  let iterator := indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV A N
  have collection : alloc.vec.FromIteratorVec.from_iter
      (core.iter.traits.collect.IntoIterator.Blanket iterator) state.quantities =
      .ok (alloc.vec.Vec.from state.quantities capacity) := by
    have yields := (BasketPublicRefinement.iteration_refines state).2
    have collected := collect_loop iterator state.quantities state.quantities yields []
    simp [alloc.vec.FromIteratorVec.from_iter,
      core.iter.traits.collect.IntoIterator.Blanket.into_iter, collected, capacity]
  obtain ⟨result, sortedCall, specified⟩ := owned_sort (comparator (N := N) order)
    (BasketOrdering.compare cmp) (comparator_law order cmp lawful)
    (alloc.vec.Vec.from state.quantities capacity) ()
  refine ⟨result, ?_, ?_, ?_⟩
  · simp only [basket.Basket.iter_sorted, basket.Basket.iter, indexmap.map.IndexMap.iter,
      bind_ok, alloc.vec.IntoIteratorVec.into_iter]
    change (do
      let entries ← alloc.vec.FromIteratorVec.from_iter
        (core.iter.traits.collect.IntoIterator.Blanket iterator) state.quantities
      let values ← basket.sorted_entries (comparator (N := N) order) entries ()
      Result.ok values) = Result.ok result
    rw [collection, bind_ok, sortedCall, bind_ok]
  · simpa [BasketOrdering.sorted] using specified
  · simpa [BasketOrdering.sorted, specified] using vector_yields result

theorem sorted_contract (order : core.cmp.Ord A) (cmp : A → A → Ordering)
    (lawful : LawfulOrdering order cmp) (laws : OrderedEffects.OrderLaws cmp)
    (state : basket.Basket A N) (capacity : state.quantities.length ≤ Usize.max) :
    ∃ result, basket.Basket.iter_sorted order state = .ok result ∧
      result.val.Perm state.quantities ∧
      result.val.Pairwise (fun left right => cmp left.1 right.1 ≠ .gt) ∧
      result.val.length = state.quantities.length := by
  obtain ⟨result, called, specified, _⟩ := sorted_refines order cmp lawful laws state capacity
  refine ⟨result, called, ?_, ?_, ?_⟩ <;> rw [specified]
  · exact BasketOrdering.permutation cmp _
  · exact BasketOrdering.ordered cmp laws _
  · exact (BasketOrdering.permutation cmp _).length_eq

theorem canonical_contract (order : core.cmp.Ord A) (cmp : A → A → Ordering)
    (lawful : LawfulOrdering order cmp) (laws : OrderedEffects.OrderLaws cmp)
    (state : basket.Basket A N) (capacity : state.quantities.length ≤ Usize.max)
    (unique : (state.quantities.map Prod.fst).Nodup)
    (isZero : N → Bool) (nonzero : ∀ entry ∈ state.quantities, isZero entry.2 = false) :
    ∃ result, basket.Basket.iter_sorted order state = .ok result ∧
      result.val.Perm state.quantities ∧ OrderedEffects.Sorted cmp result.val ∧
      (result.val.map Prod.fst).Nodup ∧
      (∀ entry ∈ result.val, isZero entry.2 = false) := by
  obtain ⟨result, called, specified, _⟩ := sorted_refines order cmp lawful laws state capacity
  refine ⟨result, called, ?_, ?_, ?_, ?_⟩ <;> rw [specified]
  · exact BasketOrdering.permutation cmp _
  · exact BasketOrdering.strictly_ordered cmp laws _ unique
  · exact BasketOrdering.unique_keys cmp _ unique
  · exact BasketOrdering.positive_contents cmp isZero _ nonzero

theorem constructed_sorted [DecidableEq A] {M Source Iter : Type}
    (equal : core.cmp.Eq A) (hash : core.hash.Hash A)
    (equality : AccountRefinement.LawfulEquality equal)
    (numbers : quantity.QuantityScalar N M) (isZero : N → Bool)
    (zeroLaw : ∀ value, numbers.num_traitsidentitiesZeroInst.is_zero value = .ok (isZero value))
    (into : core.iter.traits.collect.IntoIterator Source (A × N) Iter)
    (source : Source) (iter : Iter) (started : into.into_iter source = .ok iter)
    (values : List (A × N)) (yields : Yields into.iteratorInst.next iter values)
    (unique : (values.map Prod.fst).Nodup) (capacity : values.length ≤ Usize.max)
    (order : core.cmp.Ord A) (cmp : A → A → Ordering)
    (lawful : LawfulOrdering order cmp) (laws : OrderedEffects.OrderLaws cmp) :
    ∃ state result,
      basket.Basket.try_from_entries equal hash numbers into source = .ok (.Ok state) ∧
      basket.Basket.iter_sorted order state = .ok result ∧
      result.val = BasketOrdering.sorted cmp (BasketCollection.canonical isZero values) ∧
      OrderedEffects.Sorted cmp result.val ∧
      (result.val.map Prod.fst).Nodup ∧
      (∀ entry ∈ result.val, isZero entry.2 = false) := by
  obtain ⟨built, distinct, nonzero, fits⟩ := BasketCollectionRefinement.entries_contract
    equal hash equality numbers isZero zeroLaw into source iter started values yields unique capacity
  let state : basket.Basket A N := ⟨BasketCollection.canonical isZero values⟩
  obtain ⟨result, called, specified, _⟩ := sorted_refines order cmp lawful laws state fits
  refine ⟨state, result, built, called, specified, ?_, ?_, ?_⟩ <;> rw [specified]
  · exact BasketOrdering.strictly_ordered cmp laws _ distinct
  · exact BasketOrdering.unique_keys cmp _ distinct
  · exact BasketOrdering.positive_contents cmp isZero _ nonzero

end Axionomy.BasketSortedRefinement
