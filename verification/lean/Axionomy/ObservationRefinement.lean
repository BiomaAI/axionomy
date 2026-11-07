import Axionomy.Observations
import Axionomy.BasketOperationsRefinement
import Axionomy.EvaluationRefinement
import Axionomy.CommitRefinement
import Axionomy.TransactionDecode

namespace Axionomy.ObservationRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering)
variable (hAccountEq : LawfulEquality accountEq) (hAssetEq : LawfulEquality assetEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount)

omit [DecidableEq A] in
include hAccountEq in
theorem model_account (model : economy.Economy AccountId A RateId Role Std.U64) (id : AccountId) :
    economy.Economy.account accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model id = .ok (Account.find? id model.accounts) := by
  simp only [economy.Economy.account, indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccountEq, bind_ok]
  cases Account.find? id model.accounts <;> simp [alloc.sync.Arc.Insts.CoreConvertAsRef.as_ref]

include hAccountEq hAssetEq in
theorem model_balance (model : economy.Economy AccountId A RateId Role Std.U64) (id : AccountId) (asset : A) :
    economy.Economy.balance accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model id asset =
        .ok (((Account.find? id model.accounts).map (fun holder => Account.balance arithmetic.zero holder.balances.quantities asset)).getD arithmetic.zero) := by
  simp only [economy.Economy.balance, indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccountEq, bind_ok]
  cases Account.find? id model.accounts <;>
    simp [alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, account.Account.balance, quantity_call assetEq assetHash hAssetEq,
      quantity.Quantity.Insts.CoreDefaultDefault.default, U64.Insts.Num_traitsIdentitiesZero.zero, arithmetic]

omit [DecidableEq A] in
include hAccountOrd in
theorem visible_contains (laws : OrderedEffects.OrderLaws cmpAccount) (visible : List AccountId)
    (sorted : visible.Pairwise (fun left right => cmpAccount left right = .lt)) (id : AccountId) :
    alloc.collections.btree.set.BTreeSet.contains alloc.alloc.Global.Insts.CoreAllocAllocatorClone
      (core.borrow.Borrow.Blanket AccountId) accountOrd accountOrd visible id = .ok (decide (id ∈ visible)) := by
  have canonical : OrderedEffects.Sorted cmpAccount (visible.map fun key => (key, ())) := by
    simpa [OrderedEffects.Sorted, List.map_map, Function.comp_def] using sorted
  simp only [alloc.collections.btree.set.BTreeSet.contains, alloc.collections.btree.map.BTreeMap.contains_key,
    alloc.collections.btree.map.BTreeMap.get, EvaluationRefinement.tree_get_exact accountOrd cmpAccount hAccountOrd,
    Observations.ordered_find cmpAccount laws id _ canonical, bind_ok]
  have found := Transaction.find_exists_iff_key_mem id (visible.map fun key => (key, ()))
  cases entry : Account.find? id (visible.map fun key => (key, ())) with
  | none =>
    have absent : id ∉ visible := by simpa [entry] using found.not
    simp [absent]
  | some value =>
    have present : id ∈ visible := by simpa using found.mp ⟨value, entry⟩
    simp [present]

omit [DecidableEq A] in
include hAccountEq hAccountOrd in
theorem view_account (laws : OrderedEffects.OrderLaws cmpAccount)
    (view : economy.EconomicView AccountId A RateId Role Std.U64)
    (sorted : view.visible.Pairwise (fun left right => cmpAccount left right = .lt)) (id : AccountId) :
    economy.EconomicView.account accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar view id =
        .ok (if id ∈ view.visible then Account.find? id view.economy.accounts else none) := by
  by_cases shown : id ∈ view.visible <;>
    simp [economy.EconomicView.account, shown, visible_contains accountOrd cmpAccount hAccountOrd laws view.visible sorted id,
    model_account accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd hAccountEq]

include hAccountEq hAccountOrd hAssetEq in
theorem view_balance (laws : OrderedEffects.OrderLaws cmpAccount)
    (view : economy.EconomicView AccountId A RateId Role Std.U64)
    (sorted : view.visible.Pairwise (fun left right => cmpAccount left right = .lt)) (id : AccountId) (asset : A) :
    economy.EconomicView.balance accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar view id asset =
        .ok ((if id ∈ view.visible then Account.find? id view.economy.accounts else none).map
          (fun holder => Account.balance arithmetic.zero holder.balances.quantities asset)) := by
  simp only [economy.EconomicView.balance, view_account accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount hAccountEq hAccountOrd laws view sorted id, bind_ok]
  by_cases shown : id ∈ view.visible <;> cases Account.find? id view.economy.accounts <;>
    simp [shown, account.Account.balance, quantity_call assetEq assetHash hAssetEq]

end Axionomy.ObservationRefinement
