import Axionomy.ExpressionConstructionRefinement
import Axionomy.RateMetadataRefinement
import Axionomy.InspectionRefinement

namespace Axionomy.CallbackRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement
variable {AccountId RateId Role A N T : Type}

/-- These monadic equalities preserve any primitive conversion result, including
failure or divergence. Success premises constrain the conversion itself only. -/
theorem expression_effect (conversion : core.convert.Into T String) (input : T) :
    expression_parameter_with Role A N conversion input = (do
      let label ← conversion.into input
      .ok (.Parameter label)) := rfl

theorem expression_success (conversion : core.convert.Into T String) (input : T) (label : String)
    (converted : conversion.into input = .ok label) :
    expression_parameter_with Role A N conversion input = .ok (.Parameter label) := by
  simp [expression_effect, converted]

theorem expression_failure (conversion : core.convert.Into T String) (input : T) (error : Error)
    (failed : conversion.into input = .fail error) :
    expression_parameter_with Role A N conversion input = .fail error := by
  simp [expression_effect, failed]

theorem proposal_effect (ordRole : core.cmp.Ord Role) (conversion : core.convert.Into T String)
    (proposal : exchange.Exchange RateId Role AccountId N) (input : T) (value : N) :
    proposal_parameter_with ordRole conversion proposal input value = (do
      let label ← conversion.into input
      .ok { proposal with parameters := (RateConstruction.locate compare label proposal.parameters).put value }) := by
  have stringLaw : LawfulOrdering alloc.string.String.Insts.CoreCmpOrd compare := fun _ _ => rfl
  have equal : ∀ x y : String, compare x y = .eq → x = y := by
    intro x y same
    exact Std.LawfulEqOrd.eq_of_compare same
  simp [proposal_parameter_with, exchange.Exchange.parameter,
    TypedDataRefinement.tree_insert _ _ stringLaw equal]

theorem proposal_success (ordRole : core.cmp.Ord Role) (conversion : core.convert.Into T String)
    (proposal : exchange.Exchange RateId Role AccountId N) (input : T) (value : N) (label : String)
    (converted : conversion.into input = .ok label) :
    proposal_parameter_with ordRole conversion proposal input value = .ok
      { proposal with parameters := (RateConstruction.locate compare label proposal.parameters).put value } := by
  simp [proposal_effect, converted]

theorem proposal_failure (ordRole : core.cmp.Ord Role) (conversion : core.convert.Into T String)
    (proposal : exchange.Exchange RateId Role AccountId N) (input : T) (value : N) (error : Error)
    (failed : conversion.into input = .fail error) :
    proposal_parameter_with ordRole conversion proposal input value = .fail error := by
  simp [proposal_effect, failed]

theorem invariant_effect (equal : core.cmp.Eq A) (hash : core.hash.Hash A)
    (conversion : core.convert.Into T String) (input : T) :
    rate.LinearInvariant.new equal hash conversion input = (do
      let label ← conversion.into input
      .ok ⟨label, []⟩) := by
  simp [rate.LinearInvariant.new, indexmap.map.IndexMapKVRandomState.new]

section Conditions
variable (roleClone : core.clone.Clone Role) (ordRole : core.cmp.Ord Role)
  (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
  (cmp : Role → Role → Ordering) (cloneLaw : LawfulClone roleClone) (orderLaw : LawfulOrdering ordRole cmp)

include cloneLaw orderLaw in
theorem condition_effect (conversion : core.convert.Into T String)
    (rule : rate.Rate Role A Std.U64) (input : T)
    (left right : rate.QuantityExpression Role A Std.U64) (comparison : rate.QuantityComparison) :
    rate_condition_with roleClone ordRole assetClone assetEq assetHash conversion rule input left comparison right = (do
      let label ← conversion.into input
      let conditions ← alloc.vec.Vec.push rule.conditions ⟨label, left, comparison, right⟩
      .ok { rule with
        conditions := conditions
        roles := Discovery.conditionRoles cmp (EvaluationRefinement.decodeExpression left)
          (EvaluationRefinement.decodeExpression right) rule.roles }) := by
  simp [rate_condition_with, rate.Rate.condition, alloc.collections.btree.set.BTreeSetTGlobal.new,
    DiscoveryRefinement.roles_refines ordRole cmp orderLaw,
    alloc.collections.btree.set.BTreeSet.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter,
    RateMetadataRefinement.condition_loop roleClone ordRole cmp cloneLaw orderLaw,
    Discovery.conditionRoles, Discovery.registerAll_append]

include cloneLaw orderLaw in
theorem condition_success (conversion : core.convert.Into T String)
    (rule : rate.Rate Role A Std.U64) (input : T) (label : String)
    (converted : conversion.into input = .ok label)
    (left right : rate.QuantityExpression Role A Std.U64) (comparison : rate.QuantityComparison)
    (capacity : rule.conditions.val.length < Usize.max) :
    ∃ conditions,
      rate_condition_with roleClone ordRole assetClone assetEq assetHash conversion rule input left comparison right =
        .ok { rule with
          conditions := conditions
          roles := Discovery.conditionRoles cmp (EvaluationRefinement.decodeExpression left)
            (EvaluationRefinement.decodeExpression right) rule.roles } ∧
      conditions.val = rule.conditions.val ++ [⟨label, left, comparison, right⟩] := by
  obtain ⟨conditions, pushed, contents⟩ := ModelConstructionRefinement.push_call rule.conditions
    (⟨label, left, comparison, right⟩ : rate.RateCondition Role A Std.U64) capacity
  refine ⟨conditions, ?_, contents⟩
  simp [condition_effect roleClone ordRole assetClone assetEq assetHash cmp cloneLaw orderLaw,
    converted, pushed]

/- Failure in name conversion occurs before vector growth, so it does not
require an output-capacity premise. Preceding role operations have primitive laws. -/
include cloneLaw orderLaw in
theorem condition_failure (conversion : core.convert.Into T String)
    (rule : rate.Rate Role A Std.U64) (input : T) (error : Error)
    (failed : conversion.into input = .fail error)
    (left right : rate.QuantityExpression Role A Std.U64) (comparison : rate.QuantityComparison) :
    rate_condition_with roleClone ordRole assetClone assetEq assetHash conversion rule input left comparison right =
      .fail error := by
  simp [condition_effect roleClone ordRole assetClone assetEq assetHash cmp cloneLaw orderLaw, failed]

end Conditions
end Axionomy.CallbackRefinement
