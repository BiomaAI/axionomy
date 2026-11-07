import Axionomy.ConditionRefinement
import Axionomy.EffectsRefinement

namespace Axionomy.ComputedEffectRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

def selected (direction : economy.ComputedDirection) (effect : economy.Effect A Std.U64) : basket.Basket A Std.U64 :=
  match direction with | .Consume => effect.consume | .Produce => effect.produce | .Preserve => effect.preserve

def setChannel (direction : economy.ComputedDirection) (effect : economy.Effect A Std.U64) (entries : List (A × Std.U64)) :
    economy.Effect A Std.U64 :=
  match direction with
  | .Consume => { effect with consume := ⟨entries⟩ }
  | .Produce => { effect with produce := ⟨entries⟩ }
  | .Preserve => { effect with preserve := ⟨entries⟩ }

def rawMerge [DecidableEq A] (direction : economy.ComputedDirection) (effect : economy.Effect A Std.U64)
    (asset : A) (value : Std.U64) : Except A (economy.Effect A Std.U64) :=
  (Account.credit arithmetic (selected direction effect).quantities [(asset, value)]).map (setChannel direction effect)

def encodeOne (original : economy.Effect A Std.U64) : Except A (economy.Effect A Std.U64) →
    core.result.Result Unit A × economy.Effect A Std.U64
  | .ok result => (.Ok (), result)
  | .error asset => (.Err asset, original)

theorem merge_one_refines [DecidableEq A] (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A)
    (assetHash : core.hash.Hash A) (hEq : LawfulEquality assetEq)
    (direction : economy.ComputedDirection) (effect : economy.Effect A Std.U64) (asset : A) (value : Std.U64) :
    economy.Effect.merge_computed assetClone assetEq assetHash scalar effect direction asset value =
      .ok (encodeOne effect (rawMerge direction effect asset value)) := by
  have called (target : basket.Basket A Std.U64) := EffectsRefinement.amount_refines assetClone assetEq assetHash hEq target.quantities asset value
  simp only [merge_amount] at called
  cases direction <;>
    simp only [economy.Effect.merge_computed, called, bind_ok, rawMerge, selected]
  · cases Account.credit arithmetic effect.consume.quantities [(asset, value)] <;>
      simp [BasketOperationsRefinement.addResult, encodeOne, Except.map, setChannel]
  · cases Account.credit arithmetic effect.produce.quantities [(asset, value)] <;>
      simp [BasketOperationsRefinement.addResult, encodeOne, Except.map, setChannel]
  · cases Account.credit arithmetic effect.preserve.quantities [(asset, value)] <;>
      simp [BasketOperationsRefinement.addResult, encodeOne, Except.map, setChannel]

theorem raw_merge_decode [DecidableEq A] (direction : economy.ComputedDirection) (effect : economy.Effect A Std.U64)
    (asset : A) (value : Std.U64) :
    (rawMerge direction effect asset value).map TransactionDecode.effect =
      Transaction.mergeComputed (TransactionDecode.direction direction) asset value.val (TransactionDecode.effect effect) := by
  have mapped (target : basket.Basket A Std.U64) := Account.map_credit arithmetic (Account.naturals Quantity.u64Max)
    UScalar.val natural_hom [(asset, value)] target.quantities
  simp only [Account.mapEntries_cons, Account.mapEntries_nil] at mapped
  cases direction <;>
    simp only [rawMerge, selected, Transaction.mergeComputed, TransactionDecode.direction, TransactionDecode.effect,
      TransactionDecode.balances]
  · rw [← mapped effect.consume]
    cases Account.credit arithmetic effect.consume.quantities [(asset, value)] <;> rfl
  · rw [← mapped effect.produce]
    cases Account.credit arithmetic effect.produce.quantities [(asset, value)] <;> rfl
  · rw [← mapped effect.preserve]
    cases Account.credit arithmetic effect.preserve.quantities [(asset, value)] <;> rfl

theorem raw_merge_error [DecidableEq A] (direction : economy.ComputedDirection) (effect : economy.Effect A Std.U64)
    (asset : A) (value : Std.U64) (bad : A) (failed : rawMerge direction effect asset value = .error bad) : bad = asset := by
  unfold rawMerge at failed
  cases added : arithmetic.add (Account.balance arithmetic.zero (selected direction effect).quantities asset) value with
  | none => simpa [Account.credit, added, Except.map, eq_comm] using failed
  | some total => simp [Account.credit, added, Except.map] at failed

