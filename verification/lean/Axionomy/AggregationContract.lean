import Axionomy.AggregationRefinement
import Axionomy.OrderedEffects
import Axionomy.ValidationCompleteRefinement
import Axionomy.RateWellFormed
import Axionomy.AvailabilityRefinement

namespace Axionomy.AggregationContract
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

theorem balance_valid_iff (basket : basket.Basket A Std.U64) :
    BasketOperations.Valid (Account.naturals Quantity.u64Max) (TransactionDecode.balances basket) ↔
      BasketOperations.Valid arithmetic basket.quantities := by
  simp only [BasketOperations.Valid, TransactionDecode.balances, Account.mapEntries, List.map_map,
    Function.comp_def, List.forall_mem_map, natural_hom.isZero]

def Canonical (effect : economy.Effect A Std.U64) : Prop :=
  BasketOperations.Valid arithmetic effect.consume.quantities ∧
    BasketOperations.Valid arithmetic effect.produce.quantities ∧
    BasketOperations.Valid arithmetic effect.preserve.quantities

theorem effect_valid_iff (effect : economy.Effect A Std.U64) :
    TransactionAggregation.ValidEffect (TransactionDecode.effect effect) ↔ Canonical effect := by
  simp [TransactionAggregation.ValidEffect, TransactionDecode.effect, Canonical, balance_valid_iff]

structure Ready (cmp : AccountId → AccountId → Ordering) (accounts : List (AccountId × account.Account A Std.U64))
    (effects : List (AccountId × economy.Effect A Std.U64)) : Prop where
  present : ∀ entry ∈ effects, entry.1 ∈ accounts.map Prod.fst
  canonical : ∀ entry ∈ effects, Canonical entry.2
  sorted : (effects.map Prod.fst).Pairwise (fun x y => cmp x y = .lt)
  unique : (effects.map Prod.fst).Nodup
  length : effects.length ≤ accounts.length

theorem computedAmounts_decode (rule : rate.Rate Role A Std.U64) :
    (AggregationRefinement.computedAmounts rule).map TransactionDecode.computed =
      (TransactionDecode.rule rule).computedConsume ++ (TransactionDecode.rule rule).computedProduce ++
        (TransactionDecode.rule rule).computedPreserve := by
  simp [AggregationRefinement.computedAmounts, TransactionDecode.rule]

section Decode
variable [DecidableEq AccountId] [DecidableEq A]

theorem decoded_ready (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderedEffects.OrderLaws cmpAccount) (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (rule : rate.Rate Role A Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64))
    (boundStatic : ∀ role ∈ rule.roles, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ id ∈ model.accounts.map Prod.fst)
    (boundComputed : ∀ amount ∈ AggregationRefinement.computedAmounts rule,
      ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ id ∈ model.accounts.map Prod.fst)
    (success : Transaction.aggregate cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
      (ConditionRefinement.context cmpRole proposal model.accounts) = .ok (TransactionDecode.effects effects)) :
    Ready cmpAccount model.accounts effects := by
  have computedBound : ∀ amount ∈ (TransactionDecode.rule rule).computedConsume ++ (TransactionDecode.rule rule).computedProduce ++
      (TransactionDecode.rule rule).computedPreserve,
      ∃ id, Evaluation.orderedFind cmpRole amount.role (TransactionDecode.proposal proposal).bindings = some id ∧ id ∈ model.accounts.map Prod.fst := by
    intro amount member
    rw [← computedAmounts_decode] at member
    obtain ⟨raw, present, same⟩ := List.mem_map.mp member
    subst amount
    exact boundComputed raw present
  have result := OrderedEffects.aggregate_contract cmpAccount cmpRole laws (model.accounts.map Prod.fst)
    (TransactionDecode.proposal proposal) (TransactionDecode.rule rule) (ConditionRefinement.context cmpRole proposal model.accounts)
    (TransactionDecode.effects effects) boundStatic computedBound success
  have entryValid (entry : AccountId × economy.Effect A Std.U64) (member : entry ∈ effects) :=
    result.1 (entry.1, TransactionDecode.effect entry.2) (List.mem_map.mpr ⟨entry, member, rfl⟩)
  refine ⟨fun entry member => (entryValid entry member).1,
    fun entry member => (effect_valid_iff entry.2).mp (entryValid entry member).2, ?_, ?_, ?_⟩
  · simpa only [OrderedEffects.Sorted, TransactionDecode.effects_keys] using result.2.1
  · simpa only [TransactionDecode.effects_keys] using result.2.2.1
  · simpa [TransactionDecode.effects, Account.mapEntries] using result.2.2.2

