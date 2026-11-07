import Axionomy.PreparationRefinement
import Axionomy.BasketOperationsRefinement

namespace Axionomy.AvailabilityRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

variable {AccountId A RateId Role : Type}

section Relevant
variable [DecidableEq A]
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (hClone : LawfulClone assetClone) (hEq : LawfulEquality assetEq)

def relevantFold (balances required accumulated : List (A × Std.U64)) : List (A × Std.U64) :=
  required.foldl (fun result entry => Account.store arithmetic result entry.1
    (Account.balance arithmetic.zero balances entry.1)) accumulated

include hClone hEq in
theorem relevant_loop (balances required accumulated : List (A × Std.U64)) :
    economy.relevant_balances_loop assetClone assetEq assetHash scalar required ⟨⟨balances⟩⟩ ⟨accumulated⟩ =
      .ok ⟨relevantFold balances required accumulated⟩ := by
  induction required generalizing accumulated with
  | nil =>
    unfold economy.relevant_balances_loop
    rw [loop.eq_1]
    simp [economy.relevant_balances_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, relevantFold]
  | cons entry rest ih =>
    rcases entry with ⟨asset, value⟩
    have body : economy.relevant_balances_loop.body assetClone assetEq assetHash scalar ⟨⟨balances⟩⟩
        ((asset, value) :: rest) ⟨accumulated⟩ =
        .ok (.cont (rest, ⟨Account.store arithmetic accumulated asset (Account.balance arithmetic.zero balances asset)⟩)) := by
      simp [economy.relevant_balances_loop.body,
        indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
        hClone asset, account.Account.balance, quantity_call assetEq assetHash hEq, store_call assetEq assetHash hEq]
    unfold economy.relevant_balances_loop
    rw [loop.eq_1]
    simp only [body, bind_ok]
    exact ih _

theorem relevant_map (balances required accumulated : List (A × Std.U64)) :
    Account.mapEntries UScalar.val (relevantFold balances required accumulated) =
      (Account.mapEntries UScalar.val required).foldl (fun result entry =>
        Account.store (Account.naturals Quantity.u64Max) result entry.1
          (Account.balance 0 (Account.mapEntries UScalar.val balances) entry.1)) (Account.mapEntries UScalar.val accumulated) := by
  induction required generalizing accumulated with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, value⟩
    simp only [relevantFold, List.foldl_cons, Account.mapEntries_cons] at *
    rw [ih, Account.map_store arithmetic _ _ natural_hom,
      Account.map_balance UScalar.val arithmetic.zero balances asset]
    rfl

include hClone hEq in
theorem relevant_refines (holder : account.Account A Std.U64) (required : basket.Basket A Std.U64) :
    ∃ result, economy.relevant_balances assetClone assetEq assetHash scalar holder required = .ok result ∧
      TransactionDecode.balances result = Transaction.relevant (TransactionDecode.holder holder) (TransactionDecode.balances required) := by
  refine ⟨⟨relevantFold holder.balances.quantities required.quantities []⟩, ?_, ?_⟩
  · simp [economy.relevant_balances, basket.Basket.new, indexmap.map.IndexMapKVRandomState.new,
      basket.Basket.iter, indexmap.map.IndexMap.iter, relevant_loop assetClone assetEq assetHash hClone hEq]
  · exact relevant_map holder.balances.quantities required.quantities []

end Relevant

section Availability
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (assetClone : core.clone.Clone A)
variable (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (rateClone : core.clone.Clone RateId) (roleOrd : core.cmp.Ord Role)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)
variable (hRateClone : LawfulClone rateClone)

