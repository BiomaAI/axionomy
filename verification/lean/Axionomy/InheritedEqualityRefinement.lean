import Axionomy.InheritedCloneRefinement

namespace Axionomy.InheritedEqualityRefinement
open Aeneas Aeneas.Std AccountExtracted

/- Core types inherit std ne: negate their own eq, even if a payload overrides
ne. The equality implementation contracts discharge successful results. -/

theorem quantity_ne
  {N : Type _} (corecmpPartialEqInst : core.cmp.PartialEq N N)
  (left : quantity.Quantity N) (right : quantity.Quantity N) :
    inherited_quantity_ne corecmpPartialEqInst left right = (do
      let equal ← derived_quantity_eq corecmpPartialEqInst left right
      .ok (!equal)) := rfl

theorem amount_ne
  {A : Type _} {N : Type _} (corecmpPartialEqInst : core.cmp.PartialEq A A)
  (corecmpPartialEqInst1 : core.cmp.PartialEq N N)
  (left : amount.AssetAmount A N) (right : amount.AssetAmount A N) :
    inherited_amount_ne corecmpPartialEqInst corecmpPartialEqInst1 left right = (do
      let equal ← derived_amount_eq corecmpPartialEqInst corecmpPartialEqInst1 left right
      .ok (!equal)) := rfl

theorem expression_ne
  {Role : Type _} {A : Type _} {N : Type _} (corecmpPartialEqInst :
  core.cmp.PartialEq Role Role) (corecmpPartialEqInst1 : core.cmp.PartialEq A
  A) (corecmpPartialEqInst2 : core.cmp.PartialEq N N)
  (left : rate.QuantityExpression Role A N)
  (right : rate.QuantityExpression Role A N) :
    inherited_expression_ne corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right = (do
      let equal ← derived_expression_eq corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right
      .ok (!equal)) := rfl

theorem computed_amount_ne
  {Role : Type _} {A : Type _} {N : Type _} (corecmpPartialEqInst :
  core.cmp.PartialEq Role Role) (corecmpPartialEqInst1 : core.cmp.PartialEq A
  A) (corecmpPartialEqInst2 : core.cmp.PartialEq N N)
  (left : rate.ComputedAmount Role A N) (right : rate.ComputedAmount Role A N)
  :
    inherited_computed_amount_ne corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right = (do
      let equal ← derived_computed_amount_eq corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right
      .ok (!equal)) := rfl

theorem condition_ne
  {Role : Type _} {A : Type _} {N : Type _} (corecmpPartialEqInst :
  core.cmp.PartialEq Role Role) (corecmpPartialEqInst1 : core.cmp.PartialEq A
  A) (corecmpPartialEqInst2 : core.cmp.PartialEq N N)
  (left : rate.RateCondition Role A N) (right : rate.RateCondition Role A N) :
    inherited_condition_ne corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right = (do
      let equal ← derived_condition_eq corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right
      .ok (!equal)) := rfl

theorem comparison_ne
  (left : rate.QuantityComparison) (right : rate.QuantityComparison) :
    inherited_comparison_ne left right = (do
      let equal ← derived_comparison_eq left right
      .ok (!equal)) := rfl

theorem fingerprint_ne
  (left : economy.StateFingerprint) (right : economy.StateFingerprint) :
    inherited_fingerprint_ne left right = (do
      let equal ← derived_fingerprint_eq left right
      .ok (!equal)) := rfl

theorem assessment_status_ne
  (left : economy.AssessmentStatus) (right : economy.AssessmentStatus) :
    inherited_assessment_status_ne left right = (do
      let equal ← derived_assessment_status_eq left right
      .ok (!equal)) := rfl

theorem model_issue_ne
  {AccountId : Type _} {RateId : Type _} (corecmpPartialEqInst :
  core.cmp.PartialEq AccountId AccountId) (corecmpPartialEqInst1 :
  core.cmp.PartialEq RateId RateId)
  (left : economy.ModelIssue AccountId RateId)
  (right : economy.ModelIssue AccountId RateId) :
    inherited_model_issue_ne corecmpPartialEqInst corecmpPartialEqInst1 left right = (do
      let equal ← derived_model_issue_eq corecmpPartialEqInst corecmpPartialEqInst1 left right
      .ok (!equal)) := rfl

theorem model_error_ne
  {AccountId : Type _} {RateId : Type _} (corecmpPartialEqInst :
  core.cmp.PartialEq AccountId AccountId) (corecmpPartialEqInst1 :
  core.cmp.PartialEq RateId RateId)
  (left : economy.ModelBuildError AccountId RateId)
  (right : economy.ModelBuildError AccountId RateId) :
    inherited_model_error_ne corecmpPartialEqInst corecmpPartialEqInst1 left right = (do
      let equal ← derived_model_error_eq corecmpPartialEqInst corecmpPartialEqInst1 left right
      .ok (!equal)) := rfl

theorem basket_error_ne
  {A : Type _} (corecmpPartialEqInst : core.cmp.PartialEq A A)
  (left : basket.BasketError A) (right : basket.BasketError A) :
    inherited_basket_error_ne corecmpPartialEqInst left right = (do
      let equal ← derived_basket_error_eq corecmpPartialEqInst left right
      .ok (!equal)) := rfl

