import Lean
import Axionomy.Parametric

/-!
Fixtures for rates with variables: economies whose rates are written as the
kernel writes a `paths` economy's rates, exchanges naming them in full, and
what the specification makes of each. Identifiers are paths; roles are
numbers.
-/

namespace ParametricConformance
open Lean Axionomy Axionomy.Pattern Axionomy.Schema Axionomy.Parametric

abbrev Role := Nat

def segmentText : Path.Segment → String
  | .number value => toString value
  | .word chars => String.ofList chars

def writtenText (written : Written) : String :=
  "/".intercalate (written.map segmentText)

def patternText (pattern : Pattern) : String :=
  "/".intercalate (pattern.map fun
    | .segment segment => segmentText segment
    | .var name => "{" ++ name ++ "}")

def written (text : String) : Written := Path.ofText text

def pattern (text : String) : Pattern :=
  (text.splitOn "/").map fun segment =>
    if segment.startsWith "{" && segment.endsWith "}" then
      .var ((segment.drop 1).dropEnd 1).toString
    else .segment (Path.segmentOf segment)

def quantity (value : Nat) : Json := toJson (toString value)

def balancesJson (entries : List (Written × Nat)) : Json :=
  toJson (entries.map fun (asset, value) => (writtenText asset, toString value))

def accountsJson (entries : List (Written × List (Written × Nat))) : Json :=
  toJson (entries.map fun (id, entries) => (writtenText id, balancesJson entries))

def tagged (kind : String) (fields : List (String × Json) := []) : Json :=
  Json.mkObj (("kind", toJson kind) :: fields)

def opName : Rates.Op → String
  | .add => "add" | .sub => "subtract" | .mul => "multiply" | .min => "minimum" | .max => "maximum"

def roundingName : MulDiv.Rounding → String
  | .floor => "floor" | .ceil => "ceil"

/-- A quantity as the kernel writes a `paths` economy's. -/
def termJson : Term Role → Json
  | .constant value => tagged "constant" [("value", quantity value)]
  | .units => tagged "units"
  | .parameter name => tagged "parameter" [("name", toJson name)]
  | .var name => tagged "variable" [("name", toJson name)]
  | .balance role asset => tagged "balance" [("role", toJson role), ("asset", toJson (patternText asset))]
  | .binary op left right => tagged (opName op) [("left", termJson left), ("right", termJson right)]
  | .mulDiv rounding left right denominator => tagged "mul_div"
      [("left", termJson left), ("right", termJson right), ("denominator", termJson denominator),
       ("rounding", toJson (roundingName rounding))]

def comparisonName : Evaluation.Comparison → String
  | .equal => "equal" | .greaterOrEqual => "greater_than_or_equal" | .lessOrEqual => "less_than_or_equal"

def domainJson (declared : Variable) : Json :=
  match declared.domain with
  | .range first last => Json.mkObj [("name", toJson declared.name),
      ("range", toJson [toString first, toString last])]
  | .values listed => Json.mkObj [("name", toJson declared.name),
      ("values", toJson (listed.map segmentText))]

def basketsJson (entries : List (Role × List (Pattern × Nat))) : Json :=
  toJson (entries.map fun ((role, assets) : Role × List (Pattern × Nat)) =>
    (role, assets.map fun ((asset, amount) : Pattern × Nat) => (patternText asset, toString amount)))

def amountJson (amount : Amount Role) : Json := Json.mkObj [("role", toJson amount.role),
  ("asset", toJson (patternText amount.asset)), ("quantity", termJson amount.quantity)]

/-- A field the kernel leaves out of a rate it writes where it has nothing. -/
def optional (name : String) (values : List Json) : List (String × Json) :=
  if values.isEmpty then [] else [(name, toJson values)]

