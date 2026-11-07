import Axionomy.InspectionRefinement

namespace Axionomy.InspectionLookupRefinement
open Aeneas Aeneas.Std AccountExtracted
variable {AccountId A RateId Role N M K T V : Type}

/-- Ordered lookup invokes only the comparisons up to the first match. Its
primitive comparisons may fail or diverge; neither unused tail is evaluated. -/
def findMapM (equal : K → K → Result Bool) (key : T → K) (project : T → V)
    (query : K) : List T → Result (Option V)
  | [] => .ok none
  | value :: rest => do
    let accepted ← equal (key value) query
    if accepted then .ok (some (project value)) else findMapM equal key project query rest

theorem findMapM_pure (equal : K → K → Result Bool) (test : K → K → Bool)
    (law : ∀ left right, equal left right = .ok (test left right))
    (key : T → K) (project : T → V) (query : K) (entries : List T) :
    findMapM equal key project query entries =
      .ok ((Inspection.first test key query entries).map project) := by
  induction entries with
  | nil => rfl
  | cons value rest ih =>
    cases accepted : test (key value) query <;>
      simp [findMapM, law, accepted, Inspection.first, ih]

theorem findMapM_first (equal : K → K → Result Bool) (key : T → K) (project : T → V)
    (query : K) (leading : List T) (value : T) (tail : List T)
    (earlier : ∀ entry ∈ leading, equal (key entry) query = .ok false)
    (found : equal (key value) query = .ok true) :
    findMapM equal key project query (leading ++ value :: tail) = .ok (some (project value)) := by
  induction leading with
  | nil => simp [findMapM, found]
  | cons head rest ih =>
    have skipped := earlier head (by simp)
    have remaining : ∀ entry ∈ rest, equal (key entry) query = .ok false := by
      intro entry member
      exact earlier entry (List.mem_cons_of_mem head member)
    simpa [findMapM, skipped] using ih remaining

theorem findMapM_failure (equal : K → K → Result Bool) (key : T → K) (project : T → V)
    (query : K) (leading : List T) (value : T) (tail : List T) (error : Error)
    (earlier : ∀ entry ∈ leading, equal (key entry) query = .ok false)
    (failed : equal (key value) query = .fail error) :
    findMapM equal key project query (leading ++ value :: tail) = .fail error := by
  induction leading with
  | nil => simp [findMapM, failed]
  | cons head rest ih =>
    have skipped := earlier head (by simp)
    have remaining : ∀ entry ∈ rest, equal (key entry) query = .ok false := by
      intro entry member
      exact earlier entry (List.mem_cons_of_mem head member)
    simpa [findMapM, skipped] using ih remaining

theorem account_loop (eqId : core.cmp.PartialEq AccountId AccountId)
    (query : AccountId) (entries : List (economy.AccountAssessment AccountId A N))
    (iter : core.slice.iter.Iter (economy.AccountAssessment AccountId A N))
    (suffix : iter.slice.val.drop iter.i = entries) :
    economy.ExchangeAssessment.account_loop eqId iter query =
      findMapM eqId.eq economy.AccountAssessment.account id query entries := by
  induction entries generalizing iter with
  | nil =>
    unfold economy.ExchangeAssessment.account_loop
    rw [loop.eq_1]
    simp [economy.ExchangeAssessment.account_loop.body,
      ConservationRefinement.next_empty iter suffix, findMapM]
  | cons entry rest ih =>
    obtain ⟨advanced, tail⟩ := ConservationRefinement.next_cons iter entry rest suffix
    unfold economy.ExchangeAssessment.account_loop
    rw [loop.eq_1]
    simp only [economy.ExchangeAssessment.account_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
      economy.AccountAssessment.impl.account, core.cmp.impls.PartialEqShared.eq, findMapM]
    rw [Aeneas.Std.bind_assoc]
    congr 1
    funext accepted
    cases accepted with
    | false =>
      simpa [economy.ExchangeAssessment.account_loop, economy.ExchangeAssessment.account_loop.body,
        economy.AccountAssessment.impl.account, core.cmp.impls.PartialEqShared.eq] using
        ih { iter with i := iter.i + 1 } tail
    | true => simp

theorem shortfall_loop (eqId : core.cmp.PartialEq AccountId AccountId)
    (query : AccountId) (entries : List (economy.AccountShortfall AccountId A N))
    (iter : core.slice.iter.Iter (economy.AccountShortfall AccountId A N))
    (suffix : iter.slice.val.drop iter.i = entries) :
    economy.ExchangeAssessment.shortfall_loop eqId iter query =
      findMapM eqId.eq economy.AccountShortfall.account economy.AccountShortfall.missing query entries := by
  induction entries generalizing iter with
  | nil =>
    unfold economy.ExchangeAssessment.shortfall_loop
    rw [loop.eq_1]
    simp [economy.ExchangeAssessment.shortfall_loop.body,
      ConservationRefinement.next_empty iter suffix, findMapM]
  | cons entry rest ih =>
    obtain ⟨advanced, tail⟩ := ConservationRefinement.next_cons iter entry rest suffix
    unfold economy.ExchangeAssessment.shortfall_loop
    rw [loop.eq_1]
    simp only [economy.ExchangeAssessment.shortfall_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
      economy.AccountShortfall.impl.account, core.cmp.impls.PartialEqShared.eq, findMapM]
    rw [Aeneas.Std.bind_assoc]
    congr 1
    funext accepted
    cases accepted with
    | false =>
      simpa [economy.ExchangeAssessment.shortfall_loop, economy.ExchangeAssessment.shortfall_loop.body,
        economy.AccountShortfall.impl.account, core.cmp.impls.PartialEqShared.eq] using
        ih { iter with i := iter.i + 1 } tail
    | true => simp [economy.AccountShortfall.impl.missing]

