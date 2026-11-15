import Axionomy.Pattern
import Axionomy.Transaction
import Axionomy.OrderedEffects

/-!
# A rate with variables, and its instances

A rate may name what it moves through patterns: `at/{location}` is the asset
`at/east` where `location` is `east`. Given its variables' values, a rate
has one instance: the ordinary rate written with those values, built as a
rate written by hand is built. Entries naming the same asset after
substitution add up, as two entries for one asset add up in a rate written
by hand; a variable with whole-number values stands, in a quantity, for its
value.
-/

namespace Axionomy.Schema

open Pattern Path

/-- An identifier written out: its segments. -/
abbrev Written := List Segment

/-- A quantity a rate computes, naming assets through patterns and whole-
number variables by name. -/
inductive Term (Role : Type) where
  | constant (value : Nat)
  | units
  | parameter (name : String)
  | var (name : String)
  | balance (role : Role) (asset : Pattern)
  | binary (op : Rates.Op) (left right : Term Role)
  | mulDiv (rounding : MulDiv.Rounding) (left right denominator : Term Role)

structure Amount (Role : Type) where
  role : Role
  asset : Pattern
  quantity : Term Role

structure Check (Role : Type) where
  label : String
  left : Term Role
  comparison : Evaluation.Comparison
  right : Term Role

/-- The values some of a rate's variables may take together, one row each,
and the whole numbers each row gives: law structure that never changes,
such as which cells of a board adjoin and how far apart they are. A row
holds its variables' values, then the numbers it gives, named as `gives`
names them; its variables' values are its key. -/
structure Relation where
  names : List String
  gives : List String := []
  rows : List (List Segment)
  deriving DecidableEq, Repr

/-- The row of `relation` whose key is the values `arguments` give its
variables, if one is. -/
def Relation.row (relation : Relation) (arguments : Arguments) : Option (List Segment) :=
  match relation.names.mapM (lookup arguments) with
  | some values => relation.rows.find? (·.take relation.names.length == values)
  | none => none

/-- The numbers a row gives, by name. -/
def Relation.given (relation : Relation) (row : List Segment) : Arguments :=
  relation.gives.zip (row.drop relation.names.length)

/-- A row found for `arguments` is one of the relation's, keyed by the
values they give its variables. -/
theorem Relation.row_holds {relation : Relation} {arguments : Arguments} {row : List Segment}
    (found : relation.row arguments = some row) :
    row ∈ relation.rows ∧
      relation.names.mapM (lookup arguments) = some (row.take relation.names.length) := by
  unfold Relation.row at found
  cases values : relation.names.mapM (lookup arguments) with
  | none => simp [values] at found
  | some keyed =>
    simp only [values] at found
    have matched := List.find?_some found
    simp only [beq_iff_eq] at matched
    exact ⟨List.mem_of_find?_eq_some found, by rw [matched]⟩

/-- A whole number a rate computes from its variables' values, named as a
variable is; its identifier never names it. -/
structure Derived (Role : Type) where
  name : String
  value : Term Role

/-- A rate whose assets and accounts may be patterns. `accounts` fixes the
account a role is bound to. -/
structure Schema (Role : Type) where
  variables : List Variable
  distinctVariables : List (String × String) := []
  relations : List Relation := []
  derived : List (Derived Role) := []
  accounts : List (Role × Pattern) := []
  roles : List Role
  distinct : List (Role × Role)
  consume : List (Role × List (Pattern × Nat))
  produce : List (Role × List (Pattern × Nat))
  preserve : List (Role × List (Pattern × Nat))
  computedConsume : List (Amount Role)
  computedProduce : List (Amount Role)
  computedPreserve : List (Amount Role)
  conditions : List (Check Role)
  opens : List Role := []
  closes : List Role := []

/-- Why a rate has no instance for some values. A rate the kernel built
names only declared variables and uses only whole-number ones in quantities,
so only overflow reaches an exchange. -/
inductive Failure where
  | overflow (asset : Written)
  | unwritten
  | notNatural (name : String)
  deriving DecidableEq, Repr

variable {Role : Type}

def ops : Account.Arithmetic Nat := Account.naturals Quantity.limit

def Term.instantiate (arguments : Arguments) : Term Role → Except Failure (Evaluation.Expr Role Written Nat)
  | .constant value => .ok (.constant value)
  | .units => .ok .units
  | .parameter name => .ok (.parameter name)
  | .var name => match lookup arguments name with
    | some (.number value) => .ok (.constant value)
    | _ => .error (.notNatural name)
  | .balance role asset => match substitute arguments asset with
    | some written => .ok (.balance role written)
    | none => .error .unwritten
  | .binary op left right => do
    return .binary op (← left.instantiate arguments) (← right.instantiate arguments)
  | .mulDiv rounding left right denominator => do
    return .mulDiv rounding (← left.instantiate arguments) (← right.instantiate arguments)
      (← denominator.instantiate arguments)

