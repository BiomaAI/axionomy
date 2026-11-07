import Axionomy.InspectionRefinement

namespace Axionomy.InspectionAssetsRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement ConservationRefinement
variable {AccountId Role A RateId N M : Type}

/-- Asset metadata ignores numeric payloads. The specification therefore applies
uniformly to all strongly typed scalar backends, without arithmetic laws. -/
def expressionAssets : rate.QuantityExpression Role A N → List A
  | .Constant _ | .Units | .Parameter _ => []
  | .Balance _ asset => [asset]
  | .Add left right | .Subtract left right | .Multiply left right
  | .DivideFloor left right | .Minimum left right | .Maximum left right =>
    expressionAssets left ++ expressionAssets right

def amountAssets (amount : rate.ComputedAmount Role A N) : List A :=
  amount.asset :: expressionAssets amount.quantity

def conditionAssets (condition : rate.RateCondition Role A N) : List A :=
  expressionAssets condition.left ++ expressionAssets condition.right

@[local simp] theorem slice_val {T : Type} (values : alloc.vec.Vec T) : values.slice.val = values.val := rfl

@[local simp] theorem deref_slice {T : Type} (values : alloc.vec.Vec T) : alloc.vec.Vec.deref values = values.slice := by
  simp only [alloc.vec.Vec.deref, alloc.vec.Vec.val, Slice.val_from]

theorem expression_assets (expression : rate.QuantityExpression Role A N)
    (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (expressionAssets expression).length ≤ Usize.max) :
    ∃ result, rate.QuantityExpression.collect_assets expression accumulated = .ok result ∧
      result.val = accumulated.val ++ expressionAssets expression := by
  induction expression generalizing accumulated with
  | Constant value =>
    refine ⟨accumulated, ?_, by simp [expressionAssets]⟩
    simp only [rate.QuantityExpression.collect_assets.eq_def]
  | Units =>
    refine ⟨accumulated, ?_, by simp [expressionAssets]⟩
    simp only [rate.QuantityExpression.collect_assets.eq_def]
  | Parameter parameter =>
    refine ⟨accumulated, ?_, by simp [expressionAssets]⟩
    simp only [rate.QuantityExpression.collect_assets.eq_def]
  | Balance role asset =>
    obtain ⟨result, pushed, entries⟩ := ModelConstructionRefinement.push_call accumulated asset (by
      simp only [expressionAssets, List.length_singleton] at capacity
      omega)
    refine ⟨result, ?_, entries⟩
    rw [rate.QuantityExpression.collect_assets.eq_def]
    exact pushed
  | Add left right ihl ihr | Subtract left right ihl ihr | Multiply left right ihl ihr
  | DivideFloor left right ihl ihr | Minimum left right ihl ihr | Maximum left right ihl ihr =>
    simp only [expressionAssets, List.length_append] at capacity
    obtain ⟨middle, leftCall, middleEntries⟩ := ihl accumulated (by omega)
    obtain ⟨result, rightCall, resultEntries⟩ := ihr middle (by rw [middleEntries]; simp only [List.length_append]; omega)
    refine ⟨result, ?_, ?_⟩
    · rw [rate.QuantityExpression.collect_assets.eq_def]
      simp [leftCall, rightCall]
    · simp [resultEntries, middleEntries, expressionAssets, List.append_assoc]

theorem amount_assets_body (iter : core.slice.iter.Iter (rate.ComputedAmount Role A N))
    (amount : rate.ComputedAmount Role A N) (rest : List (rate.ComputedAmount Role A N))
    (suffix : iter.slice.val.drop iter.i = amount :: rest) (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (amountAssets amount).length ≤ Usize.max) :
    ∃ result, rate.ComputedAmount.collect_assets_loop.body iter accumulated =
      .ok (.cont ({ iter with i := iter.i + 1 }, result)) ∧
      result.val = accumulated.val ++ amountAssets amount := by
  simp only [amountAssets, List.length_cons] at capacity
  obtain ⟨middle, pushed, middleEntries⟩ := ModelConstructionRefinement.push_call accumulated amount.asset (by omega)
  obtain ⟨result, called, agreement⟩ := expression_assets amount.quantity middle (by
    rw [middleEntries]; simp only [List.length_append, List.length_singleton]; omega)
  refine ⟨result, ?_, ?_⟩
  · simp [rate.ComputedAmount.collect_assets_loop.body, (next_cons iter amount rest suffix).1, pushed, called]
  · simp [agreement, middleEntries, amountAssets, List.append_assoc]

