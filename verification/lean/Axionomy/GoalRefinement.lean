import Axionomy.ObservationRefinement
import Axionomy.DiscoveryRefinement

namespace Axionomy.GoalRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

theorem new_goal (ordInst : core.cmp.Ord AccountId) :
    economy.Goal.new A Std.U64 ordInst = .ok ⟨[]⟩ := by
  simp [economy.Goal.new, alloc.collections.btree.map.BTreeMapKVGlobal.new]

theorem default_goal (ordInst : core.cmp.Ord AccountId) :
    economy.Goal.Insts.CoreDefaultDefault.default A Std.U64 ordInst = .ok ⟨[]⟩ := by
  simp [economy.Goal.Insts.CoreDefaultDefault.default, new_goal]

theorem requirements (ordInst : core.cmp.Ord AccountId) (goal : economy.Goal AccountId A Std.U64) :
    economy.Goal.requirements ordInst goal = .ok goal.required := rfl

theorem require_goal (ordInst : core.cmp.Ord AccountId) (cmp : AccountId → AccountId → Ordering)
    (lawful : LawfulOrdering ordInst cmp) (laws : OrderedEffects.OrderLaws cmp)
    (goal : economy.Goal AccountId A Std.U64) (id : AccountId) (assets : basket.Basket A Std.U64) :
    economy.Goal.require ordInst goal id assets =
      .ok ⟨(RateConstruction.locate cmp id goal.required).put assets⟩ := by
  have same := RateConstruction.locate_key cmp laws.equal id goal.required
  simp only [economy.Goal.require, alloc.collections.btree.map.BTreeMap.insert,
    locate_exact ordInst cmp lawful, bind_ok]
  cases found : (RateConstruction.locate cmp id goal.required).old <;>
    simp [encodePosition, found, RateConstruction.Position.put, same]

section Matching
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (hAccount : LawfulEquality accountEq) (hClone : LawfulClone assetClone) (hAsset : LawfulEquality assetEq)

include hClone hAsset in
theorem basket_meets (available required : basket.Basket A Std.U64)
    (unique : (required.quantities.map Prod.fst).Nodup) :
    basket.Basket.contains assetClone assetEq assetHash scalar available required =
      .ok (Observations.meets (TransactionDecode.balances available) (TransactionDecode.balances required)) := by
  have refines := BasketOperationsRefinement.contains_refines assetClone assetEq assetHash hClone hAsset
    available.quantities required.quantities unique
  change basket.Basket.contains _ _ _ _ _ _ = _ at refines
  rw [refines]
  congr 1
  apply Bool.eq_iff_iff.mpr
  simp only [List.isEmpty_iff, Account.shortfall_empty_iff, Observations.meets, List.all_eq_true, decide_eq_true_eq]
  simp only [TransactionDecode.balances, Account.mapEntries, List.mem_map, forall_exists_index, and_imp]
  have balances (asset : A) : Account.balance 0 (available.quantities.map fun entry => (entry.1, entry.2.val)) asset =
      (Account.balance arithmetic.zero available.quantities asset).val := by
    exact (Account.map_balance UScalar.val arithmetic.zero available.quantities asset).symm
  constructor
  · intro enough amount original member same
    subst amount
    rw [balances]
    simpa [arithmetic] using enough original member
  · intro enough amount member
    have sufficient := enough (amount.1, amount.2.val) amount member rfl
    rw [balances] at sufficient
    simpa [arithmetic] using sufficient

