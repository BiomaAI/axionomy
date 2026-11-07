import Axionomy.ObservationRefinement

namespace Axionomy.ViewConstructionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}
variable (accountClone : core.clone.Clone AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (cmp : AccountId → AccountId → Ordering) (lawful : LawfulOrdering accountOrd cmp)

include lawful in
theorem view_loop (remaining : List AccountId) (iter : alloc.vec.into_iter.IntoIter AccountId)
    (contents : iter.val = remaining) (visible : List AccountId) :
    economy.Economy.view_loop accountClone accountOrd (core.iter.traits.collect.IntoIteratorVec AccountId) iter visible =
      .ok (Discovery.registerAll cmp remaining visible) := by
  induction remaining generalizing iter visible with
  | nil =>
    unfold economy.Economy.view_loop
    rw [loop.eq_1]
    simp [economy.Economy.view_loop.body, Aeneas.Std.uncurry, CommitRefinement.owning_next_none iter contents, Discovery.registerAll]
  | cons id rest ih =>
    obtain ⟨next, advanced, tail⟩ := CommitRefinement.owning_next_cons iter id rest contents
    obtain ⟨inserted, registered⟩ := register_exact accountOrd cmp lawful id visible
    unfold economy.Economy.view_loop
    rw [loop.eq_1]
    simp only [economy.Economy.view_loop.body, Aeneas.Std.uncurry, advanced, bind_ok,
      alloc.collections.btree.set.BTreeSet.insert, registered]
    change economy.Economy.view_loop accountClone accountOrd (core.iter.traits.collect.IntoIteratorVec AccountId)
      next (RateConstruction.register cmp id visible) = _
    simpa only [Discovery.registerAll, List.foldl_cons] using ih next tail (RateConstruction.register cmp id visible)

include lawful in
theorem model_view
    (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
    (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
    (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
    (model : economy.Economy AccountId A RateId Role Std.U64) (visible : alloc.vec.Vec AccountId) :
    economy.Economy.view accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      (core.iter.traits.collect.IntoIteratorVec AccountId) model visible =
        .ok ⟨model, Discovery.registerAll cmp visible.val []⟩ := by
  have called := view_loop accountClone accountOrd cmp lawful visible.val visible rfl []
  unfold economy.Economy.view
  simp only [alloc.collections.btree.set.BTreeSetTGlobal.new, bind_ok]
  simp only [core.iter.traits.collect.IntoIteratorVec, alloc.vec.IntoIteratorVec.into_iter] at called ⊢
  dsimp only [alloc.vec.into_iter.IntoIter] at called ⊢
  rw [bind_ok, called, bind_ok]

end Axionomy.ViewConstructionRefinement