/-- An entry written and added to what a role holds so far. -/
def addEntry (arguments : Arguments) (total : Transaction.Balances Written)
    (entry : Pattern × Nat) : Except Failure (Transaction.Balances Written) :=
  match substitute arguments entry.1 with
  | none => .error .unwritten
  | some written => match Account.credit ops total [(written, entry.2)] with
    | .error asset => .error (.overflow asset)
    | .ok merged => .ok merged

/-- One role's entries written and added up, entry by entry. -/
def basket (arguments : Arguments) (entries : List (Pattern × Nat)) :
    Except Failure (Transaction.Balances Written) :=
  entries.foldlM (addEntry arguments) []

/-- Each role's entries added to what the role already has, as a rate's
builder adds a basket to a role. -/
def addRole (cmpRole : Role → Role → Ordering) (arguments : Arguments)
    (target : List (Role × Transaction.Balances Written)) (entry : Role × List (Pattern × Nat)) :
    Except Failure (List (Role × Transaction.Balances Written)) := do
  let written ← basket arguments entry.2
  match RateConstruction.merge ops cmpRole target entry.1 written with
  | .error asset => .error (.overflow asset)
  | .ok merged => .ok merged

def baskets (cmpRole : Role → Role → Ordering) (arguments : Arguments)
    (entries : List (Role × List (Pattern × Nat))) :
    Except Failure (List (Role × Transaction.Balances Written)) :=
  entries.foldlM (addRole cmpRole arguments) []

def Amount.instantiate (arguments : Arguments) (amount : Amount Role) :
    Except Failure (Transaction.Computed Role Written) := do
  let some asset := substitute arguments amount.asset | .error .unwritten
  return ⟨amount.role, asset, ← amount.quantity.instantiate arguments⟩

def Check.instantiate (arguments : Arguments) (check : Check Role) :
    Except Failure (Transaction.Condition Role Written) := do
  return ⟨check.label, ← check.left.instantiate arguments, check.comparison,
    ← check.right.instantiate arguments⟩

/-- The rate's instance for `arguments`: the ordinary rate it is with those
values. -/
def Schema.instantiate (cmpRole : Role → Role → Ordering) (schema : Schema Role)
    (arguments : Arguments) : Except Failure (Transaction.Rule Role Written) := do
  return {
    roles := schema.roles
    distinct := schema.distinct
    consume := ← baskets cmpRole arguments schema.consume
    produce := ← baskets cmpRole arguments schema.produce
    preserve := ← baskets cmpRole arguments schema.preserve
    computedConsume := ← schema.computedConsume.mapM (·.instantiate arguments)
    computedProduce := ← schema.computedProduce.mapM (·.instantiate arguments)
    computedPreserve := ← schema.computedPreserve.mapM (·.instantiate arguments)
    conditions := ← schema.conditions.mapM (·.instantiate arguments)
    opens := schema.opens
    closes := schema.closes }

/-- Whether a quantity reads what only an exchange has: its units, a
parameter, or a balance. -/
def Term.reads : Term Role → Bool
  | .units | .parameter _ | .balance _ _ => true
  | .binary _ left right => left.reads || right.reads
  | .mulDiv _ left right denominator => left.reads || right.reads || denominator.reads
  | _ => false

/-- What the quantities read where nothing but the variables' values is known. -/
def unknown : Evaluation.Context Role Written Nat := ⟨0, fun _ => none, fun _ _ => .error .unboundRole⟩

/-- A derived value's number for `arguments`, if it is a whole number a
quantity can hold. -/
def Derived.evaluate (arguments : Arguments) (derived : Derived Role) : Option Nat :=
  match derived.value.instantiate arguments with
  | .error _ => none
  | .ok expression =>
    match Evaluation.eval (Evaluation.naturals Quantity.limit) unknown expression with
    | .ok value => some value
    | .error _ => none

/-- The variables' values followed by each derived value, in order, each
computed from the values before it; or the first that has no number. -/
def Schema.derive (schema : Schema Role) (arguments : Arguments) : Except String Arguments :=
  schema.derived.foldlM (init := arguments) fun known derived =>
    match derived.evaluate known with
    | some value => .ok (known ++ [(derived.name, .number value)])
    | none => .error derived.name

