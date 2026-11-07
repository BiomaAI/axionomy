import Lean
import Axionomy.Transaction
import Axionomy.Basket

namespace TransactionConformance
open Lean Axionomy

abbrev Rule := Transaction.Rule Nat Nat
abbrev Model := Transaction.Model Nat Nat Nat Nat
abbrev Proposal := Transaction.Proposal Nat Nat Nat

def balances (entries : List (Nat × Nat)) : Json :=
  toJson (entries.map fun (asset, value) => (asset, toString value))

def accounts (entries : List (Nat × List (Nat × Nat))) : Json :=
  toJson (entries.map fun (id, entries) => (id, balances entries))

def proposalJson (proposal : Proposal) : Json := Json.mkObj [
  ("rate", toJson proposal.rate), ("units", toJson (toString proposal.units)),
  ("bindings", toJson proposal.bindings),
  ("parameters", toJson (proposal.parameters.map fun (key, value) => (key, toString value)))]

def delta (value : Transaction.Delta Nat Nat) : Json := Json.mkObj [
  ("account", toJson value.account), ("consumed", balances value.consumed),
  ("produced", balances value.produced), ("preserved", balances value.preserved)]

def shortfalls (entries : List (Nat × List (Nat × Nat))) : Json :=
  toJson (entries.map fun (id, entries) => Json.mkObj [("account", toJson id), ("missing", balances entries)])

def tagged (kind : String) (fields : List (String × Json) := []) : Json := Json.mkObj (("kind", toJson kind) :: fields)

def failure : Transaction.Failure Nat Nat Nat Nat → Json
  | .structural (.missingRate id) => tagged "missing_rate" [("rate", toJson id)]
  | .structural (.missingBinding role) => tagged "missing_binding" [("role", toJson role)]
  | .structural (.unknownBinding role) => tagged "unknown_binding" [("role", toJson role)]
  | .structural (.rolesMustDiffer left right) => tagged "roles_must_differ" [("left", toJson left), ("right", toJson right)]
  | .structural (.missingAccount id) => tagged "missing_account" [("account", toJson id)]
  | .structural .zeroUnits => tagged "zero_units"
  | .structural (.missingParameter key) => tagged "missing_parameter" [("parameter", toJson key)]
  | .structural (.unknownParameter key) => tagged "unknown_parameter" [("parameter", toJson key)]
  | .rateOverflow id asset => tagged "rate_overflow" [("rate", toJson id), ("asset", toJson asset)]
  | .lawEvaluation id message => tagged "law_evaluation" [("rate", toJson id), ("message", toJson message)]
  | .conditionFailed id label => tagged "condition_failed" [("rate", toJson id), ("label", toJson label)]
  | .infeasible missing => tagged "infeasible" [("shortfalls", shortfalls missing)]
  | .balanceOverflow id asset => tagged "balance_overflow" [("account", toJson id), ("asset", toJson asset)]
  | .invariantOverflow label => tagged "invariant_overflow" [("label", toJson label)]
  | .invariantViolation label before after => tagged "invariant_violation"
      [("label", toJson label), ("before", toJson (toString before)), ("after", toJson (toString after))]

def accountAssessment (value : Transaction.AccountAssessment Nat Nat) : Json := Json.mkObj [
  ("account", toJson value.account), ("available", balances value.available), ("required", balances value.required),
  ("consumed", balances value.consumed), ("produced", balances value.produced), ("preserved", balances value.preserved)]

def assessment : Transaction.Assessment Nat Nat Nat Nat → Json
  | .applicable entries deltas => tagged "applicable" [("accounts", toJson (entries.map accountAssessment)), ("deltas", toJson (deltas.map delta))]
  | .infeasible entries missing => tagged "infeasible" [("accounts", toJson (entries.map accountAssessment)), ("shortfalls", shortfalls missing)]
  | .invalid issues => tagged "invalid" [("issues", toJson (issues.map failure))]

def normalize (rule : Rule) : Rule :=
  let amounts := rule.computedConsume ++ rule.computedProduce ++ rule.computedPreserve
  let roles := rule.roles ++ (rule.consume ++ rule.produce ++ rule.preserve).map Prod.fst ++
    amounts.flatMap (fun amount => amount.role :: Discovery.roles amount.quantity) ++
    rule.conditions.flatMap (fun condition => Discovery.roles condition.left ++ Discovery.roles condition.right) ++
    rule.distinct.flatMap (fun (left, right) => [left, right])
  { rule with roles := Discovery.registerAll compare roles []
              distinct := Discovery.registerAll (Discovery.pairCompare compare) (rule.distinct.map (fun (left, right) => Discovery.orderedPair compare left right)) [] }