/-- A rate as the kernel writes a `paths` economy's: fields with nothing in
them that a rate without variables never had are left out. -/
def schemaJson (schema : Schema Role) : Json := Json.mkObj <|
  optional "variables" (schema.variables.map domainJson) ++
  optional "distinct_variables" (schema.distinctVariables.map toJson) ++
  optional "relations" (schema.relations.map fun relation => Json.mkObj ([
    ("variables", toJson relation.names)] ++
    (if relation.gives.isEmpty then [] else [("gives", toJson relation.gives)]) ++ [
    ("rows", toJson (relation.rows.map fun row => row.map segmentText))])) ++
  optional "derived" (schema.derived.map fun derived => Json.mkObj [
    ("name", toJson derived.name), ("value", termJson derived.value)]) ++
  optional "accounts" (schema.accounts.map fun (role, account) => toJson (role, patternText account)) ++
  optional "opens" (schema.opens.map toJson) ++ optional "closes" (schema.closes.map toJson) ++ [
  ("consume", basketsJson schema.consume), ("produce", basketsJson schema.produce),
  ("preserve", basketsJson schema.preserve), ("distinct", toJson schema.distinct),
  ("computed_consume", toJson (schema.computedConsume.map amountJson)),
  ("computed_produce", toJson (schema.computedProduce.map amountJson)),
  ("computed_preserve", toJson (schema.computedPreserve.map amountJson)),
  ("conditions", toJson (schema.conditions.map fun check => Json.mkObj [
    ("name", toJson check.label), ("left", termJson check.left),
    ("comparison", toJson (comparisonName check.comparison)), ("right", termJson check.right)]))]

def ratesJson (rates : List (Law Role)) : Json :=
  toJson (rates.map fun law => (patternText law.id, schemaJson law.schema))

def proposalJson (proposal : Parametric.Proposal Role) : Json := Json.mkObj [
  ("rate", toJson (writtenText proposal.rate)), ("units", quantity proposal.units),
  ("bindings", toJson (proposal.bindings.map fun (role, account) => (role, writtenText account))),
  ("parameters", toJson (proposal.parameters.map fun (key, value) => (key, toString value)))]

def deltaJson (value : Transaction.Delta Written Written) : Json := Json.mkObj [
  ("account", toJson (writtenText value.account)), ("consumed", balancesJson value.consumed),
  ("produced", balancesJson value.produced), ("preserved", balancesJson value.preserved)]

def shortfallsJson (entries : List (Written × List (Written × Nat))) : Json :=
  toJson (entries.map fun (id, entries) =>
    Json.mkObj [("account", toJson (writtenText id)), ("missing", balancesJson entries)])

def failureJson : Parametric.Failure Role → Json
  | .structural (.missingRate id) => tagged "missing_rate" [("rate", toJson (writtenText id))]
  | .structural (.missingBinding role) => tagged "missing_binding" [("role", toJson role)]
  | .structural (.unknownBinding role) => tagged "unknown_binding" [("role", toJson role)]
  | .structural (.rolesMustDiffer left right) => tagged "roles_must_differ" [("left", toJson left), ("right", toJson right)]
  | .structural (.missingAccount id) => tagged "missing_account" [("account", toJson (writtenText id))]
  | .structural .zeroUnits => tagged "zero_units"
  | .structural (.missingParameter key) => tagged "missing_parameter" [("parameter", toJson key)]
  | .structural (.unknownParameter key) => tagged "unknown_parameter" [("parameter", toJson key)]
  | .structural (.outsideDomain id name) => tagged "outside_domain"
      [("rate", toJson (writtenText id)), ("variable", toJson name)]
  | .structural (.variablesMustDiffer id left right) => tagged "variables_must_differ"
      [("rate", toJson (writtenText id)), ("left", toJson left), ("right", toJson right)]
  | .structural (.wrongAccount role expected) => tagged "wrong_account"
      [("role", toJson role), ("expected", toJson (writtenText expected))]
  | .structural (.outsideRelation id names) => tagged "outside_relation"
      [("rate", toJson (writtenText id)), ("variables", toJson names)]
  | .structural (.derivedFails id name) => tagged "derived_fails"
      [("rate", toJson (writtenText id)), ("variable", toJson name)]
  | .rateOverflow id asset => tagged "rate_overflow" [("rate", toJson (writtenText id)), ("asset", toJson (writtenText asset))]
  | .lawEvaluation id message => tagged "law_evaluation" [("rate", toJson (writtenText id)), ("message", toJson message)]
  | .conditionFailed id label => tagged "condition_failed" [("rate", toJson (writtenText id)), ("label", toJson label)]
  | .infeasible missing => tagged "infeasible" [("shortfalls", shortfallsJson missing)]
  | .balanceOverflow id asset => tagged "balance_overflow" [("account", toJson (writtenText id)), ("asset", toJson (writtenText asset))]
  | .invariantOverflow label => tagged "invariant_overflow" [("label", toJson label)]
  | .invariantViolation label before after => tagged "invariant_violation"
      [("label", toJson label), ("before", toJson (toString before)), ("after", toJson (toString after))]
  | .accountExists id => tagged "account_exists" [("account", toJson (writtenText id))]
  | .openingShared role => tagged "opening_shared" [("role", toJson role)]
  | .closedNotEmpty id remaining => tagged "closed_not_empty" [("account", toJson (writtenText id)), ("remaining", balancesJson remaining)]

