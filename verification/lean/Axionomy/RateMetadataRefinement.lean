import Axionomy.DiscoveryRefinement

namespace Axionomy.RateMetadataRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement
open RateConstructionRefinement DiscoveryRefinement

section Construction
variable {Role A : Type}
variable (roleClone : core.clone.Clone Role) (ordInst : core.cmp.Ord Role)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (cmp : Role → Role → Ordering) (hRole : LawfulClone roleClone) (hOrd : LawfulOrdering ordInst cmp)

def empty : rate.Rate Role A Std.U64 :=
  ⟨[], [], [], [], [], alloc.vec.Vec.new _, alloc.vec.Vec.new _, alloc.vec.Vec.new _, alloc.vec.Vec.new _⟩

theorem new_refines : rate_new roleClone ordInst assetClone assetEq assetHash = .ok (empty (Role := Role) (A := A)) := by
  simp [rate_new, rate.Rate.new, empty, alloc.collections.btree.map.BTreeMapKVGlobal.new,
    alloc.collections.btree.set.BTreeSetTGlobal.new]

theorem default_refines : rate_default roleClone ordInst assetClone assetEq assetHash = .ok (empty (Role := Role) (A := A)) := by
  simpa only [rate_default, rate.Rate.Insts.CoreDefaultDefault.default, rate_new]
    using new_refines roleClone ordInst assetClone assetEq assetHash

include hRole hOrd in
theorem register_body (remaining accumulated : List Role) :
    rate.Rate.register_expression_roles_loop.body roleClone ordInst remaining accumulated =
      .ok (match remaining with
        | [] => .done accumulated
        | role :: rest => .cont (rest, RateConstruction.register cmp role accumulated)) := by
  cases remaining with
  | nil => simp [rate.Rate.register_expression_roles_loop.body,
      alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next]
  | cons role rest =>
    obtain ⟨inserted, called⟩ := register_exact ordInst cmp hOrd role accumulated
    simp [rate.Rate.register_expression_roles_loop.body,
      alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next,
      hRole role, alloc.collections.btree.set.BTreeSet.insert, called]

include hRole hOrd in
theorem register_loop (remaining accumulated : List Role) :
    rate.Rate.register_expression_roles_loop roleClone ordInst remaining accumulated =
      .ok (Discovery.registerAll cmp remaining accumulated) := by
  induction remaining generalizing accumulated with
  | nil =>
    unfold rate.Rate.register_expression_roles_loop
    rw [loop.eq_1]
    simp only [register_body roleClone ordInst cmp hRole hOrd [] accumulated, bind_ok]
    rfl
  | cons role rest ih =>
    unfold rate.Rate.register_expression_roles_loop
    rw [loop.eq_1]
    simp only [register_body roleClone ordInst cmp hRole hOrd (role :: rest) accumulated, bind_ok]
    change rate.Rate.register_expression_roles_loop roleClone ordInst rest
      (RateConstruction.register cmp role accumulated) = _
    exact ih _

include hRole hOrd in
theorem register_expression_refines (rule : rate.Rate Role A Std.U64) (target : Role)
    (expression : rate.QuantityExpression Role A Std.U64) :
    rate.Rate.register_expression_roles roleClone ordInst assetClone assetEq assetHash scalar rule target expression =
      .ok { rule with roles := Discovery.expressionRoles cmp target (decodeExpression expression) rule.roles } := by
  obtain ⟨inserted, called⟩ := register_exact ordInst cmp hOrd target rule.roles
  simp [rate.Rate.register_expression_roles, hRole target, alloc.collections.btree.set.BTreeSet.insert,
    called, alloc.collections.btree.set.BTreeSetTGlobal.new,
    roles_refines ordInst cmp hOrd,
    alloc.collections.btree.set.BTreeSet.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter,
    register_loop roleClone ordInst cmp hRole hOrd, Discovery.expressionRoles]

