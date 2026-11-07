import Axionomy.AggregationRefinement
import Axionomy.RateMetadataRefinement

namespace Axionomy.RateWellFormed
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {Role A : Type}

def RegisteredTargets (rule : rate.Rate Role A Std.U64) : Prop :=
  ∀ amount ∈ AggregationRefinement.computedAmounts rule, amount.role ∈ rule.roles

theorem empty_registered : RegisteredTargets (RateMetadataRefinement.empty (Role := Role) (A := A)) := by
  simp [RegisteredTargets, AggregationRefinement.computedAmounts, RateMetadataRefinement.empty]

theorem registered_frame (rule updated : rate.Rate Role A Std.U64) (valid : RegisteredTargets rule)
    (roles : rule.roles ⊆ updated.roles)
    (amounts : AggregationRefinement.computedAmounts updated ⊆ AggregationRefinement.computedAmounts rule) :
    RegisteredTargets updated := fun amount member => roles (valid amount (amounts member))

theorem registered_extension (rule updated : rate.Rate Role A Std.U64) (new : rate.ComputedAmount Role A Std.U64)
    (valid : RegisteredTargets rule) (roles : rule.roles ⊆ updated.roles) (target : new.role ∈ updated.roles)
    (amounts : AggregationRefinement.computedAmounts updated ⊆ new :: AggregationRefinement.computedAmounts rule) :
    RegisteredTargets updated := by
  intro amount member
  rcases List.mem_cons.mp (amounts member) with same | old
  · simpa [same] using target
  · exact roles (valid amount old)

section Construction
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (cmp : Role → Role → Ordering) (hClone : LawfulClone roleClone) (hOrd : LawfulOrdering roleOrd cmp)

theorem new_registered : ∃ rule : rate.Rate Role A Std.U64,
    rate_new roleClone roleOrd assetClone assetEq assetHash = .ok rule ∧ RegisteredTargets rule :=
  ⟨RateMetadataRefinement.empty, RateMetadataRefinement.new_refines roleClone roleOrd assetClone assetEq assetHash, empty_registered⟩

include hClone hOrd in
theorem consume_registered (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule : rate.Rate Role A Std.U64) (target : Role) (asset : A) (expression : rate.QuantityExpression Role A Std.U64)
    (valid : RegisteredTargets rule) (capacity : rule.computed_consume.val.length < Usize.max) :
    ∃ updated, rate_computed_consume roleClone roleOrd assetClone assetEq assetHash rule target asset expression = .ok updated ∧
      RegisteredTargets updated := by
  obtain ⟨effects, executed, contents⟩ := RateMetadataRefinement.consume_refines roleClone roleOrd assetClone assetEq assetHash
    cmp hClone hOrd rule target asset expression capacity
  refine ⟨_, executed, registered_extension rule _ ⟨target, asset, expression⟩ valid ?_ ?_ ?_⟩
  · intro role member
    exact (Discovery.expressionRoles_complete cmp eqLaw target role _ rule.roles).mpr (Or.inr (Or.inr member))
  · exact (Discovery.expressionRoles_complete cmp eqLaw target target _ rule.roles).mpr (Or.inl rfl)
  · intro amount member
    simpa [AggregationRefinement.computedAmounts, contents, or_assoc, or_left_comm, or_comm] using member

include hClone hOrd in
theorem produce_registered (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule : rate.Rate Role A Std.U64) (target : Role) (asset : A) (expression : rate.QuantityExpression Role A Std.U64)
    (valid : RegisteredTargets rule) (capacity : rule.computed_produce.val.length < Usize.max) :
    ∃ updated, rate_computed_produce roleClone roleOrd assetClone assetEq assetHash rule target asset expression = .ok updated ∧
      RegisteredTargets updated := by
  obtain ⟨effects, executed, contents⟩ := RateMetadataRefinement.produce_refines roleClone roleOrd assetClone assetEq assetHash
    cmp hClone hOrd rule target asset expression capacity
  refine ⟨_, executed, registered_extension rule _ ⟨target, asset, expression⟩ valid ?_ ?_ ?_⟩
  · intro role member
    exact (Discovery.expressionRoles_complete cmp eqLaw target role _ rule.roles).mpr (Or.inr (Or.inr member))
  · exact (Discovery.expressionRoles_complete cmp eqLaw target target _ rule.roles).mpr (Or.inl rfl)
  · intro amount member
    simpa [AggregationRefinement.computedAmounts, contents, or_assoc, or_left_comm, or_comm] using member

include hClone hOrd in
theorem preserve_registered (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule : rate.Rate Role A Std.U64) (target : Role) (asset : A) (expression : rate.QuantityExpression Role A Std.U64)
    (valid : RegisteredTargets rule) (capacity : rule.computed_preserve.val.length < Usize.max) :
    ∃ updated, rate_computed_preserve roleClone roleOrd assetClone assetEq assetHash rule target asset expression = .ok updated ∧
      RegisteredTargets updated := by
  obtain ⟨effects, executed, contents⟩ := RateMetadataRefinement.preserve_refines roleClone roleOrd assetClone assetEq assetHash
    cmp hClone hOrd rule target asset expression capacity
  refine ⟨_, executed, registered_extension rule _ ⟨target, asset, expression⟩ valid ?_ ?_ ?_⟩
  · intro role member
    exact (Discovery.expressionRoles_complete cmp eqLaw target role _ rule.roles).mpr (Or.inr (Or.inr member))
  · exact (Discovery.expressionRoles_complete cmp eqLaw target target _ rule.roles).mpr (Or.inl rfl)
  · intro amount member
    simpa [AggregationRefinement.computedAmounts, contents, or_assoc, or_left_comm, or_comm] using member

