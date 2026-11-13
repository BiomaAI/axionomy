import Axionomy.Ordered
import Axionomy.TransactionExecution

/-!
# Opening and closing accounts

An exchange may create accounts and remove them. A rate lists the roles whose
bound account it opens and the roles whose bound account it closes. The
proposal names each opened account; the kernel never invents identifiers.

* An opened account must not exist, and no other role may bind it. It exists,
  empty, before the exchange's effects apply.
* A closed account must be left empty by the exchange's own effects, so closing
  never destroys anything: whatever it held was moved by an explicit effect.
* A closed identifier may be opened again by a later exchange.
* Accounts are kept in the order of their identifiers: an opened account takes
  its place among them, so the accounts after an exchange are a function of
  which accounts there are, never of the order they were opened in.

The proven transition in `Transaction` does the rest unchanged: it analyzes
the exchange against the model in which the opened accounts exist.
-/

namespace Axionomy.Lifecycle
open Transaction

variable {AccountId A RateId Role : Type}

/-- The accounts bound to `roles`, in role order. Unbound roles are left to
validation, which reports them. -/
def boundTo (cmpRole : Role → Role → Ordering) (roles : List Role)
    (proposal : Proposal AccountId RateId Role) : List AccountId :=
  roles.filterMap fun role => Evaluation.orderedFind cmpRole role proposal.bindings

/-- Why an exchange cannot open its accounts. -/
def openingIssues [DecidableEq AccountId] [DecidableEq Role] (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role) (rule : Rule Role A)
    (proposal : Proposal AccountId RateId Role) : List (Failure AccountId A RateId Role) :=
  rule.opens.filterMap fun role =>
    match Evaluation.orderedFind cmpRole role proposal.bindings with
    | none => none
    | some account =>
      if (Account.find? account model.accounts).isSome then some (.accountExists account)
      else if proposal.bindings.any (fun binding => decide (binding.1 ≠ role ∧ binding.2 = account)) then
        some (.openingShared role)
      else none

/-- The model with `accounts` opened, empty, each in its place. -/
def opened (cmpAccount : AccountId → AccountId → Ordering) (model : Model AccountId A RateId Role)
    (accounts : List AccountId) : Model AccountId A RateId Role :=
  { model with
    accounts := Ordered.ordered cmpAccount (model.accounts ++ accounts.map fun account => (account, [])) }

/-- What `account` holds after the exchange's effects: its prepared balances
if the exchange touched it, else what it held before. -/
def remaining [DecidableEq AccountId] (accounts prepared : List (AccountId × Balances A))
    (account : AccountId) : Balances A :=
  match Account.find? account prepared with
  | some balances => balances
  | none => (Account.find? account accounts).getD []

/-- The closed accounts the exchange's effects would leave holding something. -/
def closingIssues [DecidableEq AccountId] (accounts prepared : List (AccountId × Balances A))
    (closed : List AccountId) : List (Failure AccountId A RateId Role) :=
  closed.filterMap fun account =>
    let left := remaining accounts prepared account
    if left.isEmpty then none else some (.closedNotEmpty account left)

/-- An analysis of the exchange against the model with its accounts opened,
refused if it would leave a closed account holding something. -/
def checkClosing [DecidableEq AccountId] (accounts : List (AccountId × Balances A)) (closed : List AccountId) :
    Analysis AccountId A RateId Role → Analysis AccountId A RateId Role
  | ⟨.applicable entries deltas, some staged⟩ =>
    match closingIssues accounts staged closed with
    | [] => ⟨.applicable entries deltas, some staged⟩
    | issue :: issues => invalid (issue :: issues)
  | other => other

def analyze [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role) :
    Option (Analysis AccountId A RateId Role) :=
  match Account.find? proposal.rate model.rates with
  | none => Transaction.analyze cmpAccount cmpRole model proposal
  | some rule =>
    match openingIssues cmpRole model rule proposal with
    | [] =>
      let extended := opened cmpAccount model (boundTo cmpRole rule.opens proposal)
      (Transaction.analyze cmpAccount cmpRole extended proposal).map
        (checkClosing extended.accounts (boundTo cmpRole rule.closes proposal))
    | issue :: issues => some (invalid (issue :: issues))