/-- Deriving values keeps the variables' values and adds after them. -/
theorem derive_extends {schema : Schema Role} {arguments values : Arguments}
    (derived : schema.derive arguments = .ok values) : ∃ added, values = arguments ++ added := by
  unfold Schema.derive at derived
  suffices general : ∀ (known : Arguments) (rest : List (Derived Role)) (result : Arguments),
      rest.foldlM (fun known derived =>
        match derived.evaluate known with
        | some value => .ok (known ++ [(derived.name, .number value)])
        | none => .error derived.name) known = (.ok result : Except String Arguments) →
      ∃ added, result = known ++ added by
    exact general arguments schema.derived values derived
  intro known rest
  induction rest generalizing known with
  | nil =>
    intro result done
    simp only [List.foldlM_nil, pure, Except.pure, Except.ok.injEq] at done
    exact ⟨[], by simp [done]⟩
  | cons head tail ih =>
    intro result done
    simp only [List.foldlM_cons, bind, Except.bind] at done
    cases evaluated : head.evaluate known with
    | none => simp [evaluated] at done
    | some value =>
      simp only [evaluated] at done
      obtain ⟨added, same⟩ := ih _ result done
      exact ⟨(head.name, .number value) :: added, by simp [same]⟩

/-- The numbers every relation's row for `arguments` gives, in the
relations' order; or the variables of the first relation no row of which is
keyed by the values `arguments` give them. -/
def Schema.relate (schema : Schema Role) (arguments : Arguments) : Except (List String) Arguments :=
  schema.relations.foldlM (init := []) fun given relation =>
    match relation.row arguments with
    | some row => .ok (given ++ relation.given row)
    | none => .error relation.names

/-- Relating values finds, for every relation, a row keyed by them. -/
theorem relate_rows {schema : Schema Role} {arguments given : Arguments}
    (related : schema.relate arguments = .ok given) :
    ∀ relation ∈ schema.relations, ∃ row ∈ relation.rows,
      relation.names.mapM (lookup arguments) = some (row.take relation.names.length) := by
  unfold Schema.relate at related
  suffices general : ∀ (known : Arguments) (rest : List Relation) (result : Arguments),
      rest.foldlM (fun given relation =>
        match relation.row arguments with
        | some row => .ok (given ++ relation.given row)
        | none => .error relation.names) known = (.ok result : Except (List String) Arguments) →
      ∀ relation ∈ rest, ∃ row ∈ relation.rows,
        relation.names.mapM (lookup arguments) = some (row.take relation.names.length) by
    exact general [] schema.relations given related
  intro known rest
  induction rest generalizing known with
  | nil =>
    intro _ _ relation member
    simp at member
  | cons head tail ih =>
    intro result done relation member
    simp only [List.foldlM_cons, bind, Except.bind] at done
    cases found : head.row arguments with
    | none => simp [found] at done
    | some row =>
      simp only [found] at done
      rcases List.mem_cons.mp member with same | later
      · subst same
        exact ⟨row, Relation.row_holds found⟩
      · exact ih _ result done relation later

/-- The accounts the rate fixes for `arguments`, role by role. -/
def Schema.fixed (schema : Schema Role) (arguments : Arguments) : List (Role × Written) :=
  schema.accounts.filterMap fun (role, account) => (role, ·) <$> substitute arguments account

def Term.variables {Role : Type} : Term Role → List String
  | .var name => [name]
  | .balance _ asset => Pattern.variables asset
  | .binary _ left right => left.variables ++ right.variables
  | .mulDiv _ left right denominator => left.variables ++ right.variables ++ denominator.variables
  | _ => []

def Term.naturals {Role : Type} : Term Role → List String
  | .var name => [name]
  | .binary _ left right => left.naturals ++ right.naturals
  | .mulDiv _ left right denominator => left.naturals ++ right.naturals ++ denominator.naturals
  | _ => []

/-- Every variable or derived value a schema's assets, accounts and
quantities name. -/
def Schema.named {Role : Type} (schema : Schema Role) : List String :=
  let amounts := schema.computedConsume ++ schema.computedProduce ++ schema.computedPreserve
  (schema.consume ++ schema.produce ++ schema.preserve).flatMap
      (fun (_, entries) => entries.flatMap fun (asset, _) => Pattern.variables asset) ++
    schema.accounts.flatMap (fun (_, account) => Pattern.variables account) ++
    amounts.flatMap (fun amount => Pattern.variables amount.asset ++ amount.quantity.variables) ++
    schema.conditions.flatMap (fun check => check.left.variables ++ check.right.variables)

/-- Every variable a schema names that only the identifier gives: those its
relations and distinct pairs constrain. -/
def Schema.identifying {Role : Type} (schema : Schema Role) : List String :=
  schema.relations.flatMap (·.names) ++
    schema.distinctVariables.flatMap (fun (left, right) => [left, right])