include hClone hOrd in
theorem condition_registered (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule : rate.Rate Role A Std.U64) (label : String) (left right : rate.QuantityExpression Role A Std.U64)
    (comparison : rate.QuantityComparison) (valid : RegisteredTargets rule) (capacity : rule.conditions.val.length < Usize.max) :
    ∃ updated, rate_condition roleClone roleOrd assetClone assetEq assetHash rule label left comparison right = .ok updated ∧
      RegisteredTargets updated := by
  obtain ⟨conditions, executed, _⟩ := RateMetadataRefinement.condition_refines roleClone roleOrd assetClone assetEq assetHash
    cmp hClone hOrd rule label left right comparison capacity
  refine ⟨_, executed, registered_frame rule _ valid ?_ (by intro amount member; exact member)⟩
  intro role member
  exact (Discovery.conditionRoles_complete cmp eqLaw role _ _ rule.roles).mpr (Or.inr (Or.inr member))

include hClone hOrd in
theorem distinct_registered (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (leLaw : ∀ left right, roleOrd.partialOrdInst.le left right = .ok (cmp left right != .gt))
    (rule : rate.Rate Role A Std.U64) (left right : Role) (valid : RegisteredTargets rule) :
    ∃ updated, rate_distinct roleClone roleOrd assetClone assetEq assetHash rule left right = .ok updated ∧
      RegisteredTargets updated := by
  refine ⟨_, RateMetadataRefinement.distinct_refines roleClone roleOrd assetClone assetEq assetHash cmp hClone hOrd leLaw rule left right,
    registered_frame rule _ valid ?_ (by intro amount member; exact member)⟩
  intro role member
  rw [Discovery.register_mem cmp eqLaw, Discovery.register_mem cmp eqLaw]
  exact Or.inr (Or.inr member)

include hClone hOrd in
theorem static_consume_registered [DecidableEq A]
    (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)
    (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule updated : rate.Rate Role A Std.U64) (target : Role) (assets : basket.Basket A Std.U64)
    (valid : RegisteredTargets rule)
    (accepted : rate_consume roleClone roleOrd assetClone assetEq assetHash rule target assets = .ok (.Ok updated)) :
    RegisteredTargets updated := by
  rw [RateConstructionRefinement.consume_refines roleClone roleOrd assetClone assetEq assetHash cmp hClone hOrd hAssetClone hAssetEq] at accepted
  unfold RateConstructionRefinement.consumeResult at accepted
  cases merged : RateConstruction.merge arithmetic cmp (RateConstructionRefinement.decodeMap rule.consume) target assets.quantities with
  | error asset => simp [merged] at accepted
  | ok entries =>
    simp only [merged, Result.ok.injEq, core.result.Result.Ok.injEq] at accepted
    subst updated
    apply registered_frame rule _ valid ?_ (by intro amount member; exact member)
    intro role member
    exact (Discovery.register_mem cmp eqLaw target role rule.roles).mpr (Or.inr member)

include hClone hOrd in
theorem static_produce_registered [DecidableEq A]
    (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)
    (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule updated : rate.Rate Role A Std.U64) (target : Role) (assets : basket.Basket A Std.U64)
    (valid : RegisteredTargets rule)
    (accepted : rate_produce roleClone roleOrd assetClone assetEq assetHash rule target assets = .ok (.Ok updated)) :
    RegisteredTargets updated := by
  rw [RateConstructionRefinement.produce_refines roleClone roleOrd assetClone assetEq assetHash cmp hClone hOrd hAssetClone hAssetEq] at accepted
  unfold RateConstructionRefinement.produceResult at accepted
  cases merged : RateConstruction.merge arithmetic cmp (RateConstructionRefinement.decodeMap rule.produce) target assets.quantities with
  | error asset => simp [merged] at accepted
  | ok entries =>
    simp only [merged, Result.ok.injEq, core.result.Result.Ok.injEq] at accepted
    subst updated
    apply registered_frame rule _ valid ?_ (by intro amount member; exact member)
    intro role member
    exact (Discovery.register_mem cmp eqLaw target role rule.roles).mpr (Or.inr member)

include hClone hOrd in
theorem static_preserve_registered [DecidableEq A]
    (hAssetClone : LawfulClone assetClone) (hAssetEq : LawfulEquality assetEq)
    (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule updated : rate.Rate Role A Std.U64) (target : Role) (assets : basket.Basket A Std.U64)
    (valid : RegisteredTargets rule)
    (accepted : rate_preserve roleClone roleOrd assetClone assetEq assetHash rule target assets = .ok (.Ok updated)) :
    RegisteredTargets updated := by
  rw [RateConstructionRefinement.preserve_refines roleClone roleOrd assetClone assetEq assetHash cmp hClone hOrd hAssetClone hAssetEq] at accepted
  unfold RateConstructionRefinement.preserveResult at accepted
  cases merged : RateConstruction.merge arithmetic cmp (RateConstructionRefinement.decodeMap rule.preserve) target assets.quantities with
  | error asset => simp [merged] at accepted
  | ok entries =>
    simp only [merged, Result.ok.injEq, core.result.Result.Ok.injEq] at accepted
    subst updated
    apply registered_frame rule _ valid ?_ (by intro amount member; exact member)
    intro role member
    exact (Discovery.register_mem cmp eqLaw target role rule.roles).mpr (Or.inr member)

end Construction
end Axionomy.RateWellFormed
