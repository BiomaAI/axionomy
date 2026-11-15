import Axionomy.Schema
import Axionomy.Lifecycle

/-!
# Economies whose rates have variables

An economy's rates are laws: each has an identifier pattern and a schema. An
exchange names a rate written in full, `stabilize/1/east`; the one law whose
pattern it matches is its rate, and the values the match gives are the
law's variables. The exchange then applies that law's instance, the
ordinary rate the law is with those values, exactly as an economy holding
only that rate would apply it: every theorem about an exchange carries over
unchanged. Before it does, the values must lie in their domains, variables
the law requires to differ must, and every role the law fixes to an account
must be bound to it.
-/

namespace Axionomy.Parametric

open Pattern Schema Transaction

/-- A rate: the identifiers it answers to, and what it does. -/
structure Law (Role : Type) where
  id : Pattern
  schema : Schema Role

variable {Role : Type} [DecidableEq Role]

structure Model (Role : Type) where
  accounts : List (Written × Balances Written)
  rates : List (Law Role)
  invariants : List (Invariant Written)

abbrev Proposal (Role : Type) := Transaction.Proposal Written Written Role
abbrev Failure (Role : Type) := Transaction.Failure Written Written Written Role

/-- The law an identifier matches, and the values the match gives. -/
def find (rates : List (Law Role)) (id : Written) : Option (Law Role × Arguments) :=
  rates.findSome? fun law => (law, ·) <$> «match» law.id id

/-- The first variable, in the law's order, whose value lies outside its
domain. -/
def outsideDomain (id : Written) (law : Law Role) (arguments : Arguments) : Option (Failure Role) :=
  law.schema.variables.findSome? fun declared =>
    match lookup arguments declared.name with
    | some value => if declared.domain.contains value then none
      else some (.structural (.outsideDomain id declared.name))
    | none => some (.structural (.outsideDomain id declared.name))

/-- Every pair of variables the law requires to differ that have one value. -/
def mustDiffer (id : Written) (law : Law Role) (arguments : Arguments) : List (Failure Role) :=
  law.schema.distinctVariables.filterMap fun (left, right) =>
    if lookup arguments left = lookup arguments right then
      some (.structural (.variablesMustDiffer id left right)) else none

/-- The instance an identifier names, with the accounts it fixes; or why
there is none. -/
def resolve (cmpRole : Role → Role → Ordering) (model : Model Role) (id : Written) :
    Except (List (Failure Role)) (Rule Role Written × List (Role × Written)) :=
  match find model.rates id with
  | none => .error [.structural (.missingRate id)]
  | some (law, arguments) =>
    match outsideDomain id law arguments with
    | some issue => .error [issue]
    | none =>
      match mustDiffer id law arguments with
      | issue :: issues => .error (issue :: issues)
      | [] =>
        match law.schema.instantiate cmpRole arguments with
        | .ok rule => .ok (rule, law.schema.fixed arguments)
        | .error (.overflow asset) => .error [.rateOverflow id asset]
        | .error _ => .error [.lawEvaluation id "a rate names a value it does not have"]

/-- Every role a rate fixes that is bound to another account. -/
def wrongAccounts (cmpRole : Role → Role → Ordering) (proposal : Proposal Role)
    (fixed : List (Role × Written)) : List (Failure Role) :=
  fixed.filterMap fun (role, account) =>
    match Evaluation.orderedFind cmpRole role proposal.bindings with
    | some bound => if bound = account then none else some (.structural (.wrongAccount role account))
    | none => none

/-- The economy holding only the instance, under the identifier it was named
by. -/
def instanceModel (model : Model Role) (id : Written) (rule : Rule Role Written) :
    Transaction.Model Written Written Written Role :=
  ⟨model.accounts, [(id, rule)], model.invariants⟩

def zero (proposal : Proposal Role) : List (Failure Role) :=
  (Validation.zeroIssues proposal.units).map .structural

def analyze (cmpRole : Role → Role → Ordering) (model : Model Role) (proposal : Proposal Role) :
    Option (Analysis Written Written Written Role) :=
  match resolve cmpRole model proposal.rate with
  | .error failures => some (invalid (zero proposal ++ failures))
  | .ok (rule, fixed) =>
    match wrongAccounts cmpRole proposal fixed with
    | [] => Lifecycle.analyze Path.compare cmpRole (instanceModel model proposal.rate rule) proposal
    | issue :: issues => some (invalid (zero proposal ++ issue :: issues))

def apply (cmpRole : Role → Role → Ordering) (model : Model Role) (proposal : Proposal Role) :
    Option (Except (Failure Role) (Lifecycle.Receipt Written Written Written Role) × Model Role) :=
  match resolve cmpRole model proposal.rate with
  | .error failures => (zero proposal ++ failures).head?.map fun error => (.error error, model)
  | .ok (rule, fixed) =>
    match wrongAccounts cmpRole proposal fixed with
    | [] =>
      (Lifecycle.apply Path.compare cmpRole (instanceModel model proposal.rate rule) proposal).map
        fun (result, after) => (result, { model with accounts := after.accounts })
    | issue :: issues => (zero proposal ++ issue :: issues).head?.map fun error => (.error error, model)

