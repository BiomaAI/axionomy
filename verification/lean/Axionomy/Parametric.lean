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

An economy may also name constants: values every law reads as it reads its
variables, `{season-length}`, which no exchange changes. They are law, not
holdings: no account holds them and no role is bound to read them.
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
  /-- The values every law names as it names its variables. -/
  constants : Arguments := []

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
      match law.schema.relate arguments with
      | .error names => .error [.structural (.outsideRelation id names)]
      | .ok given =>
        match mustDiffer id law arguments with
        | issue :: issues => .error (issue :: issues)
        | [] =>
          match law.schema.derive (arguments ++ (given ++ model.constants)) with
          | .error name => .error [.structural (.derivedFails id name)]
          | .ok values =>
            match law.schema.instantiate cmpRole values with
            | .ok rule => .ok (rule, law.schema.fixed values)
            | .error (.overflow asset) => .error [.rateOverflow id asset]
            | .error _ => .error [.lawEvaluation id "a rate names a value it does not have"]

omit [DecidableEq Role] in
/-- An identifier resolves only to an instance its rate allows: the rate it
matches, with each value in its variable's domain and keying a row of every
relation, the values that must differ apart, and the values those rows give,
the economy's constants and the derived values computed after them. -/
theorem resolve_allowed {cmpRole : Role → Role → Ordering} {model : Model Role} {id : Written}
    {rule : Rule Role Written} {fixed : List (Role × Written)}
    (resolved : resolve cmpRole model id = .ok (rule, fixed)) :
    ∃ law arguments given values, find model.rates id = some (law, arguments) ∧
      (∀ declared ∈ law.schema.variables, ∃ value,
        lookup arguments declared.name = some value ∧ declared.domain.contains value = true) ∧
      (∀ relation ∈ law.schema.relations, ∃ row ∈ relation.rows,
        relation.names.mapM (lookup arguments) = some (row.take relation.names.length)) ∧
      (∀ pair ∈ law.schema.distinctVariables, lookup arguments pair.1 ≠ lookup arguments pair.2) ∧
      law.schema.relate arguments = .ok given ∧
      law.schema.derive (arguments ++ (given ++ model.constants)) = .ok values ∧
      law.schema.instantiate cmpRole values = .ok rule ∧ fixed = law.schema.fixed values := by
  unfold resolve at resolved
  cases found : find model.rates id with
  | none => simp [found] at resolved
  | some pair =>
    obtain ⟨law, arguments⟩ := pair
    simp only [found] at resolved
    cases domain : outsideDomain id law arguments with
    | some _ => simp [domain] at resolved
    | none =>
    simp only [domain] at resolved
    cases related : law.schema.relate arguments with
    | error _ => simp [related] at resolved
    | ok given =>
    simp only [related] at resolved
    cases differ : mustDiffer id law arguments with
    | cons _ _ => simp [differ] at resolved
    | nil =>
    simp only [differ] at resolved
    cases derived : law.schema.derive (arguments ++ (given ++ model.constants)) with
    | error _ => simp [derived] at resolved
    | ok values =>
    simp only [derived] at resolved
    cases built : law.schema.instantiate cmpRole values with
    | error failure => cases failure <;> simp [built] at resolved
    | ok built_rule =>
    simp only [built, Except.ok.injEq, Prod.mk.injEq] at resolved
    obtain ⟨same, fixed_same⟩ := resolved
    subst same; subst fixed_same
    refine ⟨law, arguments, given, values, rfl, ?_, relate_rows related, ?_, related, derived, built,
      rfl⟩
    · intro declared member
      have kept := List.findSome?_eq_none_iff.mp domain declared member
      cases looked : lookup arguments declared.name with
      | none => simp [looked] at kept
      | some value =>
        simp only [looked] at kept
        refine ⟨value, rfl, ?_⟩
        by_cases contains : declared.domain.contains value = true
        · exact contains
        · simp [contains] at kept
    · intro pair member equal
      have kept := List.filterMap_eq_nil_iff.mp differ pair member
      simp [equal] at kept

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
theorem head_keeps {α : Type} {failures : List (Failure Role)} {model after : Model Role}
    {result : Except (Failure Role) α}
    (refused : failures.head?.map (fun failure => ((Except.error failure : Except (Failure Role) α), model)) =
      some (result, after)) : after = model := by
  cases first : failures.head? with
  | none => simp [first] at refused
  | some failure => simp [first] at refused; exact refused.2.symm