def Schema.naturals {Role : Type} (schema : Schema Role) : List String :=
  let amounts := schema.computedConsume ++ schema.computedProduce ++ schema.computedPreserve
  amounts.flatMap (fun amount => amount.quantity.naturals) ++
    schema.conditions.flatMap (fun check => check.left.naturals ++ check.right.naturals) ++
    schema.derived.flatMap (fun derived => derived.value.naturals)

/-- What an entry adds: its amount where it names `asset`. -/
def contribution (arguments : Arguments) (asset : Written) (entry : Pattern × Nat) : Nat :=
  if substitute arguments entry.1 = some asset then entry.2 else 0

/-- Adding an entry adds its amount to the asset it names, and keeps one
entry per asset. -/
theorem addEntry_sums {arguments : Arguments} {total merged : Transaction.Balances Written}
    {entry : Pattern × Nat} (unique : (total.map Prod.fst).Nodup)
    (added : addEntry arguments total entry = .ok merged) (asset : Written) :
    Account.balance 0 merged asset = Account.balance 0 total asset + contribution arguments asset entry ∧
      (merged.map Prod.fst).Nodup := by
  obtain ⟨pattern, amount⟩ := entry
  unfold addEntry at added
  cases written : substitute arguments pattern with
  | none => simp [written] at added
  | some name =>
    simp only [written] at added
    cases sum : ops.add (Account.balance ops.zero total name) amount with
    | none => simp [Account.credit, sum] at added
    | some updated =>
      simp only [Account.credit, sum] at added
      cases added
      have stored := Account.store_self Quantity.limit total name updated unique
      have value : updated = Account.balance 0 total name + amount := by
        simp only [ops, Account.naturals, Quantity.add, Quantity.checked] at sum
        split at sum <;> simp_all
      refine ⟨?_, by simpa [ops] using stored.2⟩
      by_cases same : name = asset
      · subst same
        simp only [contribution, written, ite_true]
        rw [show ops = Account.naturals Quantity.limit from rfl, stored.1, value]
      · have frame := Account.store_frame ops total name asset updated (Ne.symm same)
        have apart : ¬some name = some asset := by simpa using same
        simp only [contribution, written, apart, ite_false, Nat.add_zero]
        simpa [ops, Account.naturals] using frame

/-- Entries naming the same asset once written add up: a role's instance
holds, of each asset, the sum of the entries naming it, once. -/
theorem basket_sums {arguments : Arguments} {entries : List (Pattern × Nat)}
    {written : Transaction.Balances Written} (built : basket arguments entries = .ok written)
    (asset : Written) :
    Account.balance 0 written asset = (entries.map (contribution arguments asset)).sum ∧
      (written.map Prod.fst).Nodup := by
  suffices general : ∀ (total result : Transaction.Balances Written), (total.map Prod.fst).Nodup →
      entries.foldlM (addEntry arguments) total = .ok result →
      Account.balance 0 result asset =
        Account.balance 0 total asset + (entries.map (contribution arguments asset)).sum ∧
        (result.map Prod.fst).Nodup by
    simpa [Account.balance, Account.find?] using general [] written (by simp) built
  clear built
  induction entries with
  | nil =>
    intro total result unique done
    simp only [List.foldlM_nil, pure, Except.pure, Except.ok.injEq] at done
    subst done; simpa using unique
  | cons entry rest ih =>
    intro total result unique done
    simp only [List.foldlM_cons] at done
    cases added : addEntry arguments total entry with
    | error failure => simp [added, bind, Except.bind] at done
    | ok merged =>
      simp only [added, bind, Except.bind] at done
      have step := addEntry_sums unique added asset
      have later := ih merged result step.2 done
      refine ⟨?_, later.2⟩
      rw [later.1, step.1]
      simp [Nat.add_assoc]

/-! ## A rate written without variables

A rate with no variables is the rate written by hand: its instance, for any
values, is the rate itself. Every theorem about ordinary rates therefore
holds of it unchanged. -/

/-- An identifier written as a pattern with no variable. -/
def literal (path : Written) : Pattern := path.map .segment

theorem substitute_literal (arguments : Arguments) (path : Written) :
    substitute arguments (literal path) = some path := by
  induction path with
  | nil => rfl
  | cons segment rest ih =>
    simp only [literal, List.map_cons] at ih ⊢
    simp [substitute, ih]

/-- A quantity written without variables. -/
def Term.ofExpr : Evaluation.Expr Role Written Nat → Term Role
  | .constant value => .constant value
  | .units => .units
  | .parameter name => .parameter name
  | .balance role asset => .balance role (literal asset)
  | .binary op left right => .binary op (ofExpr left) (ofExpr right)
  | .mulDiv rounding left right denominator =>
    .mulDiv rounding (ofExpr left) (ofExpr right) (ofExpr denominator)

