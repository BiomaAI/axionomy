import Axionomy.DerivedCompositeRefinement

namespace Axionomy.DerivedCloneRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement
open DerivedCollectionRefinement DerivedRecordRefinement
variable {Role A N M AccountId RateId : Type}

theorem basket_clone (assetClone : core.clone.Clone A) (numberClone : core.clone.Clone N)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone) (value : basket.Basket A N) :
    derived_basket_clone assetClone numberClone value = .ok value := by
  have quantities : LawfulClone (quantity.Quantity.Insts.CoreCloneClone numberClone) := by
    intro value
    simp [quantity.Quantity.Insts.CoreCloneClone.clone, numberLaw value]
  cases value
  simp [derived_basket_clone, basket.Basket.Insts.CoreCloneClone.clone,
    indexmap.map.IndexMap.Insts.CoreCloneClone.clone, clone_exact _ _ assetLaw quantities]

theorem account_clone (assetClone : core.clone.Clone A) (numberClone : core.clone.Clone N)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone) (value : account.Account A N) :
    derived_account_clone assetClone numberClone value = .ok value := by
  have baskets := basket_clone assetClone numberClone assetLaw numberLaw
  change ∀ value, basket.Basket.Insts.CoreCloneClone.clone assetClone numberClone value = .ok value at baskets
  cases value
  simp [derived_account_clone, account.Account.Insts.CoreCloneClone.clone, baskets]

theorem account_error_clone (assetClone : core.clone.Clone A) (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (scalar : quantity.QuantityScalar N M)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone scalar.corecloneCloneInst)
    (value : account.AccountError A N M) :
    derived_account_error_clone assetClone eqInst hashInst scalar value = .ok value := by
  have baskets := basket_clone assetClone scalar.corecloneCloneInst assetLaw numberLaw
  change ∀ value, basket.Basket.Insts.CoreCloneClone.clone assetClone scalar.corecloneCloneInst value = .ok value at baskets
  simp only [LawfulClone] at assetLaw
  cases value <;>
    simp [derived_account_error_clone, account.AccountError.Insts.CoreCloneClone.clone, baskets, assetLaw]

theorem delta_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (accountLaw : LawfulClone accountClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : exchange.AccountDelta AccountId A N) :
    derived_delta_clone accountClone assetClone numberClone value = .ok value := by
  have baskets := basket_clone assetClone numberClone assetLaw numberLaw
  change ∀ value, basket.Basket.Insts.CoreCloneClone.clone assetClone numberClone value = .ok value at baskets
  simp only [LawfulClone] at accountLaw
  cases value
  simp [derived_delta_clone, exchange.AccountDelta.Insts.CoreCloneClone.clone, baskets, accountLaw]

theorem receipt_clone (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role)
    (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A) (numberClone : core.clone.Clone N)
    (rateLaw : LawfulClone rateClone) (roleLaw : LawfulClone roleClone) (accountLaw : LawfulClone accountClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : exchange.Receipt RateId Role AccountId A N) :
    derived_receipt_clone rateClone roleClone accountClone assetClone numberClone value = .ok value := by
  have deltas : LawfulClone (exchange.AccountDelta.Insts.CoreCloneClone accountClone assetClone numberClone) :=
    delta_clone accountClone assetClone numberClone accountLaw assetLaw numberLaw
  cases value
  simp [derived_receipt_clone, exchange.Receipt.Insts.CoreCloneClone.clone, vec_clone _ deltas,
    ExecutionRefinement.proposal_clone _ _ _ _ rateLaw roleLaw accountLaw numberLaw]

theorem goal_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (accountLaw : LawfulClone accountClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : economy.Goal AccountId A N) :
    derived_goal_clone accountClone assetClone numberClone value = .ok value := by
  have baskets : LawfulClone (basket.Basket.Insts.CoreCloneClone assetClone numberClone) :=
    basket_clone assetClone numberClone assetLaw numberLaw
  cases value
  simp [derived_goal_clone, economy.Goal.Insts.CoreCloneClone.clone,
    alloc.collections.btree.map.BTreeMap.Insts.CoreCloneClone.clone, clone_exact _ _ accountLaw baskets]

theorem invariant_clone (assetClone : core.clone.Clone A) (assetLaw : LawfulClone assetClone)
    (value : rate.LinearInvariant A) : derived_invariant_clone assetClone value = .ok value := by
  have numbers : LawfulClone core.clone.CloneI64 := fun _ => rfl
  cases value
  simp [derived_invariant_clone, rate.LinearInvariant.Insts.CoreCloneClone.clone,
    alloc.string.String.Insts.CoreCloneClone.clone, indexmap.map.IndexMap.Insts.CoreCloneClone.clone,
    clone_exact _ _ assetLaw numbers]

/-- The logical Arc model preserves the shared rates, invariants and account
values. Only map keys are cloned on this production path. -/
theorem economy_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role) (numberClone : core.clone.Clone N)
    (accountLaw : LawfulClone accountClone) (value : economy.Economy AccountId A RateId Role N) :
    derived_economy_clone accountClone assetClone rateClone roleClone numberClone value = .ok value :=
  ExecutionRefinement.economy_clone accountClone assetClone rateClone roleClone numberClone accountLaw value

theorem account_assessment_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (accountLaw : LawfulClone accountClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : economy.AccountAssessment AccountId A N) :
    derived_account_assessment_clone accountClone assetClone numberClone value = .ok value := by
  have baskets := basket_clone assetClone numberClone assetLaw numberLaw
  change ∀ value, basket.Basket.Insts.CoreCloneClone.clone assetClone numberClone value = .ok value at baskets
  simp only [LawfulClone] at accountLaw
  cases value
  simp [derived_account_assessment_clone, economy.AccountAssessment.Insts.CoreCloneClone.clone, baskets, accountLaw]