/-- No exchange changes the law: an economy's constants, rates and
conservation laws are those it had, whatever an exchange does. -/
theorem apply_keeps_law (cmpRole : Role → Role → Ordering) (model after : Model Role)
    (proposal : Proposal Role) (result : Except (Failure Role) (Lifecycle.Receipt Written Written Written Role))
    (applied : apply cmpRole model proposal = some (result, after)) :
    after.constants = model.constants ∧ after.rates = model.rates ∧ after.invariants = model.invariants := by
  unfold apply at applied
  cases resolved : resolve cmpRole model proposal.rate with
  | error failures =>
    simp only [resolved] at applied
    rw [head_keeps applied]; exact ⟨rfl, rfl, rfl⟩
  | ok found =>
    obtain ⟨rule, fixed⟩ := found
    simp only [resolved] at applied
    cases wrong : wrongAccounts cmpRole proposal fixed with
    | cons issue issues =>
      simp only [wrong] at applied
      rw [head_keeps applied]; exact ⟨rfl, rfl, rfl⟩
    | nil =>
      simp only [wrong] at applied
      cases lifecycle : Lifecycle.apply Path.compare cmpRole (instanceModel model proposal.rate rule) proposal with
      | none => rw [lifecycle] at applied; simp at applied
      | some outcome =>
        rw [lifecycle] at applied
        simp only [Option.map_some, Option.some.injEq, Prod.mk.injEq] at applied
        rw [← applied.2]
        exact ⟨rfl, rfl, rfl⟩

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
  | error failures => simp only [resolved] at rejected; exact head_keeps rejected
  | ok found =>
    obtain ⟨rule, fixed⟩ := found
    simp only [resolved] at rejected
    cases wrong : wrongAccounts cmpRole proposal fixed with
    | cons issue issues => simp only [wrong] at rejected; exact head_keeps rejected
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