def mapPosition {K V W : Type} (f : V → W) (position : RateConstruction.Position K V) : RateConstruction.Position K W :=
  ⟨Account.mapEntries f position.before, position.key, position.old.map f, Account.mapEntries f position.after⟩

theorem locate_map {K V W : Type} (cmp : K → K → Ordering) (f : V → W) (query : K) (entries : List (K × V)) :
    RateConstruction.locate cmp query (Account.mapEntries f entries) = mapPosition f (RateConstruction.locate cmp query entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases compared : cmp query key <;> simp [RateConstruction.locate, compared, ih, mapPosition]

theorem put_map {K V W : Type} (f : V → W) (position : RateConstruction.Position K V) (value : V) :
    Account.mapEntries f (position.put value) = (mapPosition f position).put (f value) := by
  simp [RateConstruction.Position.put, Account.mapEntries, mapPosition]

def emptyEffect : economy.Effect A Std.U64 := ⟨⟨[]⟩, ⟨[]⟩, ⟨[]⟩⟩

theorem default_refines : (economy.Effect.Insts.CoreDefaultDefault A Std.U64).default = .ok emptyEffect := by
  simp [economy.Effect.Insts.CoreDefaultDefault.default, basket.Basket.new,
    indexmap.map.IndexMapKVRandomState.new, emptyEffect]

theorem default_decode (position : RateConstruction.Position AccountId (economy.Effect A Std.U64)) :
    TransactionDecode.effect (position.old.getD emptyEffect) = (mapPosition TransactionDecode.effect position).old.getD {} := by
  cases found : position.old <;> simp [mapPosition, found, emptyEffect, TransactionDecode.effect, TransactionDecode.balances]

-- The private helper may have partially accumulated effects when it rejects;
-- the enclosing aggregation discards that map. Only its success map is public.
def outcome (result : core.result.Result Unit (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128) ×
    List (AccountId × economy.Effect A Std.U64)) : Transaction.Checked AccountId A RateId Role (List (AccountId × Transaction.Effect A)) :=
  match result.1 with
  | .Ok () => .ok (TransactionDecode.effects result.2)
  | .Err error => .error (.issue (TransactionDecode.failure error))

section Iteration
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (rateClone : core.clone.Clone RateId) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetClone : LawfulClone assetClone)
variable (hAssetEq : LawfulEquality assetEq) (hRateClone : LawfulClone rateClone) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem computed_loop_refines (remaining : List (rate.ComputedAmount Role A Std.U64))
    (iter : core.slice.iter.Iter (rate.ComputedAmount Role A Std.U64)) (suffix : iter.slice.val.drop iter.i = remaining)
    (model : economy.Economy AccountId A RateId Role Std.U64) (direction : economy.ComputedDirection)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (bound : ∀ amount ∈ remaining, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id)
    (fits : ∀ amount ∈ remaining, TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)) :
    ∃ result, economy.Economy.merge_computed_effects_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash
        rateClone roleOrd scalar iter model direction proposal effects = .ok result ∧
      outcome result = Transaction.computedEffects cmpAccount cmpRole (TransactionDecode.proposal proposal)
        (ConditionRefinement.context cmpRole proposal model.accounts) (TransactionDecode.direction direction)
        (remaining.map TransactionDecode.computed) (TransactionDecode.effects effects) := by
  induction remaining generalizing iter effects with
  | nil =>
    refine ⟨(.Ok (), effects), ?_, rfl⟩
    unfold economy.Economy.merge_computed_effects_loop
    rw [loop.eq_1]
    simp [economy.Economy.merge_computed_effects_loop.body, ConservationRefinement.next_empty iter suffix]
  | cons amount rest ih =>
    obtain ⟨advance, tail⟩ := ConservationRefinement.next_cons iter amount rest suffix
    obtain ⟨evaluated, evaluateCall, evaluateValue⟩ := EvaluationRefinement.evaluate_refines_naturals accountEq accountHash assetEq assetHash roleOrd cmpRole
      hAccountEq hAssetEq hRoleOrd proposal model.accounts amount.quantity
    change economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar amount.quantity proposal model.accounts = _ at evaluateCall
    rw [TransactionDecode.context_refines] at evaluateValue
    change Evaluation.mapResult UScalar.val evaluated = Evaluation.eval (Evaluation.naturals Quantity.u64Max)
      (ConditionRefinement.context cmpRole proposal model.accounts) (TransactionDecode.expression amount.quantity) at evaluateValue
    cases evaluated with
    | error error =>
      have errorFits := TransactionDiagnostics.expression_error_fits (Evaluation.naturals Quantity.u64Max)
        (ConditionRefinement.context cmpRole proposal model.accounts) (TransactionDiagnostics.state_balance_errors cmpRole 0 _ _ _ _)
        (TransactionDecode.expression amount.quantity) (fits amount (by simp)) error evaluateValue.symm
      have message := DiagnosticRefinement.message_refines error errorFits
      change economy.ExpressionError.into_message (EvaluationRefinement.encodeError error) = .ok error.message at message
      have body : economy.Economy.merge_computed_effects_loop.body accountClone accountEq accountHash accountOrd assetClone assetEq assetHash
          rateClone roleOrd scalar model direction proposal iter effects =
          .ok (.done (.Err (.RateLawEvaluation proposal.rate error.message), effects)) := by
        simp [economy.Economy.merge_computed_effects_loop.body, advance, rate.ComputedAmount.impl.quantity,
          evaluateCall, EvaluationRefinement.encodeResult, exchange.Exchange.impl.rate, hRateClone proposal.rate, message]
      refine ⟨(.Err (.RateLawEvaluation proposal.rate error.message), effects), ?_, ?_⟩
      · unfold economy.Economy.merge_computed_effects_loop
        rw [loop.eq_1]
        simp [body]
      · simp [Transaction.computedEffects, TransactionDecode.computed, ← evaluateValue,
          Evaluation.mapResult, outcome, TransactionDecode.failure, TransactionDecode.proposal]
    | ok value =>
      obtain ⟨id, binding⟩ := bound amount (by simp)
      let position := RateConstruction.locate cmpAccount id effects
      let initial := position.old.getD emptyEffect
      have located : treeLocate accountOrd.cmp id effects = .ok (encodePosition position) :=
        locate_exact accountOrd cmpAccount hAccountOrd id effects
      have defaulted := entry_default accountOrd (economy.Effect.Insts.CoreDefaultDefault A Std.U64) emptyEffect default_refines position
      have mergedCall := merge_one_refines assetClone assetEq assetHash hAssetEq direction initial amount.asset value
      have mergedValue := raw_merge_decode direction initial amount.asset value
      have initialValue : (RateConstruction.locate cmpAccount id (TransactionDecode.effects effects)).old.getD {} =
          TransactionDecode.effect initial := by
        rw [TransactionDecode.effects, locate_map]
        exact (default_decode position).symm
      cases merged : rawMerge direction initial amount.asset value with
      | error bad =>
        have same := raw_merge_error direction initial amount.asset value bad merged
        subst bad
        have naturalMerge : Transaction.mergeComputed (TransactionDecode.direction direction) amount.asset value.val
            (TransactionDecode.effect initial) = .error amount.asset := by rw [merged] at mergedValue; exact mergedValue.symm
        have executedMerge := mergedCall
        rw [merged] at executedMerge
        dsimp only [encodeOne, initial] at executedMerge
        have body : economy.Economy.merge_computed_effects_loop.body accountClone accountEq accountHash accountOrd assetClone assetEq assetHash
            rateClone roleOrd scalar model direction proposal iter effects =
            .ok (.done (.Err (.RateOverflow proposal.rate amount.asset), position.put initial)) := by
          simp [economy.Economy.merge_computed_effects_loop.body, advance, rate.ComputedAmount.impl.quantity,
            evaluateCall, EvaluationRefinement.encodeResult, exchange.Exchange.impl.bindings, rate.ComputedAmount.impl.role,
            alloc.collections.btree.map.BTreeMap.get, EvaluationRefinement.tree_get_exact roleOrd cmpRole hRoleOrd, binding,
            core.option.Option.expect, hAccountClone id, alloc.collections.btree.map.BTreeMap.entry,
            located, defaulted, rate.ComputedAmount.impl.asset,
            hAssetClone amount.asset, executedMerge, core.result.Result.is_err,
            exchange.Exchange.impl.rate, hRateClone proposal.rate, treeRestore, RateConstruction.Position.put, initial]
        refine ⟨(.Err (.RateOverflow proposal.rate amount.asset), position.put initial), ?_, ?_⟩
        · unfold economy.Economy.merge_computed_effects_loop
          rw [loop.eq_1]
          simp [body]
        · simp only [List.map_cons, Transaction.computedEffects, TransactionDecode.computed, ← evaluateValue,
            Evaluation.mapResult, Transaction.checked_bind_ok, TransactionDecode.proposal, binding, Transaction.require,
            initialValue, naturalMerge, Transaction.rateResult, Transaction.checked_bind_error]
          rfl
      | ok updated =>
        have naturalMerge : Transaction.mergeComputed (TransactionDecode.direction direction) amount.asset value.val
            (TransactionDecode.effect initial) = .ok (TransactionDecode.effect updated) := by rw [merged] at mergedValue; exact mergedValue.symm
        have executedMerge := mergedCall
        rw [merged] at executedMerge
        dsimp only [encodeOne, initial] at executedMerge
        have updateValue : TransactionDecode.effects (position.put updated) =
            (RateConstruction.locate cmpAccount id (TransactionDecode.effects effects)).put (TransactionDecode.effect updated) := by
          rw [TransactionDecode.effects, TransactionDecode.effects, locate_map]
          exact put_map TransactionDecode.effect position updated
        obtain ⟨result, called, decoded⟩ := ih { iter with i := iter.i + 1 } tail (position.put updated)
          (fun item member => bound item (by simp [member])) (fun item member => fits item (by simp [member]))
        have body : economy.Economy.merge_computed_effects_loop.body accountClone accountEq accountHash accountOrd assetClone assetEq assetHash
            rateClone roleOrd scalar model direction proposal iter effects =
            .ok (.cont ({ iter with i := iter.i + 1 }, position.put updated)) := by
          simp [economy.Economy.merge_computed_effects_loop.body, advance, rate.ComputedAmount.impl.quantity,
            evaluateCall, EvaluationRefinement.encodeResult, exchange.Exchange.impl.bindings, rate.ComputedAmount.impl.role,
            alloc.collections.btree.map.BTreeMap.get, EvaluationRefinement.tree_get_exact roleOrd cmpRole hRoleOrd, binding,
            core.option.Option.expect, hAccountClone id, alloc.collections.btree.map.BTreeMap.entry,
            located, defaulted, rate.ComputedAmount.impl.asset,
            hAssetClone amount.asset, executedMerge, core.result.Result.is_err,
            treeRestore, RateConstruction.Position.put]
        refine ⟨result, ?_, ?_⟩
        · unfold economy.Economy.merge_computed_effects_loop
          rw [loop.eq_1]
          simp only [body, bind_ok]
          exact called
        · rw [decoded, updateValue]
          simp only [List.map_cons, Transaction.computedEffects, TransactionDecode.computed, ← evaluateValue,
            Evaluation.mapResult, Transaction.checked_bind_ok, TransactionDecode.proposal, binding, Transaction.require,
            initialValue, naturalMerge, Transaction.rateResult]

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem computed_refines (assetOrd : core.cmp.Ord A) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId)
    (rateOrd : core.cmp.Ord RateId) (roleClone : core.clone.Clone Role)
    (model : economy.Economy AccountId A RateId Role Std.U64) (amounts : Slice (rate.ComputedAmount Role A Std.U64))
    (direction : economy.ComputedDirection) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (bound : ∀ amount ∈ amounts.val, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id)
    (fits : ∀ amount ∈ amounts.val, TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)) :
    ∃ result, economy.Economy.merge_computed_effects accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model amounts direction proposal effects = .ok result ∧
      outcome result = Transaction.computedEffects cmpAccount cmpRole (TransactionDecode.proposal proposal)
        (ConditionRefinement.context cmpRole proposal model.accounts) (TransactionDecode.direction direction)
        (amounts.val.map TransactionDecode.computed) (TransactionDecode.effects effects) := by
  obtain ⟨result, called, decoded⟩ := computed_loop_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash
    rateClone roleOrd cmpAccount cmpRole hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd
    amounts.val ⟨amounts, 0⟩ (by simp) model direction proposal effects bound fits
  refine ⟨result, ?_, decoded⟩
  simpa [economy.Economy.merge_computed_effects, SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
    core.slice.Slice.iter] using called

end Iteration

end Axionomy.ComputedEffectRefinement