theorem shortfall_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (accountLaw : LawfulClone accountClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : economy.AccountShortfall AccountId A N) :
    derived_shortfall_clone accountClone assetClone numberClone value = .ok value := by
  have baskets := basket_clone assetClone numberClone assetLaw numberLaw
  change ∀ value, basket.Basket.Insts.CoreCloneClone.clone assetClone numberClone value = .ok value at baskets
  simp only [LawfulClone] at accountLaw
  cases value
  simp [derived_shortfall_clone, economy.AccountShortfall.Insts.CoreCloneClone.clone, baskets, accountLaw]

theorem apply_error_clone (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role)
    (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A) (scalar : quantity.QuantityScalar N M)
    (rateLaw : LawfulClone rateClone) (roleLaw : LawfulClone roleClone) (accountLaw : LawfulClone accountClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone scalar.corecloneCloneInst)
    (measureLaw : LawfulClone scalar.QuantityMeasureInst.corecloneCloneInst)
    (value : economy.ApplyError RateId Role AccountId A N M) :
    derived_apply_error_clone rateClone roleClone accountClone assetClone scalar value = .ok value := by
  have shortfalls : LawfulClone
      (economy.AccountShortfall.Insts.CoreCloneClone accountClone assetClone scalar.corecloneCloneInst) :=
    shortfall_clone accountClone assetClone scalar.corecloneCloneInst accountLaw assetLaw numberLaw
  simp only [LawfulClone] at rateLaw roleLaw accountLaw assetLaw measureLaw
  cases value <;>
    simp [derived_apply_error_clone, economy.ApplyError.Insts.CoreCloneClone.clone,
      alloc.string.String.Insts.CoreCloneClone.clone, vec_clone _ shortfalls,
      rateLaw, roleLaw, accountLaw, assetLaw, measureLaw]

theorem assessment_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role) (scalar : quantity.QuantityScalar N M)
    (accountLaw : LawfulClone accountClone) (assetLaw : LawfulClone assetClone)
    (rateLaw : LawfulClone rateClone) (roleLaw : LawfulClone roleClone)
    (numberLaw : LawfulClone scalar.corecloneCloneInst) (measureLaw : LawfulClone scalar.QuantityMeasureInst.corecloneCloneInst)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) :
    derived_assessment_clone accountClone assetClone rateClone roleClone scalar value = .ok value := by
  have accounts : LawfulClone (economy.AccountAssessment.Insts.CoreCloneClone accountClone assetClone scalar.corecloneCloneInst) :=
    account_assessment_clone accountClone assetClone scalar.corecloneCloneInst accountLaw assetLaw numberLaw
  have deltas : LawfulClone (exchange.AccountDelta.Insts.CoreCloneClone accountClone assetClone scalar.corecloneCloneInst) :=
    delta_clone accountClone assetClone scalar.corecloneCloneInst accountLaw assetLaw numberLaw
  have shortfalls : LawfulClone (economy.AccountShortfall.Insts.CoreCloneClone accountClone assetClone scalar.corecloneCloneInst) :=
    shortfall_clone accountClone assetClone scalar.corecloneCloneInst accountLaw assetLaw numberLaw
  have errors : LawfulClone (economy.ApplyError.Insts.CoreCloneClone rateClone roleClone accountClone assetClone
      scalar.corecloneCloneInst scalar scalar.QuantityMeasureInst.corecloneCloneInst) :=
    apply_error_clone rateClone roleClone accountClone assetClone scalar rateLaw roleLaw accountLaw assetLaw numberLaw measureLaw
  cases value <;>
    simp [derived_assessment_clone, economy.ExchangeAssessment.Insts.CoreCloneClone.clone,
      vec_clone _ accounts, vec_clone _ deltas, vec_clone _ shortfalls, vec_clone _ errors]

/-- Distinct-role pairs use the backend's logical tuple clone model; the role
clone law is the independent Rust compatibility condition for its identity result. -/
theorem rate_clone (roleClone : core.clone.Clone Role) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (roleLaw : LawfulClone roleClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : rate.Rate Role A N) :
    derived_rate_clone roleClone assetClone numberClone value = .ok value := by
  have baskets : LawfulClone (basket.Basket.Insts.CoreCloneClone assetClone numberClone) :=
    basket_clone assetClone numberClone assetLaw numberLaw
  have pairs : LawfulClone (BuiltinClone (Role × Role)) := fun _ => rfl
  have computed : LawfulClone (rate.ComputedAmount.Insts.CoreCloneClone roleClone assetClone numberClone) :=
    computed_amount_clone roleClone assetClone numberClone roleLaw assetLaw numberLaw
  have conditions : LawfulClone (rate.RateCondition.Insts.CoreCloneClone roleClone assetClone numberClone) :=
    condition_clone roleClone assetClone numberClone roleLaw assetLaw numberLaw
  cases value
  simp [derived_rate_clone, rate.Rate.Insts.CoreCloneClone.clone,
    alloc.collections.btree.map.BTreeMap.Insts.CoreCloneClone.clone,
    alloc.collections.btree.set.BTreeSet.Insts.CoreCloneClone.clone,
    clone_exact _ _ roleLaw baskets,
    ValidationCompleteRefinement.clone_values_identity _ roleLaw,
    ValidationCompleteRefinement.clone_values_identity _ pairs,
    vec_clone _ computed, vec_clone _ conditions]

end Axionomy.DerivedCloneRefinement