theorem Term.instantiate_ofExpr (arguments : Arguments) (expr : Evaluation.Expr Role Written Nat) :
    (Term.ofExpr expr).instantiate arguments = .ok expr := by
  induction expr with
  | constant => rfl
  | units => rfl
  | parameter => rfl
  | balance role asset => simp [Term.ofExpr, Term.instantiate, substitute_literal]
  | binary op left right left_ih right_ih =>
    simp [Term.ofExpr, Term.instantiate, left_ih, right_ih, bind, Except.bind, pure, Except.pure]
  | mulDiv rounding left right denominator left_ih right_ih denominator_ih =>
    simp [Term.ofExpr, Term.instantiate, left_ih, right_ih, denominator_ih, bind, Except.bind,
      pure, Except.pure]

/-- A role's entries as a builder keeps them: one per asset, each positive
and representable. -/
def Entries (assets : Transaction.Balances Written) : Prop :=
  (assets.map Prod.fst).Nodup ∧ ∀ entry ∈ assets, 0 < entry.2 ∧ entry.2 ≤ Quantity.limit

/-- Crediting new assets appends them. -/
theorem credit_fresh (pre rest : Transaction.Balances Written)
    (unique : ((pre ++ rest).map Prod.fst).Nodup)
    (bounded : ∀ entry ∈ rest, 0 < entry.2 ∧ entry.2 ≤ Quantity.limit) :
    Account.credit ops pre rest = .ok (pre ++ rest) := by
  induction rest generalizing pre with
  | nil => simp [Account.credit]
  | cons entry rest ih =>
    obtain ⟨asset, amount⟩ := entry
    have absent : asset ∉ pre.map Prod.fst := by
      intro member
      simp only [List.map_append, List.map_cons] at unique
      exact (List.nodup_append.mp unique).2.2 _ member _ (List.mem_cons_self ..) rfl
    have zero : Account.balance ops.zero pre asset = 0 := by
      simp [Account.balance, Account.find_absent asset pre absent, ops, Account.naturals]
    have bound := bounded (asset, amount) (List.mem_cons_self ..)
    have added : ops.add 0 amount = some amount := by
      simp [ops, Account.naturals, Quantity.add, Quantity.checked, bound.2]
    have stored : Account.store ops pre asset amount = pre ++ [(asset, amount)] := by
      have positive : (Account.naturals Quantity.limit).isZero amount = false := by
        simp [Account.naturals]; omega
      simp [Account.store, ops, positive, Account.upsert_fresh asset amount pre absent]
    simp only [Account.credit, zero, added, stored]
    have := ih (pre ++ [(asset, amount)]) (by simpa using unique)
      (fun entry member => bounded entry (List.mem_cons_of_mem _ member))
    simpa using this

/-- The entries of a role written without variables are written back as
they were. -/
theorem basket_literal (arguments : Arguments) (assets : Transaction.Balances Written)
    (entries : Entries assets) :
    basket arguments (assets.map fun (asset, amount) => (literal asset, amount)) = .ok assets := by
  suffices general : ∀ (pre rest : Transaction.Balances Written), rest.Sublist assets →
      ((pre ++ rest).map Prod.fst).Nodup →
      (rest.map fun (asset, amount) => (literal asset, amount)).foldlM (addEntry arguments) pre =
        .ok (pre ++ rest) by
    simpa [basket] using general [] assets (List.Sublist.refl _) entries.1
  intro pre rest
  induction rest generalizing pre with
  | nil => intro _ _; simp only [List.map_nil, List.foldlM_nil, List.append_nil]; rfl
  | cons entry rest ih =>
    intro sub unique
    obtain ⟨asset, amount⟩ := entry
    have bound := entries.2 (asset, amount) (sub.subset (List.mem_cons_self ..))
    have step : addEntry arguments pre (literal asset, amount) = .ok (pre ++ [(asset, amount)]) := by
      have credited := credit_fresh pre [(asset, amount)]
        (by
          have := (List.sublist_append_left (pre ++ [(asset, amount)]) rest).map Prod.fst
          exact List.Nodup.sublist this (by simpa using unique))
        (by simpa using bound)
      simp [addEntry, substitute_literal, credited]
    simp only [List.map_cons, List.foldlM_cons, step, bind, Except.bind]
    simpa using ih (pre ++ [(asset, amount)]) (List.sublist_of_cons_sublist sub) (by simpa using unique)

variable (cmpRole : Role → Role → Ordering)