def ruleJson (expression : Evaluation.Expr Nat Nat Nat → Json) (rule : Rule) : Json :=
  let amount := fun value : Transaction.Computed Nat Nat => Json.mkObj [
    ("role", toJson value.role), ("asset", toJson value.asset), ("quantity", expression value.quantity)]
  let condition := fun value : Transaction.Condition Nat Nat => Json.mkObj [
    ("label", toJson value.label), ("left", expression value.left), ("right", expression value.right),
    ("comparison", toJson (match value.comparison with | .equal => "equal" | .greaterOrEqual => "greater_or_equal" | .lessOrEqual => "less_or_equal"))]
  Json.mkObj [("consume", accounts rule.consume), ("produce", accounts rule.produce), ("preserve", accounts rule.preserve),
    ("computedConsume", toJson (rule.computedConsume.map amount)), ("computedProduce", toJson (rule.computedProduce.map amount)),
    ("computedPreserve", toJson (rule.computedPreserve.map amount)), ("conditions", toJson (rule.conditions.map condition)),
    ("distinct", toJson rule.distinct)]

def scenario (expression : Evaluation.Expr Nat Nat Nat → Json) (label : String) (before : List (Nat × List (Nat × Nat)))
    (rawRule : Rule) (proposal : Proposal) (invariants : List (Transaction.Invariant Nat) := []) : Json :=
  let rule := normalize rawRule
  let before := before.map fun (id, entries) => (id, Basket.canonical entries)
  let model : Model := ⟨before, [(7, rule)], invariants⟩
  let expected := match Transaction.analyze compare compare model proposal, Transaction.apply compare compare model proposal with
    | some analyzed, some (applied, after) => Json.mkObj [
        ("assessment", assessment analyzed.assessment), ("after", accounts after.accounts),
        ("error", match applied with | .ok _ => Json.null | .error error => failure error),
        ("receipt", match applied with | .error _ => Json.null | .ok receipt => Json.mkObj [
          ("proposal", proposalJson receipt.proposal), ("deltas", toJson (receipt.deltas.map delta))])]
    | _, _ => tagged "panic"
  Json.mkObj [("label", toJson label), ("accounts", accounts before), ("rule", ruleJson expression rule), ("proposal", proposalJson proposal),
    ("invariants", toJson (invariants.map fun inv => Json.mkObj [("label", toJson inv.label),
      ("weights", toJson (inv.weights.map fun (asset, value) => (asset, toString value)))])), ("expected", expected)]

def blank : Rule := ⟨[], [], [], [], [], [], [], [], []⟩