end Decode

section Implementation
variable [DecidableEq AccountId] [DecidableEq A]
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
theorem aggregate_ready (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64) (rule : rate.Rate Role A Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (boundStatic : ∀ role ∈ rule.roles, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ id ∈ model.accounts.map Prod.fst)
    (boundComputed : ∀ amount ∈ AggregationRefinement.computedAmounts rule,
      ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ id ∈ model.accounts.map Prod.fst)
    (fits : ∀ amount ∈ AggregationRefinement.computedAmounts rule,
      TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)) :
    ∃ result, economy.Economy.aggregate_effects accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model rule proposal = .ok result ∧
      TransactionDecode.checked TransactionDecode.effects result =
        Transaction.aggregate cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
          (ConditionRefinement.context cmpRole proposal model.accounts) ∧
      ∀ effects, result = .Ok effects → Ready cmpAccount model.accounts effects := by
  obtain ⟨result, executed, decoded⟩ := AggregationRefinement.aggregate_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd model rule proposal
    (fun role member => let ⟨id, binding, _⟩ := boundStatic role member; ⟨id, binding⟩)
    (fun amount member => let ⟨id, binding, _⟩ := boundComputed amount member; ⟨id, binding⟩) fits
  refine ⟨result, executed, decoded, ?_⟩
  intro effects success
  subst result
  exact decoded_ready cmpAccount cmpRole laws model proposal rule effects boundStatic boundComputed decoded.symm

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd in
theorem validated_aggregate [DecidableEq RateId] [DecidableEq Role]
    (hRateEq : LawfulEquality rateEq) (hRoleClone : LawfulClone roleClone) (hRoleEq : LawfulEquality roleOrd.eqInst)
    (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64) (rule : rate.Rate Role A Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (ruleFits : ∀ rule, Account.find? proposal.rate model.rates = some rule →
      RateDiscoveryRefinement.RateNamesFit rule ∧ ValidationCompleteRefinement.issueBudget rule proposal ≤ Usize.max)
    (targetsRegistered : RateWellFormed.RegisteredTargets rule)
    (fits : ∀ amount ∈ AggregationRefinement.computedAmounts rule,
      TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity))
    (accepted : validate_model_proposal accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal = .ok (.Ok rule)) :
    ∃ result, economy.Economy.aggregate_effects accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model rule proposal = .ok result ∧
      TransactionDecode.checked TransactionDecode.effects result =
        Transaction.aggregate cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
          (ConditionRefinement.context cmpRole proposal model.accounts) ∧
      ∀ effects, result = .Ok effects → Ready cmpAccount model.accounts effects := by
  have preconditions := ValidationCompleteRefinement.accepted_has_accounts accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd laws.equal
    model proposal parameterFits ruleFits rule accepted
  have bound : ∀ role ∈ rule.roles, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ id ∈ model.accounts.map Prod.fst := by
    intro role member
    obtain ⟨id, holder, binding, found⟩ := preconditions.2.2 role member
    exact ⟨id, binding, (Transaction.find_exists_iff_key_mem id model.accounts).mp ⟨holder, found⟩⟩
  exact aggregate_ready accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole hAccountClone hAccountEq hAccountOrd
    hAssetClone hAssetEq hRateClone hRoleOrd laws model rule proposal bound
    (fun amount member => bound amount.role (targetsRegistered amount member)) fits

