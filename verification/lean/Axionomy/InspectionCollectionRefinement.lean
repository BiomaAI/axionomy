import Axionomy.InspectionAssetsRefinement

namespace Axionomy.InspectionCollectionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement
open InspectionAssetsRefinement
variable {AccountId Role A RateId N M : Type}

section Entries
variable (assetOrd : core.cmp.Ord A) (cmp : A → A → Ordering)
  (lawful : LawfulOrdering assetOrd cmp)

include lawful in
theorem shared_order : LawfulOrdering (Shared0A.Insts.CoreCmpOrd assetOrd) cmp := by
  intro left right
  simpa [Shared0A.Insts.CoreCmpOrd.cmp] using lawful left right

include lawful in
theorem basket_loop (remaining : List (A × N)) (collected : List A) :
    economy.collect_basket_assets_loop assetOrd remaining collected =
      .ok (Discovery.registerAll cmp (remaining.map Prod.fst) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold economy.collect_basket_assets_loop
    rw [loop.eq_1]
    simp [economy.collect_basket_assets_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, Discovery.registerAll]
  | cons entry rest ih =>
    obtain ⟨inserted, registered⟩ := register_exact _ cmp (shared_order assetOrd cmp lawful) entry.1 collected
    unfold economy.collect_basket_assets_loop
    rw [loop.eq_1]
    simp only [economy.collect_basket_assets_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
      bind_ok, Aeneas.Std.uncurry, alloc.collections.btree.set.BTreeSet.insert, registered]
    change economy.collect_basket_assets_loop assetOrd rest (RateConstruction.register cmp entry.1 collected) = _
    simpa [Discovery.registerAll] using ih (RateConstruction.register cmp entry.1 collected)

include lawful in
theorem basket_assets (contents : basket.Basket A N) (collected : List A) :
    economy.collect_basket_assets assetOrd contents collected =
      .ok (Discovery.registerAll cmp (contents.quantities.map Prod.fst) collected) := by
  simp [economy.collect_basket_assets, basket.Basket.iter, indexmap.map.IndexMap.iter,
    basket_loop assetOrd cmp lawful]

include lawful in
theorem optional_loop (remaining : List (A × N)) (collected : List A) :
    rate.Rate.collect_optional_assets_loop assetOrd remaining collected =
      .ok (Discovery.registerAll cmp (remaining.map Prod.fst) collected) := by
  simpa only [rate.Rate.collect_optional_assets_loop, rate.Rate.collect_optional_assets_loop.body,
    economy.collect_basket_assets_loop, economy.collect_basket_assets_loop.body] using
    basket_loop assetOrd cmp lawful remaining collected

include lawful in
theorem invariant_loop (remaining : List (A × Std.I64)) (collected : List A) :
    economy.collect_invariant_assets_loop assetOrd collected remaining =
      .ok (Discovery.registerAll cmp (remaining.map Prod.fst) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold economy.collect_invariant_assets_loop
    rw [loop.eq_1]
    simp [economy.collect_invariant_assets_loop.body, InspectionRefinement.weight_next, Discovery.registerAll]
  | cons entry rest ih =>
    obtain ⟨inserted, registered⟩ := register_exact _ cmp (shared_order assetOrd cmp lawful) entry.1 collected
    unfold economy.collect_invariant_assets_loop
    rw [loop.eq_1]
    simp only [economy.collect_invariant_assets_loop.body, InspectionRefinement.weight_next,
      List.head?_cons, List.tail_cons, bind_ok, Aeneas.Std.uncurry,
      alloc.collections.btree.set.BTreeSet.insert, registered]
    simpa [economy.collect_invariant_assets_loop, economy.collect_invariant_assets_loop.body,
      InspectionRefinement.weight_next, alloc.collections.btree.set.BTreeSet.insert,
      Aeneas.Std.uncurry, Discovery.registerAll] using ih (RateConstruction.register cmp entry.1 collected)

include lawful in
theorem invariant_assets (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A)
    (invariant : rate.LinearInvariant A) (collected : List A) :
    economy.collect_invariant_assets eqA hashA assetOrd invariant collected =
      .ok (Discovery.registerAll cmp (invariant.weights.map Prod.fst) collected) := by
  simp [economy.collect_invariant_assets, InspectionRefinement.invariant_weights,
    invariant_loop assetOrd cmp lawful]

include lawful in
theorem computed_loop (remaining : List A) (iter : alloc.vec.into_iter.IntoIter A)
    (suffix : iter.val = remaining) (collected : List A) :
    rate.Rate.collect_assets_loop1 assetOrd iter collected =
      .ok (Discovery.registerAll cmp remaining collected) := by
  induction remaining generalizing iter collected with
  | nil =>
    unfold rate.Rate.collect_assets_loop1
    rw [loop.eq_1]
    simp [rate.Rate.collect_assets_loop1.body, CommitRefinement.owning_next_none iter suffix,
      Discovery.registerAll]
  | cons asset rest ih =>
    obtain ⟨after, advanced, tail⟩ := CommitRefinement.owning_next_cons iter asset rest suffix
    obtain ⟨inserted, registered⟩ := register_exact _ cmp (shared_order assetOrd cmp lawful) asset collected
    unfold rate.Rate.collect_assets_loop1
    rw [loop.eq_1]
    simp only [rate.Rate.collect_assets_loop1.body, advanced, bind_ok, Aeneas.Std.uncurry,
      alloc.collections.btree.set.BTreeSet.insert, registered]
    change rate.Rate.collect_assets_loop1 assetOrd after (RateConstruction.register cmp asset collected) = _
    simpa [Discovery.registerAll] using ih after tail (RateConstruction.register cmp asset collected)

end Entries

def optionalAssets (contents : Option (basket.Basket A N)) : List A :=
  contents.toList.flatMap (fun entries => entries.quantities.map Prod.fst)

def roleAssets (cmp : Role → Role → Ordering)
    (consume produce preserve : List (Role × basket.Basket A N)) (role : Role) : List A :=
  optionalAssets (Evaluation.orderedFind cmp role consume) ++
  optionalAssets (Evaluation.orderedFind cmp role produce) ++
  optionalAssets (Evaluation.orderedFind cmp role preserve)

def rateReferences (cmp : Role → Role → Ordering) (rule : rate.Rate Role A N) : List A :=
  rule.roles.flatMap (roleAssets cmp rule.consume rule.produce rule.preserve) ++ computedReferences rule

section Rates
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
  (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
  (scalar : quantity.QuantityScalar N M) (assetOrd : core.cmp.Ord A)
  (cmpA : A → A → Ordering) (lawA : LawfulOrdering assetOrd cmpA)

include lawA in
theorem optional_assets (contents : Option (basket.Basket A N)) (collected : List A) :
    rate.Rate.collect_optional_assets roleClone roleOrd assetClone assetEq assetHash scalar assetOrd contents collected =
      .ok (Discovery.registerAll cmpA (optionalAssets contents) collected) := by
  cases contents <;> simp [rate.Rate.collect_optional_assets, optionalAssets, Discovery.registerAll,
    basket.Basket.iter, indexmap.map.IndexMap.iter, optional_loop assetOrd cmpA lawA]

variable (cmpR : Role → Role → Ordering) (lawR : LawfulOrdering roleOrd cmpR)

include lawA lawR in
theorem roles_loop (remaining : List Role)
    (consume produce preserve : List (Role × basket.Basket A N)) (collected : List A) :
    rate.Rate.collect_assets_loop0 roleClone roleOrd assetClone assetEq assetHash scalar assetOrd
      remaining consume produce preserve collected =
      .ok (Discovery.registerAll cmpA (remaining.flatMap (roleAssets cmpR consume produce preserve)) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold rate.Rate.collect_assets_loop0
    rw [loop.eq_1]
    simp [rate.Rate.collect_assets_loop0.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next, Discovery.registerAll]
  | cons role rest ih =>
    have body : rate.Rate.collect_assets_loop0.body roleClone roleOrd assetClone assetEq assetHash scalar assetOrd
        consume produce preserve (role :: rest) collected =
        .ok (.cont (rest, Discovery.registerAll cmpA (roleAssets cmpR consume produce preserve role) collected)) := by
      simp [rate.Rate.collect_assets_loop0.body,
        alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
        alloc.collections.btree.map.BTreeMap.get, EvaluationRefinement.tree_get_exact roleOrd cmpR lawR,
        optional_assets roleClone roleOrd assetClone assetEq assetHash scalar assetOrd cmpA lawA,
        roleAssets, Discovery.registerAll_append]
    unfold rate.Rate.collect_assets_loop0
    rw [loop.eq_1]
    simp only [body, bind_ok]
    change rate.Rate.collect_assets_loop0 roleClone roleOrd assetClone assetEq assetHash scalar assetOrd
      rest consume produce preserve (Discovery.registerAll cmpA (roleAssets cmpR consume produce preserve role) collected) = _
    simpa [Discovery.registerAll_append] using
      ih (Discovery.registerAll cmpA (roleAssets cmpR consume produce preserve role) collected)

include lawA lawR in
theorem rate_assets (rule : rate.Rate Role A N)
    (fits : (computedReferences rule).length ≤ Usize.max) (collected : List A) :
    rate.Rate.collect_assets roleClone roleOrd assetClone assetEq assetHash scalar assetOrd rule collected =
      .ok (Discovery.registerAll cmpA (rateReferences cmpR rule) collected) := by
  obtain ⟨computed, called, references⟩ := computed_assets roleClone roleOrd assetClone assetEq assetHash scalar rule fits
  simp only [rate.Rate.collect_assets, SharedABTreeSet.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter,
    roles_loop roleClone roleOrd assetClone assetEq assetHash scalar assetOrd cmpA lawA cmpR lawR,
    called, alloc.vec.IntoIteratorVec.into_iter, alloc.vec.into_iter.IntoIter, bind_ok,
    computed_loop assetOrd cmpA lawA _ computed references,
    rateReferences, Discovery.registerAll_append]

end Rates
end Axionomy.InspectionCollectionRefinement