include hRole hOrd in
theorem consume_refines (rule : rate.Rate Role A Std.U64) (target : Role) (asset : A)
    (expression : rate.QuantityExpression Role A Std.U64)
    (capacity : rule.computed_consume.val.length < Usize.max) :
    ∃ effects, rate_computed_consume roleClone ordInst assetClone assetEq assetHash rule target asset expression =
      .ok { rule with
        roles := Discovery.expressionRoles cmp target (decodeExpression expression) rule.roles
        computed_consume := effects } ∧
      effects.val = rule.computed_consume.val ++ [⟨target, asset, expression⟩] := by
  obtain ⟨effects, pushed, contents⟩ := ModelConstructionRefinement.push_call rule.computed_consume
    (⟨target, asset, expression⟩ : rate.ComputedAmount Role A Std.U64) capacity
  refine ⟨effects, ?_, contents⟩
  simp [rate_computed_consume, rate.Rate.consume_computed,
    register_expression_refines roleClone ordInst assetClone assetEq assetHash cmp hRole hOrd, pushed]

include hRole hOrd in
theorem produce_refines (rule : rate.Rate Role A Std.U64) (target : Role) (asset : A)
    (expression : rate.QuantityExpression Role A Std.U64)
    (capacity : rule.computed_produce.val.length < Usize.max) :
    ∃ effects, rate_computed_produce roleClone ordInst assetClone assetEq assetHash rule target asset expression =
      .ok { rule with
        roles := Discovery.expressionRoles cmp target (decodeExpression expression) rule.roles
        computed_produce := effects } ∧
      effects.val = rule.computed_produce.val ++ [⟨target, asset, expression⟩] := by
  obtain ⟨effects, pushed, contents⟩ := ModelConstructionRefinement.push_call rule.computed_produce
    (⟨target, asset, expression⟩ : rate.ComputedAmount Role A Std.U64) capacity
  refine ⟨effects, ?_, contents⟩
  simp [rate_computed_produce, rate.Rate.produce_computed,
    register_expression_refines roleClone ordInst assetClone assetEq assetHash cmp hRole hOrd, pushed]

include hRole hOrd in
theorem preserve_refines (rule : rate.Rate Role A Std.U64) (target : Role) (asset : A)
    (expression : rate.QuantityExpression Role A Std.U64)
    (capacity : rule.computed_preserve.val.length < Usize.max) :
    ∃ effects, rate_computed_preserve roleClone ordInst assetClone assetEq assetHash rule target asset expression =
      .ok { rule with
        roles := Discovery.expressionRoles cmp target (decodeExpression expression) rule.roles
        computed_preserve := effects } ∧
      effects.val = rule.computed_preserve.val ++ [⟨target, asset, expression⟩] := by
  obtain ⟨effects, pushed, contents⟩ := ModelConstructionRefinement.push_call rule.computed_preserve
    (⟨target, asset, expression⟩ : rate.ComputedAmount Role A Std.U64) capacity
  refine ⟨effects, ?_, contents⟩
  simp [rate_computed_preserve, rate.Rate.preserve_computed,
    register_expression_refines roleClone ordInst assetClone assetEq assetHash cmp hRole hOrd, pushed]

include hRole hOrd in
theorem condition_body (remaining : List Role) (rule : rate.Rate Role A Std.U64) :
    rate.Rate.condition_loop.body roleClone ordInst remaining rule =
      .ok (match remaining with
        | [] => .done rule
        | role :: rest => .cont (rest, { rule with roles := RateConstruction.register cmp role rule.roles })) := by
  cases remaining with
  | nil => simp [rate.Rate.condition_loop.body,
      alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next]
  | cons role rest =>
    obtain ⟨inserted, called⟩ := register_exact ordInst cmp hOrd role rule.roles
    simp [rate.Rate.condition_loop.body,
      alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next,
      hRole role, alloc.collections.btree.set.BTreeSet.insert, called]

include hRole hOrd in
theorem condition_loop (remaining : List Role) (rule : rate.Rate Role A Std.U64) :
    rate.Rate.condition_loop roleClone ordInst remaining rule =
      .ok { rule with roles := Discovery.registerAll cmp remaining rule.roles } := by
  induction remaining generalizing rule with
  | nil =>
    unfold rate.Rate.condition_loop
    rw [loop.eq_1]
    simp only [condition_body roleClone ordInst cmp hRole hOrd [] rule, bind_ok]
    rfl
  | cons role rest ih =>
    unfold rate.Rate.condition_loop
    rw [loop.eq_1]
    simp only [condition_body roleClone ordInst cmp hRole hOrd (role :: rest) rule, bind_ok]
    change rate.Rate.condition_loop roleClone ordInst rest
      { rule with roles := RateConstruction.register cmp role rule.roles } = _
    exact ih _

