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
      Forall₂ (PreparedEntry accounts) effects staged ∧ (staged.map Prod.fst).Nodup ∧
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

/-! ## An applicable exchange takes only what is held -/

/-- A preflight that finds no shortfall found every account the effects
name, holding, of each asset, at least what its effect consumes and at least
what it preserves. -/
theorem preflight_holds [DecidableEq AccountId] [DecidableEq A] (rate : RateId)
    (accounts : List (AccountId × Balances A)) (effects : List (AccountId × Effect A))
    (initial availability : Availability AccountId A)
    (valid : ∀ entry ∈ effects, TransactionAggregation.ValidEffect entry.2)
    (success : preflight (Role := Role) rate accounts effects initial = .ok availability)
    (enough : availability.shortfalls = []) :
    initial.shortfalls = [] ∧ ∀ entry ∈ effects, ∃ balances, Account.find? entry.1 accounts = some balances ∧
      ∀ asset, Account.balance 0 entry.2.consume asset ≤ Account.balance 0 balances asset ∧
        Account.balance 0 entry.2.preserve asset ≤ Account.balance 0 balances asset := by
  induction effects generalizing initial with
  | nil =>
    have same : initial = availability := Except.ok.inj success
    subst same
    exact ⟨enough, by simp⟩
  | cons entry rest ih =>
    obtain ⟨id, effect⟩ := entry
    cases found : Account.find? id accounts with
    | none => simp [preflight, found, require] at success
    | some balances =>
      cases credited : Account.credit (Account.naturals Quantity.limit) effect.consume effect.preserve with
      | error asset => simp [preflight, found, require, credited, rateResult] at success
      | ok required =>
        simp only [preflight, found, require, credited, rateResult, checked_bind_ok] at success
        have later := ih _ (fun entry member => valid entry (List.mem_cons_of_mem _ member)) success
        have parts := later.1
        simp only [List.append_eq_nil_iff] at parts
        refine ⟨parts.1, ?_⟩
        intro entry member
        rcases List.mem_cons.mp member with same | other
        · subst same
          have short : Account.shortfall (Account.naturals Quantity.limit) balances required = [] := by
            by_cases empty : (Account.shortfall (Account.naturals Quantity.limit) balances required).isEmpty = true
            · exact List.isEmpty_iff.mp empty
            · simp [empty] at parts
          have own := valid (id, effect) (List.mem_cons_self ..)
          have totals := Account.credit_total Quantity.limit own.1.1 credited
          refine ⟨balances, found, fun asset => ?_⟩
          have covered : Account.balance 0 effect.consume asset + Account.total effect.preserve asset ≤
              Account.balance 0 balances asset := by
            rw [← totals.1 asset]
            exact BasketOperations.shortfall_empty_balance Quantity.limit balances required totals.2 short asset
          have preserved := Account.balance_le_total effect.preserve asset
          exact ⟨Nat.le_trans (Nat.le_add_right _ _) covered,
            Nat.le_trans (Nat.le_trans preserved (Nat.le_add_left _ _)) covered⟩
        · exact later.2 entry other

/-- An exchange its stages find applicable takes, from the account each role
of its rate is bound to, no more than the account holds: what the role
consumes, times the units, and what it preserves. -/
theorem stages_holds [DecidableEq AccountId] [DecidableEq A]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (unique : ∀ entry ∈ rule.consume, (entry.2.map Prod.fst).Nodup)
    (entries : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (prepared : Option (List (AccountId × Balances A)))
    (success : stages cmpAccount cmpRole model proposal rule = .ok ⟨.applicable entries deltas, prepared⟩) :
    ∀ role ∈ rule.roles, ∀ account, Evaluation.orderedFind cmpRole role proposal.bindings = some account →
      ∃ balances, Account.find? account model.accounts = some balances ∧
        TransactionAggregation.Holds cmpRole proposal rule role balances balances := by
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
    else return (⟨.infeasible availability.accounts availability.shortfalls, none⟩ : Analysis AccountId A RateId Role)) =
      .ok ⟨.applicable entries deltas, prepared⟩ at success
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
        · have taken := TransactionAggregation.aggregate_holds cmpAccount cmpRole eqLaw proposal rule unique
            context effects aggregated
          have held := (preflight_holds proposal.rate model.accounts effects ⟨[], []⟩ availability taken.1
            available (List.isEmpty_iff.mp feasible)).2
          intro role member account bound
          obtain ⟨effect, present, holds⟩ := taken.2 role member account bound
          obtain ⟨balances, found, enough⟩ := held (account, effect) present
          exact ⟨balances, found, holds.mono (fun asset => (enough asset).1) (fun asset => (enough asset).2)⟩
        · simp only [feasible] at success
          simp [pure, Except.pure] at success

/-- An exchange the kernel finds applicable takes, from the account each role
of its rate is bound to, no more than the account holds: what the role
consumes, times the units, and what it preserves. -/
theorem analyze_holds [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (found : Account.find? proposal.rate model.rates = some rule)
    (unique : ∀ entry ∈ rule.consume, (entry.2.map Prod.fst).Nodup)
    (analysis : Analysis AccountId A RateId Role)
    (success : analyze cmpAccount cmpRole model proposal = some analysis)
    (entries : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (assessed : analysis.assessment = .applicable entries deltas) :
    ∀ role ∈ rule.roles, ∀ account, Evaluation.orderedFind cmpRole role proposal.bindings = some account →
      ∃ balances, Account.find? account model.accounts = some balances ∧
        TransactionAggregation.Holds cmpRole proposal rule role balances balances := by
  obtain ⟨assessment, prepared⟩ := analysis
  simp only at assessed
  subst assessed
  unfold analyze at success
  cases validation : Validation.validate cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
  | error issues => simp [validation, invalid] at success
  | ok selected =>
    have chosen := (Validation.accepted_rule cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters _ selected validation).1
    rw [found, Option.some.injEq] at chosen
    subst chosen
    cases execution : stages cmpAccount cmpRole model proposal rule with
    | ok result =>
      have same : result = ⟨.applicable entries deltas, prepared⟩ := by
        simpa only [validation, execution, resolveStages, Option.some.injEq] using success
      subst same
      exact stages_holds cmpAccount cmpRole eqLaw model proposal rule unique entries deltas prepared execution
    | error fault =>
      cases fault with
      | panic => simp [validation, execution, resolveStages] at success
      | issue error => simp [validation, execution, resolveStages, invalid] at success

end Axionomy.Transaction