/-- A receipt that also names the accounts the exchange opened and closed. -/
structure Receipt (AccountId A RateId Role : Type) extends Transaction.Receipt AccountId A RateId Role where
  opened : List AccountId
  closed : List AccountId

def apply [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role) :
    Option (Except (Failure AccountId A RateId Role) (Receipt AccountId A RateId Role) × Model AccountId A RateId Role) := do
  let analysis ← analyze cmpAccount cmpRole model proposal
  match analysis.assessment with
  | .invalid issues => issues.head?.map fun error => (.error error, model)
  | .infeasible _ missing => some (.error (.infeasible missing), model)
  | .applicable _ deltas => do
    let rule ← Account.find? proposal.rate model.rates
    let openedAccounts := boundTo cmpRole rule.opens proposal
    let closedAccounts := boundTo cmpRole rule.closes proposal
    let prepared ← analysis.prepared
    let committed ← install (opened cmpAccount model openedAccounts).accounts prepared
    some (.ok ⟨⟨proposal, deltas⟩, openedAccounts, closedAccounts⟩,
      { model with accounts := committed.filter fun entry => decide (entry.1 ∉ closedAccounts) })

section Properties

variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)

omit [DecidableEq RateId] [DecidableEq Role] in
theorem checkClosing_conserves (invariants : List (Invariant A)) (accounts : List (AccountId × Balances A))
    (closed : List AccountId) (analysis : Analysis AccountId A RateId Role)
    (conserved : TransactionExecution.AnalysisConserves invariants analysis) :
    TransactionExecution.AnalysisConserves invariants (checkClosing accounts closed analysis) := by
  obtain ⟨assessment, prepared⟩ := analysis
  cases assessment <;> cases prepared <;> try exact conserved
  simp only [checkClosing]
  split
  · exact conserved
  · trivial