theorem rate_error_ne
  {Role : Type _} {A : Type _} (corecmpPartialEqInst : core.cmp.PartialEq Role
  Role) (corecmpPartialEqInst1 : core.cmp.PartialEq A A)
  (left : rate.RateError Role A) (right : rate.RateError Role A) :
    inherited_rate_error_ne corecmpPartialEqInst corecmpPartialEqInst1 left right = (do
      let equal ← derived_rate_error_eq corecmpPartialEqInst corecmpPartialEqInst1 left right
      .ok (!equal)) := rfl

theorem quantity_error_ne
  (left : quantity.QuantityError) (right : quantity.QuantityError) :
    inherited_quantity_error_ne left right = (do
      let equal ← derived_quantity_error_eq left right
      .ok (!equal)) := rfl

theorem proposal_ne
  {RateId : Type _} {Role : Type _} {AccountId : Type _} {N : Type _}
  (corecmpPartialEqInst : core.cmp.PartialEq RateId RateId)
  (corecmpPartialEqInst1 : core.cmp.PartialEq Role Role) (corecmpPartialEqInst2
  : core.cmp.PartialEq AccountId AccountId) (corecmpPartialEqInst3 :
  core.cmp.PartialEq N N) (left : exchange.Exchange RateId Role AccountId N)
  (right : exchange.Exchange RateId Role AccountId N) :
    inherited_proposal_ne corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 corecmpPartialEqInst3 left right = (do
      let equal ← derived_proposal_eq corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 corecmpPartialEqInst3 left right
      .ok (!equal)) := rfl

theorem trace_ne
  {RateId : Type _} {Role : Type _} {AccountId : Type _} {N : Type _}
  (corecmpPartialEqInst : core.cmp.PartialEq RateId RateId)
  (corecmpPartialEqInst1 : core.cmp.PartialEq Role Role) (corecmpPartialEqInst2
  : core.cmp.PartialEq AccountId AccountId) (corecmpPartialEqInst3 :
  core.cmp.PartialEq N N) (left : exchange.Trace RateId Role AccountId N)
  (right : exchange.Trace RateId Role AccountId N) :
    inherited_trace_ne corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 corecmpPartialEqInst3 left right = (do
      let equal ← derived_trace_eq corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 corecmpPartialEqInst3 left right
      .ok (!equal)) := rfl

theorem observation_key_ne
  {AccountId : Type _} {A : Type _} {N : Type _} (corecmpPartialEqInst :
  core.cmp.PartialEq AccountId AccountId) (corecmpPartialEqInst1 :
  core.cmp.PartialEq A A) (corecmpPartialEqInst2 : core.cmp.PartialEq N N)
  (left : economy.ObservationKey AccountId A N)
  (right : economy.ObservationKey AccountId A N) :
    inherited_observation_key_ne corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right = (do
      let equal ← derived_observation_key_eq corecmpPartialEqInst corecmpPartialEqInst1 corecmpPartialEqInst2 left right
      .ok (!equal)) := rfl

theorem account_error_ne
  {A : Type _} {N : Type _} {Clause2_SignedMeasure : Type _} (corecmpEqInst :
  core.cmp.Eq A) (corehashHashInst : core.hash.Hash A)
  (quantityQuantityScalarInst : quantity.QuantityScalar N
  Clause2_SignedMeasure)
  (left : account.AccountError A N Clause2_SignedMeasure)
  (right : account.AccountError A N Clause2_SignedMeasure) :
    inherited_account_error_ne corecmpEqInst corehashHashInst quantityQuantityScalarInst left right = (do
      let equal ← derived_account_error_eq corecmpEqInst corehashHashInst quantityQuantityScalarInst left right
      .ok (!equal)) := rfl

theorem basket_ne
  {A : Type _} {N : Type _} (corecmpEqInst : core.cmp.Eq A) (corehashHashInst :
  core.hash.Hash A) (corecmpPartialEqInst : core.cmp.PartialEq N N)
  (left : basket.Basket A N) (right : basket.Basket A N) :
    inherited_basket_ne corecmpEqInst corehashHashInst corecmpPartialEqInst left right = (do
      let equal ← basket.Basket.Insts.CoreCmpPartialEqBasket.eq corecmpEqInst corehashHashInst corecmpPartialEqInst left right
      .ok (!equal)) := rfl

theorem account_ne
  {A : Type _} {N : Type _} (corecmpEqInst : core.cmp.Eq A) (corehashHashInst :
  core.hash.Hash A) (corecmpPartialEqInst : core.cmp.PartialEq N N)
  (left : account.Account A N) (right : account.Account A N) :
    inherited_account_ne corecmpEqInst corehashHashInst corecmpPartialEqInst left right = (do
      let equal ← account.Account.Insts.CoreCmpPartialEqAccount.eq corecmpEqInst corehashHashInst corecmpPartialEqInst left right
      .ok (!equal)) := rfl

end Axionomy.InheritedEqualityRefinement
