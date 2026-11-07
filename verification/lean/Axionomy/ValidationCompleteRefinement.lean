import Axionomy.ValidationParametersRefinement

namespace Axionomy.ValidationCompleteRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement RateConstructionRefinement
open ValidationRefinement ValidationAccountsRefinement ValidationParametersRefinement

variable {AccountId A RateId Role : Type}

def metadata (rule : rate.Rate Role A Std.U64) : Validation.Metadata Role :=
  ⟨rule.roles, Discovery.registerAll compare (RateDiscoveryRefinement.parameterReferences rule) [], rule.distinct⟩

def issueBudget (rule : rate.Rate Role A Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64) : Nat :=
  1 + rule.roles.length + proposal.bindings.length + (metadata rule).parameters.length +
    proposal.parameters.length + rule.distinct.length + rule.roles.length

-- The resource premises have concrete witnesses, for both valid and zero units.
theorem empty_inputs_fit (id : RateId) (units : Std.U64) :
    let rule : rate.Rate Role A Std.U64 := RateMetadataRefinement.empty
    let proposal : exchange.Exchange RateId Role AccountId Std.U64 := ⟨id, [], units, []⟩
    ParametersFit proposal.parameters ∧ RateDiscoveryRefinement.RateNamesFit rule ∧
      issueBudget rule proposal ≤ Usize.max := by
  have emptyVec {T : Type} : (alloc.vec.Vec.new T).val = [] := rfl
  simp [ParametersFit, RateDiscoveryRefinement.RateNamesFit, RateMetadataRefinement.empty,
    issueBudget, metadata, RateDiscoveryRefinement.parameterReferences,
    Discovery.registerAll, emptyVec]
  scalar_tac

def Agrees (actual : core.result.Result (rate.Rate Role A Std.U64) (alloc.vec.Vec (RuntimeIssue AccountId A RateId Role)))
    (expected : Except (List (Validation.Issue AccountId RateId Role)) (rate.Rate Role A Std.U64)) : Prop :=
  match actual, expected with
  | .Ok actual, .ok expected => actual = expected
  | .Err actual, .error expected => actual.val = expected.map encodeIssue
  | _, _ => False

theorem clone_values_identity {T : Type} (cloneInst : core.clone.Clone T) (lawful : LawfulClone cloneInst) (values : List T) :
    cloneValues cloneInst values = .ok values := by
  induction values with
  | nil => rfl
  | cons value rest ih => simp [cloneValues, lawful value, ih]

theorem clone_references (values : List Str) :
    alloc.collections.btree.set.BTreeSet.Insts.CoreCloneClone.clone (Shared0T.Insts.CoreCloneClone Str)
      alloc.alloc.Global.Insts.CoreAllocAllocatorClone values = .ok values := by
  exact clone_values_identity _ (by intro value; rfl) values

theorem zero_issues (units : Std.U64) :
    ∃ issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role),
      (if units.val = 0 then alloc.vec.Vec.push (alloc.vec.Vec.new _) economy.ApplyError.ZeroUnits
        else .ok (alloc.vec.Vec.new _)) = .ok issues ∧
      issues.val = (Validation.zeroIssues (AccountId := AccountId) (RateId := RateId) (Role := Role) units.val).map encodeIssue ∧
      issues.val.length ≤ 1 := by
  by_cases zero : units.val = 0
  · obtain ⟨issues, pushed, entries⟩ := ModelConstructionRefinement.push_call
      (alloc.vec.Vec.new (RuntimeIssue AccountId A RateId Role)) .ZeroUnits (by simp; scalar_tac)
    refine ⟨issues, ?_, ?_, ?_⟩
    · simpa [zero] using pushed
    · simpa [Validation.zeroIssues, zero, encodeIssue] using entries
    · simp [entries, alloc.vec.Vec.new]
  · refine ⟨alloc.vec.Vec.new _, ?_, ?_, ?_⟩ <;> simp [zero, Validation.zeroIssues, alloc.vec.Vec.new]

theorem collected_length {T E F : Type} (items : List T) (emit : T → Option E) (encode : E → F)
    (before after : alloc.vec.Vec F) (contents : after.val = before.val ++ (items.filterMap emit).map encode) :
    after.val.length ≤ before.val.length + items.length := by
  rw [contents]
  simp only [List.length_append, List.length_map]
  exact Nat.add_le_add_left (List.length_filterMap_le emit items) _