theorem assessment_account (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId) :
    economy.ExchangeAssessment.account scalar eqId value query =
      findMapM eqId.eq economy.AccountAssessment.account id query (TypedDataRefinement.assessmentAccounts value) := by
  cases value <;>
    simp only [economy.ExchangeAssessment.account, economy.ExchangeAssessment.accounts, bind_ok,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
      TypedDataRefinement.assessmentAccounts]
  all_goals exact account_loop eqId query _ _ (by simp [alloc.vec.Vec.deref, Array.to_slice, Std.Array.empty])

theorem assessment_shortfall (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId) :
    economy.ExchangeAssessment.shortfall scalar eqId value query =
      findMapM eqId.eq economy.AccountShortfall.account economy.AccountShortfall.missing query
        (TypedDataRefinement.assessmentShortfalls value) := by
  cases value <;>
    simp only [economy.ExchangeAssessment.shortfall, economy.ExchangeAssessment.shortfalls, bind_ok,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
      TypedDataRefinement.assessmentShortfalls]
  all_goals exact shortfall_loop eqId query _ _ (by simp [alloc.vec.Vec.deref, Array.to_slice, Std.Array.empty])

theorem account_first_match (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId) (equal : AccountId → AccountId → Bool)
    (law : ∀ left right, eqId.eq left right = .ok (equal left right))
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId) :
    economy.ExchangeAssessment.account scalar eqId value query = .ok
      (Inspection.first equal economy.AccountAssessment.account query
        (TypedDataRefinement.assessmentAccounts value)) := by
  rw [assessment_account, findMapM_pure eqId.eq equal law]
  simp

theorem shortfall_first_match (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId) (equal : AccountId → AccountId → Bool)
    (law : ∀ left right, eqId.eq left right = .ok (equal left right))
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId) :
    economy.ExchangeAssessment.shortfall scalar eqId value query = .ok
      ((Inspection.first equal economy.AccountShortfall.account query
        (TypedDataRefinement.assessmentShortfalls value)).map economy.AccountShortfall.missing) := by
  rw [assessment_shortfall, findMapM_pure eqId.eq equal law]

/-- A successful first match needs no termination or equality assumption on
comparisons with later entries, including caller-constructed duplicates. -/
theorem account_prefix (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId)
    (leading : List (economy.AccountAssessment AccountId A N))
    (entry : economy.AccountAssessment AccountId A N) (tail : List (economy.AccountAssessment AccountId A N))
    (contents : TypedDataRefinement.assessmentAccounts value = leading ++ entry :: tail)
    (earlier : ∀ record ∈ leading, eqId.eq record.account query = .ok false)
    (found : eqId.eq entry.account query = .ok true) :
    economy.ExchangeAssessment.account scalar eqId value query = .ok (some entry) := by
  rw [assessment_account, contents]
  exact findMapM_first eqId.eq _ id query leading entry tail earlier found

theorem account_comparison_failure (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId)
    (leading : List (economy.AccountAssessment AccountId A N))
    (entry : economy.AccountAssessment AccountId A N) (tail : List (economy.AccountAssessment AccountId A N))
    (contents : TypedDataRefinement.assessmentAccounts value = leading ++ entry :: tail)
    (earlier : ∀ record ∈ leading, eqId.eq record.account query = .ok false)
    (error : Error) (failed : eqId.eq entry.account query = .fail error) :
    economy.ExchangeAssessment.account scalar eqId value query = .fail error := by
  rw [assessment_account, contents]
  exact findMapM_failure eqId.eq _ id query leading entry tail error earlier failed

theorem shortfall_prefix (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId)
    (leading : List (economy.AccountShortfall AccountId A N))
    (entry : economy.AccountShortfall AccountId A N) (tail : List (economy.AccountShortfall AccountId A N))
    (contents : TypedDataRefinement.assessmentShortfalls value = leading ++ entry :: tail)
    (earlier : ∀ record ∈ leading, eqId.eq record.account query = .ok false)
    (found : eqId.eq entry.account query = .ok true) :
    economy.ExchangeAssessment.shortfall scalar eqId value query = .ok (some entry.missing) := by
  rw [assessment_shortfall, contents]
  exact findMapM_first eqId.eq _ economy.AccountShortfall.missing query leading entry tail earlier found

theorem shortfall_comparison_failure (scalar : quantity.QuantityScalar N M)
    (eqId : core.cmp.PartialEq AccountId AccountId)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) (query : AccountId)
    (leading : List (economy.AccountShortfall AccountId A N))
    (entry : economy.AccountShortfall AccountId A N) (tail : List (economy.AccountShortfall AccountId A N))
    (contents : TypedDataRefinement.assessmentShortfalls value = leading ++ entry :: tail)
    (earlier : ∀ record ∈ leading, eqId.eq record.account query = .ok false)
    (error : Error) (failed : eqId.eq entry.account query = .fail error) :
    economy.ExchangeAssessment.shortfall scalar eqId value query = .fail error := by
  rw [assessment_shortfall, contents]
  exact findMapM_failure eqId.eq _ economy.AccountShortfall.missing query leading entry tail error earlier failed

end Axionomy.InspectionLookupRefinement
