import Axionomy.DerivedAccountEqualityRefinement

namespace Axionomy.InheritedCloneRefinement
open Aeneas Aeneas.Std AccountExtracted

/- Exact delegation to the covered whole-value clone implementations. These
contracts preserve modeled callback failures as well as successful results;
physical drops, allocation reuse and state after unwinding remain external. -/

theorem quantity_clone_from
  {N : Type _} (corecloneCloneInst : core.clone.Clone N)
  (target : quantity.Quantity N) (source : quantity.Quantity N) :
    inherited_quantity_clone_from corecloneCloneInst target source =
      derived_quantity_clone corecloneCloneInst source := rfl

theorem amount_clone_from
  {A : Type _} {N : Type _} (corecloneCloneInst : core.clone.Clone A)
  (corecloneCloneInst1 : core.clone.Clone N) (target : amount.AssetAmount A N)
  (source : amount.AssetAmount A N) :
    inherited_amount_clone_from corecloneCloneInst corecloneCloneInst1 target source =
      derived_amount_clone corecloneCloneInst corecloneCloneInst1 source := rfl

theorem expression_clone_from
  {Role : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone Role) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : rate.QuantityExpression Role A N)
  (source : rate.QuantityExpression Role A N) :
    inherited_expression_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_expression_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem computed_amount_clone_from
  {Role : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone Role) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : rate.ComputedAmount Role A N)
  (source : rate.ComputedAmount Role A N) :
    inherited_computed_amount_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_computed_amount_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem condition_clone_from
  {Role : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone Role) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : rate.RateCondition Role A N) (source : rate.RateCondition Role A N)
  :
    inherited_condition_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_condition_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem comparison_clone_from
  (target : rate.QuantityComparison) (source : rate.QuantityComparison) :
    inherited_comparison_clone_from target source =
      derived_comparison_clone source := rfl

theorem fingerprint_clone_from
  (target : economy.StateFingerprint) (source : economy.StateFingerprint) :
    inherited_fingerprint_clone_from target source =
      derived_fingerprint_clone source := rfl

theorem assessment_status_clone_from
  (target : economy.AssessmentStatus) (source : economy.AssessmentStatus) :
    inherited_assessment_status_clone_from target source =
      derived_assessment_status_clone source := rfl

theorem model_issue_clone_from
  {AccountId : Type _} {RateId : Type _} (corecloneCloneInst : core.clone.Clone
  AccountId) (corecloneCloneInst1 : core.clone.Clone RateId)
  (target : economy.ModelIssue AccountId RateId)
  (source : economy.ModelIssue AccountId RateId) :
    inherited_model_issue_clone_from corecloneCloneInst corecloneCloneInst1 target source =
      derived_model_issue_clone corecloneCloneInst corecloneCloneInst1 source := rfl

theorem model_error_clone_from
  {AccountId : Type _} {RateId : Type _} (corecloneCloneInst : core.clone.Clone
  AccountId) (corecloneCloneInst1 : core.clone.Clone RateId)
  (target : economy.ModelBuildError AccountId RateId)
  (source : economy.ModelBuildError AccountId RateId) :
    inherited_model_error_clone_from corecloneCloneInst corecloneCloneInst1 target source =
      derived_model_error_clone corecloneCloneInst corecloneCloneInst1 source := rfl

theorem basket_error_clone_from
  {A : Type _} (corecloneCloneInst : core.clone.Clone A)
  (target : basket.BasketError A) (source : basket.BasketError A) :
    inherited_basket_error_clone_from corecloneCloneInst target source =
      derived_basket_error_clone corecloneCloneInst source := rfl

theorem rate_error_clone_from
  {Role : Type _} {A : Type _} (corecloneCloneInst : core.clone.Clone Role)
  (corecloneCloneInst1 : core.clone.Clone A) (target : rate.RateError Role A)
  (source : rate.RateError Role A) :
    inherited_rate_error_clone_from corecloneCloneInst corecloneCloneInst1 target source =
      derived_rate_error_clone corecloneCloneInst corecloneCloneInst1 source := rfl

