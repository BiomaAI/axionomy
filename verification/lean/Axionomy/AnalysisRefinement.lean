import Axionomy.AggregationContract
import Axionomy.InvariantRefinement
import Axionomy.ValidationDecode

namespace Axionomy.AnalysisRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

def invalidOne (error : economy.ApplyError RateId Role AccountId A Std.U64 Std.I128) :
    economy.Analysis AccountId A RateId Role Std.U64 Std.I128 :=
  ⟨.Invalid (alloc.vec.Vec.from [error] (by simp; scalar_tac)), none⟩

theorem invalidOne_decode (error : economy.ApplyError RateId Role AccountId A Std.U64 Std.I128) :
    TransactionDecode.analysis (invalidOne error) = Transaction.invalid [TransactionDecode.failure error] := by
  simp [invalidOne, TransactionDecode.analysis, TransactionDecode.exchangeAssessment, Transaction.invalid]

theorem stages_spec [DecidableEq AccountId] [DecidableEq A]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : economy.Economy AccountId A RateId Role Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (rule : rate.Rate Role A Std.U64) :
    Transaction.stages cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) (TransactionDecode.rule rule) = (do
      Transaction.liftIssue (Transaction.checkConditions proposal.rate (ConditionRefinement.context cmpRole proposal model.accounts)
        (TransactionDecode.rule rule).conditions)
      let effects ← Transaction.aggregate cmpAccount cmpRole (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)
        (ConditionRefinement.context cmpRole proposal model.accounts)
      let availability ← Transaction.preflight proposal.rate (TransactionDecode.accounts model.accounts) effects ⟨[], []⟩
      if availability.shortfalls.isEmpty then
        let prepared ← Transaction.prepare (TransactionDecode.accounts model.accounts) effects ⟨[], []⟩
        Transaction.checkInvariants (TransactionDecode.accounts model.accounts) prepared (model.invariants.val.map TransactionDecode.invariant)
        return ⟨.applicable availability.accounts prepared.deltas, some prepared.accounts⟩
      else return ⟨.infeasible availability.accounts availability.shortfalls, none⟩) := rfl

