import Axionomy.ViewConstructionRefinement
import Axionomy.Observations
import Axionomy.IteratorContract

namespace Axionomy.IteratorViewRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement IteratorContract

variable {AccountId A RateId Role N Measure Source Iter : Type}
variable (accountClone : core.clone.Clone AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (cmp : AccountId → AccountId → Ordering) (lawful : LawfulOrdering accountOrd cmp)
variable (into : core.iter.traits.collect.IntoIterator Source AccountId Iter)

include lawful in
theorem view_loop (iter : Iter) (values : List AccountId)
    (yields : Yields into.iteratorInst.next iter values) (visible : List AccountId) :
    economy.Economy.view_loop accountClone accountOrd into iter visible =
      .ok (Discovery.registerAll cmp values visible) := by
  induction yields generalizing visible with
  | done called =>
    unfold economy.Economy.view_loop
    rw [loop.eq_1]
    simp [economy.Economy.view_loop.body, called, Discovery.registerAll]
  | @step state after value rest called tail ih =>
    obtain ⟨inserted, registered⟩ := register_exact accountOrd cmp lawful value visible
    unfold economy.Economy.view_loop
    rw [loop.eq_1]
    simp only [economy.Economy.view_loop.body, Aeneas.Std.uncurry, called, bind_ok,
      alloc.collections.btree.set.BTreeSet.insert, registered]
    change economy.Economy.view_loop accountClone accountOrd into after
      (RateConstruction.register cmp value visible) = _
    simpa only [Discovery.registerAll, List.foldl_cons] using ih (RateConstruction.register cmp value visible)

include lawful in
theorem view_loop_failure (error : Error) (iter : Iter) (values : List AccountId)
    (fails : FailsAfter into.iteratorInst.next error iter values) (visible : List AccountId) :
    economy.Economy.view_loop accountClone accountOrd into iter visible = .fail error := by
  induction fails generalizing visible with
  | stopped called =>
    unfold economy.Economy.view_loop
    rw [loop.eq_1]
    simp [economy.Economy.view_loop.body, called]
  | @step state after value rest called tail ih =>
    obtain ⟨inserted, registered⟩ := register_exact accountOrd cmp lawful value visible
    unfold economy.Economy.view_loop
    rw [loop.eq_1]
    simp only [economy.Economy.view_loop.body, Aeneas.Std.uncurry, called, bind_ok,
      alloc.collections.btree.set.BTreeSet.insert, registered]
    exact ih (RateConstruction.register cmp value visible)

variable (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (numbers : quantity.QuantityScalar N Measure)

include lawful in
theorem model_view (model : economy.Economy AccountId A RateId Role N) (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List AccountId)
    (yields : Yields into.iteratorInst.next iter values) :
    economy.Economy.view accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd numbers into model source =
        .ok ⟨model, Discovery.registerAll cmp values []⟩ := by
  simp [economy.Economy.view, alloc.collections.btree.set.BTreeSetTGlobal.new,
    started, view_loop accountClone accountOrd cmp lawful into iter values yields []]

include lawful in
theorem model_view_contract (laws : OrderedEffects.OrderLaws cmp)
    (model : economy.Economy AccountId A RateId Role N) (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter) (values : List AccountId)
    (yields : Yields into.iteratorInst.next iter values) (capacity : values.length ≤ Usize.max) :
    ∃ result, economy.Economy.view accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd numbers into model source = .ok result ∧
      result.economy = model ∧ result.visible = Discovery.registerAll cmp values [] ∧
      result.visible.Pairwise (fun x y => cmp x y = .lt) ∧ result.visible.Nodup ∧
      result.visible.length ≤ Usize.max := by
  refine ⟨⟨model, Discovery.registerAll cmp values []⟩,
    model_view accountClone accountOrd cmp lawful into accountEq accountHash assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd numbers model source iter started values yields,
    rfl, rfl, Observations.registerAll_sorted cmp laws values [] (by simp),
    Observations.registerAll_unique cmp laws values, ?_⟩
  have bound := Discovery.registerAll_length_le cmp values []
  simp only [List.length_nil, Nat.zero_add] at bound
  exact Nat.le_trans bound capacity

include lawful in
theorem model_view_failure (error : Error) (model : economy.Economy AccountId A RateId Role N)
    (source : Source) (iter : Iter) (started : into.into_iter source = .ok iter)
    (values : List AccountId) (fails : FailsAfter into.iteratorInst.next error iter values) :
    economy.Economy.view accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd numbers into model source = .fail error := by
  simp [economy.Economy.view, alloc.collections.btree.set.BTreeSetTGlobal.new,
    started, view_loop_failure accountClone accountOrd cmp lawful into error iter values fails []]

theorem model_view_start_failure (error : Error) (model : economy.Economy AccountId A RateId Role N)
    (source : Source) (failed : into.into_iter source = .fail error) :
    economy.Economy.view accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd numbers into model source = .fail error := by
  simp [economy.Economy.view, alloc.collections.btree.set.BTreeSetTGlobal.new, failed]

end Axionomy.IteratorViewRefinement
