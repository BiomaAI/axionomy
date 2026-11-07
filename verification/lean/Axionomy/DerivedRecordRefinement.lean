import Axionomy.DerivedExpressionRefinement

namespace Axionomy.DerivedRecordRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement DerivedValueRefinement DerivedExpressionRefinement
variable {Role A N AccountId RateId : Type}

deriving instance DecidableEq for rate.QuantityComparison
deriving instance DecidableEq for economy.AssessmentStatus
deriving instance DecidableEq for quantity.QuantityError
deriving instance DecidableEq for basket.BasketError
deriving instance DecidableEq for rate.RateError
deriving instance DecidableEq for economy.ModelIssue
deriving instance DecidableEq for rate.ComputedAmount
deriving instance DecidableEq for rate.RateCondition

theorem comparison_eq (left right : rate.QuantityComparison) :
    derived_comparison_eq left right = .ok (decide (left = right)) := by
  cases left <;> cases right <;>
    simp [derived_comparison_eq, rate.QuantityComparison.Insts.CoreCmpPartialEqQuantityComparison.eq, rate.QuantityComparison.read_discriminant]

theorem comparison_clone (value : rate.QuantityComparison) : derived_comparison_clone value = .ok value := rfl

theorem assessment_status_eq (left right : economy.AssessmentStatus) :
    derived_assessment_status_eq left right = .ok (decide (left = right)) := by
  cases left <;> cases right <;>
    simp [derived_assessment_status_eq, economy.AssessmentStatus.Insts.CoreCmpPartialEqAssessmentStatus.eq, economy.AssessmentStatus.read_discriminant]

theorem assessment_status_clone (value : economy.AssessmentStatus) : derived_assessment_status_clone value = .ok value := rfl

theorem quantity_error_eq (left right : quantity.QuantityError) :
    derived_quantity_error_eq left right = .ok (decide (left = right)) := by
  cases left; cases right
  simp [derived_quantity_error_eq, quantity.QuantityError.Insts.CoreCmpPartialEqQuantityError.eq]

theorem quantity_error_clone (value : quantity.QuantityError) : derived_quantity_error_clone value = .ok value := rfl

theorem basket_error_eq [DecidableEq A] (inst : core.cmp.PartialEq A A)
    (law : LawfulPartialEquality inst) (left right : basket.BasketError A) :
    derived_basket_error_eq inst left right = .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at law
  cases left; cases right
  simp [derived_basket_error_eq, basket.BasketError.Insts.CoreCmpPartialEqBasketError.eq,
    core.cmp.impls.PartialEqShared.eq, law]

theorem basket_error_clone (inst : core.clone.Clone A) (law : LawfulClone inst)
    (value : basket.BasketError A) : derived_basket_error_clone inst value = .ok value := by
  simp only [LawfulClone] at law
  cases value
  simp [derived_basket_error_clone, basket.BasketError.Insts.CoreCloneClone.clone, law]

theorem rate_error_eq [DecidableEq Role] [DecidableEq A]
    (roleEq : core.cmp.PartialEq Role Role) (assetEq : core.cmp.PartialEq A A)
    (roleLaw : LawfulPartialEquality roleEq) (assetLaw : LawfulPartialEquality assetEq)
    (left right : rate.RateError Role A) :
    derived_rate_error_eq roleEq assetEq left right = .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at roleLaw assetLaw
  cases left; cases right
  simp [derived_rate_error_eq, rate.RateError.Insts.CoreCmpPartialEqRateError.eq,
    core.cmp.impls.PartialEqShared.eq, roleLaw, assetLaw]

theorem rate_error_clone (roleClone : core.clone.Clone Role) (assetClone : core.clone.Clone A)
    (roleLaw : LawfulClone roleClone) (assetLaw : LawfulClone assetClone)
    (value : rate.RateError Role A) : derived_rate_error_clone roleClone assetClone value = .ok value := by
  simp only [LawfulClone] at roleLaw assetLaw
  cases value
  simp [derived_rate_error_clone, rate.RateError.Insts.CoreCloneClone.clone, roleLaw, assetLaw]