theorem quantity_error_clone_from
  (target : quantity.QuantityError) (source : quantity.QuantityError) :
    inherited_quantity_error_clone_from target source =
      derived_quantity_error_clone source := rfl

theorem proposal_clone_from
  {RateId : Type _} {Role : Type _} {AccountId : Type _} {N : Type _}
  (corecloneCloneInst : core.clone.Clone RateId) (corecloneCloneInst1 :
  core.clone.Clone Role) (corecloneCloneInst2 : core.clone.Clone AccountId)
  (corecloneCloneInst3 : core.clone.Clone N)
  (target : exchange.Exchange RateId Role AccountId N)
  (source : exchange.Exchange RateId Role AccountId N) :
    inherited_proposal_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 target source =
      derived_proposal_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 source := rfl

theorem trace_clone_from
  {RateId : Type _} {Role : Type _} {AccountId : Type _} {N : Type _}
  (corecloneCloneInst : core.clone.Clone RateId) (corecloneCloneInst1 :
  core.clone.Clone Role) (corecloneCloneInst2 : core.clone.Clone AccountId)
  (corecloneCloneInst3 : core.clone.Clone N)
  (target : exchange.Trace RateId Role AccountId N)
  (source : exchange.Trace RateId Role AccountId N) :
    inherited_trace_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 target source =
      derived_trace_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 source := rfl

theorem observation_key_clone_from
  {AccountId : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone AccountId) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : economy.ObservationKey AccountId A N)
  (source : economy.ObservationKey AccountId A N) :
    inherited_observation_key_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_observation_key_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem basket_clone_from
  {A : Type _} {N : Type _} (corecloneCloneInst : core.clone.Clone A)
  (corecloneCloneInst1 : core.clone.Clone N) (target : basket.Basket A N)
  (source : basket.Basket A N) :
    inherited_basket_clone_from corecloneCloneInst corecloneCloneInst1 target source =
      derived_basket_clone corecloneCloneInst corecloneCloneInst1 source := rfl

theorem account_clone_from
  {A : Type _} {N : Type _} (corecloneCloneInst : core.clone.Clone A)
  (corecloneCloneInst1 : core.clone.Clone N) (target : account.Account A N)
  (source : account.Account A N) :
    inherited_account_clone_from corecloneCloneInst corecloneCloneInst1 target source =
      derived_account_clone corecloneCloneInst corecloneCloneInst1 source := rfl

theorem account_error_clone_from
  {A : Type _} {N : Type _} {Clause3_SignedMeasure : Type _}
  (corecloneCloneInst : core.clone.Clone A) (corecmpEqInst : core.cmp.Eq A)
  (corehashHashInst : core.hash.Hash A) (quantityQuantityScalarInst :
  quantity.QuantityScalar N Clause3_SignedMeasure)
  (target : account.AccountError A N Clause3_SignedMeasure)
  (source : account.AccountError A N Clause3_SignedMeasure) :
    inherited_account_error_clone_from corecloneCloneInst corecmpEqInst corehashHashInst quantityQuantityScalarInst target source =
      derived_account_error_clone corecloneCloneInst corecmpEqInst corehashHashInst quantityQuantityScalarInst source := rfl

theorem delta_clone_from
  {AccountId : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone AccountId) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : exchange.AccountDelta AccountId A N)
  (source : exchange.AccountDelta AccountId A N) :
    inherited_delta_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_delta_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem receipt_clone_from
  {RateId : Type _} {Role : Type _} {AccountId : Type _} {A : Type _} {N :
  Type _} (corecloneCloneInst : core.clone.Clone RateId) (corecloneCloneInst1 :
  core.clone.Clone Role) (corecloneCloneInst2 : core.clone.Clone AccountId)
  (corecloneCloneInst3 : core.clone.Clone A) (corecloneCloneInst4 :
  core.clone.Clone N) (target : exchange.Receipt RateId Role AccountId A N)
  (source : exchange.Receipt RateId Role AccountId A N) :
    inherited_receipt_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 corecloneCloneInst4 target source =
      derived_receipt_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 corecloneCloneInst4 source := rfl

