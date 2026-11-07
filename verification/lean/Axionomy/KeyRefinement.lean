import Axionomy.KeyOrdering
import Axionomy.CanonicalKeys
import Axionomy.ObservationRefinement

namespace Axionomy.KeyRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

/-- The concrete typed balances, retaining all identifiers and exact u64 values. -/
def world (accounts : List (AccountId × account.Account A Std.U64)) : CanonicalKeys.World (AccountId := AccountId) (Asset := A) (Q := Std.U64) :=
  Account.mapEntries (fun holder => holder.balances.quantities) accounts

theorem key_visible (key : economy.ObservationKey AccountId A Std.U64) :
    economy.ObservationKey.impl.visible_accounts key = .ok key.visible_accounts.slice := by
  simp [economy.ObservationKey.impl.visible_accounts, alloc.vec.Vec.deref, alloc.vec.Vec.val]

theorem key_existing (key : economy.ObservationKey AccountId A Std.U64) :
    economy.ObservationKey.impl.existing_accounts key = .ok key.existing_accounts.slice := by
  simp [economy.ObservationKey.impl.existing_accounts, alloc.vec.Vec.deref, alloc.vec.Vec.val]

theorem key_balances (key : economy.ObservationKey AccountId A Std.U64) :
    economy.ObservationKey.impl.balances key = .ok key.balances.slice := by
  simp [economy.ObservationKey.impl.balances, alloc.vec.Vec.deref, alloc.vec.Vec.val]