end Implementation

section Downstream
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)
variable (hRateClone : LawfulClone rateClone)

include hAccountClone hAccountEq hAssetClone hAssetEq hRateClone in
theorem ready_preflight (cmp : AccountId → AccountId → Ordering)
    (model : economy.Economy AccountId A RateId Role Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64)) (ready : Ready cmp model.accounts effects)
    (capacity : model.accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.preflight_effects accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal effects = .ok result ∧
      TransactionDecode.checked TransactionDecode.availability result =
        Transaction.preflight proposal.rate (TransactionDecode.accounts model.accounts) (TransactionDecode.effects effects) ⟨[], []⟩ := by
  exact AvailabilityRefinement.preflight_refines accountClone accountEq accountHash assetClone assetEq assetHash rateClone roleOrd
    hAccountClone hAccountEq hAssetClone hAssetEq hRateClone accountOrd assetOrd rateEq rateHash rateOrd roleClone model proposal effects
    ready.present (fun entry member => (ready.canonical entry member).1) (Nat.le_trans ready.length capacity)

include hAccountClone hAccountEq hAssetClone hAssetEq in
theorem ready_prepare (cmp : AccountId → AccountId → Ordering)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (effects : List (AccountId × economy.Effect A Std.U64)) (ready : Ready cmp model.accounts effects)
    (capacity : model.accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.prepare_effects accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model effects = .ok result ∧
      TransactionDecode.checked TransactionDecode.preparation result =
        Transaction.prepare (TransactionDecode.accounts model.accounts) (TransactionDecode.effects effects) ⟨[], []⟩ ∧
      ∀ prepared, result = .Ok prepared →
        prepared.accounts.val.map Prod.fst = effects.map Prod.fst ∧
        prepared.deltas.val.map TransactionDecode.delta = (TransactionDecode.effects effects).map Transaction.effectDelta ∧
        List.Forall₂ (Transaction.PreparedEntry (TransactionDecode.accounts model.accounts))
          (TransactionDecode.effects effects) (TransactionDecode.accounts prepared.accounts.val) ∧
        (∀ entry ∈ prepared.accounts.val, entry.1 ∈ model.accounts.map Prod.fst) ∧
        (prepared.accounts.val.map Prod.fst).Nodup := by
  obtain ⟨result, executed, decoded⟩ := PreparationRefinement.prepare_refines accountClone accountEq accountHash assetClone assetEq assetHash
    hAccountClone hAccountEq hAssetClone hAssetEq accountOrd assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model effects
    ready.present (fun entry member => (ready.canonical entry member).1.1) (Nat.le_trans ready.length capacity)
  refine ⟨result, executed, decoded, ?_⟩
  intro prepared success
  subst result
  have specification : Transaction.prepare (RateId := RateId) (Role := Role)
      (TransactionDecode.accounts model.accounts) (TransactionDecode.effects effects) ⟨[], []⟩ =
      .ok (TransactionDecode.preparation prepared) := decoded.symm
  have contract := Transaction.preparation_contract _ _ _ specification
  have keys : prepared.accounts.val.map Prod.fst = effects.map Prod.fst := by
    simpa [TransactionDecode.preparation, TransactionDecode.accounts_keys, TransactionDecode.effects_keys] using contract.1
  refine ⟨keys, contract.2.1, contract.2.2.1, ?_, ?_⟩
  · intro entry member
    have present := contract.2.2.2.1 (entry.1, TransactionDecode.holder entry.2) (List.mem_map.mpr ⟨entry, member, rfl⟩)
    simpa only [TransactionDecode.accounts_keys] using present
  · rw [keys]; exact ready.unique

end Downstream
end Axionomy.AggregationContract