omit [DecidableEq Role] in
/-- An exchange refused for its first failure changes nothing. -/
theorem head_rejected {α : Type} {failures : List (Failure Role)} {model after : Model Role}
    {error : Failure Role}
    (rejected : failures.head?.map (fun failure => ((Except.error failure : Except (Failure Role) α), model)) =
      some (.error error, after)) : after = model := by
  cases first : failures.head? with
  | none => simp [first] at rejected
  | some failure => simp [first] at rejected; exact rejected.2.symm

omit [DecidableEq Role] in
/-- An exchange refused for its first failure is not accepted. -/
theorem head_not_accepted {α : Type} {failures : List (Failure Role)} {model after : Model Role}
    {accepted : α}
    (claimed : failures.head?.map (fun failure => ((Except.error failure : Except (Failure Role) α), model)) =
      some (.ok accepted, after)) : False := by
  cases first : failures.head? with
  | none => simp [first] at claimed
  | some failure => simp [first] at claimed

/-- A rejected exchange changes nothing. -/
theorem rejection_atomic (cmpRole : Role → Role → Ordering) (model after : Model Role)
    (proposal : Proposal Role) (error : Failure Role)
    (rejected : apply cmpRole model proposal = some (.error error, after)) : after = model := by
  unfold apply at rejected
  cases resolved : resolve cmpRole model proposal.rate with
  | error failures => simp only [resolved] at rejected; exact head_rejected rejected
  | ok found =>
    obtain ⟨rule, fixed⟩ := found
    simp only [resolved] at rejected
    cases wrong : wrongAccounts cmpRole proposal fixed with
    | cons issue issues => simp only [wrong] at rejected; exact head_rejected rejected
    | nil =>
      simp only [wrong] at rejected
      cases applied : Lifecycle.apply Path.compare cmpRole (instanceModel model proposal.rate rule) proposal with
      | none => rw [applied] at rejected; simp at rejected
      | some outcome =>
        obtain ⟨result, changed⟩ := outcome
        rw [applied] at rejected
        simp only [Option.map_some, Option.some.injEq, Prod.mk.injEq] at rejected
        obtain ⟨failed, accounts⟩ := rejected
        subst failed
        have same := Lifecycle.rejection_atomic Path.compare cmpRole _ _ proposal error applied
        subst same
        rw [← accounts]
        rfl

/-- An accepted exchange keeps every declared conservation law. -/
theorem apply_conserves (cmpRole : Role → Role → Ordering) (model after : Model Role)
    (proposal : Proposal Role) (receipt : Lifecycle.Receipt Written Written Written Role)
    (success : apply cmpRole model proposal = some (.ok receipt, after)) :
    after.invariants = model.invariants ∧
      ∀ invariant ∈ model.invariants, TransactionExecution.ReceiptConserves invariant receipt.toReceipt := by
  unfold apply at success
  cases resolved : resolve cmpRole model proposal.rate with
  | error failures => simp only [resolved] at success; exact (head_not_accepted success).elim
  | ok found =>
    obtain ⟨rule, fixed⟩ := found
    simp only [resolved] at success
    cases wrong : wrongAccounts cmpRole proposal fixed with
    | cons issue issues => simp only [wrong] at success; exact (head_not_accepted success).elim
    | nil =>
      simp only [wrong] at success
      cases applied : Lifecycle.apply Path.compare cmpRole (instanceModel model proposal.rate rule) proposal with
      | none => rw [applied] at success; simp at success
      | some outcome =>
        obtain ⟨result, changed⟩ := outcome
        rw [applied] at success
        simp only [Option.map_some, Option.some.injEq, Prod.mk.injEq] at success
        obtain ⟨accepted, accounts⟩ := success
        subst accepted
        have conserved := Lifecycle.apply_conserves Path.compare cmpRole _ proposal receipt _ applied
        exact ⟨by rw [← accounts], conserved.2⟩

/-- An accepted exchange binds every role its rate fixes to no account but
the one fixed. -/
theorem fixed_bound (cmpRole : Role → Role → Ordering) (model after : Model Role)
    (proposal : Proposal Role) (receipt : Lifecycle.Receipt Written Written Written Role)
    (rule : Rule Role Written) (fixed : List (Role × Written))
    (resolved : resolve cmpRole model proposal.rate = .ok (rule, fixed))
    (success : apply cmpRole model proposal = some (.ok receipt, after)) :
    ∀ role account, (role, account) ∈ fixed → ∀ bound,
      Evaluation.orderedFind cmpRole role proposal.bindings = some bound → bound = account := by
  intro role account member bound found
  unfold apply at success
  rw [resolved] at success
  cases wrong : wrongAccounts cmpRole proposal fixed with
  | cons issue issues =>
    simp only [wrong] at success
    exact (head_not_accepted success).elim
  | nil =>
    by_cases same : bound = account
    · exact same
    · have reported : Failure.structural (.wrongAccount role account) ∈ wrongAccounts cmpRole proposal fixed :=
        List.mem_filterMap.mpr ⟨(role, account), member, by simp [found, same]⟩
      simp [wrong] at reported