/-- Every exchange accepted takes only what is held: each account it binds
held, before it, at least what its rate's instance consumes from that role,
times the units, and what it preserves there. An account the exchange opens
held nothing before it, so it is taken nothing. A kernel may therefore
refuse, before assessing it, an exchange whose bound accounts do not hold
that much. -/
theorem accepted_holds {cmpRole : Role → Role → Ordering} (laws : Ordered.OrderLaws cmpRole)
    {model after : Model Role} {proposal : Proposal Role}
    {receipt : Lifecycle.Receipt Written Written Written Role} {rule : Rule Role Written}
    {fixed : List (Role × Written)} {role : Role} {account : Written}
    (resolved : resolve cmpRole model proposal.rate = .ok (rule, fixed))
    (success : apply cmpRole model proposal = some (.ok receipt, after))
    (member : role ∈ rule.roles)
    (bound : Evaluation.orderedFind cmpRole role proposal.bindings = some account) :
    (∀ asset q, (asset, q) ∈ (Evaluation.orderedFind cmpRole role rule.consume).getD [] →
      q * proposal.units ≤ Account.balance 0 ((Account.find? account model.accounts).getD []) asset) ∧
    (∀ asset q, (asset, q) ∈ (Evaluation.orderedFind cmpRole role rule.preserve).getD [] →
      q ≤ Account.balance 0 ((Account.find? account model.accounts).getD []) asset) := by
  obtain ⟨law, arguments, given, values, -, -, -, -, -, -, built, -⟩ := resolve_allowed resolved
  have canonical := instantiate_canonical cmpRole laws law.schema values built
  have unique : ∀ entry ∈ rule.consume, (entry.2.map Prod.fst).Nodup :=
    fun entry listed => (canonical.consume.2 entry listed).1
  unfold apply at success
  rw [resolved] at success
  cases wrong : wrongAccounts cmpRole proposal fixed with
  | cons issue issues =>
    simp only [wrong] at success
    exact (head_not_accepted success).elim
  | nil =>
    simp only [wrong] at success
    cases lifecycle : Lifecycle.apply Path.compare cmpRole (instanceModel model proposal.rate rule) proposal with
    | none => rw [lifecycle] at success; simp at success
    | some outcome =>
      obtain ⟨result, changed⟩ := outcome
      rw [lifecycle] at success
      simp only [Option.map_some, Option.some.injEq, Prod.mk.injEq] at success
      obtain ⟨accepted, -⟩ := success
      subst accepted
      exact Lifecycle.accepted_holds Path.compare cmpRole Path.orderLaws (instanceModel model proposal.rate rule)
        changed proposal receipt rule (by simp [instanceModel, Account.find?]) unique lifecycle member bound

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
  /-- A rate gives a name two values: its identifier names it twice, or it
  is at once two of a variable, a value a relation gives, a derived value
  and a constant. -/
  | repeatedVariable (rate : Pattern) (name : String)
  /-- A rate names a variable its identifier does not have. -/
  | undeclaredVariable (rate : Pattern) (name : String)
  /-- A rate declares a variable its identifier does not have. -/
  | unreadVariable (rate : Pattern) (name : String)
  /-- A quantity names a value that may be a word: a variable with listed
  values, a value a relation gives a word for, or a word constant. -/
  | notNatural (rate : Pattern) (name : String)
  /-- A variable's values repeat one. -/
  | repeatedValue (rate : Pattern) (name : String)
  /-- An identifier matches both rates. -/
  | overlapping (rate other : Pattern)
  /-- A row of a relation holds other than a value for each of its
  variables, then a value for each it gives: a word, or a whole number a
  quantity can hold. -/
  | malformedRelation (rate : Pattern) (names : List String)
  /-- Two rows of a relation are keyed by the same values. -/
  | repeatedRow (rate : Pattern) (names : List String)
  /-- A derived value reads what only an exchange has. -/
  | derivedReads (rate : Pattern) (name : String)
  /-- Two constants have one name. -/
  | repeatedConstant (name : String)
  /-- A constant is a number no quantity can hold. -/
  | malformedConstant (name : String)
  deriving DecidableEq, Repr

/-- A value a quantity cannot read as a number: a word. -/
def word : Path.Segment → Bool
  | .number _ => false
  | .word _ => true

/-- The names a relation gives a word for in some row. -/
def Relation.words (relation : Relation) : List String :=
  relation.gives.zipIdx.filterMap fun (name, index) =>
    if relation.rows.any (fun row => (row[relation.names.length + index]?).any word) then some name
    else none

