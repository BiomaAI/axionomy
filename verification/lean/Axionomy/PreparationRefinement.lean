import Axionomy.TransactionDecode
import Axionomy.CommitRefinement
import Axionomy.TransactionPreparation

namespace Axionomy.PreparationRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

variable {AccountId A RateId Role : Type}

def accountError : account.AccountError A Std.U64 Std.I128 → Account.UpdateError A Nat
  | .Overflow asset => .overflow asset
  | .InsufficientBalance missing => .insufficient (TransactionDecode.balances missing)

theorem map_error_refines (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
    (id : AccountId) (error : account.AccountError A Std.U64 Std.I128) :
    ∃ result, economy.map_account_error RateId Role assetEq assetHash scalar id error = .ok result ∧
      TransactionDecode.failure result = Transaction.accountFailure id (accountError error) := by
  cases error with
  | Overflow asset => exact ⟨.BalanceOverflow id asset, rfl, rfl⟩
  | InsufficientBalance missing =>
    let values := alloc.vec.Vec.from [economy.AccountShortfall.mk id missing] (by simp; scalar_tac)
    refine ⟨.Infeasible values, ?_, ?_⟩
    · simp [economy.map_account_error, economy.AccountShortfall.new, Std.Array.to_slice,
        Std.Array.make, alloc.slice.Slice.into_vec, lift, alloc.vec.Vec.from, values]
    · simp [TransactionDecode.failure, TransactionDecode.shortfall, Transaction.accountFailure, accountError, values]

section Preparation
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (assetClone : core.clone.Clone A)
variable (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)

include hAssetClone hAssetEq in
theorem prepare_one_refines (model : economy.Economy AccountId A RateId Role Std.U64)
    (id : AccountId) (effect : economy.Effect A Std.U64)
    (present : id ∈ model.accounts.map Prod.fst)
    (unique : (effect.consume.quantities.map Prod.fst).Nodup) :
    ∃ selected result, Account.find? id model.accounts = some selected ∧
      account.Account.prepare_exchange assetClone assetEq assetHash scalar selected effect.consume effect.produce = .ok result ∧
      Transaction.prepareOne (RateId := RateId) (Role := Role) (TransactionDecode.accounts model.accounts) id (TransactionDecode.effect effect) =
        match result with
        | .Ok updated => .ok (TransactionDecode.holder updated)
        | .Err error => .error (.issue (Transaction.accountFailure id (accountError error))) := by
  obtain ⟨selected, found⟩ := (Transaction.find_exists_iff_key_mem id model.accounts).mpr present
  obtain ⟨result, called, decoded⟩ := prepare_refines_naturals assetClone assetEq assetHash hAssetClone hAssetEq
    selected.balances.quantities effect.consume.quantities effect.produce.quantities unique
  have mapped := Account.map_find TransactionDecode.holder model.accounts id
  rw [found] at mapped
  refine ⟨selected, result, found, called, ?_⟩
  simp only [Transaction.prepareOne, TransactionDecode.accounts, ← mapped, Option.map_some,
    Transaction.require, TransactionDecode.effect, TransactionDecode.holder,
    TransactionDecode.balances]
  change Transaction.liftIssue ((Account.prepare (Account.naturals Quantity.u64Max)
    (Account.mapEntries UScalar.val selected.balances.quantities)
    (Account.mapEntries UScalar.val effect.consume.quantities)
    (Account.mapEntries UScalar.val effect.produce.quantities)).mapError (Transaction.accountFailure id)) = _
  rw [← decoded]
  cases result with
  | Ok updated => rfl
  | Err error => cases error <;> rfl

include hAccountClone hAccountEq hAssetClone hAssetEq in
theorem prepare_loop_refines (remaining : List (AccountId × economy.Effect A Std.U64))
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (prepared : alloc.vec.Vec (AccountId × account.Account A Std.U64))
    (deltas : alloc.vec.Vec (exchange.AccountDelta AccountId A Std.U64))
    (present : ∀ entry ∈ remaining, entry.1 ∈ model.accounts.map Prod.fst)
    (unique : ∀ entry ∈ remaining, (entry.2.consume.quantities.map Prod.fst).Nodup)
    (accountCapacity : prepared.val.length + remaining.length ≤ Usize.max)
    (deltaCapacity : deltas.val.length + remaining.length ≤ Usize.max) :
    ∃ result, economy.Economy.prepare_effects_loop accountClone accountEq accountHash assetClone assetEq assetHash scalar
        remaining model prepared deltas = .ok result ∧
      TransactionDecode.checked TransactionDecode.preparation result =
        Transaction.prepare (TransactionDecode.accounts model.accounts) (TransactionDecode.effects remaining)
          (TransactionDecode.preparation ⟨prepared, deltas⟩) := by
  induction remaining generalizing prepared deltas with
  | nil =>
    refine ⟨.Ok ⟨prepared, deltas⟩, ?_, rfl⟩
    unfold economy.Economy.prepare_effects_loop
    rw [loop.eq_1]
    simp [economy.Economy.prepare_effects_loop.body,
      alloc.collections.btree.map.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next]
  | cons entry rest ih =>
    rcases entry with ⟨id, effect⟩
    obtain ⟨selected, result, found, called, decoded⟩ := prepare_one_refines assetClone assetEq assetHash
      hAssetClone hAssetEq model id effect (present (id, effect) (by simp)) (unique (id, effect) (by simp))
    cases result with
    | Err error =>
      obtain ⟨mapped, mappedCall, mappedValue⟩ := map_error_refines (RateId := RateId) (Role := Role) assetEq assetHash id error
      have body : economy.Economy.prepare_effects_loop.body accountClone accountEq accountHash assetClone assetEq assetHash scalar
          model ((id, effect) :: rest) prepared deltas = .ok (.done (.Err mapped)) := by
        simp [economy.Economy.prepare_effects_loop.body,
          alloc.collections.btree.map.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next,
          indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccountEq, found,
          core.option.Option.expect, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          called, hAccountClone id, mappedCall]
      refine ⟨.Err mapped, ?_, ?_⟩
      · unfold economy.Economy.prepare_effects_loop
        rw [loop.eq_1]
        simp [body]
      · simp [TransactionDecode.effects, Transaction.prepare, decoded, TransactionDecode.checked, mappedValue]
    | Ok updated =>
      have accountRoom : prepared.val.length < Usize.max := by simp only [List.length_cons] at accountCapacity; omega
      have deltaRoom : deltas.val.length < Usize.max := by simp only [List.length_cons] at deltaCapacity; omega
      let change := exchange.AccountDelta.mk id effect.consume effect.produce effect.preserve
      obtain ⟨nextPrepared, pushedAccounts, accountEntries⟩ := ModelConstructionRefinement.push_call prepared (id, updated) accountRoom
      obtain ⟨nextDeltas, pushedDeltas, deltaEntries⟩ := ModelConstructionRefinement.push_call deltas change deltaRoom
      have nextAccountCapacity : nextPrepared.val.length + rest.length ≤ Usize.max := by
        rw [accountEntries]; simp only [List.length_append, List.length_cons, List.length_nil] at *; omega
      have nextDeltaCapacity : nextDeltas.val.length + rest.length ≤ Usize.max := by
        rw [deltaEntries]; simp only [List.length_append, List.length_cons, List.length_nil] at *; omega
      obtain ⟨result, executed, related⟩ := ih nextPrepared nextDeltas
        (fun item member => present item (by simp [member]))
        (fun item member => unique item (by simp [member])) nextAccountCapacity nextDeltaCapacity
      dsimp only [change] at pushedDeltas
      have body : economy.Economy.prepare_effects_loop.body accountClone accountEq accountHash assetClone assetEq assetHash scalar
          model ((id, effect) :: rest) prepared deltas = .ok (.cont (rest, nextPrepared, nextDeltas)) := by
        simp [economy.Economy.prepare_effects_loop.body,
          alloc.collections.btree.map.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next,
          indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccountEq, found,
          core.option.Option.expect, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          called, hAccountClone id, exchange.AccountDelta.new, pushedDeltas,
          alloc.sync.Arc.new, pushedAccounts]
      refine ⟨result, ?_, ?_⟩
      · unfold economy.Economy.prepare_effects_loop
        rw [loop.eq_1]
        simp only [body, bind_ok]
        exact executed
      · rw [related]
        simp only [TransactionDecode.effects, Account.mapEntries_cons, Transaction.prepare, decoded,
          Transaction.checked_bind_ok]
        simp [TransactionDecode.preparation,
          TransactionDecode.accounts, Account.mapEntries, accountEntries, deltaEntries,
          TransactionDecode.delta, TransactionDecode.effect, change]

include hAccountClone hAccountEq hAssetClone hAssetEq in
theorem prepare_refines (accountOrd : core.cmp.Ord AccountId) (assetOrd : core.cmp.Ord A)
    (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId)
    (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (present : ∀ entry ∈ effects, entry.1 ∈ model.accounts.map Prod.fst)
    (unique : ∀ entry ∈ effects, (entry.2.consume.quantities.map Prod.fst).Nodup)
    (capacity : effects.length ≤ Usize.max) :
    ∃ result, economy.Economy.prepare_effects accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model effects = .ok result ∧
      TransactionDecode.checked TransactionDecode.preparation result =
        Transaction.prepare (TransactionDecode.accounts model.accounts) (TransactionDecode.effects effects) ⟨[], []⟩ := by
  have bound : effects.length < 2^System.Platform.numBits := by
    have upper : Usize.max < 2^System.Platform.numBits := by scalar_tac
    omega
  obtain ⟨result, called, decoded⟩ := prepare_loop_refines accountClone accountEq accountHash assetClone assetEq assetHash
    hAccountClone hAccountEq hAssetClone hAssetEq effects model (alloc.vec.Vec.new _) (alloc.vec.Vec.new _)
    present unique (by simpa using capacity) (by simpa using capacity)
  refine ⟨result, ?_, decoded⟩
  simpa [economy.Economy.prepare_effects, alloc.collections.btree.map.BTreeMap.len, bound,
    alloc.vec.Vec.with_capacity,
    alloc.collections.btree.map.BTreeMap.Insts.CoreIterTraitsCollectIntoIteratorPairIntoIter.into_iter] using called

include hAccountClone hAccountEq hAssetClone hAssetEq in
theorem successful_preparation (effects : List (AccountId × economy.Effect A Std.U64))
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (present : ∀ entry ∈ effects, entry.1 ∈ model.accounts.map Prod.fst)
    (unique : ∀ entry ∈ effects, (entry.2.consume.quantities.map Prod.fst).Nodup)
    (capacity : effects.length ≤ Usize.max) (prepared : economy.Preparation AccountId A Std.U64)
    (success : economy.Economy.prepare_effects_loop accountClone accountEq accountHash assetClone assetEq assetHash scalar
      effects model (alloc.vec.Vec.new _) (alloc.vec.Vec.new _) = .ok (.Ok prepared)) :
    prepared.accounts.val.map Prod.fst = effects.map Prod.fst ∧
      prepared.deltas.val.map TransactionDecode.delta = (TransactionDecode.effects effects).map Transaction.effectDelta ∧
      List.Forall₂ (Transaction.PreparedEntry (TransactionDecode.accounts model.accounts))
        (TransactionDecode.effects effects) (TransactionDecode.accounts prepared.accounts.val) ∧
      (∀ entry ∈ prepared.accounts.val, entry.1 ∈ model.accounts.map Prod.fst) := by
  obtain ⟨result, called, decoded⟩ := prepare_loop_refines accountClone accountEq accountHash assetClone assetEq assetHash
    hAccountClone hAccountEq hAssetClone hAssetEq effects model (alloc.vec.Vec.new _) (alloc.vec.Vec.new _)
    present unique (by simpa using capacity) (by simpa using capacity)
  have same : result = .Ok prepared := Result.ok_injective (called.symm.trans success)
  subst result
  have specification : Transaction.prepare (RateId := RateId) (Role := Role)
      (TransactionDecode.accounts model.accounts) (TransactionDecode.effects effects) ⟨[], []⟩ =
        .ok (TransactionDecode.preparation prepared) := decoded.symm
  have contract := Transaction.preparation_contract _ _ _ specification
  refine ⟨?_, contract.2.1, contract.2.2.1, ?_⟩
  · simpa [TransactionDecode.preparation] using contract.1
  · intro entry member
    have existing := contract.2.2.2.1 (entry.1, TransactionDecode.holder entry.2)
      (by exact List.mem_map.mpr ⟨entry, member, rfl⟩)
    simpa using existing

-- Account-existence for the commit loop is derived from the actual successful
-- preparation, rather than supplied as an independent commit assumption.
include hAccountClone hAccountEq hAssetClone hAssetEq in
theorem prepared_commit (effects : List (AccountId × economy.Effect A Std.U64))
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (present : ∀ entry ∈ effects, entry.1 ∈ model.accounts.map Prod.fst)
    (unique : ∀ entry ∈ effects, (entry.2.consume.quantities.map Prod.fst).Nodup)
    (capacity : effects.length ≤ Usize.max) (prepared : economy.Preparation AccountId A Std.U64)
    (success : economy.Economy.prepare_effects_loop accountClone accountEq accountHash assetClone assetEq assetHash scalar
      effects model (alloc.vec.Vec.new _) (alloc.vec.Vec.new _) = .ok (.Ok prepared)) :
    ∃ committed, economy.Economy.apply_loop accountEq accountHash prepared.accounts model = .ok committed ∧
      committed.rates = model.rates ∧ committed.invariants = model.invariants ∧
      committed.accounts.map Prod.fst = model.accounts.map Prod.fst ∧
      ∀ id, id ∉ effects.map Prod.fst → Account.find? id committed.accounts = Account.find? id model.accounts := by
  have contract := successful_preparation accountClone accountEq accountHash assetClone assetEq assetHash
    hAccountClone hAccountEq hAssetClone hAssetEq effects model present unique capacity prepared success
  obtain ⟨committed, called, rates, invariants, keys, frame⟩ := CommitRefinement.apply_loop_frames
    accountEq accountHash hAccountEq prepared.accounts.val prepared.accounts rfl model contract.2.2.2
  refine ⟨committed, called, rates, invariants, keys, ?_⟩
  intro id untouched
  exact frame id (contract.1 ▸ untouched)

end Preparation
end Axionomy.PreparationRefinement
