import Axionomy.RateMetadataRefinement
import Axionomy.ConservationRefinement

namespace Axionomy.RateDiscoveryRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement
open DiscoveryRefinement ConservationRefinement

variable {Role A : Type}

@[local simp] theorem slice_val {T : Type} (values : alloc.vec.Vec T) : values.slice.val = values.val := rfl

@[local simp] theorem deref_slice {T : Type} (values : alloc.vec.Vec T) : alloc.vec.Vec.deref values = values.slice := by
  simp only [alloc.vec.Vec.deref, alloc.vec.Vec.val, Slice.val_from]

def amountParameters (amount : rate.ComputedAmount Role A Std.U64) : List String :=
  Discovery.parameters (decodeExpression amount.quantity)

def conditionParameters (condition : rate.RateCondition Role A Std.U64) : List String :=
  Discovery.parameters (decodeExpression condition.left) ++ Discovery.parameters (decodeExpression condition.right)

def amountAssets (amount : rate.ComputedAmount Role A Std.U64) : List A :=
  amount.asset :: Discovery.assets (decodeExpression amount.quantity)

def conditionAssets (condition : rate.RateCondition Role A Std.U64) : List A :=
  Discovery.assets (decodeExpression condition.left) ++ Discovery.assets (decodeExpression condition.right)

def ConditionNamesFit (condition : rate.RateCondition Role A Std.U64) : Prop :=
  NamesFit condition.left ∧ NamesFit condition.right