include hAccountClone hAccountEq hAssetClone hAssetEq hRateClone in
theorem preflight_loop_refines (remaining : List (AccountId × economy.Effect A Std.U64))
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (assessed : alloc.vec.Vec (economy.AccountAssessment AccountId A Std.U64))
    (missing : alloc.vec.Vec (economy.AccountShortfall AccountId A Std.U64))
    (present : ∀ entry ∈ remaining, entry.1 ∈ model.accounts.map Prod.fst)
    (valid : ∀ entry ∈ remaining, BasketOperations.Valid arithmetic entry.2.consume.quantities)
    (accountCapacity : assessed.val.length + remaining.length ≤ Usize.max)
    (missingCapacity : missing.val.length + remaining.length ≤ Usize.max) :
    ∃ result, economy.Economy.preflight_effects_loop accountClone accountEq accountHash assetClone assetEq assetHash rateClone roleOrd scalar
        remaining model proposal assessed missing = .ok result ∧
      TransactionDecode.checked TransactionDecode.availability result =
        Transaction.preflight proposal.rate (TransactionDecode.accounts model.accounts) (TransactionDecode.effects remaining)
          (TransactionDecode.availability ⟨assessed, missing⟩) := by
  induction remaining generalizing assessed missing with
  | nil =>
    refine ⟨.Ok ⟨assessed, missing⟩, ?_, rfl⟩
    unfold economy.Economy.preflight_effects_loop
    rw [loop.eq_1]
    simp [economy.Economy.preflight_effects_loop.body,
      alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest ih =>
    rcases entry with ⟨id, effect⟩
    obtain ⟨holder, found⟩ := (Transaction.find_exists_iff_key_mem id model.accounts).mpr (present (id, effect) (by simp))
    have naturalFound : Account.find? id (TransactionDecode.accounts model.accounts) = some (TransactionDecode.holder holder) := by
      rw [TransactionDecode.accounts, ← Account.map_find, found]; rfl
    have addition := BasketOperationsRefinement.add_refines assetClone assetEq assetHash hAssetClone hAssetEq
      effect.consume.quantities effect.preserve.quantities
    change basket.Basket.checked_add assetClone assetEq assetHash scalar effect.consume effect.preserve = _ at addition
    have naturalAddition := Account.map_credit arithmetic (Account.naturals Quantity.u64Max) UScalar.val natural_hom
      effect.preserve.quantities effect.consume.quantities
    cases sum : Account.credit arithmetic effect.consume.quantities effect.preserve.quantities with
    | error asset =>
      have rejected : Account.credit (Account.naturals Quantity.u64Max) (TransactionDecode.balances effect.consume)
          (TransactionDecode.balances effect.preserve) = .error asset := by rw [sum] at naturalAddition; exact naturalAddition.symm
      have body : economy.Economy.preflight_effects_loop.body accountClone accountEq accountHash assetClone assetEq assetHash rateClone roleOrd scalar
          model proposal ((id, effect) :: rest) assessed missing = .ok (.done (.Err (.RateOverflow proposal.rate asset))) := by
        simp [economy.Economy.preflight_effects_loop.body,
          alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
          indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccountEq, found,
          core.option.Option.expect, clone_call assetClone hAssetClone, addition,
          BasketOperationsRefinement.addResult, sum, exchange.Exchange.impl.rate, hRateClone proposal.rate]
      refine ⟨.Err (.RateOverflow proposal.rate asset), ?_, ?_⟩
      · unfold economy.Economy.preflight_effects_loop
        rw [loop.eq_1]
        simp [body]
      · simp only [TransactionDecode.effects, Account.mapEntries_cons, Transaction.preflight, naturalFound,
          Transaction.require, Transaction.checked_bind_ok]
        simp [TransactionDecode.effect, rejected, Transaction.rateResult, TransactionDecode.checked, TransactionDecode.failure]
    | ok required =>
      have accepted : Account.credit (Account.naturals Quantity.u64Max) (TransactionDecode.balances effect.consume)
          (TransactionDecode.balances effect.preserve) = .ok (Account.mapEntries UScalar.val required) := by
        rw [sum] at naturalAddition; exact naturalAddition.symm
      have requiredValid := BasketOperations.credit_valid arithmetic effect.preserve.quantities effect.consume.quantities required
        (valid (id, effect) (by simp)) sum
      obtain ⟨available, availableCall, availableValue⟩ := relevant_refines assetClone assetEq assetHash hAssetClone hAssetEq holder ⟨required⟩
      let short := Account.shortfall arithmetic holder.balances.quantities required
      have shortCall := shortfall_refines assetClone assetEq assetHash hAssetClone hAssetEq holder.balances.quantities required requiredValid.1
      have shortValue : Account.shortfall (Account.naturals Quantity.u64Max) (TransactionDecode.holder holder)
          (Account.mapEntries UScalar.val required) = Account.mapEntries UScalar.val short :=
        (Account.map_shortfall arithmetic _ _ natural_hom required holder.balances.quantities).symm
      let assessment := economy.AccountAssessment.mk id available ⟨required⟩ effect.consume effect.produce effect.preserve
      have accountRoom : assessed.val.length < Usize.max := by simp only [List.length_cons] at accountCapacity; omega
      have missingRoom : missing.val.length < Usize.max := by simp only [List.length_cons] at missingCapacity; omega
      obtain ⟨nextAssessed, pushedAssessment, assessmentEntries⟩ := ModelConstructionRefinement.push_call assessed assessment accountRoom
      have nextAccountCapacity : nextAssessed.val.length + rest.length ≤ Usize.max := by
        rw [assessmentEntries]; simp only [List.length_append, List.length_cons, List.length_nil] at *; omega
      have bodyPrefix : economy.Economy.preflight_effects_loop.body accountClone accountEq accountHash assetClone assetEq assetHash rateClone roleOrd scalar
          model proposal ((id, effect) :: rest) assessed missing =
          if short.isEmpty then .ok (.cont (rest, nextAssessed, missing))
          else (do
            let nextMissing ← alloc.vec.Vec.push missing ⟨id, ⟨short⟩⟩
            .ok (.cont (rest, nextAssessed, nextMissing))) := by
        dsimp only [assessment] at pushedAssessment
        simp [economy.Economy.preflight_effects_loop.body,
          alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
          indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccountEq, found,
          core.option.Option.expect, clone_call assetClone hAssetClone, addition,
          BasketOperationsRefinement.addResult, sum, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          availableCall, account.Account.impl.balances, shortCall, hAccountClone id,
          economy.AccountAssessment.new, pushedAssessment, basket.Basket.is_empty, indexmap.map.IndexMap.is_empty,
          economy.AccountShortfall.new, short]
      cases empty : short.isEmpty with
      | true =>
        obtain ⟨result, called, decoded⟩ := ih nextAssessed missing
          (fun item member => present item (by simp [member])) (fun item member => valid item (by simp [member]))
          nextAccountCapacity (by simp only [List.length_cons] at missingCapacity; omega)
        refine ⟨result, ?_, ?_⟩
        · unfold economy.Economy.preflight_effects_loop
          rw [loop.eq_1]
          simp only [bodyPrefix, empty, ↓reduceIte, bind_ok]
          exact called
        · rw [decoded]
          simp only [TransactionDecode.effects, Account.mapEntries_cons, Transaction.preflight, naturalFound,
            Transaction.require, Transaction.checked_bind_ok, TransactionDecode.effect, accepted,
            Transaction.rateResult, Transaction.checked_bind_ok, shortValue]
          simp only [TransactionDecode.availability, assessmentEntries, List.map_append, List.map_cons, List.map_nil,
            TransactionDecode.assessment, assessment]
          rw [availableValue]
          simp [TransactionDecode.balances, Account.mapEntries, empty]
      | false =>
        obtain ⟨nextMissing, pushedMissing, missingEntries⟩ := ModelConstructionRefinement.push_call missing ⟨id, ⟨short⟩⟩ missingRoom
        have nextMissingCapacity : nextMissing.val.length + rest.length ≤ Usize.max := by
          rw [missingEntries]; simp only [List.length_append, List.length_cons, List.length_nil] at *; omega
        obtain ⟨result, called, decoded⟩ := ih nextAssessed nextMissing
          (fun item member => present item (by simp [member])) (fun item member => valid item (by simp [member]))
          nextAccountCapacity nextMissingCapacity
        refine ⟨result, ?_, ?_⟩
        · unfold economy.Economy.preflight_effects_loop
          rw [loop.eq_1]
          simp only [bodyPrefix, empty, Bool.false_eq_true, ↓reduceIte, pushedMissing, bind_ok]
          exact called
        · rw [decoded]
          simp only [TransactionDecode.effects, Account.mapEntries_cons, Transaction.preflight, naturalFound,
            Transaction.require, Transaction.checked_bind_ok, TransactionDecode.effect, accepted,
            Transaction.rateResult, Transaction.checked_bind_ok, shortValue]
          simp only [TransactionDecode.availability, assessmentEntries, missingEntries, List.map_append, List.map_cons, List.map_nil,
            TransactionDecode.assessment, assessment]
          rw [availableValue]
          simp [TransactionDecode.balances, TransactionDecode.shortfall, Account.mapEntries, empty]

include hAccountClone hAccountEq hAssetClone hAssetEq hRateClone in
theorem preflight_refines (accountOrd : core.cmp.Ord AccountId) (assetOrd : core.cmp.Ord A)
    (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (present : ∀ entry ∈ effects, entry.1 ∈ model.accounts.map Prod.fst)
    (valid : ∀ entry ∈ effects, BasketOperations.Valid arithmetic entry.2.consume.quantities)
    (capacity : effects.length ≤ Usize.max) :
    ∃ result, economy.Economy.preflight_effects accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal effects = .ok result ∧
      TransactionDecode.checked TransactionDecode.availability result =
        Transaction.preflight proposal.rate (TransactionDecode.accounts model.accounts) (TransactionDecode.effects effects) ⟨[], []⟩ := by
  have bound : effects.length < 2^System.Platform.numBits := by
    have upper : Usize.max < 2^System.Platform.numBits := by scalar_tac
    omega
  obtain ⟨result, called, decoded⟩ := preflight_loop_refines accountClone accountEq accountHash assetClone assetEq assetHash rateClone roleOrd
    hAccountClone hAccountEq hAssetClone hAssetEq hRateClone effects model proposal (alloc.vec.Vec.new _) (alloc.vec.Vec.new _)
    present valid (by simpa using capacity) (by simpa using capacity)
  refine ⟨result, ?_, decoded⟩
  simpa [economy.Economy.preflight_effects, alloc.collections.btree.map.BTreeMap.len, bound, alloc.vec.Vec.with_capacity,
    SharedABTreeMap.Insts.CoreIterTraitsCollectIntoIteratorPairSharedAKSharedAVIter.into_iter] using called

end Availability
end Axionomy.AvailabilityRefinement