omit [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
/-- A closing check that leaves an analysis applicable found every closed
account empty after the exchange's effects. -/
theorem checkClosing_empty (accounts : List (AccountId × Balances A)) (closed : List AccountId)
    (analysis : Analysis AccountId A RateId Role) (entries : List (AccountAssessment AccountId A))
    (deltas : List (Delta AccountId A)) (staged : List (AccountId × Balances A))
    (applicable : checkClosing accounts closed analysis = ⟨.applicable entries deltas, some staged⟩) :
    ∀ account ∈ closed, remaining accounts staged account = [] := by
  obtain ⟨assessment, prepared⟩ := analysis
  cases assessment <;> cases prepared <;> simp only [checkClosing] at applicable <;>
    try (simp at applicable; done)
  rename_i innerEntries innerDeltas innerStaged
  split at applicable
  · rename_i issues
    simp only [Analysis.mk.injEq, Assessment.applicable.injEq, Option.some.injEq] at applicable
    obtain ⟨⟨-, -⟩, rfl⟩ := applicable
    intro account member
    have quiet := List.filterMap_eq_nil_iff.mp issues account member
    by_cases empty : (remaining accounts innerStaged account).isEmpty
    · exact List.isEmpty_iff.mp empty
    · simp [empty] at quiet
  · simp [invalid] at applicable

theorem analyze_conserves (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (analysis : Analysis AccountId A RateId Role)
    (success : analyze cmpAccount cmpRole model proposal = some analysis) :
    TransactionExecution.AnalysisConserves model.invariants analysis := by
  unfold analyze at success
  cases rule : Account.find? proposal.rate model.rates with
  | none =>
    simp only [rule] at success
    exact TransactionExecution.analysis_conserves cmpAccount cmpRole model proposal analysis success
  | some found =>
    simp only [rule] at success
    cases issues : openingIssues cmpRole model found proposal with
    | nil =>
      simp only [issues] at success
      cases inner : Transaction.analyze cmpAccount cmpRole (opened cmpAccount model (boundTo cmpRole found.opens proposal)) proposal with
      | none => simp [inner] at success
      | some result =>
        simp only [inner, Option.map_some, Option.some.injEq] at success
        subst success
        exact checkClosing_conserves _ _ _ _
          (TransactionExecution.analysis_conserves cmpAccount cmpRole
            (opened cmpAccount model (boundTo cmpRole found.opens proposal)) proposal result inner)
    | cons issue rest =>
      simp only [issues, Option.some.injEq] at success
      subst success
      trivial

/-- A rejected exchange changes nothing, whatever it would have opened or closed. -/
theorem rejection_atomic (model after : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (error : Failure AccountId A RateId Role)
    (rejected : apply cmpAccount cmpRole model proposal = some (.error error, after)) : after = model := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at rejected
  | some analysis =>
    cases assessed : analysis.assessment with
    | invalid issues =>
      cases first : issues.head? with
      | none => simp [apply, analyzed, assessed, first] at rejected
      | some issue =>
        exact (by simpa [apply, analyzed, assessed, first] using rejected : issue = error ∧ model = after).2.symm
    | infeasible accounts missing =>
      exact (by simpa [apply, analyzed, assessed] using rejected : Failure.infeasible missing = error ∧ model = after).2.symm
    | applicable accounts deltas =>
      cases rule : Account.find? proposal.rate model.rates with
      | none => simp [apply, analyzed, assessed, rule] at rejected
      | some found =>
        cases prepared : analysis.prepared with
        | none => simp [apply, analyzed, assessed, rule, prepared] at rejected
        | some staged =>
          cases installed : install (opened cmpAccount model (boundTo cmpRole found.opens proposal)).accounts staged <;>
            simp [apply, analyzed, assessed, rule, prepared, installed] at rejected

/-- Every receipt conserves every declared invariant, and the invariants are unchanged. -/
theorem apply_conserves (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (receipt : Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    after.invariants = model.invariants ∧
      ∀ invariant ∈ model.invariants, TransactionExecution.ReceiptConserves invariant receipt.toReceipt := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at success
  | some analysis =>
    have conserved := analyze_conserves cmpAccount cmpRole model proposal analysis analyzed
    cases assessed : analysis.assessment with
    | invalid issues => cases first : issues.head? <;> simp [apply, analyzed, assessed, first] at success
    | infeasible accounts missing => simp [apply, analyzed, assessed] at success
    | applicable accounts deltas =>
      cases rule : Account.find? proposal.rate model.rates with
      | none => simp [apply, analyzed, assessed, rule] at success
      | some found =>
        cases prepared : analysis.prepared with
        | none => simp [apply, analyzed, assessed, rule, prepared] at success
        | some staged =>
          cases installed : install (opened cmpAccount model (boundTo cmpRole found.opens proposal)).accounts staged with
          | none => simp [apply, analyzed, assessed, rule, prepared, installed] at success
          | some committed =>
            simp only [apply, analyzed, assessed, rule, prepared, installed, Option.bind_eq_bind,
              Option.bind_some, Option.some.injEq, Prod.mk.injEq, Except.ok.injEq] at success
            obtain ⟨rfl, rfl⟩ := success
            refine ⟨rfl, ?_⟩
            intro invariant member
            simp only [TransactionExecution.AnalysisConserves, assessed] at conserved
            exact Conservation.conservation_sound _ _ id (Conservation.contribution invariant.weights)
              (fun _ _ _ equal => (Option.some.inj equal).symm)
              (fun _ _ _ added => (Conservation.exactAdd_exact _ _ _).mp added) 0 deltas
              (conserved invariant member)

omit [DecidableEq A] [DecidableEq RateId] in
theorem opening_absent (model : Model AccountId A RateId Role) (rule : Rule Role A)
    (proposal : Proposal AccountId RateId Role) (quiet : openingIssues cmpRole model rule proposal = []) :
    ∀ account ∈ boundTo cmpRole rule.opens proposal, Account.find? account model.accounts = none := by
  intro account member
  obtain ⟨role, opening, bound⟩ := List.mem_filterMap.mp member
  have silent := List.filterMap_eq_nil_iff.mp quiet role opening
  simp only [bound] at silent
  by_cases present : (Account.find? account model.accounts).isSome
  · simp [present] at silent
  · simpa using present

theorem applicable_opening (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (analysis : Analysis AccountId A RateId Role) (rule : Rule Role A)
    (analyzed : analyze cmpAccount cmpRole model proposal = some analysis)
    (found : Account.find? proposal.rate model.rates = some rule)
    (accounts : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (assessed : analysis.assessment = .applicable accounts deltas) :
    openingIssues cmpRole model rule proposal = [] := by
  unfold analyze at analyzed
  simp only [found] at analyzed
  cases issues : openingIssues cmpRole model rule proposal with
  | nil => rfl
  | cons issue rest =>
    simp only [issues, Option.some.injEq] at analyzed
    subst analyzed
    simp [invalid] at assessed

/-- Every account an exchange opens was absent before it. -/
theorem apply_opens_new (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (receipt : Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    ∀ account ∈ receipt.opened, Account.find? account model.accounts = none := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at success
  | some analysis =>
    cases assessed : analysis.assessment with
    | invalid issues => cases first : issues.head? <;> simp [apply, analyzed, assessed, first] at success
    | infeasible accounts missing => simp [apply, analyzed, assessed] at success
    | applicable accounts deltas =>
      cases rule : Account.find? proposal.rate model.rates with
      | none => simp [apply, analyzed, assessed, rule] at success
      | some found =>
        cases prepared : analysis.prepared with
        | none => simp [apply, analyzed, assessed, rule, prepared] at success
        | some staged =>
          cases installed : install (opened cmpAccount model (boundTo cmpRole found.opens proposal)).accounts staged with
          | none => simp [apply, analyzed, assessed, rule, prepared, installed] at success
          | some committed =>
            simp only [apply, analyzed, assessed, rule, prepared, installed, Option.bind_eq_bind,
              Option.bind_some, Option.some.injEq, Prod.mk.injEq, Except.ok.injEq] at success
            obtain ⟨rfl, rfl⟩ := success
            exact opening_absent cmpRole model found proposal
              (applicable_opening cmpAccount cmpRole model proposal analysis found analyzed rule accounts deltas assessed)

/-- After an exchange, the accounts are those before it and those it opened,
in the order of their identifiers, less those it closed. -/
theorem apply_keys (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (receipt : Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    after.accounts.map Prod.fst =
      ((Ordered.ordered cmpAccount
          (model.accounts ++ receipt.opened.map fun account => (account, []))).map Prod.fst).filter
        fun account => decide (account ∉ receipt.closed) := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at success
  | some analysis =>
    cases assessed : analysis.assessment with
    | invalid issues => cases first : issues.head? <;> simp [apply, analyzed, assessed, first] at success
    | infeasible accounts missing => simp [apply, analyzed, assessed] at success
    | applicable accounts deltas =>
      cases rule : Account.find? proposal.rate model.rates with
      | none => simp [apply, analyzed, assessed, rule] at success
      | some found =>
        cases prepared : analysis.prepared with
        | none => simp [apply, analyzed, assessed, rule, prepared] at success
        | some staged =>
          cases installed : install (opened cmpAccount model (boundTo cmpRole found.opens proposal)).accounts staged with
          | none => simp [apply, analyzed, assessed, rule, prepared, installed] at success
          | some committed =>
            simp only [apply, analyzed, assessed, rule, prepared, installed, Option.bind_eq_bind,
              Option.bind_some, Option.some.injEq, Prod.mk.injEq, Except.ok.injEq] at success
            obtain ⟨rfl, rfl⟩ := success
            have keys := install_keys _ staged committed installed
            simp [opened] at keys
            dsimp only
            rw [← keys, List.filter_map]
            rfl

/-- After an exchange, the accounts are exactly those before it and those it
opened, less those it closed. -/
theorem apply_accounts (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (receipt : Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    (after.accounts.map Prod.fst).Perm
      ((model.accounts.map Prod.fst ++ receipt.opened).filter fun account => decide (account ∉ receipt.closed)) := by
  rw [apply_keys cmpAccount cmpRole model proposal receipt after success]
  apply List.Perm.filter
  have keys := (Ordered.ordered_perm cmpAccount
    (model.accounts ++ receipt.opened.map fun account => (account, []))).map Prod.fst
  simpa [Function.comp_def] using keys

/-- After an exchange, the accounts are in the order of their identifiers,
whatever order they were in before it and whatever it opened. -/
theorem apply_ordered (lawful : Ordered.Lawful cmpAccount) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role)
    (receipt : Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    (after.accounts.map Prod.fst).Pairwise fun left right => cmpAccount left right ≠ .gt := by
  rw [apply_keys cmpAccount cmpRole model proposal receipt after success]
  apply List.Pairwise.filter
  exact List.pairwise_map.mpr (Ordered.ordered_pairwise lawful _)

/-- An applicable exchange leaves nothing in the accounts it closes: whatever
they held was moved by its own effects, so closing destroys nothing. -/
theorem applicable_closes_empty (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (analysis : Analysis AccountId A RateId Role) (rule : Rule Role A)
    (analyzed : analyze cmpAccount cmpRole model proposal = some analysis)
    (found : Account.find? proposal.rate model.rates = some rule)
    (accounts : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (assessed : analysis.assessment = .applicable accounts deltas)
    (staged : List (AccountId × Balances A)) (prepared : analysis.prepared = some staged) :
    ∀ account ∈ boundTo cmpRole rule.closes proposal,
      remaining (opened cmpAccount model (boundTo cmpRole rule.opens proposal)).accounts staged account = [] := by
  have quiet := applicable_opening cmpAccount cmpRole model proposal analysis rule analyzed found accounts deltas assessed
  unfold analyze at analyzed
  simp only [found, quiet] at analyzed
  cases inner : Transaction.analyze cmpAccount cmpRole (opened cmpAccount model (boundTo cmpRole rule.opens proposal)) proposal with
  | none => simp [inner] at analyzed
  | some result =>
    simp only [inner, Option.map_some, Option.some.injEq] at analyzed
    apply checkClosing_empty _ _ result accounts deltas staged
    rw [analyzed]
    obtain ⟨assessment, staging⟩ := analysis
    simp only at assessed prepared
    rw [assessed, prepared]

theorem accepted_conserves {model after : Model AccountId A RateId Role}
    {actions : List (Proposal AccountId RateId Role)} {receipts : List (Receipt AccountId A RateId Role)}
    (history : Execution.Accepted (apply cmpAccount cmpRole) model actions receipts after) :
    after.invariants = model.invariants ∧
      ∀ receipt ∈ receipts, ∀ invariant ∈ model.invariants,
        TransactionExecution.ReceiptConserves invariant receipt.toReceipt := by
  induction history with
  | nil => exact ⟨rfl, by simp⟩
  | @cons before middle after action receipt actions receipts executed following ih =>
    obtain ⟨same, checked⟩ := apply_conserves cmpAccount cmpRole before action receipt middle executed
    refine ⟨ih.1.trans same, ?_⟩
    intro candidate member invariant registered
    rcases List.mem_cons.mp member with equal | later
    · subst candidate; exact checked invariant registered
    · exact ih.2 candidate later invariant (by simpa [same] using registered)

/-- Every receipt of a successful replay, including those that open and close
accounts, conserves every declared invariant. -/
theorem replay_conserves (model after : Model AccountId A RateId Role)
    (actions : List (Proposal AccountId RateId Role)) (receipts : List (Receipt AccountId A RateId Role))
    (success : Execution.run (apply cmpAccount cmpRole) model actions [] = some (.ok receipts, after)) :
    after.invariants = model.invariants ∧
      ∀ receipt ∈ receipts, ∀ invariant ∈ model.invariants,
        TransactionExecution.ReceiptConserves invariant receipt.toReceipt := by
  obtain ⟨produced, same, history⟩ := Execution.success_history _ actions model after [] receipts success
  simp only [List.nil_append] at same
  subst produced
  exact accepted_conserves cmpAccount cmpRole history

end Properties

end Axionomy.Lifecycle
