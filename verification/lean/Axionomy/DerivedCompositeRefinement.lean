import Axionomy.DerivedCollectionRefinement

namespace Axionomy.DerivedCompositeRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement DerivedValueRefinement
open DerivedCollectionRefinement DerivedRecordRefinement
variable {Role A N AccountId RateId : Type}

deriving instance DecidableEq for exchange.Exchange
deriving instance DecidableEq for exchange.Trace
deriving instance DecidableEq for economy.ModelBuildError

theorem proposal_eq [DecidableEq RateId] [DecidableEq Role] [DecidableEq AccountId] [DecidableEq N]
    (rateEq : core.cmp.PartialEq RateId RateId) (roleEq : core.cmp.PartialEq Role Role)
    (accountEq : core.cmp.PartialEq AccountId AccountId) (numberEq : core.cmp.PartialEq N N)
    (rateLaw : LawfulPartialEquality rateEq) (roleLaw : LawfulPartialEquality roleEq)
    (accountLaw : LawfulPartialEquality accountEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : exchange.Exchange RateId Role AccountId N) :
    derived_proposal_eq rateEq roleEq accountEq numberEq left right = .ok (decide (left = right)) := by
  have strings : LawfulPartialEquality alloc.string.String.Insts.CoreCmpPartialEqString := fun _ _ => rfl
  have quantities : LawfulPartialEquality (quantity.Quantity.Insts.CoreCmpPartialEqQuantity numberEq) := numberLaw
  have bindings := map_eq roleEq accountEq roleLaw accountLaw alloc.alloc.Global.Insts.CoreAllocAllocatorClone
  have parameters := map_eq _ _ strings quantities alloc.alloc.Global.Insts.CoreAllocAllocatorClone
  simp only [LawfulPartialEquality] at rateLaw numberLaw
  cases left; cases right
  simp [derived_proposal_eq, exchange.Exchange.Insts.CoreCmpPartialEqExchange.eq,
    quantity.Quantity.Insts.CoreCmpPartialEqQuantity.eq, rateLaw, numberLaw, bindings, parameters]

theorem proposal_clone (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role)
    (accountClone : core.clone.Clone AccountId) (numberClone : core.clone.Clone N)
    (rateLaw : LawfulClone rateClone) (roleLaw : LawfulClone roleClone)
    (accountLaw : LawfulClone accountClone) (numberLaw : LawfulClone numberClone)
    (value : exchange.Exchange RateId Role AccountId N) :
    derived_proposal_clone rateClone roleClone accountClone numberClone value = .ok value :=
  ExecutionRefinement.proposal_clone rateClone roleClone accountClone numberClone
    rateLaw roleLaw accountLaw numberLaw value

theorem trace_eq [DecidableEq RateId] [DecidableEq Role] [DecidableEq AccountId] [DecidableEq N]
    (rateEq : core.cmp.PartialEq RateId RateId) (roleEq : core.cmp.PartialEq Role Role)
    (accountEq : core.cmp.PartialEq AccountId AccountId) (numberEq : core.cmp.PartialEq N N)
    (rateLaw : LawfulPartialEquality rateEq) (roleLaw : LawfulPartialEquality roleEq)
    (accountLaw : LawfulPartialEquality accountEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : exchange.Trace RateId Role AccountId N) :
    derived_trace_eq rateEq roleEq accountEq numberEq left right = .ok (decide (left = right)) := by
  have proposals : LawfulPartialEquality
      (exchange.Exchange.Insts.CoreCmpPartialEqExchange rateEq roleEq accountEq numberEq) :=
    proposal_eq rateEq roleEq accountEq numberEq rateLaw roleLaw accountLaw numberLaw
  cases left; cases right
  simp [derived_trace_eq, exchange.Trace.Insts.CoreCmpPartialEqTrace.eq,
    vec_eq _ proposals, alloc.vec.Vec.eq_iff]