theorem zero_call_natural (units : Std.U64) :
    quantity.Quantity.is_zero scalar units = .ok (decide (units.val = 0)) := by
  have equal : units = 0#u64 ↔ units.val = 0 := by
    constructor
    · intro same; simpa using congrArg UScalar.val same
    · intro same; exact UScalar.eq_of_val_eq same
  simp [AccountRefinement.zero_call, AccountRefinement.arithmetic, equal]

theorem arithmetic_zero_value (units : Std.U64) :
    AccountRefinement.arithmetic.isZero units = decide (units.val = 0) :=
  Result.ok_injective ((zero_call units).symm.trans (zero_call_natural units))

theorem agrees_ok (rule : rate.Rate Role A Std.U64)
    (expected : Except (List (Validation.Issue AccountId RateId Role)) (rate.Rate Role A Std.U64))
    (agreement : Agrees (.Ok rule) expected) : expected = .ok rule := by
  cases expected with
  | error errors => contradiction
  | ok selected => simp only [Agrees] at agreement; simp [agreement]

section Complete
variable [DecidableEq AccountId] [DecidableEq RateId] [DecidableEq Role]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq) (hAccountOrd : LawfulOrdering accountOrd cmpAccount)
variable (hRateClone : LawfulClone rateClone) (hRateEq : LawfulEquality rateEq)
variable (hRoleClone : LawfulClone roleClone) (hRoleEq : LawfulEquality roleOrd.eqInst) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

include hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem validate_refines (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ParametersFit proposal.parameters)
    (ruleFits : ∀ rule, Account.find? proposal.rate model.rates = some rule →
      RateDiscoveryRefinement.RateNamesFit rule ∧ issueBudget rule proposal ≤ Usize.max) :
    ∃ result, validate_model_proposal accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal = .ok result ∧
      Agrees result (Validation.validate cmpAccount cmpRole metadata proposal.units.val proposal.rate model.rates
        proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome)) := by
  obtain ⟨initial, initialCall, initialEntries, initialBound⟩ := zero_issues (AccountId := AccountId)
    (A := A) (RateId := RateId) (Role := Role) proposal.units
  cases found : Account.find? proposal.rate model.rates with
  | none =>
    have room : initial.val.length < Usize.max := by
      have large : 2 ≤ Usize.max := by scalar_tac
      omega
    obtain ⟨errors, pushed, contents⟩ := ModelConstructionRefinement.push_call initial (.MissingRate proposal.rate) room
    refine ⟨.Err errors, ?_, ?_⟩
    · simp [validate_model_proposal, economy.Economy.validate_structure,
        exchange.Exchange.impl.units, arithmetic_zero_value, initialCall,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, exchange.Exchange.impl.rate,
        indexmap.map.IndexMap.get, equivalent_eq, get_exact rateEq hRateEq,
        found, hRateClone proposal.rate, pushed]
    · simp [Agrees, Validation.validate, found, contents, initialEntries, encodeIssue]
  | some rule =>
    obtain ⟨namesFit, budget⟩ := ruleFits rule found
    simp only [issueBudget] at budget
    obtain ⟨first, firstCall, firstEntries⟩ := missing_roles_refines roleClone roleOrd cmpRole hRoleClone hRoleOrd
      proposal rule.roles initial (by omega)
    have firstBound := collected_length _ _ _ _ _ firstEntries
    obtain ⟨second, secondCall, secondEntries⟩ := unknown_roles_refines roleClone roleOrd assetClone assetEq assetHash
      hRoleClone hRoleEq rule (proposal.bindings.map Prod.fst) first (by simp only [List.length_map]; omega)
    have secondBound := collected_length _ _ _ _ _ secondEntries
    simp only [List.length_map] at secondBound
    obtain ⟨names, namesCall, namesRelated⟩ := RateDiscoveryRefinement.parameters_refines
      roleClone roleOrd assetClone assetEq assetHash rule namesFit
    simp only [rate_parameters] at namesCall
    change DiscoveryRefinement.Strings (metadata rule).parameters names at namesRelated
    obtain ⟨third, thirdCall, thirdEntries⟩ := missing_parameters_refines roleOrd proposal parameterFits
      (metadata rule).parameters names namesRelated second (by omega)
    have thirdBound := collected_length _ _ _ _ _ thirdEntries
    have parameterNamesFit : ∀ parameter ∈ proposal.parameters.map Prod.fst, parameter.toByteArray.size ≤ Usize.max := by
      intro parameter member
      obtain ⟨entry, contained, equal⟩ := List.mem_map.mp member
      subst parameter
      exact parameterFits entry contained
    obtain ⟨fourth, fourthCall, fourthEntries⟩ := unknown_parameters_refines (metadata rule).parameters names namesRelated
      (proposal.parameters.map Prod.fst) parameterNamesFit third (by simp only [List.length_map]; omega)
    have fourthBound := collected_length _ _ _ _ _ fourthEntries
    simp only [List.length_map] at fourthBound
    obtain ⟨fifth, fifthCall, fifthEntries⟩ := distinct_refines accountClone accountEq roleClone roleOrd cmpRole
      hRoleClone hAccountEq hRoleOrd proposal rule.distinct fourth (by omega)
    have fifthBound := collected_length _ _ _ _ _ fifthEntries
    let missing := Discovery.registerAll cmpAccount (rule.roles.filterMap (emitted model.accounts proposal cmpRole)) []
    have missingCall := accounts_loop accountClone accountEq accountHash accountOrd roleOrd cmpAccount cmpRole
      hAccountClone hAccountEq hAccountOrd hRoleOrd model.accounts proposal rule.roles []
    change economy.Economy.validate_structure_loop5 accountClone accountEq accountHash accountOrd roleOrd
      rule.roles model.accounts proposal [] = .ok missing at missingCall
    have missingBound : missing.length ≤ rule.roles.length := by
      have bound := Discovery.registerAll_length_le cmpAccount (rule.roles.filterMap (emitted model.accounts proposal cmpRole)) []
      have filtered := List.length_filterMap_le (emitted model.accounts proposal cmpRole) rule.roles
      simp only [List.length_nil, Nat.zero_add] at bound
      exact Nat.le_trans bound filtered
    obtain ⟨errors, finalCall, finalEntries⟩ := append_accounts missing fifth (by omega)
    let failures : List (Validation.Issue AccountId RateId Role) :=
      Validation.issues cmpAccount cmpRole proposal.units.val (metadata rule) proposal.bindings proposal.parameters
        (fun id => (Account.find? id model.accounts).isSome)
    have allEntries : errors.val = failures.map encodeIssue := by
      simp only [failures, Validation.issues, List.map_append]
      rw [finalEntries, fifthEntries, fourthEntries, thirdEntries, secondEntries, firstEntries, initialEntries]
      simp only [metadata, Validation.missingAccounts, missing, emitted, List.map_map, Function.comp_def]
    have executed : validate_model_proposal accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal =
        .ok (if failures.isEmpty then .Ok rule else .Err errors) := by
      by_cases clean : failures = [] <;>
      simp [clean, validate_model_proposal, economy.Economy.validate_structure,
        exchange.Exchange.impl.units, arithmetic_zero_value, initialCall,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, exchange.Exchange.impl.rate,
        indexmap.map.IndexMap.get, equivalent_eq, get_exact rateEq hRateEq, found,
        rate.Rate.impl.roles, alloc.collections.btree.set.BTreeSet.iter, firstCall,
        exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.keys, secondCall,
        namesCall, clone_references,
        alloc.collections.btree.set.BTreeSet.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter,
        thirdCall, exchange.Exchange.impl.parameters, fourthCall, rate.Rate.distinct_roles, fifthCall,
        alloc.collections.btree.set.BTreeSetTGlobal.new, missingCall, finalCall, alloc.vec.Vec.is_empty, allEntries]
    refine ⟨_, executed, ?_⟩
    unfold Validation.validate
    rw [found]
    change Agrees (if failures.isEmpty then .Ok rule else .Err errors)
      (if failures.isEmpty then .ok rule else .error failures)
    cases failures.isEmpty <;> simp [Agrees, allEntries]

include hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem accepted_has_accounts (identity : ∀ x y, cmpAccount x y = .eq → x = y)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ParametersFit proposal.parameters)
    (ruleFits : ∀ rule, Account.find? proposal.rate model.rates = some rule →
      RateDiscoveryRefinement.RateNamesFit rule ∧ issueBudget rule proposal ≤ Usize.max)
    (rule : rate.Rate Role A Std.U64)
    (accepted : validate_model_proposal accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal = .ok (.Ok rule)) :
    Account.find? proposal.rate model.rates = some rule ∧ proposal.units.val ≠ 0 ∧
      ∀ role ∈ rule.roles, ∃ id holder,
        Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ Account.find? id model.accounts = some holder := by
  obtain ⟨result, executed, agreement⟩ := validate_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd model proposal parameterFits ruleFits
  have same : result = .Ok rule := Result.ok_injective (executed.symm.trans accepted)
  subst result
  have specified := agrees_ok rule _ agreement
  obtain ⟨found, clean⟩ := Validation.accepted_rule cmpAccount cmpRole metadata proposal.units.val proposal.rate model.rates
    proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) rule specified
  refine ⟨found, Validation.valid_units cmpAccount cmpRole _ _ _ _ _ clean, ?_⟩
  intro role member
  obtain ⟨id, bound⟩ := Validation.required_binding cmpAccount cmpRole _ _ _ _ _ clean role member
  have present := Validation.required_account cmpAccount cmpRole identity _ _ _ _ _ clean role member id bound
  cases accountFound : Account.find? id model.accounts with
  | none => simp [accountFound] at present
  | some holder => exact ⟨id, holder, bound, accountFound⟩

include hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem accepted_parameters_and_distinct
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ParametersFit proposal.parameters)
    (ruleFits : ∀ rule, Account.find? proposal.rate model.rates = some rule →
      RateDiscoveryRefinement.RateNamesFit rule ∧ issueBudget rule proposal ≤ Usize.max)
    (rule : rate.Rate Role A Std.U64)
    (accepted : validate_model_proposal accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal = .ok (.Ok rule)) :
    (∀ parameter ∈ (metadata rule).parameters, ∃ value,
      Evaluation.orderedFind compare parameter proposal.parameters = some value) ∧
    (∀ left right, (left, right) ∈ rule.distinct → ∀ first second,
      Evaluation.orderedFind cmpRole left proposal.bindings = some first →
      Evaluation.orderedFind cmpRole right proposal.bindings = some second → first ≠ second) := by
  obtain ⟨result, executed, agreement⟩ := validate_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd model proposal parameterFits ruleFits
  have same : result = .Ok rule := Result.ok_injective (executed.symm.trans accepted)
  subst result
  obtain ⟨_, clean⟩ := Validation.accepted_rule cmpAccount cmpRole metadata proposal.units.val proposal.rate model.rates
    proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) rule (agrees_ok rule _ agreement)
  exact ⟨Validation.required_parameter cmpAccount cmpRole _ _ _ _ _ clean,
    fun left right member first second => Validation.bound_accounts_differ cmpAccount cmpRole _ _ _ _ _ clean
      left right member first second⟩

include hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem rejected_has_issue
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ParametersFit proposal.parameters)
    (ruleFits : ∀ rule, Account.find? proposal.rate model.rates = some rule →
      RateDiscoveryRefinement.RateNamesFit rule ∧ issueBudget rule proposal ≤ Usize.max)
    (errors : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (rejected : validate_model_proposal accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal = .ok (.Err errors)) :
    errors.val ≠ [] := by
  obtain ⟨result, executed, agreement⟩ := validate_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd model proposal parameterFits ruleFits
  have same : result = .Err errors := Result.ok_injective (executed.symm.trans rejected)
  subst result
  cases specified : Validation.validate cmpAccount cmpRole metadata proposal.units.val proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
  | ok rule => simp [specified, Agrees] at agreement
  | error failures =>
    have nonempty := Validation.rejected_nonempty cmpAccount cmpRole metadata proposal.units.val proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) failures specified
    simp only [specified, Agrees] at agreement
    simpa [agreement] using nonempty

end Complete
end Axionomy.ValidationCompleteRefinement