/-- Laws no identifier matches two of. -/
def Apart (rates : List (Law Role)) : Prop :=
  rates.Pairwise fun left right => disjoint left.id right.id = true

omit [DecidableEq Role] in
/-- An identifier names the one law whose pattern it matches, with the values
the match gives. -/
theorem find_matched {rates : List (Law Role)} (apart : Apart rates) {law : Law Role}
    (member : law ∈ rates) {id : Written} {arguments : Arguments}
    (matched : «match» law.id id = some arguments) : find rates id = some (law, arguments) := by
  induction rates with
  | nil => simp at member
  | cons first rest ih =>
    simp only [Apart, List.pairwise_cons] at apart
    rcases List.mem_cons.mp member with same | later
    · subst same; simp [find, List.findSome?, matched]
    · have other : «match» first.id id = none := by
        rcases disjoint_match (apart.1 law later) id with none | none
        · exact none
        · simp [matched] at none
      simp [find, List.findSome?, other]
      exact ih apart.2 later

omit [DecidableEq Role] in
/-- The values an identifier gives write it again: a law whose pattern names
no variable twice reads an identifier and writes it back. -/
theorem find_written {rates : List (Law Role)} {id : Written} {law : Law Role}
    {arguments : Arguments} (found : find rates id = some (law, arguments))
    (nodup : (variables law.id).Nodup) : substitute arguments law.id = some id := by
  unfold find at found
  obtain ⟨candidate, _, matched⟩ := List.exists_of_findSome?_eq_some found
  cases result : «match» candidate.id id with
  | none => simp [result] at matched
  | some values =>
    simp only [result, Option.map_eq_map, Option.map_some, Option.some.injEq, Prod.mk.injEq] at matched
    obtain ⟨same, values_same⟩ := matched
    subst same; subst values_same
    exact match_substitute nodup result

/-- What makes an economy's rates unusable, found when it is built. -/
inductive ModelIssue where
  /-- A variable appears twice in a rate's identifier. -/
  | repeatedVariable (rate : Pattern) (name : String)
  /-- A rate names a variable its identifier does not have. -/
  | undeclaredVariable (rate : Pattern) (name : String)
  /-- A rate declares a variable its identifier does not have. -/
  | unreadVariable (rate : Pattern) (name : String)
  /-- A quantity names a variable whose values are not whole numbers. -/
  | notNatural (rate : Pattern) (name : String)
  /-- A variable's values repeat one. -/
  | repeatedValue (rate : Pattern) (name : String)
  /-- An identifier matches both rates. -/
  | overlapping (rate other : Pattern)
  deriving DecidableEq, Repr

/-- What is wrong with one rate on its own: its identifier's variables, the
variables it names and declares, and their values. -/
def Law.issues (law : Law Role) : List ModelIssue :=
  let read := Pattern.variables law.id
  let declared := law.schema.variables.map (·.name)
  let repeated := (read.zipIdx.filter fun (name, index) => name ∈ read.take index).map
    fun (name, _) => ModelIssue.repeatedVariable law.id name
  let undeclared := ((read ++ law.schema.named).eraseDups.filter (· ∉ declared)).map
    (ModelIssue.undeclaredVariable law.id)
  let unread := (declared.filter (· ∉ read)).map (ModelIssue.unreadVariable law.id)
  let natural := law.schema.naturals.eraseDups.filter fun name =>
    law.schema.variables.any fun declared => declared.name = name &&
      match declared.domain with | .values _ => true | .range _ _ => false
  let values := law.schema.variables.filter fun declared =>
    match declared.domain with | .values listed => !listed.Nodup | .range _ _ => false
  repeated ++ undeclared ++ unread ++ natural.map (ModelIssue.notNatural law.id) ++
    values.map (fun declared => ModelIssue.repeatedValue law.id declared.name)

/-- Every pair of rates, each before the ones after it, an identifier matches
both of. -/
def overlaps : List (Law Role) → List ModelIssue
  | [] => []
  | law :: rest => (rest.filter fun other => !disjoint law.id other.id).map
      (fun other => ModelIssue.overlapping law.id other.id) ++ overlaps rest

/-- What is wrong with an economy's rates: each rate's own issues, then the
rates an identifier matches two of. -/
def check (rates : List (Law Role)) : List ModelIssue :=
  rates.flatMap Law.issues ++ overlaps rates

omit [DecidableEq Role] in
/-- An economy whose rates check clean names each identifier's rate once. -/
theorem check_apart {rates : List (Law Role)} (clean : check rates = []) : Apart rates := by
  have none : overlaps rates = [] := by
    simp only [check, List.append_eq_nil_iff] at clean
    exact clean.2
  clear clean
  induction rates with
  | nil => exact List.Pairwise.nil
  | cons law rest ih =>
    simp only [overlaps, List.append_eq_nil_iff, List.map_eq_nil_iff, List.filter_eq_nil_iff] at none
    refine List.Pairwise.cons ?_ (ih none.2)
    intro other member
    simpa using none.1 other member

end Axionomy.Parametric
