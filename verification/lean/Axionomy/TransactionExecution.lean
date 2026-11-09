import Axionomy.Execution
import Axionomy.TransactionReceipts

namespace Axionomy.TransactionExecution
open Transaction

variable {AccountId A RateId Role : Type}
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)

theorem rejection_atomic (model after : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (error : Failure AccountId A RateId Role)
    (rejected : apply cmpAccount cmpRole model proposal = some (.error error, after)) : after = model := by
  unfold apply at rejected
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [analyzed] at rejected
  | some analysis =>
    exact applyAnalysis_rejection_atomic model proposal analysis error after (by simpa [analyzed] using rejected)

theorem replay_error_prefix (model after : Model AccountId A RateId Role) (actions : List (Proposal AccountId RateId Role))
    (error : Failure AccountId A RateId Role)
    (failed : Execution.run (apply cmpAccount cmpRole) model actions [] = some (.error error, after)) :
    ∃ leading action trailing receipts,
      actions = leading ++ action :: trailing ∧
      Execution.run (apply cmpAccount cmpRole) model leading [] = some (.ok receipts, after) ∧
      apply cmpAccount cmpRole after action = some (.error error, after) :=
  Execution.atomic_error_prefix _ (fun state action error after rejected => rejection_atomic cmpAccount cmpRole state after action error rejected)
    actions model after [] error failed

theorem replay_receipt_order (laws : OrderedEffects.OrderLaws cmpAccount)
    (model after : Model AccountId A RateId Role) (actions : List (Proposal AccountId RateId Role))
    (receipts : List (Receipt AccountId A RateId Role))
    (success : Execution.run (apply cmpAccount cmpRole) model actions [] = some (.ok receipts, after)) :
    receipts.map Receipt.proposal = actions := by
  obtain ⟨produced, same, history⟩ := Execution.success_history _ actions model after [] receipts success
  simp only [List.nil_append] at same
  subst produced
  exact history.receipt_order _ Receipt.proposal
    (fun state action receipt after accepted => (apply_receipt_changes cmpAccount cmpRole laws state action receipt after accepted).1)

/-- Every receipt in a successful history describes the actual balance change
at that step, including when later transactions revisit the same account. -/
def Changes : Model AccountId A RateId Role → List (Proposal AccountId RateId Role) →
    List (Receipt AccountId A RateId Role) → Model AccountId A RateId Role → Prop
  | before, [], [], after => after = before
  | before, action :: actions, receipt :: receipts, after =>
      ∃ middle, receipt.proposal = action ∧ ReceiptChanges before middle receipt ∧ Changes middle actions receipts after
  | _, _, _, _ => False

theorem accepted_changes (laws : OrderedEffects.OrderLaws cmpAccount)
    {model after : Model AccountId A RateId Role} {actions : List (Proposal AccountId RateId Role)}
    {receipts : List (Receipt AccountId A RateId Role)}
    (history : Execution.Accepted (apply cmpAccount cmpRole) model actions receipts after) :
    Changes model actions receipts after := by
  induction history with
  | nil => rfl
  | @cons before middle after action receipt actions receipts executed following ih =>
    have changed := apply_receipt_changes cmpAccount cmpRole laws before action receipt middle executed
    exact ⟨middle, changed.1, changed.2, ih⟩

def AnalysisConserves (invariants : List (Invariant A)) (analysis : Analysis AccountId A RateId Role) : Prop :=
  match analysis.assessment with
  | .applicable _ deltas => ∀ invariant ∈ invariants,
      Conservation.conserves (fun asset amount => some (Conservation.contribution invariant.weights asset amount))
        Conservation.exactAdd 0 deltas = some true
  | _ => True

omit [DecidableEq RateId] [DecidableEq Role] in
theorem checks_conserve (accounts : List (AccountId × Balances A)) (prepared : Preparation AccountId A)
    (invariants : List (Invariant A))
    (checked : checkInvariants (RateId := RateId) (Role := Role) accounts prepared invariants = .ok ()) :
    ∀ invariant ∈ invariants,
      Conservation.conserves (fun asset amount => some (Conservation.contribution invariant.weights asset amount))
        Conservation.exactAdd 0 prepared.deltas = some true := by
  induction invariants with
  | nil => simp
  | cons invariant rest ih =>
    let weighted := fun asset amount => some (Conservation.contribution invariant.weights asset amount)
    cases conserved : Conservation.conserves weighted Conservation.exactAdd 0 prepared.deltas with
    | none => simp [checkInvariants, weighted, conserved] at checked
    | some result =>
      cases result with
      | true =>
        have following : checkInvariants (RateId := RateId) (Role := Role) accounts prepared rest = .ok () := by
          simpa [checkInvariants, weighted, conserved] using checked
        intro candidate member
        rcases List.mem_cons.mp member with same | later
        · subst candidate; exact conserved
        · exact ih following candidate later
      | false =>
        unfold checkInvariants at checked
        change (match Conservation.conserves weighted Conservation.exactAdd 0 prepared.deltas with
          | none => _ | some true => _ | some false => _) = _ at checked
        rw [conserved] at checked
        cases before : Conservation.accumulate weighted Conservation.exactAdd 0 (accounts.flatMap Prod.snd) <;>
          dsimp only [weighted] at before <;>
          simp only [before, checked_bind_error, reduceCtorEq, checked_bind_ok] at checked
        case some value =>
          cases installed : install accounts prepared.accounts <;>
            simp only [installed, require, checked_bind_error, reduceCtorEq, checked_bind_ok] at checked
          case some updated =>
            cases after : Conservation.accumulate weighted Conservation.exactAdd 0 (updated.flatMap Prod.snd) <;>
              dsimp only [weighted] at after <;>
              simp only [after, reduceCtorEq] at checked

omit [DecidableEq RateId] [DecidableEq Role] in
theorem stages_conserve
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (analysis : Analysis AccountId A RateId Role)
    (success : stages cmpAccount cmpRole model proposal rule = .ok analysis) : AnalysisConserves model.invariants analysis := by
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
              cases value
              exact checks_conserve model.accounts prepared model.invariants invariants
        · simp only [feasible] at success
          have same : (⟨.infeasible availability.accounts availability.shortfalls, none⟩ : Analysis AccountId A RateId Role) = analysis :=
            Except.ok.inj success
          rw [← same]
          trivial
theorem analysis_conserves
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (analysis : Analysis AccountId A RateId Role)
    (success : analyze cmpAccount cmpRole model proposal = some analysis) : AnalysisConserves model.invariants analysis := by
  unfold analyze at success
  cases validation : Validation.validate cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
  | error issues =>
    have same : invalid (issues.map (Failure.structural (A := A))) = analysis := by simpa only [validation, Option.some.injEq] using success
    rw [← same]
    trivial
  | ok rule =>
    cases execution : stages cmpAccount cmpRole model proposal rule with
    | ok result =>
      have same : result = analysis := by simpa only [validation, execution, resolveStages, Option.some.injEq] using success
      exact same ▸ stages_conserve cmpAccount cmpRole model proposal rule result execution
    | error fault =>
      cases fault with
      | panic => simp [validation, execution, resolveStages] at success
      | issue error =>
        have same : invalid [error] = analysis := by simpa only [validation, execution, resolveStages, Option.some.injEq] using success
        rw [← same]
        trivial

/-- Exact mathematical conservation of each registered weighted transfer. -/
def ReceiptConserves (invariant : Invariant A) (receipt : Receipt AccountId A RateId Role) : Prop :=
  (receipt.deltas.map fun delta => Conservation.measure (Conservation.contribution invariant.weights) delta.consumed).sum =
    (receipt.deltas.map fun delta => Conservation.measure (Conservation.contribution invariant.weights) delta.produced).sum

theorem apply_conserves (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (receipt : Receipt AccountId A RateId Role)
    (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    after.invariants = model.invariants ∧ ∀ invariant ∈ model.invariants, ReceiptConserves invariant receipt := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at success
  | some analysis =>
    have conserved := analysis_conserves cmpAccount cmpRole model proposal analysis analyzed
    cases assessed : analysis.assessment with
    | invalid issues =>
      cases first : issues.head? <;> simp [apply, analyzed, applyAnalysis, assessed, first] at success
    | infeasible accounts missing => simp [apply, analyzed, applyAnalysis, assessed] at success
    | applicable accounts deltas =>
      cases preparation : analysis.prepared with
      | none => simp [apply, analyzed, applyAnalysis, assessed, preparation] at success
      | some staged =>
        cases installed : install model.accounts staged with
        | none => simp [apply, analyzed, applyAnalysis, assessed, preparation, installed] at success
        | some committed =>
          have same : (⟨proposal, deltas⟩ : Receipt AccountId A RateId Role) = receipt ∧
              { model with accounts := committed } = after := by
            simpa [apply, analyzed, applyAnalysis, assessed, preparation, installed] using success
          rcases same with ⟨rfl, rfl⟩
          refine ⟨rfl, ?_⟩
          intro invariant member
          simp only [AnalysisConserves, assessed] at conserved
          have checked := conserved invariant member
          exact Conservation.conservation_sound _ _ id (Conservation.contribution invariant.weights)
            (fun _ _ _ equal => (Option.some.inj equal).symm)
            (fun _ _ _ added => (Conservation.exactAdd_exact _ _ _).mp added) 0 deltas checked

theorem accepted_conserves {model after : Model AccountId A RateId Role}
    {actions : List (Proposal AccountId RateId Role)} {receipts : List (Receipt AccountId A RateId Role)}
    (history : Execution.Accepted (apply cmpAccount cmpRole) model actions receipts after) :
    after.invariants = model.invariants ∧
      ∀ receipt ∈ receipts, ∀ invariant ∈ model.invariants, ReceiptConserves invariant receipt := by
  induction history with
  | nil => exact ⟨rfl, by simp⟩
  | @cons before middle after action receipt actions receipts executed following ih =>
    obtain ⟨same, checked⟩ := apply_conserves cmpAccount cmpRole before action receipt middle executed
    refine ⟨ih.1.trans same, ?_⟩
    intro candidate member invariant registered
    rcases List.mem_cons.mp member with equal | later
    · subst candidate; exact checked invariant registered
    · exact ih.2 candidate later invariant (by simpa [same] using registered)

theorem replay_conserves (model after : Model AccountId A RateId Role)
    (actions : List (Proposal AccountId RateId Role)) (receipts : List (Receipt AccountId A RateId Role))
    (success : Execution.run (apply cmpAccount cmpRole) model actions [] = some (.ok receipts, after)) :
    after.invariants = model.invariants ∧
      ∀ receipt ∈ receipts, ∀ invariant ∈ model.invariants, ReceiptConserves invariant receipt := by
  obtain ⟨produced, same, history⟩ := Execution.success_history _ actions model after [] receipts success
  simp only [List.nil_append] at same
  subst produced
  exact accepted_conserves cmpAccount cmpRole history

end Axionomy.TransactionExecution