def accountAssessmentJson (value : Transaction.AccountAssessment Written Written) : Json := Json.mkObj [
  ("account", toJson (writtenText value.account)), ("available", balancesJson value.available),
  ("required", balancesJson value.required), ("consumed", balancesJson value.consumed),
  ("produced", balancesJson value.produced), ("preserved", balancesJson value.preserved)]

def assessmentJson : Transaction.Assessment Written Written Written Role → Json
  | .applicable entries deltas => tagged "applicable"
      [("accounts", toJson (entries.map accountAssessmentJson)), ("deltas", toJson (deltas.map deltaJson))]
  | .infeasible entries missing => tagged "infeasible"
      [("accounts", toJson (entries.map accountAssessmentJson)), ("shortfalls", shortfallsJson missing)]
  | .invalid issues => tagged "invalid" [("issues", toJson (issues.map failureJson))]

def receiptJson (value : Lifecycle.Receipt Written Written Written Role) : Json := Json.mkObj [
  ("proposal", proposalJson value.proposal), ("deltas", toJson (value.deltas.map deltaJson)),
  ("opened", toJson (value.opened.map writtenText)), ("closed", toJson (value.closed.map writtenText))]

/-- Every role a schema uses, registered as a rate's builder registers them. -/
def Term.roles : Term Role → List Role
  | .balance role _ => [role]
  | .binary _ left right => Term.roles left ++ Term.roles right
  | .mulDiv _ left right denominator => Term.roles left ++ Term.roles right ++ Term.roles denominator
  | _ => []

def normalize (schema : Schema Role) : Schema Role :=
  let amounts := schema.computedConsume ++ schema.computedProduce ++ schema.computedPreserve
  let roles := schema.roles ++ (schema.consume ++ schema.produce ++ schema.preserve).map Prod.fst ++
    amounts.flatMap (fun amount => amount.role :: Term.roles amount.quantity) ++
    schema.conditions.flatMap (fun check => Term.roles check.left ++ Term.roles check.right) ++
    schema.distinct.flatMap (fun (left, right) => [left, right]) ++ schema.opens ++ schema.closes ++
    schema.accounts.map Prod.fst
  { schema with roles := Discovery.registerAll compare roles [] }

def law (id : String) (schema : Schema Role) : Law Role := ⟨pattern id, normalize schema⟩

