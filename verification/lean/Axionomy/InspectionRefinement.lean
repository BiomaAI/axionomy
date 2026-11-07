import Axionomy.Inspection
import Axionomy.TypedDataRefinement
import Axionomy.IteratorContract

namespace Axionomy.InspectionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement
variable {AccountId A RateId Role N M : Type}

/-! These inspection operations transport generic numeric values without
invoking arithmetic. Iterator contracts describe every suffix, not only the
first element returned by a fresh iterator. -/

theorem iterator_length (count : Nat) (fits : count ≤ Usize.max) :
    iteratorLength count = .ok (Usize.ofNatCore count (by scalar_tac)) := by
  simp only [iteratorLength, dite_eq_left (show count < 2^UScalarTy.Usize.numBits by scalar_tac)]

theorem exact_hint (count : Nat) (fits : count ≤ Usize.max) :
    exactIteratorHint count = .ok (Usize.ofNatCore count (by scalar_tac),
      some (Usize.ofNatCore count (by scalar_tac))) := by
  simp [exactIteratorHint, iterator_length count fits]

theorem account_next (entries : List (AccountId × account.Account A N)) :
    economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.next entries =
      .ok (entries.head?, entries.tail) := by
  cases entries with
  | nil => simp [economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.next,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨id, holder⟩
    simp [economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.next,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
      alloc.sync.Arc.Insts.CoreConvertAsRef.as_ref]

theorem accounts_yield (entries : List (AccountId × account.Account A N)) :
    IteratorContract.Yields
      economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.next entries entries := by
  induction entries with
  | nil => exact .done (account_next [])
  | cons entry rest ih => exact .step (account_next (entry :: rest)) ih

theorem accounts_hint (entries : List (AccountId × account.Account A N)) :
    economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.size_hint entries =
      exactIteratorHint entries.length := rfl

theorem weight_next (entries : List (A × Std.I64)) :
    rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.next entries =
      .ok (entries.head?, entries.tail) := by
  cases entries <;> simp [rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.next,
    indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]

theorem weights_yield (entries : List (A × Std.I64)) :
    IteratorContract.Yields rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.next entries entries := by
  induction entries with
  | nil => exact .done (weight_next [])
  | cons entry rest ih => exact .step (weight_next (entry :: rest)) ih

theorem weights_hint (entries : List (A × Std.I64)) :
    rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.size_hint entries =
      exactIteratorHint entries.length := rfl

theorem keys_yield {K V : Type} (keys : List K) :
    IteratorContract.Yields
      (indexmap.map.iter.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next (V := V)) keys keys := by
  induction keys with
  | nil => exact .done rfl
  | cons key rest ih => exact .step rfl ih

section Invariants
variable (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A)

theorem invariant_new {T : Type} (conversion : core.convert.Into T String)
    (input : T) (label : String) (law : conversion.into input = .ok label) :
    rate.LinearInvariant.new eqA hashA conversion input = .ok ⟨label, []⟩ := by
  simp [rate.LinearInvariant.new, law, indexmap.map.IndexMapKVRandomState.new]

theorem invariant_new_failure {T : Type} (conversion : core.convert.Into T String)
    (input : T) (error : Error) (law : conversion.into input = .fail error) :
    rate.LinearInvariant.new eqA hashA conversion input = .fail error := by
  simp [rate.LinearInvariant.new, law]

theorem invariant_name (value : rate.LinearInvariant A)
    (fits : value.name.toByteArray.size ≤ Usize.max) :
    rate.LinearInvariant.impl.name eqA hashA value = .ok (encodeString value.name fits) := by
  exact DiagnosticRefinement.deref_exact value.name fits

theorem invariant_weights (value : rate.LinearInvariant A) :
    rate.LinearInvariant.impl.weights eqA hashA value = .ok value.weights := by
  simp [rate.LinearInvariant.impl.weights, indexmap.map.IndexMap.iter]

theorem invariant_weight [DecidableEq A] (law : LawfulEquality eqA)
    (value : rate.LinearInvariant A) (asset : A) (weight : Std.I64) :
    rate.LinearInvariant.weight eqA hashA value asset weight =
      .ok { value with weights := if weight = 0#i64 then Account.erase asset value.weights
        else Account.upsert asset weight value.weights } := by
  by_cases zero : weight = 0#i64
  · simp [rate.LinearInvariant.weight, zero, indexmap.map.IndexMap.shift_remove,
      equivalent_eq, remove_exact eqA law]
  · simp [rate.LinearInvariant.weight, zero, indexmap.map.IndexMap.insert,
      insert_exact eqA law]

theorem invariant_weight_contract [DecidableEq A] (law : LawfulEquality eqA)
    (value : rate.LinearInvariant A) (asset : A) (weight : Std.I64)
    (valid : Inspection.ValidWeights (Account.mapEntries IScalar.val value.weights)) :
    ∃ after, rate.LinearInvariant.weight eqA hashA value asset weight = .ok after ∧
      after.name = value.name ∧
      Account.mapEntries IScalar.val after.weights =
        Inspection.weight (Account.mapEntries IScalar.val value.weights) asset weight.val ∧
      Inspection.ValidWeights (Account.mapEntries IScalar.val after.weights) := by
  let after : rate.LinearInvariant A := { value with
    weights := if weight = 0#i64 then Account.erase asset value.weights
      else Account.upsert asset weight value.weights }
  have zero : weight = 0#i64 ↔ weight.val = 0 := by
    constructor
    · intro h; subst weight; rfl
    · intro h; exact IScalar.eq_of_val_eq h
  have decoded : Account.mapEntries IScalar.val after.weights =
      Inspection.weight (Account.mapEntries IScalar.val value.weights) asset weight.val := by
    by_cases empty : weight = 0#i64
    · simp [after, empty, Inspection.weight, Account.map_erase]
    · simp [after, empty, Inspection.weight, mt zero.mpr empty, Account.map_upsert]
  refine ⟨after, invariant_weight eqA hashA law value asset weight, rfl, decoded, ?_⟩
  rw [decoded]
  exact Inspection.weights_valid _ _ _ valid

end Invariants

section Rates
variable (cloneRole : core.clone.Clone Role) (ordRole : core.cmp.Ord Role)
  (cloneA : core.clone.Clone A) (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A)
  (scalar : quantity.QuantityScalar N M)

theorem rate_roles (rule : rate.Rate Role A N) :
    rate.Rate.impl.roles cloneRole ordRole cloneA eqA hashA scalar rule = .ok rule.roles := rfl

theorem rate_distinct (rule : rate.Rate Role A N) :
    rate.Rate.distinct_roles cloneRole ordRole cloneA eqA hashA scalar rule = .ok rule.distinct := rfl

theorem rate_computed_consumed (rule : rate.Rate Role A N) :
    rate.Rate.computed_consumed cloneRole ordRole cloneA eqA hashA scalar rule =
      .ok (alloc.vec.Vec.deref rule.computed_consume) := rfl

theorem rate_computed_produced (rule : rate.Rate Role A N) :
    rate.Rate.computed_produced cloneRole ordRole cloneA eqA hashA scalar rule =
      .ok (alloc.vec.Vec.deref rule.computed_produce) := rfl

theorem rate_computed_preserved (rule : rate.Rate Role A N) :
    rate.Rate.computed_preserved cloneRole ordRole cloneA eqA hashA scalar rule =
      .ok (alloc.vec.Vec.deref rule.computed_preserve) := rfl

theorem rate_conditions (rule : rate.Rate Role A N) :
    rate.Rate.impl.conditions cloneRole ordRole cloneA eqA hashA scalar rule =
      .ok (alloc.vec.Vec.deref rule.conditions) := rfl

theorem rate_consumed (cmp : Role → Role → Ordering)
    (law : RateConstructionRefinement.LawfulOrdering ordRole cmp)
    (rule : rate.Rate Role A N) (role : Role) :
    rate.Rate.consumed cloneRole ordRole cloneA eqA hashA scalar rule role =
      .ok (Evaluation.orderedFind cmp role rule.consume) := by
  exact EvaluationRefinement.tree_get_exact ordRole cmp law role rule.consume

theorem rate_produced (cmp : Role → Role → Ordering)
    (law : RateConstructionRefinement.LawfulOrdering ordRole cmp)
    (rule : rate.Rate Role A N) (role : Role) :
    rate.Rate.produced cloneRole ordRole cloneA eqA hashA scalar rule role =
      .ok (Evaluation.orderedFind cmp role rule.produce) := by
  exact EvaluationRefinement.tree_get_exact ordRole cmp law role rule.produce

theorem rate_preserved (cmp : Role → Role → Ordering)
    (law : RateConstructionRefinement.LawfulOrdering ordRole cmp)
    (rule : rate.Rate Role A N) (role : Role) :
    rate.Rate.preserved cloneRole ordRole cloneA eqA hashA scalar rule role =
      .ok (Evaluation.orderedFind cmp role rule.preserve) := by
  exact EvaluationRefinement.tree_get_exact ordRole cmp law role rule.preserve

end Rates

section Models
variable (cloneId : core.clone.Clone AccountId) (eqId : core.cmp.Eq AccountId)
  (hashId : core.hash.Hash AccountId) (ordId : core.cmp.Ord AccountId)
  (cloneA : core.clone.Clone A) (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A) (ordA : core.cmp.Ord A)
  (cloneRate : core.clone.Clone RateId) (eqRate : core.cmp.Eq RateId)
  (hashRate : core.hash.Hash RateId) (ordRate : core.cmp.Ord RateId)
  (cloneRole : core.clone.Clone Role) (ordRole : core.cmp.Ord Role) (scalar : quantity.QuantityScalar N M)

theorem model_accounts (model : economy.Economy AccountId A RateId Role N) :
    economy.Economy.impl.accounts cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok model.accounts := by
  simp [economy.Economy.impl.accounts, indexmap.map.IndexMap.iter]

theorem model_rate_ids (model : economy.Economy AccountId A RateId Role N) :
    economy.Economy.rate_ids cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok (model.rates.map Prod.fst) := by
  simp [economy.Economy.rate_ids, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, indexmap.map.IndexMap.keys]

theorem model_invariants (model : economy.Economy AccountId A RateId Role N) :
    economy.Economy.impl.invariants cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok ⟨alloc.vec.Vec.deref model.invariants, 0⟩ := by
  simp [economy.Economy.impl.invariants, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    core.slice.Slice.iter]

theorem model_rate [DecidableEq RateId] (law : LawfulEquality eqRate)
    (model : economy.Economy AccountId A RateId Role N) (id : RateId) :
    economy.Economy.rate cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model id =
      .ok (Account.find? id model.rates) := by
  simp [economy.Economy.rate, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    indexmap.map.IndexMap.get, equivalent_eq, get_exact eqRate law]

end Models
end Axionomy.InspectionRefinement