theorem model_issue_eq [DecidableEq AccountId] [DecidableEq RateId]
    (accountEq : core.cmp.PartialEq AccountId AccountId) (rateEq : core.cmp.PartialEq RateId RateId)
    (accountLaw : LawfulPartialEquality accountEq) (rateLaw : LawfulPartialEquality rateEq)
    (left right : economy.ModelIssue AccountId RateId) :
    derived_model_issue_eq accountEq rateEq left right = .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at accountLaw rateLaw
  cases left <;> cases right <;>
    simp [derived_model_issue_eq, economy.ModelIssue.Insts.CoreCmpPartialEqModelIssue.eq,
      economy.ModelIssue.read_discriminant, core.cmp.impls.PartialEqShared.eq, accountLaw, rateLaw]

theorem model_issue_clone (accountClone : core.clone.Clone AccountId) (rateClone : core.clone.Clone RateId)
    (accountLaw : LawfulClone accountClone) (rateLaw : LawfulClone rateClone)
    (value : economy.ModelIssue AccountId RateId) :
    derived_model_issue_clone accountClone rateClone value = .ok value := by
  simp only [LawfulClone] at accountLaw rateLaw
  cases value <;>
    simp [derived_model_issue_clone, economy.ModelIssue.Insts.CoreCloneClone.clone, accountLaw, rateLaw]

theorem computed_amount_clone (roleClone : core.clone.Clone Role) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (roleLaw : LawfulClone roleClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : rate.ComputedAmount Role A N) :
    derived_computed_amount_clone roleClone assetClone numberClone value = .ok value := by
  have expressionLaw := clone_expression roleClone assetClone numberClone roleLaw assetLaw numberLaw
  simp only [LawfulClone] at roleLaw assetLaw numberLaw
  cases value
  simp [derived_computed_amount_clone, rate.ComputedAmount.Insts.CoreCloneClone.clone,
    expressionLaw, roleLaw, assetLaw]

theorem computed_amount_eq [DecidableEq Role] [DecidableEq A] [DecidableEq N]
    (roleEq : core.cmp.PartialEq Role Role) (assetEq : core.cmp.PartialEq A A)
    (numberEq : core.cmp.PartialEq N N) (roleLaw : LawfulPartialEquality roleEq)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : rate.ComputedAmount Role A N) :
    derived_computed_amount_eq roleEq assetEq numberEq left right = .ok (decide (left = right)) := by
  have expressionLaw := equal_expression roleEq assetEq numberEq roleLaw assetLaw numberLaw
  simp only [LawfulPartialEquality] at roleLaw assetLaw numberLaw
  cases left; cases right
  simp [derived_computed_amount_eq, rate.ComputedAmount.Insts.CoreCmpPartialEqComputedAmount.eq,
    expressionLaw, roleLaw, assetLaw]

theorem condition_clone (roleClone : core.clone.Clone Role) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (roleLaw : LawfulClone roleClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : rate.RateCondition Role A N) :
    derived_condition_clone roleClone assetClone numberClone value = .ok value := by
  have expressionLaw := clone_expression roleClone assetClone numberClone roleLaw assetLaw numberLaw
  simp only [LawfulClone] at roleLaw assetLaw numberLaw
  cases value
  simp [derived_condition_clone, rate.RateCondition.Insts.CoreCloneClone.clone,
    alloc.string.String.Insts.CoreCloneClone.clone, rate.QuantityComparison.Insts.CoreCloneClone.clone,
    expressionLaw]

theorem condition_eq [DecidableEq Role] [DecidableEq A] [DecidableEq N]
    (roleEq : core.cmp.PartialEq Role Role) (assetEq : core.cmp.PartialEq A A)
    (numberEq : core.cmp.PartialEq N N) (roleLaw : LawfulPartialEquality roleEq)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : rate.RateCondition Role A N) :
    derived_condition_eq roleEq assetEq numberEq left right = .ok (decide (left = right)) := by
  have expressionLaw := equal_expression roleEq assetEq numberEq roleLaw assetLaw numberLaw
  have comparisonLaw := comparison_eq
  change ∀ left right, rate.QuantityComparison.Insts.CoreCmpPartialEqQuantityComparison.eq left right =
    .ok (decide (left = right)) at comparisonLaw
  simp only [LawfulPartialEquality] at roleLaw assetLaw numberLaw
  cases left; cases right
  simp [derived_condition_eq, rate.RateCondition.Insts.CoreCmpPartialEqRateCondition.eq,
    alloc.string.String.Insts.CoreCmpPartialEqString.eq, expressionLaw, comparisonLaw]

end Axionomy.DerivedRecordRefinement
