import Axionomy.TransactionPreparation
import Axionomy.OrderedEffects

namespace Axionomy.Transaction

variable {AccountId A RateId Role : Type}

-- The successful branch carries the correspondence between aggregated effects,
-- original-account preparation, receipt deltas, and uniquely staged accounts.
def AnalysisReady [DecidableEq AccountId] [DecidableEq A]
    (accounts : List (AccountId × Balances A)) (analysis : Analysis AccountId A RateId Role) : Prop :=
  match analysis.assessment with
  | .invalid issues => issues ≠ []
  | .infeasible _ _ => True
  | .applicable _ deltas => ∃ effects staged,
      analysis.prepared = some staged ∧ deltas = effects.map effectDelta ∧
      List.Forall₂ (PreparedEntry accounts) effects staged ∧ (staged.map Prod.fst).Nodup ∧
      ∀ entry ∈ staged, entry.1 ∈ accounts.map Prod.fst

theorem stages_ready [DecidableEq AccountId] [DecidableEq A]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderedEffects.OrderLaws cmpAccount) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (analysis : Analysis AccountId A RateId Role)
    (success : stages cmpAccount cmpRole model proposal rule = .ok analysis) : AnalysisReady model.accounts analysis := by
  let context := Evaluation.fromState cmpRole 0 proposal.units proposal.parameters proposal.bindings model.accounts
  unfold stages at success
  change (do
    liftIssue (checkConditions proposal.rate context rule.conditions)
    let effects ← aggregate cmpAccount cmpRole proposal rule context
    let availability ← preflight proposal.rate model.accounts effects ⟨[], []⟩
    if availability.shortfalls.isEmpty then
      let prepared ← prepare model.accounts effects ⟨[], []⟩
      checkInvariants model.accounts prepared model.invariants
      return (⟨.applicable availability.accounts prepared.deltas, some prepared.accounts⟩ : Analysis AccountId A RateId Role)
    else return (⟨.infeasible availability.accounts availability.shortfalls, none⟩ : Analysis AccountId A RateId Role)) = .ok analysis at success
  cases conditions : liftIssue (AccountId := AccountId) (A := A) (checkConditions proposal.rate context rule.conditions) with
  | error error => simp only [conditions, checked_bind_error, reduceCtorEq] at success
  | ok value =>
    simp only [conditions, checked_bind_ok] at success
    cases aggregated : aggregate cmpAccount cmpRole proposal rule context with
    | error error => simp only [aggregated, checked_bind_error, reduceCtorEq] at success
    | ok effects =>
      simp only [aggregated, checked_bind_ok] at success
      cases available : preflight (Role := Role) proposal.rate model.accounts effects ⟨[], []⟩ with
      | error error => simp only [available, checked_bind_error, reduceCtorEq] at success
      | ok availability =>
        simp only [available, checked_bind_ok] at success
        by_cases feasible : availability.shortfalls.isEmpty = true
        · simp only [feasible, ite_true] at success
          cases preparation : prepare (RateId := RateId) (Role := Role) model.accounts effects ⟨[], []⟩ with
          | error error => simp only [preparation, checked_bind_error, reduceCtorEq] at success
          | ok prepared =>
            simp only [preparation, checked_bind_ok] at success
            cases invariants : checkInvariants (RateId := RateId) (Role := Role) model.accounts prepared model.invariants with
            | error error => simp only [invariants, checked_bind_error, reduceCtorEq] at success
            | ok value =>
              simp only [invariants, checked_bind_ok] at success
              have same : (⟨.applicable availability.accounts prepared.deltas, some prepared.accounts⟩ : Analysis AccountId A RateId Role) = analysis :=
                Except.ok.inj success
              rw [← same]
              have contract := preparation_contract model.accounts effects prepared preparation
              have unique := OrderedEffects.sorted_unique cmpAccount laws effects
                (OrderedEffects.aggregate_sorted cmpAccount cmpRole laws proposal rule context effects aggregated)
              exact ⟨effects, prepared.accounts, rfl, contract.2.1, contract.2.2.1,
                (by rw [contract.1]; exact unique), contract.2.2.2.1⟩
        · simp only [feasible] at success
          have same : (⟨.infeasible availability.accounts availability.shortfalls, none⟩ : Analysis AccountId A RateId Role) = analysis :=
            Except.ok.inj success
          rw [← same]
          trivial

theorem analyze_ready [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderedEffects.OrderLaws cmpAccount) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (analysis : Analysis AccountId A RateId Role)
    (success : analyze cmpAccount cmpRole model proposal = some analysis) : AnalysisReady model.accounts analysis := by
  unfold analyze at success
  cases validation : Validation.validate cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
  | error issues =>
    have nonempty := Validation.rejected_nonempty cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) issues validation
    have same : invalid (issues.map (Failure.structural (A := A))) = analysis := by simpa only [validation, Option.some.injEq] using success
    rw [← same]
    simpa [AnalysisReady, invalid] using nonempty
  | ok rule =>
    cases execution : stages cmpAccount cmpRole model proposal rule with
    | ok result =>
      have same : result = analysis := by simpa only [validation, execution, resolveStages, Option.some.injEq] using success
      exact same ▸ stages_ready cmpAccount cmpRole laws model proposal rule result execution
    | error fault =>
      cases fault with
      | panic => simp [validation, execution, resolveStages] at success
      | issue error =>
        have same : invalid [error] = analysis := by simpa only [validation, execution, resolveStages, Option.some.injEq] using success
        rw [← same]
        simp [AnalysisReady, invalid]

end Axionomy.Transaction