/-- What is wrong with one rate on its own, in an economy whose constants
are `constants`: its identifier's variables, the values it names and
declares, and their values. -/
def Law.issues (constants : Arguments) (law : Law Role) : List ModelIssue :=
  let read := Pattern.variables law.id
  let declared := law.schema.variables.map (·.name)
  let given := law.schema.relations.flatMap (·.gives)
  let derived := law.schema.derived.map (·.name)
  let fixed := constants.map Prod.fst
  let repeated := [read, declared].flatMap fun names =>
    (names.zipIdx.filter fun (name, index) => name ∈ names.take index).map
      fun (name, _) => ModelIssue.repeatedVariable law.id name
  -- A value a relation gives and a derived value are each named once, and
  -- never as a variable is; no variable, value given or derived value is
  -- named as a constant.
  let fresh := given ++ derived
  let renamed := (fresh.zipIdx.filter fun (name, index) =>
    name ∈ declared || name ∈ fresh.take index).map
    fun (name, _) => ModelIssue.repeatedVariable law.id name
  let shadowing := ((declared ++ fresh).eraseDups.filter (· ∈ fixed)).map
    (ModelIssue.repeatedVariable law.id)
  -- The identifier, relations' keys and distinct pairs name variables;
  -- assets, accounts and quantities also the values relations give, the
  -- constants and derived values; a derived value only the variables,
  -- those values, the constants and the derived values before it.
  let unknown := (read ++ law.schema.identifying).filter (· ∉ declared) ++
    law.schema.named.filter (· ∉ declared ++ given ++ fixed ++ derived) ++
    (law.schema.derived.zipIdx.flatMap fun (value, index) =>
      value.value.variables.filter (· ∉ declared ++ given ++ fixed ++ derived.take index))
  let undeclared := unknown.eraseDups.map (ModelIssue.undeclaredVariable law.id)
  let unread := (declared.filter (· ∉ read)).map (ModelIssue.unreadVariable law.id)
  -- A quantity reads only what is always a number.
  let words := law.schema.relations.flatMap Relation.words ++
    (constants.filter (word ·.2)).map Prod.fst
  let natural := law.schema.naturals.eraseDups.filter fun name =>
    name ∈ words || law.schema.variables.any fun declared => declared.name = name &&
      match declared.domain with | .values _ => true | .range _ _ => false
  let values := law.schema.variables.filter fun declared =>
    match declared.domain with | .values listed => !listed.Nodup | .range _ _ => false
  let malformed := law.schema.relations.filter fun relation =>
    relation.rows.any fun row =>
      row.length != relation.names.length + relation.gives.length ||
        (row.drop relation.names.length).any fun value =>
          match value with | .number value => value > Quantity.limit | .word _ => false
  let rows := law.schema.relations.filter fun relation =>
    !(relation.rows.map (·.take relation.names.length)).Nodup
  let reads := law.schema.derived.filter (·.value.reads)
  repeated ++ renamed ++ shadowing ++ undeclared ++ unread ++
    natural.map (ModelIssue.notNatural law.id) ++
    values.map (fun declared => ModelIssue.repeatedValue law.id declared.name) ++
    malformed.map (fun relation => ModelIssue.malformedRelation law.id relation.names) ++
    rows.map (fun relation => ModelIssue.repeatedRow law.id relation.names) ++
    reads.map (fun derived => ModelIssue.derivedReads law.id derived.name)

/-- Every pair of rates, each before the ones after it, an identifier matches
both of. -/
def overlaps : List (Law Role) → List ModelIssue
  | [] => []
  | law :: rest => (rest.filter fun other => !disjoint law.id other.id).map
      (fun other => ModelIssue.overlapping law.id other.id) ++ overlaps rest

/-- What is wrong with an economy's constants: a name given twice, and a
number no quantity can hold. -/
def constantIssues (constants : Arguments) : List ModelIssue :=
  let names := constants.map Prod.fst
  ((names.zipIdx.filter fun (name, index) => name ∈ names.take index).map
    fun (name, _) => ModelIssue.repeatedConstant name) ++
  (constants.filter fun (_, value) => match value with
    | .number value => value > Quantity.limit | .word _ => false).map
    fun (name, _) => ModelIssue.malformedConstant name

/-- What is wrong with an economy's constants and rates: the constants',
each rate's own issues, then the rates an identifier matches two of. -/
def check (constants : Arguments) (rates : List (Law Role)) : List ModelIssue :=
  constantIssues constants ++ rates.flatMap (Law.issues constants) ++ overlaps rates

omit [DecidableEq Role] in
/-- An economy whose rates check clean names each identifier's rate once. -/
theorem check_apart {constants : Arguments} {rates : List (Law Role)}
    (clean : check constants rates = []) : Apart rates := by
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