/-- A role after every role a list holds is put at its end. -/
theorem locate_last {V : Type} (role : Role) (entries : List (Role × V))
    (after : ∀ key ∈ entries.map Prod.fst, cmpRole role key = .gt) :
    RateConstruction.locate cmpRole role entries = ⟨entries, role, none, []⟩ := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    obtain ⟨key, value⟩ := entry
    have greater := after key (List.mem_cons_self ..)
    have later := ih (fun other member => after other (List.mem_cons_of_mem _ member))
    simp [RateConstruction.locate, greater, later]

/-- Each role's entries as a builder keeps them: roles in order, each role's
entries one per asset. -/
def Built (entries : List (Role × Transaction.Balances Written)) : Prop :=
  OrderedEffects.Sorted cmpRole entries ∧ ∀ entry ∈ entries, Entries entry.2

theorem baskets_literal (laws : Ordered.OrderLaws cmpRole) (arguments : Arguments)
    (entries : List (Role × Transaction.Balances Written)) (built : Built cmpRole entries) :
    baskets cmpRole arguments
        (entries.map fun (role, assets) => (role, assets.map fun (asset, amount) => (literal asset, amount))) =
      .ok entries := by
  suffices general : ∀ (pre rest : List (Role × Transaction.Balances Written)), rest.Sublist entries →
      OrderedEffects.Sorted cmpRole (pre ++ rest) →
      (rest.map fun (role, assets) => (role, assets.map fun (asset, amount) => (literal asset, amount))).foldlM
          (addRole cmpRole arguments) pre = .ok (pre ++ rest) by
    simpa [baskets] using general [] entries (List.Sublist.refl _) built.1
  intro pre rest
  induction rest generalizing pre with
  | nil => intro _ _; simp only [List.map_nil, List.foldlM_nil, List.append_nil]; rfl
  | cons entry rest ih =>
    intro sub sorted
    obtain ⟨role, assets⟩ := entry
    have own := built.2 (role, assets) (sub.subset (List.mem_cons_self ..))
    have after : ∀ key ∈ pre.map Prod.fst, cmpRole role key = .gt := by
      intro key member
      have less : cmpRole key role = .lt := by
        simp only [OrderedEffects.Sorted, List.map_append, List.map_cons] at sorted
        exact (List.pairwise_append.mp sorted).2.2 key member role (List.mem_cons_self ..)
      cases compared : cmpRole role key with
      | gt => rfl
      | eq =>
        have := laws.equal role key compared
        subst this
        rw [laws.reflexive] at less
        contradiction
      | lt =>
        have := laws.transitive role key role compared less
        rw [laws.reflexive] at this
        contradiction
    have merged : RateConstruction.merge ops cmpRole pre role assets = .ok (pre ++ [(role, assets)]) := by
      have credited := credit_fresh [] assets (by simpa using own.1) own.2
      simp [RateConstruction.merge, locate_last cmpRole role pre after, credited,
        RateConstruction.Position.put]
    have step : addRole cmpRole arguments pre (role, assets.map fun (asset, amount) => (literal asset, amount)) =
        .ok (pre ++ [(role, assets)]) := by
      simp [addRole, basket_literal arguments assets own, merged, bind, Except.bind]
    simp only [List.map_cons, List.foldlM_cons, step, bind, Except.bind]
    simpa using ih (pre ++ [(role, assets)]) (List.sublist_of_cons_sublist sub) (by simpa using sorted)

theorem mapM_literal {α β : Type} (embed : α → β) (instantiate : β → Except Failure α)
    (back : ∀ value, instantiate (embed value) = .ok value) (values : List α) :
    (values.map embed).mapM instantiate = .ok values := by
  induction values with
  | nil => rfl
  | cons value rest ih =>
    simp [List.mapM_cons, back, ih, bind, Except.bind, pure, Except.pure]

/-- A rule written as a rate with no variables. -/
def Schema.ofRule (rule : Transaction.Rule Role Written) : Schema Role :=
  let entries := fun (entries : List (Role × Transaction.Balances Written)) =>
    entries.map fun (role, assets) => (role, assets.map fun (asset, amount) => (literal asset, amount))
  { variables := []
    roles := rule.roles
    distinct := rule.distinct
    consume := entries rule.consume
    produce := entries rule.produce
    preserve := entries rule.preserve
    computedConsume := rule.computedConsume.map fun amount =>
      ⟨amount.role, literal amount.asset, .ofExpr amount.quantity⟩
    computedProduce := rule.computedProduce.map fun amount =>
      ⟨amount.role, literal amount.asset, .ofExpr amount.quantity⟩
    computedPreserve := rule.computedPreserve.map fun amount =>
      ⟨amount.role, literal amount.asset, .ofExpr amount.quantity⟩
    conditions := rule.conditions.map fun check =>
      ⟨check.label, .ofExpr check.left, check.comparison, .ofExpr check.right⟩
    opens := rule.opens
    closes := rule.closes }