section Accepted
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetClone : LawfulClone assetClone)
variable (hAssetEq : LawfulEquality assetEq) (hRateClone : LawfulClone rateClone) (hRateEq : LawfulEquality rateEq)
variable (hRoleClone : LawfulClone roleClone) (hRoleEq : LawfulEquality roleOrd.eqInst) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem accepted_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64) (rule : rate.Rate Role A Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (ruleFits : ∀ rule, Account.find? proposal.rate model.rates = some rule →
      RateDiscoveryRefinement.RateNamesFit rule ∧ ValidationCompleteRefinement.issueBudget rule proposal ≤ Usize.max)
    (targetsRegistered : RateWellFormed.RegisteredTargets rule)
    (conditionFits : ∀ condition ∈ rule.conditions.val, ConditionRefinement.NamesFit condition)
    (computedFits : ∀ amount ∈ AggregationRefinement.computedAmounts rule,
      TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity))
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max)
    (accepted : validate_model_proposal accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model proposal = .ok (.Ok rule)) :
    ∃ result, economy.Economy.analyze accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model proposal = .ok result ∧
      some (TransactionDecode.analysis result) = Transaction.resolveStages
        (Transaction.stages cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) (TransactionDecode.rule rule)) := by
  have validationCall := accepted
  change economy.Economy.validate_structure accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok (.Ok rule) at validationCall
  obtain ⟨conditions, conditionCall, conditionValue⟩ := ConditionRefinement.conditions_refines accountEq accountHash assetEq assetHash rateClone roleOrd cmpRole
    hAccountEq hAssetEq hRateClone hRoleOrd accountClone accountOrd assetClone assetOrd rateEq rateHash rateOrd roleClone model rule proposal conditionFits
  cases conditions with
  | Err error =>
    refine ⟨invalidOne error, ?_, ?_⟩
    · simp [economy.Economy.analyze, validationCall, conditionCall, economy.invalid_analysis, invalidOne,
        Std.Array.to_slice, Std.Array.make, lift, alloc.slice.Slice.into_vec, alloc.vec.Vec.is_empty]
      simp [alloc.vec.Vec.from, alloc.vec.Vec.val]
    · rw [stages_spec]
      simp [← conditionValue, TransactionDecode.checked, Transaction.resolveStages, invalidOne_decode]
  | Ok value =>
    cases value
    obtain ⟨aggregated, aggregateCall, aggregateValue, aggregateReady⟩ := AggregationContract.validated_aggregate
      accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd
      cmpAccount cmpRole hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRoleOrd
      hRateEq hRoleClone hRoleEq laws model rule proposal parameterFits ruleFits targetsRegistered computedFits accepted
    cases aggregated with
    | Err error =>
      refine ⟨invalidOne error, ?_, ?_⟩
      · simp [economy.Economy.analyze, validationCall, conditionCall, aggregateCall, economy.invalid_analysis, invalidOne,
          Std.Array.to_slice, Std.Array.make, lift, alloc.slice.Slice.into_vec, alloc.vec.Vec.is_empty]
        simp [alloc.vec.Vec.from, alloc.vec.Vec.val]
      · rw [stages_spec]
        simp [← conditionValue, ← aggregateValue, TransactionDecode.checked, Transaction.resolveStages, invalidOne_decode]
    | Ok effects =>
      have ready := aggregateReady effects rfl
      obtain ⟨preflight, preflightCall, preflightValue⟩ := AggregationContract.ready_preflight
        accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd
        hAccountClone hAccountEq hAssetClone hAssetEq hRateClone cmpAccount model proposal effects ready accountCapacity
      cases preflight with
      | Err error =>
        refine ⟨invalidOne error, ?_, ?_⟩
        · simp [economy.Economy.analyze, validationCall, conditionCall, aggregateCall, preflightCall, economy.invalid_analysis, invalidOne,
            Std.Array.to_slice, Std.Array.make, lift, alloc.slice.Slice.into_vec, alloc.vec.Vec.is_empty]
          simp [alloc.vec.Vec.from, alloc.vec.Vec.val]
        · rw [stages_spec]
          simp [← conditionValue, ← aggregateValue, ← preflightValue, TransactionDecode.checked, Transaction.resolveStages, invalidOne_decode]
      | Ok availability =>
        by_cases empty : availability.shortfalls.val = []
        · obtain ⟨preparation, prepareCall, prepareValue, preparationReady⟩ := AggregationContract.ready_prepare
            accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd
            hAccountClone hAccountEq hAssetClone hAssetEq cmpAccount model effects ready accountCapacity
          cases preparation with
          | Err error =>
            refine ⟨invalidOne error, ?_, ?_⟩
            · simp [economy.Economy.analyze, validationCall, conditionCall, aggregateCall, preflightCall, prepareCall, empty,
                economy.invalid_analysis, invalidOne, Std.Array.to_slice, Std.Array.make, lift, alloc.slice.Slice.into_vec, alloc.vec.Vec.is_empty]
              simp [alloc.vec.Vec.from, alloc.vec.Vec.val]
            · rw [stages_spec]
              simp [← conditionValue, ← aggregateValue, ← preflightValue, ← prepareValue, TransactionDecode.checked,
                TransactionDecode.availability, empty, Transaction.resolveStages, invalidOne_decode]
          | Ok prepared =>
            have preparedContract := preparationReady prepared rfl
            obtain ⟨invariants, invariantCall, invariantValue⟩ := InvariantRefinement.invariants_refines
              accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd
              hAccountClone hAccountEq hAssetEq model (alloc.vec.Vec.deref prepared.deltas) prepared.accounts preparedContract.2.2.2.1 invariantFits
            simp only [alloc.vec.Vec.deref, Slice.from_val] at invariantValue
            change TransactionDecode.checked id invariants = Transaction.checkInvariants (TransactionDecode.accounts model.accounts)
              (TransactionDecode.preparation prepared) (model.invariants.val.map TransactionDecode.invariant) at invariantValue
            cases invariants with
            | Err error =>
              refine ⟨invalidOne error, ?_, ?_⟩
              · simp [economy.Economy.analyze, validationCall, conditionCall, aggregateCall, preflightCall, prepareCall, invariantCall, empty,
                  economy.invalid_analysis, invalidOne, Std.Array.to_slice, Std.Array.make, lift, alloc.slice.Slice.into_vec, alloc.vec.Vec.is_empty]
                simp [alloc.vec.Vec.from, alloc.vec.Vec.val]
              · rw [stages_spec]
                simp [← conditionValue, ← aggregateValue, ← preflightValue, ← prepareValue, ← invariantValue, TransactionDecode.checked,
                  TransactionDecode.availability, empty, Transaction.resolveStages, invalidOne_decode]
            | Ok value =>
              refine ⟨⟨.Applicable availability.accounts prepared.deltas, some prepared.accounts⟩, ?_, ?_⟩
              · simp [economy.Economy.analyze, validationCall, conditionCall, aggregateCall, preflightCall, prepareCall, invariantCall, empty,
                  alloc.vec.Vec.is_empty]
              · rw [stages_spec]
                simp [← conditionValue, ← aggregateValue, ← preflightValue, ← prepareValue, ← invariantValue, TransactionDecode.checked,
                  TransactionDecode.availability, empty, Transaction.resolveStages]
                rfl
        · refine ⟨⟨.Infeasible availability.accounts availability.shortfalls, none⟩, ?_, ?_⟩
          · simp [economy.Economy.analyze, validationCall, conditionCall, aggregateCall, preflightCall, empty, alloc.vec.Vec.is_empty]
          · rw [stages_spec]
            simp [← conditionValue, ← aggregateValue, ← preflightValue, TransactionDecode.checked,
              TransactionDecode.availability, empty, Transaction.resolveStages, TransactionDecode.analysis,
              TransactionDecode.exchangeAssessment]