theorem trace_clone (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role)
    (accountClone : core.clone.Clone AccountId) (numberClone : core.clone.Clone N)
    (rateLaw : LawfulClone rateClone) (roleLaw : LawfulClone roleClone)
    (accountLaw : LawfulClone accountClone) (numberLaw : LawfulClone numberClone)
    (value : exchange.Trace RateId Role AccountId N) :
    derived_trace_clone rateClone roleClone accountClone numberClone value = .ok value := by
  have proposals : LawfulClone
      (exchange.Exchange.Insts.CoreCloneClone rateClone roleClone accountClone numberClone) :=
    proposal_clone rateClone roleClone accountClone numberClone rateLaw roleLaw accountLaw numberLaw
  cases value
  simp [derived_trace_clone, exchange.Trace.Insts.CoreCloneClone.clone, vec_clone _ proposals]

theorem model_error_eq [DecidableEq AccountId] [DecidableEq RateId]
    (accountEq : core.cmp.PartialEq AccountId AccountId) (rateEq : core.cmp.PartialEq RateId RateId)
    (accountLaw : LawfulPartialEquality accountEq) (rateLaw : LawfulPartialEquality rateEq)
    (left right : economy.ModelBuildError AccountId RateId) :
    derived_model_error_eq accountEq rateEq left right = .ok (decide (left = right)) := by
  have issues : LawfulPartialEquality (economy.ModelIssue.Insts.CoreCmpPartialEqModelIssue accountEq rateEq) :=
    model_issue_eq accountEq rateEq accountLaw rateLaw
  cases left; cases right
  simp [derived_model_error_eq, economy.ModelBuildError.Insts.CoreCmpPartialEqModelBuildError.eq,
    vec_eq _ issues, alloc.vec.Vec.eq_iff]

theorem model_error_clone (accountClone : core.clone.Clone AccountId) (rateClone : core.clone.Clone RateId)
    (accountLaw : LawfulClone accountClone) (rateLaw : LawfulClone rateClone)
    (value : economy.ModelBuildError AccountId RateId) :
    derived_model_error_clone accountClone rateClone value = .ok value := by
  have issues : LawfulClone (economy.ModelIssue.Insts.CoreCloneClone accountClone rateClone) :=
    model_issue_clone accountClone rateClone accountLaw rateLaw
  cases value
  simp [derived_model_error_clone, economy.ModelBuildError.Insts.CoreCloneClone.clone, vec_clone _ issues]

/-- A different rate stops comparison before touching bindings, quantities or
parameters. No law on those unvisited callbacks is needed. -/
theorem proposal_unequal_rate (rateEq : core.cmp.PartialEq RateId RateId)
    (roleEq : core.cmp.PartialEq Role Role) (accountEq : core.cmp.PartialEq AccountId AccountId)
    (numberEq : core.cmp.PartialEq N N) (left right : exchange.Exchange RateId Role AccountId N)
    (different : rateEq.eq left.rate right.rate = .ok false) :
    derived_proposal_eq rateEq roleEq accountEq numberEq left right = .ok false := by
  simp [derived_proposal_eq, exchange.Exchange.Insts.CoreCmpPartialEqExchange.eq, different]

theorem proposal_rate_comparison_failure (rateEq : core.cmp.PartialEq RateId RateId)
    (roleEq : core.cmp.PartialEq Role Role) (accountEq : core.cmp.PartialEq AccountId AccountId)
    (numberEq : core.cmp.PartialEq N N) (left right : exchange.Exchange RateId Role AccountId N)
    (error : Error) (failed : rateEq.eq left.rate right.rate = .fail error) :
    derived_proposal_eq rateEq roleEq accountEq numberEq left right = .fail error := by
  simp [derived_proposal_eq, exchange.Exchange.Insts.CoreCmpPartialEqExchange.eq, failed]

end Axionomy.DerivedCompositeRefinement