/-- A rule as a rate's builder keeps it. -/
structure Canonical (rule : Transaction.Rule Role Written) : Prop where
  consume : Built cmpRole rule.consume
  produce : Built cmpRole rule.produce
  preserve : Built cmpRole rule.preserve

/-- A rate with no variables is the rate written by hand: its instance, for
any values, is the rule itself. -/
theorem literal_agrees (laws : Ordered.OrderLaws cmpRole) (rule : Transaction.Rule Role Written)
    (canonical : Canonical cmpRole rule) (arguments : Arguments) :
    (Schema.ofRule rule).instantiate cmpRole arguments = .ok rule := by
  have amount := fun (computed : Transaction.Computed Role Written) =>
    show Amount.instantiate arguments ⟨computed.role, literal computed.asset, .ofExpr computed.quantity⟩ =
      .ok computed by
      simp [Amount.instantiate, substitute_literal, Term.instantiate_ofExpr, bind, Except.bind,
        pure, Except.pure]
  have check := fun (condition : Transaction.Condition Role Written) =>
    show Check.instantiate arguments ⟨condition.label, .ofExpr condition.left, condition.comparison,
        .ofExpr condition.right⟩ = .ok condition by
      simp [Check.instantiate, Term.instantiate_ofExpr, bind, Except.bind, pure, Except.pure]
  simp only [Schema.instantiate, Schema.ofRule]
  rw [baskets_literal cmpRole laws arguments _ canonical.consume,
    baskets_literal cmpRole laws arguments _ canonical.produce,
    baskets_literal cmpRole laws arguments _ canonical.preserve,
    mapM_literal _ _ amount, mapM_literal _ _ amount, mapM_literal _ _ amount,
    mapM_literal _ _ check]
  rfl

/-! ## Every instance is canonical

An instance's baskets are kept as a builder keeps a rate's: roles in order,
each holding each asset once, in a positive amount that fits. These are
exactly the rules `literal_agrees` writes back as they are. -/

theorem upsert_mem (asset : Written) (value : Nat) (entries : Transaction.Balances Written) :
    ∀ entry ∈ Account.upsert asset value entries, entry = (asset, value) ∨ entry ∈ entries := by
  induction entries with
  | nil => simp [Account.upsert]
  | cons head rest ih =>
    obtain ⟨key, old⟩ := head
    intro entry member
    by_cases equal : asset = key
    · subst equal
      simp only [Account.upsert, ite_true, List.mem_cons] at member
      rcases member with same | later
      · exact Or.inl same
      · exact Or.inr (List.mem_cons_of_mem _ later)
    · simp only [Account.upsert, equal, ite_false, List.mem_cons] at member
      rcases member with same | later
      · exact Or.inr (by simp [same])
      · rcases ih entry later with new | kept
        · exact Or.inl new
        · exact Or.inr (List.mem_cons_of_mem _ kept)

theorem erase_mem (asset : Written) (entries : Transaction.Balances Written) :
    ∀ entry ∈ Account.erase asset entries, entry ∈ entries := by
  induction entries with
  | nil => simp [Account.erase]
  | cons head rest ih =>
    obtain ⟨key, old⟩ := head
    intro entry member
    by_cases equal : asset = key
    · simp only [Account.erase, equal, ite_true] at member
      exact List.mem_cons_of_mem _ member
    · simp only [Account.erase, equal, ite_false, List.mem_cons] at member
      rcases member with same | later
      · simp [same]
      · exact List.mem_cons_of_mem _ (ih entry later)

/-- Crediting keeps a role's entries as a builder keeps them. -/
theorem credit_entries {total merged : Transaction.Balances Written} (assets : Transaction.Balances Written)
    (valid : Entries total) (credited : Account.credit ops total assets = .ok merged) :
    Entries merged := by
  induction assets generalizing total with
  | nil =>
    simp only [Account.credit, Except.ok.injEq] at credited
    subst credited; exact valid
  | cons entry rest ih =>
    obtain ⟨asset, amount⟩ := entry
    simp only [Account.credit] at credited
    cases sum : ops.add (Account.balance ops.zero total asset) amount with
    | none => simp [sum] at credited
    | some updated =>
      simp only [sum] at credited
      refine ih ?_ credited
      have bound : updated ≤ Quantity.limit := by
        simp only [ops, Account.naturals, Quantity.add, Quantity.checked] at sum
        split at sum <;> simp_all
      refine ⟨(Account.store_self Quantity.limit total asset updated valid.1).2, ?_⟩
      intro stored member
      unfold Account.store at member
      by_cases zero : (Account.naturals Quantity.limit).isZero updated = true
      · simp only [ops, zero, ite_true] at member
        exact valid.2 stored (erase_mem asset total stored member)
      · simp only [ops, zero] at member
        rcases upsert_mem asset updated total stored member with new | kept
        · subst new
          have : updated ≠ 0 := by simpa [Account.naturals] using zero
          exact ⟨Nat.pos_of_ne_zero this, bound⟩
        · exact valid.2 stored kept