def fixtures (expression : Evaluation.Expr Nat Nat Nat → Json) : Array Json := Id.run do
  let limit := Quantity.u64Max
  let proposal : Proposal := ⟨7, [(0, 1), (1, 0)], 1, []⟩
  let before := [(9, [(7, 11)]), (1, [(0, 5)]), (0, [(0, 1)])]
  let transfer : Rule := { blank with consume := [(0, [(0, 2)])], produce := [(1, [(0, 2)])] }
  let mut cases := #[]
  for source in [0, 1, 5, limit] do
    for target in [0, 1, limit] do
      for units in [0, 1, 2, limit] do
        for alias in [false, true] do
          for reserve in [0, 1, 3] do
            for output in [0, 2] do
              cases := cases.push (scenario expression "static-grid" [(9, [(7, 11)]), (1, [(0, source)]), (0, [(0, target)])]
                { transfer with produce := [(1, Basket.canonical [(0, output)])], preserve := [(0, Basket.canonical [(0, reserve)])] }
                { proposal with units := units, bindings := [(0, 1), (1, if alias then 1 else 0)] })
  let add := fun label accounts rule action invariants => scenario expression label accounts rule action invariants
  cases := cases.push (add "empty-rate" before blank { proposal with bindings := [] } [])
  cases := cases.push (add "snapshot-computed" before { transfer with computedProduce := [⟨1, 1, .balance 0 0⟩] } proposal [])
  cases := cases.push (add "computed-unscaled" before { blank with computedConsume := [⟨0, 0, .constant 2⟩], computedProduce := [⟨1, 0, .constant 2⟩], computedPreserve := [⟨0, 0, .constant 1⟩] } { proposal with units := 3 } [])
  cases := cases.push (add "aliases-all-channels" before { transfer with consume := [(0, [(0, 1)]), (1, [(0, 1)])], preserve := [(0, [(0, 1)]), (1, [(0, 1)])], computedProduce := [⟨0, 0, .constant 1⟩, ⟨1, 0, .constant 1⟩] }
    { proposal with bindings := [(0, 1), (1, 1)] } [])
  let divZero : Evaluation.Expr Nat Nat Nat := .binary .div (.constant 1) (.constant 0)
  cases := cases.push (add "condition-first-false" before { transfer with conditions := [⟨"first", .constant 1, .equal, .constant 2⟩,
    ⟨"later", divZero, .equal, .constant 0⟩] } proposal [])
  cases := cases.push (add "condition-first-error" before { transfer with conditions := [⟨"bad", divZero, .equal, .constant 0⟩,
    ⟨"later", .constant 1, .equal, .constant 2⟩] } proposal [])
  cases := cases.push (add "computed-error-order" before { transfer with computedConsume := [⟨0, 0, divZero⟩], computedProduce := [⟨1, 0, .binary .sub (.constant 0) (.constant 1)⟩] } proposal [])
  cases := cases.push (add "computed-merge-overflow" before { transfer with computedConsume := [⟨0, 0, .constant limit⟩] } proposal [])
  cases := cases.push (add "preserve-preflight-overflow" before { transfer with preserve := [(0, [(0, limit)])] } proposal [])
  cases := cases.push (add "complete-shortfalls" before { blank with consume := [(0, [(0, 9), (1, 3)]), (1, [(0, 9), (2, 4)])] } proposal [])
  cases := cases.push (add "late-deposit-rollback" [(0, []), (1, [(0, limit)]), (9, [(7, 11)])]
    { blank with produce := [(0, [(0, 1)]), (1, [(0, 1)])] } { proposal with bindings := [(0, 0), (1, 1)] } [])
  cases := cases.push (add "conserved-transfer" before transfer proposal [⟨"mass", [(0, 1)]⟩])
  cases := cases.push (add "invariant-first-violation" before { transfer with produce := [(1, [(0, 1)])] } proposal
    [⟨"first", [(0, -2)]⟩, ⟨"later", [(0, 1)]⟩])
  let hugeWeights := [(0, (2^63 - 1 : Int)), (1, 2^63 - 1), (2, 2^63 - 1)]
  let hugeBasket := [(0, limit), (1, limit), (2, limit)]
  cases := cases.push (add "conservation-intermediate-overflow" [(0, hugeBasket)]
    { blank with consume := [(0, hugeBasket)], produce := [(0, hugeBasket)] } { proposal with bindings := [(0, 0)] }
    [⟨"huge", hugeWeights⟩])
  cases := cases.push (add "before-measure-overflow" [(0, hugeBasket)]
    { blank with consume := [(0, [(0, 1)])] } { proposal with bindings := [(0, 0)] } [⟨"huge", hugeWeights⟩])
  cases := cases.push (add "after-measure-overflow" [(0, [(0, limit), (1, limit)])]
    { blank with produce := [(0, [(2, limit)])] } { proposal with bindings := [(0, 0)] } [⟨"huge", hugeWeights⟩])
  cases := cases.push (add "negative-contribution" before { transfer with produce := [(1, [(0, 1)])] } proposal [⟨"negative", [(0, -(2^63))]⟩])
  cases := cases.push (add "missing-rate-and-units" before transfer { proposal with rate := 99, units := 0 } [])
  cases := cases.push (add "multiple-structural-issues" before { transfer with distinct := [(0, 1)], computedConsume := [⟨0, 0, .parameter "éλ"⟩] }
    { proposal with units := 0, bindings := [(0, 77), (1, 77), (2, 88)], parameters := [("unknown", 1)] } [])
  cases := cases.push (add "parameter-and-comparison" before { transfer with computedPreserve := [⟨0, 0, .parameter "éλ"⟩], conditions := [⟨"lower", .balance 0 0, .greaterOrEqual, .parameter "éλ"⟩, ⟨"upper", .parameter "éλ", .lessOrEqual, .units⟩] }
    { proposal with parameters := [("éλ", 1)] } [])
  return cases

end TransactionConformance