structure RuleReady (proposal : exchange.Exchange RateId Role AccountId Std.U64) (rule : rate.Rate Role A Std.U64) : Prop where
  discovery : RateDiscoveryRefinement.RateNamesFit rule
  issueCapacity : ValidationCompleteRefinement.issueBudget rule proposal ≤ Usize.max
  targets : RateWellFormed.RegisteredTargets rule
  conditions : ∀ condition ∈ rule.conditions.val, ConditionRefinement.NamesFit condition
  computed : ∀ amount ∈ AggregationRefinement.computedAmounts rule,
    TransactionDiagnostics.MessageNamesFit (TransactionDecode.expression amount.quantity)

omit [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
theorem issue_decode (issue : Validation.Issue AccountId RateId Role) :
    TransactionDecode.failure (ValidationRefinement.encodeIssue (A := A) issue) = Transaction.Failure.structural issue := by
  cases issue <;> rfl

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem analyze_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ rule, Account.find? proposal.rate model.rates = some rule → RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.analyze accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model proposal = .ok result ∧
      Transaction.analyze cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) =
        some (TransactionDecode.analysis result) := by
  have ruleFits (rule : rate.Rate Role A Std.U64) (found : Account.find? proposal.rate model.rates = some rule) :=
    And.intro (rulesReady rule found).discovery (rulesReady rule found).issueCapacity
  obtain ⟨validation, validationCall, agreement⟩ := ValidationCompleteRefinement.validate_refines
    accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd
    cmpAccount cmpRole hAccountClone hAccountEq hAccountOrd hRateClone hRateEq hRoleClone hRoleEq hRoleOrd model proposal parameterFits ruleFits
  cases validation with
  | Ok rule =>
    have specified := ValidationCompleteRefinement.agrees_ok rule _ agreement
    have found := (Validation.accepted_rule cmpAccount cmpRole ValidationCompleteRefinement.metadata proposal.units.val proposal.rate
      model.rates proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) rule specified).1
    have ready := rulesReady rule found
    obtain ⟨result, executed, decoded⟩ := accepted_refines accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
      hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
      laws model rule proposal parameterFits ruleFits ready.targets ready.conditions ready.computed invariantFits accountCapacity validationCall
    refine ⟨result, executed, ?_⟩
    unfold Transaction.analyze
    rw [← ValidationDecode.validation_decode, specified]
    exact decoded.symm
  | Err issues =>
    cases specified : Validation.validate cmpAccount cmpRole ValidationCompleteRefinement.metadata proposal.units.val proposal.rate
        model.rates proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
    | ok rule => simp [specified, ValidationCompleteRefinement.Agrees] at agreement
    | error errors =>
      have contents : issues.val = errors.map ValidationRefinement.encodeIssue := by
        simpa only [specified, ValidationCompleteRefinement.Agrees] using agreement
      have errorsNonempty := Validation.rejected_nonempty cmpAccount cmpRole ValidationCompleteRefinement.metadata proposal.units.val proposal.rate
        model.rates proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) errors specified
      have nonempty : issues.val ≠ [] := by simpa [contents] using errorsNonempty
      change economy.Economy.validate_structure accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok (.Err issues) at validationCall
      refine ⟨⟨.Invalid issues, none⟩, ?_, ?_⟩
      · simp [economy.Economy.analyze, validationCall, economy.invalid_analysis, alloc.vec.Vec.is_empty, nonempty]
      · unfold Transaction.analyze
        rw [← ValidationDecode.validation_decode, specified]
        simp [Except.map, TransactionDecode.analysis, TransactionDecode.exchangeAssessment, Transaction.invalid,
          contents, List.map_map, Function.comp_def, issue_decode]

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem assess_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ rule, Account.find? proposal.rate model.rates = some rule → RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.assess accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model proposal = .ok result ∧
      (Transaction.analyze cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal)).map
        Transaction.Analysis.assessment = some (TransactionDecode.exchangeAssessment result) := by
  obtain ⟨analysis, executed, decoded⟩ := analyze_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws model proposal parameterFits rulesReady invariantFits accountCapacity
  refine ⟨analysis.assessment, ?_, ?_⟩
  · simp [economy.Economy.assess, executed]
  · rw [decoded]; rfl

end Accepted
end Axionomy.AnalysisRefinement