theorem addRole_built (laws : Ordered.OrderLaws cmpRole) (arguments : Arguments)
    {target next : List (Role × Transaction.Balances Written)} (entry : Role × List (Pattern × Nat))
    (valid : Built cmpRole target) (added : addRole cmpRole arguments target entry = .ok next) :
    Built cmpRole next := by
  unfold addRole at added
  cases written : basket arguments entry.2 with
  | error => simp [written, bind, Except.bind] at added
  | ok assets =>
    simp only [written, bind, Except.bind] at added
    cases merged : RateConstruction.merge ops cmpRole target entry.1 assets with
    | error => simp [merged] at added
    | ok result =>
      simp only [merged, Except.ok.injEq] at added
      subst added
      obtain ⟨credited, credit, put⟩ := (RateConstruction.merge_success ops cmpRole target entry.1 assets _).mp merged
      subst put
      have old : Entries ((RateConstruction.locate cmpRole entry.1 target).old.getD []) := by
        cases found : (RateConstruction.locate cmpRole entry.1 target).old with
        | none => simp [Entries]
        | some value =>
          have restored := RateConstruction.locate_restore cmpRole entry.1 target
          have member : ((RateConstruction.locate cmpRole entry.1 target).key, value) ∈
              (RateConstruction.locate cmpRole entry.1 target).restore := by
            simp [RateConstruction.Position.restore, found]
          rw [restored] at member
          simpa using valid.2 _ member
      exact ⟨OrderedEffects.put_sorted cmpRole laws entry.1 target credited valid.1,
        TransactionAggregation.put_valid cmpRole laws.equal entry.1 target (fun _ value => Entries value)
          credited (credit_entries assets old credit) valid.2⟩

/-- Each role's entries, once written, are kept as a builder keeps them. -/
theorem baskets_built (laws : Ordered.OrderLaws cmpRole) (arguments : Arguments)
    (entries : List (Role × List (Pattern × Nat))) {result : List (Role × Transaction.Balances Written)}
    (built : baskets cmpRole arguments entries = .ok result) : Built cmpRole result := by
  suffices general : ∀ target, Built cmpRole target → ∀ result,
      entries.foldlM (addRole cmpRole arguments) target = .ok result → Built cmpRole result from
    general [] ⟨by simp [OrderedEffects.Sorted], by simp⟩ result built
  clear built
  induction entries with
  | nil =>
    intro target valid result done
    simp only [List.foldlM_nil, pure, Except.pure, Except.ok.injEq] at done
    subst done; exact valid
  | cons entry rest ih =>
    intro target valid result done
    simp only [List.foldlM_cons, bind, Except.bind] at done
    cases added : addRole cmpRole arguments target entry with
    | error => simp [added] at done
    | ok next =>
      simp only [added] at done
      exact ih next (addRole_built cmpRole laws arguments entry valid added) result done

/-- Every instance of a rate is canonical. -/
theorem instantiate_canonical (laws : Ordered.OrderLaws cmpRole) (schema : Schema Role)
    (arguments : Arguments) {rule : Transaction.Rule Role Written}
    (built : schema.instantiate cmpRole arguments = .ok rule) : Canonical cmpRole rule := by
  unfold Schema.instantiate at built
  cases consume : baskets cmpRole arguments schema.consume with
  | error => simp [consume, bind, Except.bind] at built
  | ok consumed =>
  cases produce : baskets cmpRole arguments schema.produce with
  | error => simp [consume, produce, bind, Except.bind] at built
  | ok produced =>
  cases preserve : baskets cmpRole arguments schema.preserve with
  | error => simp [consume, produce, preserve, bind, Except.bind] at built
  | ok preserved =>
  simp only [consume, produce, preserve, bind, Except.bind] at built
  repeat' split at built
  all_goals first
    | contradiction
    | (simp only [pure, Except.pure, Except.ok.injEq] at built
       subst built
       exact ⟨baskets_built cmpRole laws arguments _ consume,
         baskets_built cmpRole laws arguments _ produce,
         baskets_built cmpRole laws arguments _ preserve⟩)

end Axionomy.Schema