theorem amount_assets_loop (remaining : List (rate.ComputedAmount Role A N))
    (iter : core.slice.iter.Iter (rate.ComputedAmount Role A N))
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

theorem amount_assets_refines (amounts : Slice (rate.ComputedAmount Role A N))
    (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (amounts.val.flatMap amountAssets).length ≤ Usize.max) :
    ∃ result, rate.ComputedAmount.collect_assets amounts accumulated = .ok result ∧
      result.val = accumulated.val ++ amounts.val.flatMap amountAssets := by
  obtain ⟨result, called, agreement⟩ := amount_assets_loop amounts.val ⟨amounts, 0⟩ (by simp)
    accumulated capacity
  refine ⟨result, ?_, agreement⟩
  simpa [rate.ComputedAmount.collect_assets,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, core.slice.Slice.iter] using called

theorem condition_assets_body (iter : core.slice.iter.Iter (rate.RateCondition Role A N))
    (condition : rate.RateCondition Role A N) (rest : List (rate.RateCondition Role A N))
    (suffix : iter.slice.val.drop iter.i = condition :: rest) (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (conditionAssets condition).length ≤ Usize.max) :
    ∃ result, rate.Rate.computed_asset_keys_loop.body iter accumulated =
      .ok (.cont ({ iter with i := iter.i + 1 }, result)) ∧
      result.val = accumulated.val ++ conditionAssets condition := by
  simp only [conditionAssets, List.length_append] at capacity
  obtain ⟨middle, leftCall, middleEntries⟩ := expression_assets condition.left accumulated (by omega)
  obtain ⟨result, rightCall, agreement⟩ := expression_assets condition.right middle (by
    rw [middleEntries]; simp only [List.length_append]; omega)
  refine ⟨result, ?_, ?_⟩
  · simp [rate.Rate.computed_asset_keys_loop.body, (next_cons iter condition rest suffix).1, leftCall, rightCall]
  · simp [agreement, middleEntries, conditionAssets, List.append_assoc]

theorem condition_assets_loop (remaining : List (rate.RateCondition Role A N))
    (iter : core.slice.iter.Iter (rate.RateCondition Role A N))
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

def computedReferences (rule : rate.Rate Role A N) : List A :=
  rule.computed_consume.val.flatMap amountAssets ++
  rule.computed_produce.val.flatMap amountAssets ++
  rule.computed_preserve.val.flatMap amountAssets ++
  rule.conditions.val.flatMap conditionAssets

section Computed
variable (roleClone : core.clone.Clone Role) (ordInst : core.cmp.Ord Role)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (scalar : quantity.QuantityScalar N M)

theorem computed_assets (rule : rate.Rate Role A N)
    (capacity : (computedReferences rule).length ≤ Usize.max) :
    ∃ result, rate.Rate.computed_asset_keys roleClone ordInst assetClone assetEq assetHash scalar rule = .ok result ∧
      result.val = computedReferences rule := by
  simp only [computedReferences, List.length_append] at capacity
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
  · simp [rate.Rate.computed_asset_keys, deref_slice,
      consumeCall, produceCall, preserveCall,
      SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter,
      core.slice.Slice.iter, conditionCall]
  · simp [agreement, thirdEntries, secondEntries, firstEntries, computedReferences, alloc.vec.Vec.new]

end Computed
end Axionomy.InspectionAssetsRefinement