def blank : Schema Role := ⟨[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []⟩

def relation (names : List String) (rows : List (List String)) (gives : List String := []) :
    Relation :=
  ⟨names, gives, rows.map fun row => row.map Path.segmentOf⟩

def range (name : String) (first last : Nat) : Variable := ⟨name, .range first last⟩
def values (name : String) (listed : List String) : Variable := ⟨name, .values (listed.map Path.segmentOf)⟩

def entries (listed : List (String × Nat)) : List (Pattern × Nat) :=
  listed.map fun (asset, amount) => (pattern asset, amount)

def account (id : String) (holdings : List (String × Nat)) : Written × List (Written × Nat) :=
  (written id, holdings.map fun (asset, amount) => (written asset, amount))

def proposal (rate : String) (bindings : List (Role × String)) (units : Nat := 1) : Parametric.Proposal Role :=
  ⟨written rate, bindings.map (fun (role, account) => (role, written account)), units, []⟩

/-- An economy's constants as the kernel writes a `paths` economy's: each
name with its value, in order. -/
def constantsJson (constants : Arguments) : Json :=
  toJson (constants.map fun (name, value) => (name, segmentText value))

def constant (name value : String) : String × Path.Segment := (name, Path.segmentOf value)

def scenario (label : String) (accounts : List (Written × List (Written × Nat))) (constants : Arguments)
    (rates : List (Law Role)) (proposal : Parametric.Proposal Role) : Json :=
  -- Built as the kernel builds an economy: its accounts in their order.
  let model : Parametric.Model Role :=
    { accounts := Ordered.ordered Path.compare accounts, rates, invariants := [], constants }
  let expected := match Parametric.analyze compare model proposal, Parametric.apply compare model proposal with
    | some analyzed, some (applied, after) => Json.mkObj [
        ("assessment", assessmentJson analyzed.assessment), ("after", accountsJson after.accounts),
        ("error", match applied with | .ok _ => Json.null | .error error => failureJson error),
        ("receipt", match applied with | .error _ => Json.null | .ok receipt => receiptJson receipt)]
    | _, _ => tagged "panic"
  Json.mkObj [("label", toJson label), ("accounts", accountsJson accounts),
    ("constants", constantsJson constants), ("rates", ratesJson rates),
    ("proposal", proposalJson proposal), ("expected", expected)]

-- A responder stabilizes a patient where both are: the patient is fixed by
-- the rate's identifier, the place is a value of the identifier too.
def stabilize : Law Role := law "stabilize/{patient}/{location}" { blank with
  variables := [range "patient" 1 3, values "location" ["east", "north"]]
  accounts := [(1, pattern "patient/{patient}")]
  consume := [(0, entries [("kit", 1)])]
  preserve := [(0, entries [("at/{location}", 1)]), (1, entries [("at/{location}", 1)])]
  produce := [(1, entries [("stabilized", 1)])]
  conditions := [⟨"alive", .balance 1 (pattern "vitality"), .greaterOrEqual, .constant 1⟩] }

-- Moving between two different places.
def drive : Law Role := law "drive/{from}/{to}" { blank with
  variables := [values "from" ["east", "north", "south"], values "to" ["east", "north", "south"]]
  distinctVariables := [("from", "to")]
  consume := [(0, entries [("at/{from}", 1)])]
  produce := [(0, entries [("at/{to}", 1)])] }

-- Two entries that name one asset once written add up.
def pile : Law Role := law "pile/{left}/{right}" { blank with
  variables := [values "left" ["a", "b"], values "right" ["a", "b"]]
  produce := [(0, entries [("token/{left}", 2), ("token/{right}", 3)])] }

def brim : Law Role := law "brim/{left}/{right}" { blank with
  variables := [values "left" ["a", "b"], values "right" ["a", "b"]]
  produce := [(0, entries [("token/{left}", Quantity.limit), ("token/{right}", 1)])] }

-- A piece drops into a column at the row its height says: the row is a
-- value of the identifier, checked against the column's height.
def drop : Law Role := law "drop/{column}/{row}" { blank with
  variables := [range "column" 0 1, range "row" 0 2]
  accounts := [(0, pattern "column/{column}"), (1, pattern "cell/{column}/{row}")]
  conditions := [⟨"lands", .balance 0 (pattern "height"), .equal, .var "row"⟩]
  produce := [(0, entries [("height", 1)]), (1, entries [("piece", 1)])] }

-- Admitting a patient opens the account the identifier names.
def admit : Law Role := law "admit/{patient}" { blank with
  variables := [range "patient" 1 9]
  accounts := [(1, pattern "patient/{patient}")]
  opens := [1]
  consume := [(0, entries [("bed", 1)])]
  produce := [(1, entries [("admitted", 1)])] }

-- A step between cells that adjoin: which do is law structure, a relation
-- over the identifier's variables.
def step : Law Role := law "step/{from}/{to}" { blank with
  variables := [values "from" ["a", "b", "c"], values "to" ["a", "b", "c"]]
  relations := [relation ["from", "to"] [["a", "b"], ["b", "a"], ["b", "c"], ["c", "b"]]]
  consume := [(0, entries [("at/{from}", 1)])]
  produce := [(0, entries [("at/{to}", 1)])] }

-- Hauling between depots a road joins: which roads there are and how long
-- each is are law structure, a relation whose rows give the distance; the
-- fuel burnt is derived from it.
def haul : Law Role := law "haul/{from}/{to}" { blank with
  variables := [values "from" ["a", "b", "c"], values "to" ["a", "b", "c"]]
  relations := [relation ["from", "to"] [["a", "b", "4"], ["b", "c", "7"]] ["distance"]]
  derived := [⟨"fuel", .binary .mul (.var "distance") (.constant 2)⟩]
  consume := [(0, entries [("at/{from}", 1)])]
  produce := [(0, entries [("at/{to}", 1), ("leg/{distance}", 1)])]
  computedConsume := [⟨0, pattern "fuel", .var "fuel"⟩] }

-- A toll without variables: its one row gives the fee every crossing pays.
def toll : Law Role := law "toll" { blank with
  relations := [relation [] [["3"]] ["fee"]]
  produce := [(0, entries [("paid/{fee}", 1)])]
  computedConsume := [⟨0, pattern "kit", .var "fee"⟩] }

-- Delivering a parcel to where it goes: each parcel's destination, a word,
-- and its weight, a number, are law structure, a relation's row; the
-- courier must be at the destination and burns the weight in fuel.
def deliver : Law Role := law "deliver/{parcel}" { blank with
  variables := [range "parcel" 1 3]
  relations := [relation ["parcel"] [["1", "east", "4"], ["2", "north", "2"]] ["destination", "weight"]]
  preserve := [(0, entries [("at/{destination}", 1)])]
  produce := [(0, entries [("delivered/{parcel}", 1)])]
  computedConsume := [⟨0, pattern "fuel", .var "weight"⟩] }

-- A day of the season: how long the season is and where every day is
-- spent are the economy's constants, read as variables are.
def season : Law Role := law "season/{day}" { blank with
  variables := [range "day" 1 9]
  conditions := [⟨"in-season", .var "day", .lessOrEqual, .var "season-length"⟩]
  preserve := [(0, entries [("at/{home}", 1)])]
  produce := [(0, entries [("day/{day}", 1)])] }

-- Climbing from a row to the next: the next row is derived from the row,
-- and so is the cost.
def climb : Law Role := law "climb/{row}" { blank with
  variables := [range "row" 0 3]
  derived := [⟨"next", .binary .add (.var "row") (.constant 1)⟩,
    ⟨"cost", .binary .mul (.var "next") (.constant 2)⟩]
  consume := [(0, entries [("row/{row}", 1)])]
  produce := [(0, entries [("row/{next}", 1)])]
  computedConsume := [⟨0, pattern "kit", .var "cost"⟩] }

-- Descending to the row before: none is before the first.
def descend : Law Role := law "descend/{row}" { blank with
  variables := [range "row" 0 3]
  derived := [⟨"previous", .binary .sub (.var "row") (.constant 1)⟩]
  consume := [(0, entries [("row/{row}", 1)])]
  produce := [(0, entries [("row/{previous}", 1)])] }

-- A rate with no variables beside them.
def rest : Law Role := law "rest" { blank with
  consume := [(0, entries [("kit", 1)])]
  produce := [(0, entries [("calm", 1)])] }

def world : List (Written × List (Written × Nat)) := [
  account "responder" [("at/east", 1), ("kit", 2), ("bed", 1)],
  account "patient/1" [("at/east", 1), ("vitality", 3)],
  account "patient/2" [("at/north", 1), ("vitality", 1)],
  account "patient/10" [("at/east", 1)],
  account "column/0" [("height", 1)],
  account "cell/0/1" [],
  account "cell/0/0" [("piece", 1)],
  account "climber" [("row/0", 1), ("row/1", 1), ("kit", 6), ("at/a", 1)],
  account "hauler" [("at/a", 1), ("at/b", 1), ("fuel", 10)],
  account "courier" [("at/east", 1), ("fuel", 5)]]

def constants : Arguments := [constant "season-length" "3", constant "home" "east"]

def fixtures : Array Json := Id.run do
  let rates := [stabilize, drive, pile, brim, drop, admit, step, haul, toll, deliver, season, climb,
    descend, rest]
  let mut cases := #[]
  for (label, proposal) in [
      ("instance", proposal "stabilize/1/east" [(0, "responder"), (1, "patient/1")]),
      ("outside-a-range", proposal "stabilize/4/east" [(0, "responder"), (1, "patient/1")]),
      ("outside-values", proposal "stabilize/1/west" [(0, "responder"), (1, "patient/1")]),
      ("zero-units-first", proposal "stabilize/1/west" [(0, "responder"), (1, "patient/1")] 0),
      ("wrong-account", proposal "stabilize/1/east" [(0, "responder"), (1, "patient/2")]),
      ("fixed-unbound", proposal "stabilize/1/east" [(0, "responder")]),
      ("instance-infeasible", proposal "stabilize/2/north" [(0, "responder"), (1, "patient/2")]),
      ("no-rate-matches", proposal "stabilize/1" [(0, "responder")]),
      ("must-differ", proposal "drive/east/east" [(0, "responder")]),
      ("differ", proposal "drive/east/north" [(0, "responder")]),
      ("entries-add-up", proposal "pile/a/a" [(0, "responder")]),
      ("entries-apart", proposal "pile/a/b" [(0, "responder")]),
      ("entries-overflow", proposal "brim/b/b" [(0, "responder")]),
      ("variable-in-condition", proposal "drop/0/1" [(0, "column/0"), (1, "cell/0/1")]),
      ("variable-condition-fails", proposal "drop/0/0" [(0, "column/0"), (1, "cell/0/0")]),
      ("opens-a-written-account", proposal "admit/3" [(0, "responder"), (1, "patient/3")]),
      ("opens-an-existing-account", proposal "admit/2" [(0, "responder"), (1, "patient/2")]),
      ("numbers-order-naturally", proposal "admit/9" [(0, "responder"), (1, "patient/9")]),
      ("related", proposal "step/a/b" [(0, "climber")]),
      ("outside-a-relation", proposal "step/a/c" [(0, "responder")]),
      ("relation-before-differ", proposal "step/a/a" [(0, "responder")]),
      ("given", proposal "haul/a/b" [(0, "hauler")]),
      ("given-amount-infeasible", proposal "haul/b/c" [(0, "hauler")]),
      ("given-outside-a-relation", proposal "haul/a/c" [(0, "hauler")]),
      ("given-without-variables", proposal "toll" [(0, "climber")]),
      ("given-without-variables-infeasible", proposal "toll" [(0, "responder")]),
      ("given-a-word", proposal "deliver/1" [(0, "courier")]),
      ("given-a-word-elsewhere", proposal "deliver/2" [(0, "courier")]),
      ("given-a-word-outside-a-relation", proposal "deliver/3" [(0, "courier")]),
      ("constant", proposal "season/2" [(0, "responder")]),
      ("constant-in-a-condition", proposal "season/4" [(0, "responder")]),
      ("constant-in-a-pattern", proposal "season/1" [(0, "hauler")]),
      ("derived", proposal "climb/1" [(0, "climber")]),
      ("derived-amount-infeasible", proposal "climb/0" [(0, "climber")] 2),
      ("derived-fails", proposal "descend/0" [(0, "climber")]),
      ("derived-after-domain", proposal "descend/4" [(0, "climber")]),
      ("derived-zero-units-first", proposal "descend/0" [(0, "climber")] 0),
      ("no-variables", proposal "rest" [(0, "responder")])] do
    cases := cases.push (scenario label world constants rates proposal)
  return cases

def issueJson : ModelIssue → Json
  | .repeatedVariable rate name => tagged "repeated_variable" [("rate", toJson (patternText rate)), ("variable", toJson name)]
  | .undeclaredVariable rate name => tagged "undeclared_variable" [("rate", toJson (patternText rate)), ("variable", toJson name)]
  | .unreadVariable rate name => tagged "unread_variable" [("rate", toJson (patternText rate)), ("variable", toJson name)]
  | .notNatural rate name => tagged "not_natural" [("rate", toJson (patternText rate)), ("variable", toJson name)]
  | .repeatedValue rate name => tagged "repeated_value" [("rate", toJson (patternText rate)), ("variable", toJson name)]
  | .overlapping rate other => tagged "overlapping_rates" [("rate", toJson (patternText rate)), ("other", toJson (patternText other))]
  | .malformedRelation rate names => tagged "malformed_relation" [("rate", toJson (patternText rate)), ("variables", toJson names)]
  | .repeatedRow rate names => tagged "repeated_row" [("rate", toJson (patternText rate)), ("variables", toJson names)]
  | .derivedReads rate name => tagged "derived_reads" [("rate", toJson (patternText rate)), ("variable", toJson name)]
  | .repeatedConstant name => tagged "repeated_constant" [("constant", toJson name)]
  | .malformedConstant name => tagged "malformed_constant" [("constant", toJson name)]

/-- A model's constants and rates, in the order the kernel keeps them, and
what is wrong with them. -/
def model (label : String) (rates : List (Law Role)) (constants : Arguments := []) : Json :=
  let ordered := (Ordered.ordered Path.compareText (rates.map fun law => (patternText law.id, law))).map Prod.snd
  let kept := Ordered.ordered Path.compareText constants
  Json.mkObj [("label", toJson label), ("constants", constantsJson constants), ("rates", ratesJson rates),
    ("issues", toJson ((check kept ordered).map issueJson))]

def models : Array Json := #[
  model "clean" [stabilize, drive, pile, drop, admit, step, haul, toll, deliver, season, climb, descend,
    rest] constants,
  model "relation-names-an-undeclared-variable" [law "step/{from}" { blank with
    variables := [values "from" ["a"]], relations := [relation ["from", "to"] [["a", "b"]]] }],
  model "malformed-relation" [law "step/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from", "to"] [["a", "b"], ["a"]]] }],
  model "repeated-row" [law "step/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from", "to"] [["a", "b"], ["a", "b"]]] }],
  model "repeated-key" [law "haul/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from", "to"] [["a", "b", "4"], ["a", "b", "5"]] ["distance"]] }],
  model "gives-a-word-read-as-a-number" [law "haul/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from", "to"] [["a", "b", "far"]] ["distance"]],
    computedConsume := [⟨0, pattern "fuel", .var "distance"⟩] }],
  model "gives-words-and-numbers-read-as-a-number" [law "haul/{from}" { blank with
    variables := [values "from" ["a", "b"]],
    relations := [relation ["from"] [["a", "4"], ["b", "far"]] ["distance"]],
    produce := [(0, entries [("leg/{distance}", 1)])],
    conditions := [⟨"short", .var "distance", .lessOrEqual, .constant 9⟩] }],
  model "gives-too-much" [law "haul/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from", "to"] [["a", "b", toString (Quantity.limit + 1)]] ["distance"]] }],
  model "gives-short" [law "haul/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from", "to"] [["a", "b"]] ["distance"]] }],
  model "gives-a-variable" [law "haul/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from"] [["a", "4"]] ["to"]] }],
  model "gives-twice" [law "haul/{from}/{to}" { blank with
    variables := [values "from" ["a"], values "to" ["b"]],
    relations := [relation ["from"] [["a", "4"]] ["distance"], relation ["to"] [["b", "5"]] ["distance"]],
    derived := [⟨"distance", .constant 1⟩] }],
  model "given-in-the-identifier" [law "haul/{from}/{distance}" { blank with
    variables := [values "from" ["a"]],
    relations := [relation ["from"] [["a", "4"]] ["distance"]] }],
  model "given-read-by-a-derived-value" [law "haul/{from}" { blank with
    variables := [values "from" ["a"]],
    relations := [relation ["from"] [["a", "4"]] ["distance"]],
    derived := [⟨"fuel", .binary .mul (.var "distance") (.constant 2)⟩],
    computedConsume := [⟨0, pattern "fuel", .var "fuel"⟩] }],
  model "derived-reads-an-exchange" [law "climb/{row}" { blank with
    variables := [range "row" 0 1], derived := [⟨"next", .binary .add (.var "row") .units⟩] }],
  model "derived-repeats-a-variable" [law "climb/{row}" { blank with
    variables := [range "row" 0 1], derived := [⟨"row", .constant 1⟩] }],
  model "derived-reads-a-later-value" [law "climb/{row}" { blank with
    variables := [range "row" 0 1],
    derived := [⟨"next", .var "after"⟩, ⟨"after", .binary .add (.var "row") (.constant 2)⟩] }],
  model "derived-in-the-identifier" [law "climb/{row}/{next}" { blank with
    variables := [range "row" 0 1], derived := [⟨"next", .binary .add (.var "row") (.constant 1)⟩] }],
  model "derived-from-listed-values" [law "go/{to}" { blank with
    variables := [values "to" ["x"]], derived := [⟨"next", .binary .add (.var "to") (.constant 1)⟩] }],
  model "repeated-variable" [law "swap/{a}/{a}" { blank with variables := [values "a" ["x", "y"]] }],
  model "repeated-declaration" [law "move/{x}" { blank with variables := [range "x" 1 2, range "x" 1 2] }],
  model "undeclared-variable" [law "move/{to}" { blank with
    variables := [values "to" ["x"]], produce := [(0, entries [("at/{from}", 1)])] }],
  model "unread-variable" [law "move/{to}" { blank with variables := [values "to" ["x"], values "from" ["y"]] }],
  model "not-natural" [law "fill/{place}" { blank with
    variables := [values "place" ["x"]], conditions := [⟨"full", .var "place", .equal, .constant 1⟩] }],
  model "repeated-value" [law "go/{to}" { blank with variables := [values "to" ["x", "x"]] }],
  model "overlapping" [law "advance/{to}" { blank with variables := [range "to" 1 3] }, law "advance/final" blank],
  model "apart-by-a-written-segment" [law "move/{to}/fast" { blank with variables := [values "to" ["x"]] },
    law "move/{to}/slow" { blank with variables := [values "to" ["x"]] }],
  model "constant-named-as-a-variable" [law "season/{home}" { blank with
    variables := [values "home" ["east"]] }] constants,
  model "constant-named-as-a-derived-value" [law "climb/{row}" { blank with
    variables := [range "row" 0 1], derived := [⟨"season-length", .constant 2⟩] }] constants,
  model "constant-in-the-identifier" [law "go/{home}" blank] constants,
  model "word-constant-read-as-a-number" [law "count" { blank with
    computedConsume := [⟨0, pattern "kit", .var "home"⟩] }] constants,
  model "constant-read-by-a-derived-value" [law "climb/{row}" { blank with
    variables := [range "row" 0 1],
    derived := [⟨"last", .binary .add (.var "row") (.var "season-length")⟩],
    produce := [(0, entries [("row/{last}", 1)])] }] constants,
  model "repeated-constant" [rest] [constant "fee" "1", constant "fee" "2"],
  model "constant-too-large" [rest] [constant "fee" (toString (Quantity.limit + 1)), constant "home" "east"],
  model "apart-by-length" [law "move/{to}" { blank with variables := [values "to" ["x"]] },
    law "move/{to}/{how}" { blank with variables := [values "to" ["x"], values "how" ["y"]] }]]

end ParametricConformance
