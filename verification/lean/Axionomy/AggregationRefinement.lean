import Axionomy.StaticEffectRefinement

namespace Axionomy.AggregationRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type} [DecidableEq AccountId] [DecidableEq A]

def computedAmounts (rule : rate.Rate Role A Std.U64) : List (rate.ComputedAmount Role A Std.U64) :=
  rule.computed_consume.val ++ rule.computed_produce.val ++ rule.computed_preserve.val

variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetClone : LawfulClone assetClone)
variable (hAssetEq : LawfulEquality assetEq) (hRateClone : LawfulClone rateClone) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem empty_body_refines (model : economy.Economy AccountId A RateId Role Std.U64)
    (rule : rate.Rate Role A Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (bound : ∀ amount ∈ computedAmounts rule, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id)
    (fits : ∀ amount ∈ computedAmounts rule, TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)) :
    ∃ result, economy.Economy.aggregate_effects_loop.body accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model rule proposal [] effects = .ok (.done result) ∧
      TransactionDecode.checked TransactionDecode.effects result =
        Transaction.computedChannels cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
          (ConditionRefinement.context cmpRole proposal model.accounts) (TransactionDecode.effects effects) := by
  have compute (amounts : Slice (rate.ComputedAmount Role A Std.U64)) (direction : economy.ComputedDirection)
      (initial : List (AccountId × economy.Effect A Std.U64))
      (subset : ∀ amount ∈ amounts.val, amount ∈ computedAmounts rule) :=
    ComputedEffectRefinement.computed_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash
      rateClone roleOrd cmpAccount cmpRole hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd
      assetOrd rateEq rateHash rateOrd roleClone model amounts direction proposal initial
      (fun amount member => bound amount (subset amount member)) (fun amount member => fits amount (subset amount member))
  obtain ⟨⟨consumeResult, consumed⟩, consumeCall, consumeValue⟩ := compute
    (alloc.vec.Vec.deref rule.computed_consume) .Consume effects (by intro amount member; simp [alloc.vec.Vec.deref] at member; simp [computedAmounts, member])
  simp [TransactionDecode.direction, alloc.vec.Vec.deref] at consumeValue
  cases consumeResult with
  | Err error =>
    refine ⟨.Err error, ?_, ?_⟩
    · simp [economy.Economy.aggregate_effects_loop.body,
        alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
        rate.Rate.computed_consumed, consumeCall, core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
    · simp [Transaction.computedChannels, TransactionDecode.rule, ← consumeValue,
        ComputedEffectRefinement.outcome, TransactionDecode.checked]
  | Ok value =>
    cases value
    obtain ⟨⟨produceResult, produced⟩, produceCall, produceValue⟩ := compute
      (alloc.vec.Vec.deref rule.computed_produce) .Produce consumed (by intro amount member; simp [alloc.vec.Vec.deref] at member; simp [computedAmounts, member])
    simp [TransactionDecode.direction, alloc.vec.Vec.deref] at produceValue
    cases produceResult with
    | Err error =>
      refine ⟨.Err error, ?_, ?_⟩
      · simp [economy.Economy.aggregate_effects_loop.body,
          alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
          rate.Rate.computed_consumed, rate.Rate.computed_produced, consumeCall, produceCall, core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
      · simp [Transaction.computedChannels, TransactionDecode.rule, ← consumeValue, ← produceValue,
          ComputedEffectRefinement.outcome, TransactionDecode.checked]
    | Ok value =>
      cases value
      obtain ⟨⟨preserveResult, preserved⟩, preserveCall, preserveValue⟩ := compute
        (alloc.vec.Vec.deref rule.computed_preserve) .Preserve produced (by intro amount member; simp [alloc.vec.Vec.deref] at member; simp [computedAmounts, member])
      simp [TransactionDecode.direction, alloc.vec.Vec.deref] at preserveValue
      cases preserveResult with
      | Err error =>
        refine ⟨.Err error, ?_, ?_⟩
        · simp [economy.Economy.aggregate_effects_loop.body,
            alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
            rate.Rate.computed_consumed, rate.Rate.computed_produced, rate.Rate.computed_preserved,
            consumeCall, produceCall, preserveCall, core.result.Result.Insts.CoreOpsTry.branch,
            core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
        · simp [Transaction.computedChannels, TransactionDecode.rule, ← consumeValue, ← produceValue, ← preserveValue,
            ComputedEffectRefinement.outcome, TransactionDecode.checked]
      | Ok value =>
        refine ⟨.Ok preserved, ?_, ?_⟩
        · simp [economy.Economy.aggregate_effects_loop.body,
            alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
            rate.Rate.computed_consumed, rate.Rate.computed_produced, rate.Rate.computed_preserved,
            consumeCall, produceCall, preserveCall, core.result.Result.Insts.CoreOpsTry.branch]
        · simp [Transaction.computedChannels, TransactionDecode.rule, ← consumeValue, ← produceValue, ← preserveValue,
            ComputedEffectRefinement.outcome, TransactionDecode.checked]

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem aggregate_loop_refines (remaining : List Role)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (rule : rate.Rate Role A Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (boundStatic : ∀ role ∈ remaining, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id)
    (boundComputed : ∀ amount ∈ computedAmounts rule, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id)
    (fits : ∀ amount ∈ computedAmounts rule, TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)) :
    ∃ result, economy.Economy.aggregate_effects_loop accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        remaining model rule proposal effects = .ok result ∧
      TransactionDecode.checked TransactionDecode.effects result = (do
        let accumulated ← Transaction.staticEffects cmpAccount cmpRole (TransactionDecode.proposal proposal)
          (TransactionDecode.rule rule) remaining (TransactionDecode.effects effects)
        Transaction.computedChannels cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
          (ConditionRefinement.context cmpRole proposal model.accounts) accumulated) := by
  induction remaining generalizing effects with
  | nil =>
    obtain ⟨result, called, decoded⟩ := empty_body_refines accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
      hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd model rule proposal effects boundComputed fits
    refine ⟨result, ?_, ?_⟩
    · unfold economy.Economy.aggregate_effects_loop
      rw [loop.eq_1]
      simp [called]
    · simpa [Transaction.staticEffects] using decoded
  | cons role rest ih =>
    obtain ⟨id, binding⟩ := boundStatic role (by simp)
    let position := RateConstruction.locate cmpAccount id effects
    let initial := position.old.getD ComputedEffectRefinement.emptyEffect
    have called := StaticEffectRefinement.static_body_refines accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
      hAccountClone hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd model rule proposal role rest effects id binding
    have bindingDecoded : Evaluation.orderedFind cmpRole role (TransactionDecode.proposal proposal).bindings = some id := binding
    have mergedValue := StaticEffectRefinement.raw_merge_decode cmpRole proposal rule role initial
    have initialValue : (RateConstruction.locate cmpAccount id (TransactionDecode.effects effects)).old.getD {} =
        TransactionDecode.effect initial := by
      rw [TransactionDecode.effects, ComputedEffectRefinement.locate_map]
      exact (ComputedEffectRefinement.default_decode position).symm
    cases merged : StaticEffectRefinement.rawMerge cmpRole proposal.units rule role initial with
    | error asset =>
      have naturalMerge : Transaction.mergeStatic cmpRole (TransactionDecode.proposal proposal)
          (TransactionDecode.rule rule) role (TransactionDecode.effect initial) =
          .error (.issue (.rateOverflow proposal.rate asset)) := by
        rw [merged] at mergedValue
        exact mergedValue.symm
      refine ⟨.Err (.RateOverflow proposal.rate asset), ?_, ?_⟩
      · unfold economy.Economy.aggregate_effects_loop
        rw [loop.eq_1]
        dsimp only [initial, position] at merged
        rw [merged] at called
        simp [called]
      · simp only [Transaction.staticEffects, bindingDecoded, Transaction.require,
          Transaction.checked_bind_ok, initialValue, naturalMerge, Transaction.checked_bind_error]
        rfl
    | ok updated =>
      have naturalMerge : Transaction.mergeStatic cmpRole (TransactionDecode.proposal proposal)
          (TransactionDecode.rule rule) role (TransactionDecode.effect initial) = .ok (TransactionDecode.effect updated) := by
        rw [merged] at mergedValue
        exact mergedValue.symm
      have updateValue : TransactionDecode.effects (position.put updated) =
          (RateConstruction.locate cmpAccount id (TransactionDecode.effects effects)).put (TransactionDecode.effect updated) := by
        rw [TransactionDecode.effects, TransactionDecode.effects, ComputedEffectRefinement.locate_map]
        exact ComputedEffectRefinement.put_map TransactionDecode.effect position updated
      obtain ⟨result, executed, decoded⟩ := ih (position.put updated) (fun value member => boundStatic value (by simp [member]))
      refine ⟨result, ?_, ?_⟩
      · unfold economy.Economy.aggregate_effects_loop
        rw [loop.eq_1]
        dsimp only [initial, position] at merged
        rw [merged] at called
        simp only [called, bind_ok]
        exact executed
      · rw [decoded, updateValue]
        simp only [Transaction.staticEffects, bindingDecoded, Transaction.require,
          Transaction.checked_bind_ok, initialValue, naturalMerge]

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem aggregate_refines (model : economy.Economy AccountId A RateId Role Std.U64)
    (rule : rate.Rate Role A Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (boundStatic : ∀ role ∈ rule.roles, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id)
    (boundComputed : ∀ amount ∈ computedAmounts rule, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id)
    (fits : ∀ amount ∈ computedAmounts rule, TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)) :
    ∃ result, economy.Economy.aggregate_effects accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model rule proposal = .ok result ∧
      TransactionDecode.checked TransactionDecode.effects result =
        Transaction.aggregate cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
          (ConditionRefinement.context cmpRole proposal model.accounts) := by
  obtain ⟨result, called, decoded⟩ := aggregate_loop_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd rule.roles model rule proposal []
    boundStatic boundComputed fits
  refine ⟨result, ?_, decoded⟩
  simpa [economy.Economy.aggregate_effects, alloc.collections.btree.map.BTreeMapKVGlobal.new,
    rate.Rate.impl.roles, alloc.collections.btree.set.BTreeSet.iter] using called

end Axionomy.AggregationRefinement