include hRole hOrd in
theorem condition_refines (rule : rate.Rate Role A Std.U64) (label : String)
    (left right : rate.QuantityExpression Role A Std.U64) (comparison : rate.QuantityComparison)
    (capacity : rule.conditions.val.length < Usize.max) :
    ∃ conditions, rate_condition roleClone ordInst assetClone assetEq assetHash rule label left comparison right =
      .ok { rule with
        roles := Discovery.conditionRoles cmp (decodeExpression left) (decodeExpression right) rule.roles
        conditions := conditions } ∧
      conditions.val = rule.conditions.val ++ [⟨label, left, comparison, right⟩] := by
  obtain ⟨conditions, pushed, contents⟩ := ModelConstructionRefinement.push_call rule.conditions
    (⟨label, left, comparison, right⟩ : rate.RateCondition Role A Std.U64) capacity
  refine ⟨conditions, ?_, contents⟩
  simp [rate_condition, rate.Rate.condition, alloc.collections.btree.set.BTreeSetTGlobal.new,
    roles_refines ordInst cmp hOrd,
    alloc.collections.btree.set.BTreeSet.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter,
    condition_loop roleClone ordInst cmp hRole hOrd, core.convert.IntoFrom.into,
    core.convert.FromSame.from, pushed, Discovery.conditionRoles, Discovery.registerAll_append]

include hOrd in
theorem pair_ordering_lawful : LawfulOrdering (Pair.Insts.CoreCmpOrd ordInst ordInst) (Discovery.pairCompare cmp) := by
  intro left right
  cases order : cmp left.1 right.1 <;>
    simp [Pair.Insts.CoreCmpOrd.cmp, hOrd left.1 right.1, order,
      hOrd left.2 right.2, Discovery.pairCompare]

include hRole hOrd in
theorem distinct_refines
    (leLaw : ∀ left right, ordInst.partialOrdInst.le left right = .ok (cmp left right != .gt))
    (rule : rate.Rate Role A Std.U64) (left right : Role) :
    rate_distinct roleClone ordInst assetClone assetEq assetHash rule left right =
      .ok { rule with
        roles := RateConstruction.register cmp right (RateConstruction.register cmp left rule.roles)
        distinct := RateConstruction.register (Discovery.pairCompare cmp)
          (Discovery.orderedPair cmp left right) rule.distinct } := by
  obtain ⟨first, insertedLeft⟩ := register_exact ordInst cmp hOrd left rule.roles
  obtain ⟨second, insertedRight⟩ := register_exact ordInst cmp hOrd right (RateConstruction.register cmp left rule.roles)
  obtain ⟨third, insertedPair⟩ := register_exact (Pair.Insts.CoreCmpOrd ordInst ordInst)
    (Discovery.pairCompare cmp) (pair_ordering_lawful ordInst cmp hOrd)
    (Discovery.orderedPair cmp left right) rule.distinct
  cases order : cmp left right <;>
    simp [rate_distinct, rate.Rate.impl.distinct, hRole left, hRole right,
      alloc.collections.btree.set.BTreeSet.insert, insertedLeft, insertedRight,
      leLaw left right, order, Discovery.orderedPair] at insertedPair ⊢ <;>
    simp [insertedPair]

theorem u64_less_equal_lawful (left right : Std.U64) :
    core.cmp.OrdU64.partialOrdInst.le left right = .ok (compare left.val right.val != .gt) := by
  change Result.ok (decide (left.val ≤ right.val)) = .ok (compare left.val right.val != .gt)
  congr 1
  apply Bool.eq_iff_iff.mpr
  simp [Nat.compare_eq_gt]

end Construction
end Axionomy.RateMetadataRefinement