section Collection
variable (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
variable (hAccountClone : LawfulClone accountClone) (hAssetClone : LawfulClone assetClone)

include hAccountClone hAssetClone in
theorem asset_loop (id : AccountId) (remaining : List (A × Std.U64))
    (collected : alloc.vec.Vec (AccountId × A × Std.U64))
    (capacity : collected.val.length + remaining.length ≤ Usize.max) :
    ∃ result, economy.Economy.state_key_loop0_loop0 accountClone assetClone scalar remaining collected id = .ok result ∧
      result.val = collected.val ++ CanonicalKeys.tag id remaining := by
  induction remaining generalizing collected with
  | nil =>
    refine ⟨collected, ?_, by simp [CanonicalKeys.tag]⟩
    unfold economy.Economy.state_key_loop0_loop0
    rw [loop.eq_1]
    simp [economy.Economy.state_key_loop0_loop0.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest ih =>
    rcases entry with ⟨asset, value⟩
    obtain ⟨pushed, pushedCall, pushedEntries⟩ := ModelConstructionRefinement.push_call collected (id, asset, value) (by
      simp only [List.length_cons] at capacity
      omega)
    obtain ⟨result, executed, entries⟩ := ih pushed (by
      rw [pushedEntries]
      simp only [List.length_append, List.length_singleton]
      simp only [List.length_cons] at capacity
      omega)
    have body : economy.Economy.state_key_loop0_loop0.body accountClone assetClone scalar id ((asset, value) :: rest) collected =
        .ok (.cont (rest, pushed)) := by
      simp [economy.Economy.state_key_loop0_loop0.body,
        indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
        hAccountClone id, hAssetClone asset, quantity.Quantity.Insts.CoreCloneClone.clone, pushedCall]
    refine ⟨result, ?_, ?_⟩
    · unfold economy.Economy.state_key_loop0_loop0
      rw [loop.eq_1]
      simp only
      rw [body, bind_ok]
      exact executed
    · simp [entries, pushedEntries, CanonicalKeys.tag, List.append_assoc]

include hAccountClone hAssetClone in
theorem observation_asset_loop (id : AccountId) (remaining : List (A × Std.U64))
    (collected : alloc.vec.Vec (AccountId × A × Std.U64))
    (capacity : collected.val.length + remaining.length ≤ Usize.max) :
    ∃ result, economy.Economy.observation_key_for_loop0_loop0 accountClone assetClone scalar remaining collected id = .ok result ∧
      result.val = collected.val ++ CanonicalKeys.tag id remaining := by
  have identical : economy.Economy.observation_key_for_loop0_loop0 accountClone assetClone scalar remaining collected id =
      economy.Economy.state_key_loop0_loop0 accountClone assetClone scalar remaining collected id := rfl
  rw [identical]
  exact asset_loop accountClone assetClone hAccountClone hAssetClone id remaining collected capacity

include hAccountClone hAssetClone in
theorem state_loop (remaining : List (AccountId × account.Account A Std.U64))
    (collected : alloc.vec.Vec (AccountId × A × Std.U64))
    (capacity : collected.val.length + (CanonicalKeys.entries (world remaining)).length ≤ Usize.max) :
    ∃ result, economy.Economy.state_key_loop0 accountClone assetClone scalar remaining collected = .ok result ∧
      result.val = collected.val ++ CanonicalKeys.entries (world remaining) := by
  induction remaining generalizing collected with
  | nil =>
    refine ⟨collected, ?_, by simp [CanonicalKeys.entries, world]⟩
    unfold economy.Economy.state_key_loop0
    rw [loop.eq_1]
    simp [economy.Economy.state_key_loop0.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest ih =>
    rcases entry with ⟨id, holder⟩
    have sizes : collected.val.length + holder.balances.quantities.length +
        (CanonicalKeys.entries (world rest)).length ≤ Usize.max := by
      simpa [world, Account.mapEntries_cons, CanonicalKeys.entries, CanonicalKeys.tag, List.length_flatMap, Nat.add_assoc] using capacity
    obtain ⟨middle, firstCall, firstEntries⟩ := asset_loop accountClone assetClone hAccountClone hAssetClone id holder.balances.quantities collected (by omega)
    obtain ⟨result, tailCall, tailEntries⟩ := ih middle (by
      rw [firstEntries]
      simpa [CanonicalKeys.tag] using sizes)
    have body : economy.Economy.state_key_loop0.body accountClone assetClone scalar ((id, holder) :: rest) collected =
        .ok (.cont (rest, middle)) := by
      simp [economy.Economy.state_key_loop0.body,
        indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, account.Account.impl.balances,
        basket.Basket.iter, indexmap.map.IndexMap.iter, firstCall]
    refine ⟨result, ?_, ?_⟩
    · unfold economy.Economy.state_key_loop0
      rw [loop.eq_1]
      simp only
      rw [body, bind_ok]
      exact tailCall
    · simp [tailEntries, firstEntries, world, CanonicalKeys.entries, List.append_assoc]

end Collection
section Keys
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpAsset : A → A → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAssetClone : LawfulClone assetClone)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetOrd : LawfulOrdering assetOrd cmpAsset)

include hAccountClone hAssetClone hAccountOrd hAssetOrd in
theorem state_key (model : economy.Economy AccountId A RateId Role Std.U64)
    (capacity : (CanonicalKeys.entries (world model.accounts)).length ≤ Usize.max) :
    ∃ result, economy.Economy.state_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model = .ok result ∧
      result.val = CanonicalSort.sort (KeyOrdering.triples cmpAccount cmpAsset) (CanonicalKeys.entries (world model.accounts)) := by
  obtain ⟨collected, collection, entries⟩ := state_loop accountClone assetClone hAccountClone hAssetClone model.accounts
    (alloc.vec.Vec.new _) (by simpa [alloc.vec.Vec.new] using capacity)
  obtain ⟨sorted, sorting, contents⟩ := CanonicalSort.slice_sort
    (TupleVUT.Insts.CoreCmpOrd accountOrd assetOrd (quantity.Quantity.Insts.CoreCmpOrd core.cmp.OrdU64))
    (KeyOrdering.triples cmpAccount cmpAsset) (KeyOrdering.triples_ordering accountOrd assetOrd cmpAccount cmpAsset hAccountOrd hAssetOrd)
    collected.slice
  refine ⟨⟨sorted⟩, ?_, ?_⟩
  · simp [economy.Economy.state_key,
      SharedAIndexMap.Insts.CoreIterTraitsCollectIntoIteratorPairSharedAKSharedAVIter.into_iter,
      collection, alloc.vec.Vec.deref_mut, lift, sorting, Aeneas.Std.uncurry]
  · change sorted.val = CanonicalSort.sort (KeyOrdering.triples cmpAccount cmpAsset) collected.val at contents
    change sorted.val = _
    simpa [entries, alloc.vec.Vec.new] using contents

include hAccountClone hAssetClone hAccountOrd hAssetOrd in
theorem state_key_contract (accountLaws : OrderedEffects.OrderLaws cmpAccount) (assetLaws : OrderedEffects.OrderLaws cmpAsset)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (capacity : (CanonicalKeys.entries (world model.accounts)).length ≤ Usize.max) :
    ∃ result, economy.Economy.state_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model = .ok result ∧
      result.val.Perm (CanonicalKeys.entries (world model.accounts)) ∧
      result.val.Pairwise (CanonicalSort.before (KeyOrdering.triples cmpAccount cmpAsset)) := by
  obtain ⟨result, executed, specified⟩ := state_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpAsset hAccountClone hAssetClone hAccountOrd hAssetOrd model capacity
  refine ⟨result, executed, ?_, ?_⟩
  · rw [specified]; exact CanonicalSort.sort_perm _ _
  · rw [specified]; exact CanonicalSort.sort_sorted _ (KeyOrdering.triples_laws _ _ accountLaws assetLaws) _

variable [DecidableEq AccountId]
variable (hAccountEq : LawfulEquality accountEq)

theorem world_find (accounts : List (AccountId × account.Account A Std.U64)) (id : AccountId) :
    Account.find? id (world accounts) = (Account.find? id accounts).map (fun holder => holder.balances.quantities) :=
  (Account.map_find _ accounts id).symm

include hAccountClone hAssetClone hAccountEq in
theorem observation_loop (model : economy.Economy AccountId A RateId Role Std.U64) (remaining : List AccountId)
    (balances : alloc.vec.Vec (AccountId × A × Std.U64)) (visible present : alloc.vec.Vec AccountId)
    (balanceCapacity : balances.val.length + (CanonicalKeys.visibleEntries (world model.accounts) remaining).length ≤ Usize.max)
    (visibleCapacity : visible.val.length + remaining.length ≤ Usize.max)
    (presentCapacity : present.val.length + (CanonicalKeys.existing (world model.accounts) remaining).length ≤ Usize.max) :
    ∃ result, economy.Economy.observation_key_for_loop0 accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar remaining model balances visible present = .ok result ∧
      result.1.val = balances.val ++ CanonicalKeys.visibleEntries (world model.accounts) remaining ∧
      result.2.1.val = visible.val ++ remaining ∧
      result.2.2.val = present.val ++ CanonicalKeys.existing (world model.accounts) remaining := by
  induction remaining generalizing balances visible present with
  | nil =>
    refine ⟨(balances, visible, present), ?_, by simp [CanonicalKeys.visibleEntries], by simp, by simp [CanonicalKeys.existing]⟩
    unfold economy.Economy.observation_key_for_loop0
    rw [loop.eq_1]
    simp [economy.Economy.observation_key_for_loop0.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next]
  | cons id rest ih =>
    obtain ⟨shown, shownCall, shownValues⟩ := ModelConstructionRefinement.push_call visible id (by
      simp only [List.length_cons] at visibleCapacity
      omega)
    have visibleTail : shown.val.length + rest.length ≤ Usize.max := by
      rw [shownValues]
      simp only [List.length_append, List.length_singleton]
      simp only [List.length_cons] at visibleCapacity
      omega
    have accountCall := ObservationRefinement.model_account accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd hAccountEq model id
    cases found : Account.find? id model.accounts with
    | none =>
      have balanceStep : CanonicalKeys.visibleEntries (world model.accounts) (id :: rest) =
          CanonicalKeys.visibleEntries (world model.accounts) rest := by
        simp [CanonicalKeys.visibleEntries, world_find, found, CanonicalKeys.tag]
      have presentStep : CanonicalKeys.existing (world model.accounts) (id :: rest) =
          CanonicalKeys.existing (world model.accounts) rest := by simp [CanonicalKeys.existing, world_find, found]
      obtain ⟨result, tailCall, tailBalances, tailVisible, tailPresent⟩ := ih balances shown present
        (by simpa [balanceStep] using balanceCapacity) visibleTail (by simpa [presentStep] using presentCapacity)
      have body : economy.Economy.observation_key_for_loop0.body accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
          rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model (id :: rest) balances visible present =
            .ok (.cont (rest, balances, shown, present)) := by
        simp [economy.Economy.observation_key_for_loop0.body,
          alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
          hAccountClone id, shownCall, accountCall, found]
      refine ⟨result, ?_, ?_, ?_, ?_⟩
      · unfold economy.Economy.observation_key_for_loop0
        rw [loop.eq_1]
        simp only
        rw [body, bind_ok]
        exact tailCall
      · simpa [balanceStep] using tailBalances
      · simpa [shownValues, List.append_assoc] using tailVisible
      · simpa [presentStep] using tailPresent
    | some holder =>
      have balanceStep : CanonicalKeys.visibleEntries (world model.accounts) (id :: rest) =
          CanonicalKeys.tag id holder.balances.quantities ++ CanonicalKeys.visibleEntries (world model.accounts) rest := by
        simp [CanonicalKeys.visibleEntries, world_find, found]
      have presentStep : CanonicalKeys.existing (world model.accounts) (id :: rest) =
          id :: CanonicalKeys.existing (world model.accounts) rest := by simp [CanonicalKeys.existing, world_find, found]
      have balanceSizes : balances.val.length + holder.balances.quantities.length +
          (CanonicalKeys.visibleEntries (world model.accounts) rest).length ≤ Usize.max := by
        simpa [balanceStep, CanonicalKeys.tag, Nat.add_assoc] using balanceCapacity
      obtain ⟨recorded, recordedCall, recordedValues⟩ := ModelConstructionRefinement.push_call present id (by
        rw [presentStep] at presentCapacity
        simp only [List.length_cons] at presentCapacity
        omega)
      obtain ⟨collected, collectCall, collectValues⟩ := observation_asset_loop accountClone assetClone hAccountClone hAssetClone id
        holder.balances.quantities balances (by omega)
      obtain ⟨result, tailCall, tailBalances, tailVisible, tailPresent⟩ := ih collected shown recorded
        (by rw [collectValues]; simpa [CanonicalKeys.tag] using balanceSizes) visibleTail (by
          rw [recordedValues]
          rw [presentStep] at presentCapacity
          simp only [List.length_append, List.length_singleton]
          simp only [List.length_cons] at presentCapacity
          omega)
      have body : economy.Economy.observation_key_for_loop0.body accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
          rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model (id :: rest) balances visible present =
            .ok (.cont (rest, collected, shown, recorded)) := by
        simp [economy.Economy.observation_key_for_loop0.body,
          alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
          hAccountClone id, shownCall, accountCall, found, recordedCall, account.Account.impl.balances,
          basket.Basket.iter, indexmap.map.IndexMap.iter, collectCall]
      refine ⟨result, ?_, ?_, ?_, ?_⟩
      · unfold economy.Economy.observation_key_for_loop0
        rw [loop.eq_1]
        simp only
        rw [body, bind_ok]
        exact tailCall
      · simpa [balanceStep, collectValues, List.append_assoc] using tailBalances
      · simpa [shownValues, List.append_assoc] using tailVisible
      · simpa [presentStep, recordedValues, List.append_assoc] using tailPresent

def decode (key : economy.ObservationKey AccountId A Std.U64) : CanonicalKeys.Key AccountId A Std.U64 :=
  ⟨key.visible_accounts.val, key.existing_accounts.val, key.balances.val⟩

include hAccountClone hAssetClone hAccountEq hAccountOrd hAssetOrd in
theorem observation_key (model : economy.Economy AccountId A RateId Role Std.U64) (visible : List AccountId)
    (balanceCapacity : (CanonicalKeys.visibleEntries (world model.accounts) visible).length ≤ Usize.max)
    (visibleCapacity : visible.length ≤ Usize.max) :
    ∃ key, economy.Economy.observation_key_for accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model visible = .ok key ∧
      decode key = CanonicalKeys.observe (KeyOrdering.triples cmpAccount cmpAsset) (world model.accounts) visible := by
  obtain ⟨⟨balances, shown, present⟩, collection, balanceValues, shownValues, presentValues⟩ :=
    observation_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd hAccountClone hAssetClone hAccountEq model visible
      (alloc.vec.Vec.new _) (alloc.vec.Vec.new _) (alloc.vec.Vec.new _)
      (by simpa [alloc.vec.Vec.new] using balanceCapacity)
      (by simpa [alloc.vec.Vec.new] using visibleCapacity)
      (by simpa [alloc.vec.Vec.new] using Nat.le_trans (CanonicalKeys.existing_length (world model.accounts) visible) visibleCapacity)
  obtain ⟨sorted, sorting, contents⟩ := CanonicalSort.slice_sort
    (TupleVUT.Insts.CoreCmpOrd accountOrd assetOrd (quantity.Quantity.Insts.CoreCmpOrd core.cmp.OrdU64))
    (KeyOrdering.triples cmpAccount cmpAsset) (KeyOrdering.triples_ordering accountOrd assetOrd cmpAccount cmpAsset hAccountOrd hAssetOrd)
    balances.slice
  refine ⟨⟨shown, present, ⟨sorted⟩⟩, ?_, ?_⟩
  · simp [economy.Economy.observation_key_for,
      SharedABTreeSet.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter,
      collection, alloc.vec.Vec.deref_mut, lift, sorting, Aeneas.Std.uncurry]
  · change sorted.val = CanonicalSort.sort (KeyOrdering.triples cmpAccount cmpAsset) balances.val at contents
    change CanonicalKeys.Key.mk shown.val present.val sorted.val = _
    unfold CanonicalKeys.observe
    simp only [CanonicalKeys.Key.mk.injEq]
    exact ⟨by simpa [alloc.vec.Vec.new] using shownValues,
      by simpa [alloc.vec.Vec.new] using presentValues,
      by simpa [balanceValues, alloc.vec.Vec.new] using contents⟩

include hAccountClone hAssetClone hAccountEq hAccountOrd hAssetOrd in
theorem view_key (view : economy.EconomicView AccountId A RateId Role Std.U64)
    (balanceCapacity : (CanonicalKeys.visibleEntries (world view.economy.accounts) view.visible).length ≤ Usize.max)
    (visibleCapacity : view.visible.length ≤ Usize.max) :
    ∃ key, economy.EconomicView.observation_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar view = .ok key ∧
      decode key = CanonicalKeys.observe (KeyOrdering.triples cmpAccount cmpAsset) (world view.economy.accounts) view.visible := by
  exact observation_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpAsset hAccountClone hAssetClone hAccountOrd hAssetOrd hAccountEq
    view.economy view.visible balanceCapacity visibleCapacity

theorem world_unique
    (accounts : List (AccountId × account.Account A Std.U64))
    (unique : ∀ id holder, Account.find? id accounts = some holder → (holder.balances.quantities.map Prod.fst).Nodup) :
    ∀ id entries, Account.find? id (world accounts) = some entries → (entries.map Prod.fst).Nodup := by
  intro id entries found
  rw [world_find] at found
  cases physical : Account.find? id accounts with
  | none => simp [physical] at found
  | some holder =>
    have same : holder.balances.quantities = entries := by simpa [physical] using found
    rw [← same]
    exact unique id holder physical

variable [DecidableEq A]
variable (hAssetEq : LawfulEquality assetEq)

theorem query_spec (view : economy.EconomicView AccountId A RateId Role Std.U64) (id : AccountId) (asset : A) :
    Observations.balance arithmetic.zero (world view.economy.accounts) view.visible id asset =
      ((if id ∈ view.visible then Account.find? id view.economy.accounts else none).map
        (fun holder => Account.balance arithmetic.zero holder.balances.quantities asset)) := by
  by_cases shown : id ∈ view.visible <;>
    simp [Observations.balance, Observations.account, world_find, shown, Option.map_map, Function.comp_def]

include hAccountClone hAssetClone hAccountEq hAccountOrd hAssetOrd hAssetEq in
/-- Equality of successfully generated observation keys implies equality of the
actual public balance queries, including Some(0) versus None. The two views may
come from different account domains. -/
theorem view_key_sound (accountLaws : OrderedEffects.OrderLaws cmpAccount)
    (left right : economy.EconomicView AccountId A RateId Role Std.U64)
    (leftSorted : left.visible.Pairwise (fun x y => cmpAccount x y = .lt))
    (rightSorted : right.visible.Pairwise (fun x y => cmpAccount x y = .lt))
    (leftCapacity : (CanonicalKeys.visibleEntries (world left.economy.accounts) left.visible).length ≤ Usize.max)
    (rightCapacity : (CanonicalKeys.visibleEntries (world right.economy.accounts) right.visible).length ≤ Usize.max)
    (leftVisible : left.visible.length ≤ Usize.max) (rightVisible : right.visible.length ≤ Usize.max)
    (leftUnique : ∀ id holder, Account.find? id left.economy.accounts = some holder → (holder.balances.quantities.map Prod.fst).Nodup)
    (rightUnique : ∀ id holder, Account.find? id right.economy.accounts = some holder → (holder.balances.quantities.map Prod.fst).Nodup)
    (same : economy.EconomicView.observation_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar left =
      economy.EconomicView.observation_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar right)
    (id : AccountId) (asset : A) :
    economy.EconomicView.balance accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar left id asset =
      economy.EconomicView.balance accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar right id asset := by
  obtain ⟨leftKey, leftCall, leftSpec⟩ := view_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpAsset hAccountClone hAssetClone hAccountOrd hAssetOrd hAccountEq left leftCapacity leftVisible
  obtain ⟨rightKey, rightCall, rightSpec⟩ := view_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpAsset hAccountClone hAssetClone hAccountOrd hAssetOrd hAccountEq right rightCapacity rightVisible
  have keySame : leftKey = rightKey := by simpa [leftCall, rightCall] using same
  have specified : CanonicalKeys.observe (KeyOrdering.triples cmpAccount cmpAsset) (world left.economy.accounts) left.visible =
      CanonicalKeys.observe (KeyOrdering.triples cmpAccount cmpAsset) (world right.economy.accounts) right.visible := by
    rw [← leftSpec, ← rightSpec, keySame]
  have queries := CanonicalKeys.queries_equal arithmetic.zero _ _ _ _ _ specified
    (world_unique _ leftUnique) (world_unique _ rightUnique)
  have atQuery := congrFun (congrFun queries id) asset
  rw [query_spec, query_spec] at atQuery
  rw [ObservationRefinement.view_balance accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount hAccountEq hAssetEq hAccountOrd accountLaws left leftSorted id asset,
    ObservationRefinement.view_balance accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount hAccountEq hAssetEq hAccountOrd accountLaws right rightSorted id asset,
    atQuery]

end Keys
end Axionomy.KeyRefinement