include hAccount hClone hAsset in
theorem matches_body (model : economy.Economy AccountId A RateId Role Std.U64)
    (remaining : List (AccountId × basket.Basket A Std.U64))
    (unique : ∀ entry ∈ remaining, (entry.2.quantities.map Prod.fst).Nodup) :
    economy.Economy.matches_loop.body accountEq accountHash assetClone assetEq assetHash scalar model remaining =
      .ok (match remaining with
        | [] => .done true
        | (id, required) :: rest => match Account.find? id model.accounts with
          | none => .done false
          | some holder => if Observations.meets (TransactionDecode.balances holder.balances)
              (TransactionDecode.balances required) then .cont rest else .done false) := by
  cases remaining with
  | nil => simp [economy.Economy.matches_loop.body,
      alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨id, required⟩
    simp only [economy.Economy.matches_loop.body,
      alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, bind_ok, Aeneas.Std.uncurry,
      indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccount]
    cases found : Account.find? id model.accounts with
    | none => simp
    | some holder =>
      simp only [bind_ok, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, account.Account.impl.balances]
      rw [basket_meets assetClone assetEq assetHash hClone hAsset holder.balances required (unique (id, required) (by simp))]
      simp only [bind_ok]
      split <;> rfl

include hAccount hClone hAsset in
theorem matches_loop (model : economy.Economy AccountId A RateId Role Std.U64)
    (remaining : List (AccountId × basket.Basket A Std.U64))
    (unique : ∀ entry ∈ remaining, (entry.2.quantities.map Prod.fst).Nodup) :
    economy.Economy.matches_loop accountEq accountHash assetClone assetEq assetHash scalar remaining model =
      .ok (Observations.satisfies (TransactionDecode.accounts model.accounts)
        (Account.mapEntries TransactionDecode.balances remaining)) := by
  induction remaining with
  | nil =>
    unfold economy.Economy.matches_loop
    rw [loop.eq_1]
    simp [matches_body accountEq accountHash assetClone assetEq assetHash hAccount hClone hAsset model [] unique,
      Observations.satisfies]
  | cons entry rest ih =>
    rcases entry with ⟨id, required⟩
    unfold economy.Economy.matches_loop
    rw [loop.eq_1, matches_body accountEq accountHash assetClone assetEq assetHash hAccount hClone hAsset model _ unique]
    simp only [Account.mapEntries_cons, Observations.satisfies, TransactionDecode.accounts, ← Account.map_find]
    cases found : Account.find? id model.accounts with
    | none => simp
    | some holder =>
      simp only [Option.map_some, TransactionDecode.holder]
      cases enough : Observations.meets (TransactionDecode.balances holder.balances) (TransactionDecode.balances required) with
      | false => simp
      | true =>
        simp only [ ↓reduceIte, bind_ok, Bool.true_and]
        change economy.Economy.matches_loop accountEq accountHash assetClone assetEq assetHash scalar rest model = _
        exact ih (fun item member => unique item (by simp [member]))

include hAccount hClone hAsset in
theorem matches_goal
    (accountClone : core.clone.Clone AccountId) (accountOrd : core.cmp.Ord AccountId) (assetOrd : core.cmp.Ord A)
    (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
    (model : economy.Economy AccountId A RateId Role Std.U64) (goal : economy.Goal AccountId A Std.U64)
    (unique : ∀ entry ∈ goal.required, (entry.2.quantities.map Prod.fst).Nodup) :
    economy.Economy.matches accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model goal =
        .ok (Observations.satisfies (TransactionDecode.accounts model.accounts)
          (Account.mapEntries TransactionDecode.balances goal.required)) := by
  simp [economy.Economy.matches, economy.Goal.requirements,
    SharedABTreeMap.Insts.CoreIterTraitsCollectIntoIteratorPairSharedAKSharedAVIter.into_iter,
    matches_loop accountEq accountHash assetClone assetEq assetHash hAccount hClone hAsset model goal.required unique]

end Matching
section Assets
variable (accountOrd : core.cmp.Ord AccountId) (assetOrd : core.cmp.Ord A)
variable (cmp : A → A → Ordering) (lawful : LawfulOrdering assetOrd cmp)

include lawful in
theorem collect_assets_loop (remaining : List (A × Std.U64)) (collected : List A) :
    economy.Goal.collect_assets_loop assetOrd remaining collected =
      .ok (Discovery.registerAll cmp (remaining.map Prod.fst) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold economy.Goal.collect_assets_loop
    rw [loop.eq_1]
    simp [economy.Goal.collect_assets_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, Discovery.registerAll]
  | cons entry rest ih =>
    have shared : LawfulOrdering (Shared0A.Insts.CoreCmpOrd assetOrd) cmp := by
      intro left right
      simpa [Shared0A.Insts.CoreCmpOrd.cmp] using lawful left right
    obtain ⟨inserted, registered⟩ := register_exact _ cmp shared entry.1 collected
    unfold economy.Goal.collect_assets_loop
    rw [loop.eq_1]
    simp only [economy.Goal.collect_assets_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
      bind_ok, Aeneas.Std.uncurry, alloc.collections.btree.set.BTreeSet.insert, registered]
    change economy.Goal.collect_assets_loop assetOrd rest (RateConstruction.register cmp entry.1 collected) = _
    simpa [Discovery.registerAll] using ih (RateConstruction.register cmp entry.1 collected)

include lawful in
theorem collect_assets (required : basket.Basket A Std.U64) (collected : List A) :
    economy.Goal.collect_assets accountOrd assetOrd required collected =
      .ok (Discovery.registerAll cmp (required.quantities.map Prod.fst) collected) := by
  simp [economy.Goal.collect_assets, basket.Basket.iter, indexmap.map.IndexMap.iter,
    collect_assets_loop assetOrd cmp lawful]

include lawful in
theorem asset_keys_loop (remaining : List (AccountId × basket.Basket A Std.U64)) (collected : List A) :
    economy.Goal.asset_keys_loop accountOrd assetOrd remaining collected =
      .ok (Discovery.registerAll cmp (remaining.flatMap fun entry => entry.2.quantities.map Prod.fst) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold economy.Goal.asset_keys_loop
    rw [loop.eq_1]
    simp [economy.Goal.asset_keys_loop.body,
      alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, Discovery.registerAll]
  | cons entry rest ih =>
    have body : economy.Goal.asset_keys_loop.body accountOrd assetOrd (entry :: rest) collected =
        .ok (.cont (rest, Discovery.registerAll cmp (entry.2.quantities.map Prod.fst) collected)) := by
      simp [economy.Goal.asset_keys_loop.body,
        alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
        Aeneas.Std.uncurry, collect_assets accountOrd assetOrd cmp lawful]
    unfold economy.Goal.asset_keys_loop
    rw [loop.eq_1]
    simp only
    rw [body, bind_ok]
    change economy.Goal.asset_keys_loop accountOrd assetOrd rest
      (Discovery.registerAll cmp (entry.2.quantities.map Prod.fst) collected) = _
    simpa [List.flatMap_cons, Discovery.registerAll_append] using
      ih (Discovery.registerAll cmp (entry.2.quantities.map Prod.fst) collected)

include lawful in
theorem asset_keys (goal : economy.Goal AccountId A Std.U64) :
    economy.Goal.asset_keys accountOrd assetOrd goal =
      .ok (Discovery.registerAll cmp (goal.required.flatMap fun entry => entry.2.quantities.map Prod.fst) []) := by
  simp [economy.Goal.asset_keys, alloc.collections.btree.set.BTreeSetTGlobal.new,
    SharedABTreeMap.Insts.CoreIterTraitsCollectIntoIteratorPairSharedAKSharedAVIter.into_iter,
    asset_keys_loop accountOrd assetOrd cmp lawful]

end Assets
end Axionomy.GoalRefinement