theorem goal_clone_from
  {AccountId : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone AccountId) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : economy.Goal AccountId A N) (source : economy.Goal AccountId A N) :
    inherited_goal_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_goal_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem rate_clone_from
  {Role : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone Role) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N) (target : rate.Rate Role A N)
  (source : rate.Rate Role A N) :
    inherited_rate_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_rate_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem invariant_clone_from
  {A : Type _} (corecloneCloneInst : core.clone.Clone A)
  (target : rate.LinearInvariant A) (source : rate.LinearInvariant A) :
    inherited_invariant_clone_from corecloneCloneInst target source =
      derived_invariant_clone corecloneCloneInst source := rfl

theorem economy_clone_from
  {AccountId : Type _} {A : Type _} {RateId : Type _} {Role : Type _} {N :
  Type _} (corecloneCloneInst : core.clone.Clone AccountId)
  (corecloneCloneInst1 : core.clone.Clone A) (corecloneCloneInst2 :
  core.clone.Clone RateId) (corecloneCloneInst3 : core.clone.Clone Role)
  (corecloneCloneInst4 : core.clone.Clone N)
  (target : economy.Economy AccountId A RateId Role N)
  (source : economy.Economy AccountId A RateId Role N) :
    inherited_economy_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 corecloneCloneInst4 target source =
      derived_economy_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 corecloneCloneInst4 source := rfl

theorem account_assessment_clone_from
  {AccountId : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone AccountId) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : economy.AccountAssessment AccountId A N)
  (source : economy.AccountAssessment AccountId A N) :
    inherited_account_assessment_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_account_assessment_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem shortfall_clone_from
  {AccountId : Type _} {A : Type _} {N : Type _} (corecloneCloneInst :
  core.clone.Clone AccountId) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone N)
  (target : economy.AccountShortfall AccountId A N)
  (source : economy.AccountShortfall AccountId A N) :
    inherited_shortfall_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 target source =
      derived_shortfall_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 source := rfl

theorem assessment_clone_from
  {AccountId : Type _} {A : Type _} {RateId : Type _} {Role : Type _} {N :
  Type _} {Clause4_SignedMeasure : Type _} (corecloneCloneInst :
  core.clone.Clone AccountId) (corecloneCloneInst1 : core.clone.Clone A)
  (corecloneCloneInst2 : core.clone.Clone RateId) (corecloneCloneInst3 :
  core.clone.Clone Role) (quantityQuantityScalarInst : quantity.QuantityScalar
  N Clause4_SignedMeasure)
  (target : economy.ExchangeAssessment AccountId A RateId Role N
  Clause4_SignedMeasure)
  (source : economy.ExchangeAssessment AccountId A RateId Role N
  Clause4_SignedMeasure) :
    inherited_assessment_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 quantityQuantityScalarInst target source =
      derived_assessment_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 quantityQuantityScalarInst source := rfl

theorem apply_error_clone_from
  {RateId : Type _} {Role : Type _} {AccountId : Type _} {A : Type _} {N :
  Type _} {Clause4_SignedMeasure : Type _} (corecloneCloneInst :
  core.clone.Clone RateId) (corecloneCloneInst1 : core.clone.Clone Role)
  (corecloneCloneInst2 : core.clone.Clone AccountId) (corecloneCloneInst3 :
  core.clone.Clone A) (quantityQuantityScalarInst : quantity.QuantityScalar N
  Clause4_SignedMeasure)
  (target : economy.ApplyError RateId Role AccountId A N Clause4_SignedMeasure)
  (source : economy.ApplyError RateId Role AccountId A N Clause4_SignedMeasure)
  :
    inherited_apply_error_clone_from corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 quantityQuantityScalarInst target source =
      derived_apply_error_clone corecloneCloneInst corecloneCloneInst1 corecloneCloneInst2 corecloneCloneInst3 quantityQuantityScalarInst source := rfl

end Axionomy.InheritedCloneRefinement
