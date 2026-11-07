import Axionomy.ComputedEffectRefinement

namespace Axionomy.StaticEffectRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type} [DecidableEq A]

def scaledChannel (target : basket.Basket A Std.U64) (source : Option (basket.Basket A Std.U64)) (units : Std.U64) :
    Except A (List (A × Std.U64)) :=
  match source with
  | none => .ok target.quantities
  | some source => Effects.scaled arithmetic U64.checked_mul target.quantities source.quantities units

def preservedChannel (target : basket.Basket A Std.U64) (source : Option (basket.Basket A Std.U64)) :
    Except A (List (A × Std.U64)) :=
  match source with
  | none => .ok target.quantities
  | some source => Account.credit arithmetic target.quantities source.quantities

def assemble (consumed produced preserved : Except A (List (A × Std.U64))) : Except A (economy.Effect A Std.U64) :=
  match consumed with
  | .error asset => .error asset
  | .ok consume => match produced with
    | .error asset => .error asset
    | .ok produce => match preserved with
      | .error asset => .error asset
      | .ok preserve => .ok ⟨⟨consume⟩, ⟨produce⟩, ⟨preserve⟩⟩

def rawMerge (cmp : Role → Role → Ordering) (units : Std.U64) (rule : rate.Rate Role A Std.U64)
    (role : Role) (effect : economy.Effect A Std.U64) : Except A (economy.Effect A Std.U64) :=
  assemble (scaledChannel effect.consume (Evaluation.orderedFind cmp role rule.consume) units)
    (scaledChannel effect.produce (Evaluation.orderedFind cmp role rule.produce) units)
    (preservedChannel effect.preserve (Evaluation.orderedFind cmp role rule.preserve))

theorem scaled_channel_decode (id : RateId) (target : basket.Basket A Std.U64)
    (source : Option (basket.Basket A Std.U64)) (units : Std.U64) :
    Transaction.staticScaled (AccountId := AccountId) (Role := Role) id units.val (TransactionDecode.balances target)
        (source.map TransactionDecode.balances) =
      Transaction.rateResult id ((scaledChannel target source units).map (Account.mapEntries UScalar.val)) := by
  cases source with
  | none => rfl
  | some source =>
    simp only [Option.map_some, scaledChannel, Transaction.staticScaled]
    rw [Effects.scaled_map arithmetic _ UScalar.val natural_hom U64.checked_mul _ BasketOperationsRefinement.mul_exact]
    rfl

theorem preserved_channel_decode (id : RateId) (target : basket.Basket A Std.U64)
    (source : Option (basket.Basket A Std.U64)) :
    Transaction.staticPreserved (AccountId := AccountId) (Role := Role) id (TransactionDecode.balances target)
        (source.map TransactionDecode.balances) =
      Transaction.rateResult id ((preservedChannel target source).map (Account.mapEntries UScalar.val)) := by
  cases source with
  | none => rfl
  | some source =>
    simp only [Option.map_some, preservedChannel, Transaction.staticPreserved]
    rw [Account.map_credit arithmetic _ UScalar.val natural_hom]
    rfl

theorem raw_merge_decode (cmp : Role → Role → Ordering) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (rule : rate.Rate Role A Std.U64) (role : Role) (effect : economy.Effect A Std.U64) :
    Transaction.rateResult proposal.rate ((rawMerge cmp proposal.units rule role effect).map TransactionDecode.effect) =
      Transaction.mergeStatic cmp (TransactionDecode.proposal proposal) (TransactionDecode.rule rule) role (TransactionDecode.effect effect) := by
  simp only [Transaction.mergeStatic, TransactionDecode.proposal, TransactionDecode.rule, TransactionDecode.effect,
    ← TransactionDecode.orderedFind_map]
  rw [scaled_channel_decode, scaled_channel_decode, preserved_channel_decode]
  unfold rawMerge
  cases scaledChannel effect.consume (Evaluation.orderedFind cmp role rule.consume) proposal.units <;>
    cases scaledChannel effect.produce (Evaluation.orderedFind cmp role rule.produce) proposal.units <;>
    cases preservedChannel effect.preserve (Evaluation.orderedFind cmp role rule.preserve) <;>
    simp [assemble, Except.map, Transaction.rateResult, TransactionDecode.effect, TransactionDecode.balances]

section Body
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountOrd : LawfulOrdering accountOrd cmpAccount)
variable (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)
variable (hRateClone : LawfulClone rateClone) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

include hAccountClone hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem static_body_refines (model : economy.Economy AccountId A RateId Role Std.U64)
    (rule : rate.Rate Role A Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (role : Role) (rest : List Role) (effects : List (AccountId × economy.Effect A Std.U64))
    (id : AccountId) (binding : Evaluation.orderedFind cmpRole role proposal.bindings = some id) :
    economy.Economy.aggregate_effects_loop.body accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      model rule proposal (role :: rest) effects =
      .ok (match rawMerge cmpRole proposal.units rule role ((RateConstruction.locate cmpAccount id effects).old.getD ComputedEffectRefinement.emptyEffect) with
        | .error asset => .done (.Err (.RateOverflow proposal.rate asset))
        | .ok updated => .cont (rest, (RateConstruction.locate cmpAccount id effects).put updated)) := by
  let position := RateConstruction.locate cmpAccount id effects
  have located : treeLocate accountOrd.cmp id effects = .ok (encodePosition position) := locate_exact accountOrd cmpAccount hAccountOrd id effects
  have defaulted := entry_default accountOrd (economy.Effect.Insts.CoreDefaultDefault A Std.U64)
    ComputedEffectRefinement.emptyEffect ComputedEffectRefinement.default_refines position
  have scaleCall (target source : basket.Basket A Std.U64) := EffectsRefinement.scaled_refines assetClone assetEq assetHash
    hAssetClone hAssetEq target.quantities source.quantities proposal.units
  simp only [merge_effects] at scaleCall
  have addCall (target source : basket.Basket A Std.U64) := BasketOperationsRefinement.add_refines assetClone assetEq assetHash
    hAssetClone hAssetEq target.quantities source.quantities
  simp only [basket_add] at addCall
  simp [economy.Economy.aggregate_effects_loop.body,
    alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next, bind_ok,
    exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
    EvaluationRefinement.tree_get_exact roleOrd cmpRole hRoleOrd, binding, core.option.Option.expect,
    hAccountClone id, alloc.collections.btree.map.BTreeMap.entry, located, defaulted,
    rate.Rate.consumed, rate.Rate.produced, rate.Rate.preserved, exchange.Exchange.impl.units,
    rawMerge]
  cases consume : Evaluation.orderedFind cmpRole role rule.consume <;>
    cases produce : Evaluation.orderedFind cmpRole role rule.produce <;>
    cases preserve : Evaluation.orderedFind cmpRole role rule.preserve <;>
    simp only [scaledChannel, preservedChannel, scaleCall, addCall, bind_ok]
  all_goals simp_all only [BasketOperationsRefinement.addResult, assemble]
  all_goals
    repeat' (split <;> simp_all [exchange.Exchange.impl.rate, hRateClone proposal.rate,
      treeRestore, RateConstruction.Position.put, position])
  all_goals rfl

end Body
end Axionomy.StaticEffectRefinement