theorem amount_parameters_body (iter : core.slice.iter.Iter (rate.ComputedAmount Role A Std.U64))
    (amount : rate.ComputedAmount Role A Std.U64) (rest : List (rate.ComputedAmount Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = amount :: rest)
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (fits : NamesFit amount.quantity) :
    ∃ result, rate.ComputedAmount.collect_parameters_loop.body iter physical =
      .ok (.cont ({ iter with i := iter.i + 1 }, result)) ∧
      Strings (Discovery.registerAll compare (amountParameters amount) logical) result := by
  obtain ⟨result, called, agreement⟩ := parameters_refines amount.quantity logical physical related fits
  refine ⟨result, ?_, agreement⟩
  simp [rate.ComputedAmount.collect_parameters_loop.body, (next_cons iter amount rest suffix).1, called]

theorem amount_parameters_loop (remaining : List (rate.ComputedAmount Role A Std.U64))
    (iter : core.slice.iter.Iter (rate.ComputedAmount Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = remaining)
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (fits : ∀ amount ∈ remaining, NamesFit amount.quantity) :
    ∃ result, rate.ComputedAmount.collect_parameters_loop iter physical = .ok result ∧
      Strings (Discovery.registerAll compare (remaining.flatMap amountParameters) logical) result := by
  induction remaining generalizing iter logical physical with
  | nil =>
    refine ⟨physical, ?_, related⟩
    unfold rate.ComputedAmount.collect_parameters_loop
    rw [loop.eq_1]
    simp [rate.ComputedAmount.collect_parameters_loop.body, next_empty iter suffix]
  | cons amount rest ih =>
    obtain ⟨middle, body, middleRelated⟩ := amount_parameters_body iter amount rest suffix
      logical physical related (fits amount (by simp))
    obtain ⟨result, called, agreement⟩ := ih { iter with i := iter.i + 1 } (next_cons iter amount rest suffix).2 _ middle middleRelated
      (fun amount member => fits amount (by simp [member]))
    refine ⟨result, ?_, ?_⟩
    · unfold rate.ComputedAmount.collect_parameters_loop
      rw [loop.eq_1]
      simp only [body, bind_ok]
      exact called
    · simpa only [List.flatMap_cons, Discovery.registerAll_append] using agreement

theorem amount_parameters_refines (amounts : Slice (rate.ComputedAmount Role A Std.U64))
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (fits : ∀ amount ∈ amounts.val, NamesFit amount.quantity) :
    ∃ result, rate.ComputedAmount.collect_parameters amounts physical = .ok result ∧
      Strings (Discovery.registerAll compare (amounts.val.flatMap amountParameters) logical) result := by
  obtain ⟨result, called, agreement⟩ := amount_parameters_loop amounts.val ⟨amounts, 0⟩ (by simp)
    logical physical related fits
  refine ⟨result, ?_, agreement⟩
  simpa [rate.ComputedAmount.collect_parameters,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, core.slice.Slice.iter] using called

theorem condition_parameters_body (iter : core.slice.iter.Iter (rate.RateCondition Role A Std.U64))
    (condition : rate.RateCondition Role A Std.U64) (rest : List (rate.RateCondition Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = condition :: rest)
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (fits : ConditionNamesFit condition) :
    ∃ result, rate.Rate.parameter_names_loop.body iter physical =
      .ok (.cont ({ iter with i := iter.i + 1 }, result)) ∧
      Strings (Discovery.registerAll compare (conditionParameters condition) logical) result := by
  obtain ⟨middle, leftCall, middleRelated⟩ := parameters_refines condition.left logical physical related fits.1
  obtain ⟨result, rightCall, agreement⟩ := parameters_refines condition.right _ middle middleRelated fits.2
  refine ⟨result, ?_, ?_⟩
  · simp [rate.Rate.parameter_names_loop.body, (next_cons iter condition rest suffix).1, leftCall, rightCall]
  · simpa only [conditionParameters, Discovery.registerAll_append] using agreement

theorem condition_parameters_loop (remaining : List (rate.RateCondition Role A Std.U64))
    (iter : core.slice.iter.Iter (rate.RateCondition Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = remaining)
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (fits : ∀ condition ∈ remaining, ConditionNamesFit condition) :
    ∃ result, rate.Rate.parameter_names_loop iter physical = .ok result ∧
      Strings (Discovery.registerAll compare (remaining.flatMap conditionParameters) logical) result := by
  induction remaining generalizing iter logical physical with
  | nil =>
    refine ⟨physical, ?_, related⟩
    unfold rate.Rate.parameter_names_loop
    rw [loop.eq_1]
    simp [rate.Rate.parameter_names_loop.body, next_empty iter suffix]
  | cons condition rest ih =>
    obtain ⟨middle, body, middleRelated⟩ := condition_parameters_body iter condition rest suffix
      logical physical related (fits condition (by simp))
    obtain ⟨result, called, agreement⟩ := ih { iter with i := iter.i + 1 } (next_cons iter condition rest suffix).2 _ middle middleRelated
      (fun condition member => fits condition (by simp [member]))
    refine ⟨result, ?_, ?_⟩
    · unfold rate.Rate.parameter_names_loop
      rw [loop.eq_1]
      simp only [body, bind_ok]
      exact called
    · simpa only [List.flatMap_cons, Discovery.registerAll_append] using agreement

theorem amount_assets_body (iter : core.slice.iter.Iter (rate.ComputedAmount Role A Std.U64))
    (amount : rate.ComputedAmount Role A Std.U64) (rest : List (rate.ComputedAmount Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = amount :: rest) (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (amountAssets amount).length ≤ Usize.max) :
    ∃ result, rate.ComputedAmount.collect_assets_loop.body iter accumulated =
      .ok (.cont ({ iter with i := iter.i + 1 }, result)) ∧
      result.val = accumulated.val ++ amountAssets amount := by
  simp only [amountAssets, List.length_cons] at capacity
  obtain ⟨middle, pushed, middleEntries⟩ := ModelConstructionRefinement.push_call accumulated amount.asset (by omega)
  obtain ⟨result, called, agreement⟩ := assets_refines amount.quantity middle (by
    rw [middleEntries]; simp only [List.length_append, List.length_singleton]; omega)
  refine ⟨result, ?_, ?_⟩
  · simp [rate.ComputedAmount.collect_assets_loop.body, (next_cons iter amount rest suffix).1, pushed, called]
  · simp [agreement, middleEntries, amountAssets, List.append_assoc]

theorem amount_assets_loop (remaining : List (rate.ComputedAmount Role A Std.U64))
    (iter : core.slice.iter.Iter (rate.ComputedAmount Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = remaining) (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (remaining.flatMap amountAssets).length ≤ Usize.max) :
    ∃ result, rate.ComputedAmount.collect_assets_loop iter accumulated = .ok result ∧
      result.val = accumulated.val ++ remaining.flatMap amountAssets := by
  induction remaining generalizing iter accumulated with
  | nil =>
    refine ⟨accumulated, ?_, by simp⟩
    unfold rate.ComputedAmount.collect_assets_loop
    rw [loop.eq_1]
    simp [rate.ComputedAmount.collect_assets_loop.body, next_empty iter suffix]
  | cons amount rest ih =>
    simp only [List.flatMap_cons, List.length_append] at capacity
    obtain ⟨middle, body, middleEntries⟩ := amount_assets_body iter amount rest suffix accumulated (by omega)
    obtain ⟨result, called, agreement⟩ := ih { iter with i := iter.i + 1 } (next_cons iter amount rest suffix).2 middle (by
      rw [middleEntries]; simp only [List.length_append]; omega)
    refine ⟨result, ?_, ?_⟩
    · unfold rate.ComputedAmount.collect_assets_loop
      rw [loop.eq_1]
      simp only [body, bind_ok]
      exact called
    · simp [agreement, middleEntries, List.append_assoc]

theorem amount_assets_refines (amounts : Slice (rate.ComputedAmount Role A Std.U64))
    (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (amounts.val.flatMap amountAssets).length ≤ Usize.max) :
    ∃ result, rate.ComputedAmount.collect_assets amounts accumulated = .ok result ∧
      result.val = accumulated.val ++ amounts.val.flatMap amountAssets := by
  obtain ⟨result, called, agreement⟩ := amount_assets_loop amounts.val ⟨amounts, 0⟩ (by simp)
    accumulated capacity
  refine ⟨result, ?_, agreement⟩
  simpa [rate.ComputedAmount.collect_assets,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, core.slice.Slice.iter] using called

theorem condition_assets_body (iter : core.slice.iter.Iter (rate.RateCondition Role A Std.U64))
    (condition : rate.RateCondition Role A Std.U64) (rest : List (rate.RateCondition Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = condition :: rest) (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (conditionAssets condition).length ≤ Usize.max) :
    ∃ result, rate.Rate.computed_asset_keys_loop.body iter accumulated =
      .ok (.cont ({ iter with i := iter.i + 1 }, result)) ∧
      result.val = accumulated.val ++ conditionAssets condition := by
  simp only [conditionAssets, List.length_append] at capacity
  obtain ⟨middle, leftCall, middleEntries⟩ := assets_refines condition.left accumulated (by omega)
  obtain ⟨result, rightCall, agreement⟩ := assets_refines condition.right middle (by
    rw [middleEntries]; simp only [List.length_append]; omega)
  refine ⟨result, ?_, ?_⟩
  · simp [rate.Rate.computed_asset_keys_loop.body, (next_cons iter condition rest suffix).1, leftCall, rightCall]
  · simp [agreement, middleEntries, conditionAssets, List.append_assoc]

theorem condition_assets_loop (remaining : List (rate.RateCondition Role A Std.U64))
    (iter : core.slice.iter.Iter (rate.RateCondition Role A Std.U64))
    (suffix : iter.slice.val.drop iter.i = remaining) (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (remaining.flatMap conditionAssets).length ≤ Usize.max) :
    ∃ result, rate.Rate.computed_asset_keys_loop iter accumulated = .ok result ∧
      result.val = accumulated.val ++ remaining.flatMap conditionAssets := by
  induction remaining generalizing iter accumulated with
  | nil =>
    refine ⟨accumulated, ?_, by simp⟩
    unfold rate.Rate.computed_asset_keys_loop
    rw [loop.eq_1]
    simp [rate.Rate.computed_asset_keys_loop.body, next_empty iter suffix]
  | cons condition rest ih =>
    simp only [List.flatMap_cons, List.length_append] at capacity
    obtain ⟨middle, body, middleEntries⟩ := condition_assets_body iter condition rest suffix accumulated (by omega)
    obtain ⟨result, called, agreement⟩ := ih { iter with i := iter.i + 1 } (next_cons iter condition rest suffix).2 middle (by
      rw [middleEntries]; simp only [List.length_append]; omega)
    refine ⟨result, ?_, ?_⟩
    · unfold rate.Rate.computed_asset_keys_loop
      rw [loop.eq_1]
      simp only [body, bind_ok]
      exact called
    · simp [agreement, middleEntries, List.append_assoc]

def parameterReferences (rule : rate.Rate Role A Std.U64) : List String :=
  rule.computed_consume.val.flatMap amountParameters ++
  rule.computed_produce.val.flatMap amountParameters ++
  rule.computed_preserve.val.flatMap amountParameters ++
  rule.conditions.val.flatMap conditionParameters

def assetReferences (rule : rate.Rate Role A Std.U64) : List A :=
  rule.computed_consume.val.flatMap amountAssets ++
  rule.computed_produce.val.flatMap amountAssets ++
  rule.computed_preserve.val.flatMap amountAssets ++
  rule.conditions.val.flatMap conditionAssets

def RateNamesFit (rule : rate.Rate Role A Std.U64) : Prop :=
  (∀ amount ∈ rule.computed_consume.val ++ rule.computed_produce.val ++ rule.computed_preserve.val,
    NamesFit amount.quantity) ∧ (∀ condition ∈ rule.conditions.val, ConditionNamesFit condition)

section WholeRate
variable (roleClone : core.clone.Clone Role) (ordInst : core.cmp.Ord Role)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)

theorem parameters_refines (rule : rate.Rate Role A Std.U64) (fits : RateNamesFit rule) :
    ∃ result, rate_parameters roleClone ordInst assetClone assetEq assetHash rule = .ok result ∧
      Strings (Discovery.registerAll compare (parameterReferences rule) []) result := by
  obtain ⟨first, consumeCall, firstRelated⟩ := amount_parameters_refines rule.computed_consume.slice [] [] .nil
    (fun amount member => fits.1 amount (by
      change amount ∈ rule.computed_consume.val at member
      simp [member]))
  obtain ⟨second, produceCall, secondRelated⟩ := amount_parameters_refines rule.computed_produce.slice _ first firstRelated
    (fun amount member => fits.1 amount (by
      change amount ∈ rule.computed_produce.val at member
      simp [member]))
  obtain ⟨third, preserveCall, thirdRelated⟩ := amount_parameters_refines rule.computed_preserve.slice _ second secondRelated
    (fun amount member => fits.1 amount (by
      change amount ∈ rule.computed_preserve.val at member
      simp [member]))
  obtain ⟨result, conditionCall, agreement⟩ := condition_parameters_loop rule.conditions.val
    ⟨rule.conditions.slice, 0⟩ (by simp) _ third thirdRelated fits.2
  refine ⟨result, ?_, ?_⟩
  · simp [rate_parameters, rate.Rate.parameter_names,
      alloc.collections.btree.set.BTreeSetTGlobal.new, deref_slice,
      consumeCall, produceCall, preserveCall,
      SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter,
      core.slice.Slice.iter, conditionCall]
  · simpa only [parameterReferences, Discovery.registerAll_append, slice_val] using agreement

theorem assets_refines (rule : rate.Rate Role A Std.U64)
    (capacity : (assetReferences rule).length ≤ Usize.max) :
    ∃ result, rate_assets roleClone ordInst assetClone assetEq assetHash rule = .ok result ∧
      result.val = assetReferences rule := by
  simp only [assetReferences, List.length_append] at capacity
  obtain ⟨first, consumeCall, firstEntries⟩ := amount_assets_refines rule.computed_consume.slice (alloc.vec.Vec.new A) (by
    simp only [alloc.vec.Vec.new, alloc.vec.Vec.from_val, slice_val, List.length_nil]; omega)
  obtain ⟨second, produceCall, secondEntries⟩ := amount_assets_refines rule.computed_produce.slice first (by
    rw [firstEntries]; simp only [List.length_append, alloc.vec.Vec.new, alloc.vec.Vec.from_val, slice_val, List.length_nil]; omega)
  obtain ⟨third, preserveCall, thirdEntries⟩ := amount_assets_refines rule.computed_preserve.slice second (by
    rw [secondEntries, firstEntries]; simp only [List.length_append, alloc.vec.Vec.new, alloc.vec.Vec.from_val, slice_val, List.length_nil]; omega)
  obtain ⟨result, conditionCall, agreement⟩ := condition_assets_loop rule.conditions.val
    ⟨rule.conditions.slice, 0⟩ (by simp) third (by
      rw [thirdEntries, secondEntries, firstEntries]
      simp only [List.length_append, alloc.vec.Vec.new, alloc.vec.Vec.from_val, slice_val, List.length_nil]; omega)
  refine ⟨result, ?_, ?_⟩
  · simp [rate_assets, rate.Rate.computed_asset_keys, deref_slice,
      consumeCall, produceCall, preserveCall,
      SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter,
      core.slice.Slice.iter, conditionCall]
  · simp [agreement, thirdEntries, secondEntries, firstEntries, assetReferences, alloc.vec.Vec.new]

theorem parameters_complete (rule : rate.Rate Role A Std.U64) (fits : RateNamesFit rule) :
    ∃ logical physical, rate_parameters roleClone ordInst assetClone assetEq assetHash rule = .ok physical ∧
      Strings logical physical ∧ ∀ parameter, parameter ∈ logical ↔ parameter ∈ parameterReferences rule := by
  obtain ⟨physical, called, related⟩ := parameters_refines roleClone ordInst assetClone assetEq assetHash rule fits
  refine ⟨Discovery.registerAll compare (parameterReferences rule) [], physical, called, related, ?_⟩
  intro parameter
  simpa using Discovery.registerAll_mem compare
    (fun (left right : String) equal => Std.LawfulEqOrd.compare_eq_iff_eq.mp equal)
    parameter (parameterReferences rule) []

end WholeRate

end Axionomy.RateDiscoveryRefinement
