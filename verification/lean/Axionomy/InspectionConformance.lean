import Lean
import Axionomy.InspectionLookupRefinement
import Axionomy.InspectionModelAssetsRefinement

namespace InspectionConformance
open Lean Axionomy Aeneas Aeneas.Std AccountExtracted
open InspectionAssetsRefinement InspectionModelAssetsRefinement

/-- Executable oracles use the same first-match and weight-update definitions
as the proofs. A failing comparison is a stopping point, just like a match. -/
def lookupResult (entries : List (Nat × Nat)) (query : Nat) (panicAt : Option Nat) : Json :=
  let found := Inspection.first (fun left right => left == right || some left == panicAt)
    (fun entry => entry.1.1) query entries.zipIdx
  let failed := found.any (fun entry => some entry.1.1 == panicAt)
  Json.mkObj [("panics", toJson failed),
    ("value", toJson (if failed then none else found.map (fun entry => entry.1.2))),
    ("comparisons", toJson (found.map (fun entry => entry.2 + 1) |>.getD entries.length))]

def lookups : Array Json := Id.run do
  let mut cases := #[]
  for entries in [[], [(0, 1)], [(2, 7), (1, 3), (2, 9)],
      [(0, 1), (99, 2), (0, 3)], [(99, 1), (2, 2)], [(2, 1), (1, 2), (99, 3)]] do
    for query in [0, 1, 2, 9] do
      for panicAt in [none, some 99] do
        for kind in ["applicable", "infeasible", "invalid"] do
          let accounts := if kind == "invalid" then [] else entries
          let shortfalls := if kind == "infeasible" then entries.reverse else []
          cases := cases.push (Json.mkObj [("entries", toJson entries), ("query", toJson query),
            ("panicAt", toJson panicAt), ("kind", toJson kind),
            ("account", lookupResult accounts query panicAt),
            ("shortfall", lookupResult shortfalls query panicAt)])
  return cases

def weightsJson (entries : List (Nat × Int)) : Json :=
  toJson (entries.map fun (asset, value) => (asset, toString value))

def invariantCase (label : String) (updates : List (Nat × Int)) : Json := Id.run do
  let mut current := []
  let mut snapshots := #[weightsJson current]
  for (asset, value) in updates do
    current := Inspection.weight current asset value
    snapshots := snapshots.push (weightsJson current)
  return Json.mkObj [("name", toJson label), ("updates", weightsJson updates),
    ("snapshots", toJson snapshots)]

def invariants : Array Json := Id.run do
  let mut cases := #[]
  let atoms : List (Nat × Int) := [(0, 0), (0, 1), (0, -1), (1, 7), (1, 0),
    (2, -(2^63)), (2, 2^63 - 1)]
  for label in ["", "éλ🛠", "repeated name"] do
    cases := cases.push (invariantCase label [])
    for first in atoms do
      for second in atoms do
        cases := cases.push (invariantCase label [first, second])
    cases := cases.push (invariantCase label
      [(2, 7), (0, 1), (1, -3), (0, 9), (2, 0), (2, -(2^63)), (1, 0), (0, 0), (2, 0)])
  return cases

abbrev Expression := rate.QuantityExpression Nat Nat Nat
abbrev Rule := rate.Rate Nat Nat Nat
abbrev World := economy.Economy Nat Nat Nat Nat Nat

def binaryJson (kind : String) (left right : Json) : Json :=
  Json.mkObj [("kind", toJson kind), ("left", left), ("right", right)]

def expressionJson : Expression → Json
  | .Constant value => Json.mkObj [("kind", toJson "constant"), ("value", toJson (toString value))]
  | .Units => Json.mkObj [("kind", toJson "units")]
  | .Parameter label => Json.mkObj [("kind", toJson "parameter"), ("name", toJson label)]
  | .Balance role asset => Json.mkObj [("kind", toJson "balance"), ("role", toJson role), ("asset", toJson asset)]
  | .Add left right => binaryJson "add" (expressionJson left) (expressionJson right)
  | .Subtract left right => binaryJson "subtract" (expressionJson left) (expressionJson right)
  | .Multiply left right => binaryJson "multiply" (expressionJson left) (expressionJson right)
  | .Minimum left right => binaryJson "minimum" (expressionJson left) (expressionJson right)
  | .Maximum left right => binaryJson "maximum" (expressionJson left) (expressionJson right)
  | .MulDiv left right denominator rounding => Json.mkObj [("kind", toJson "mul_div"),
      ("left", expressionJson left), ("right", expressionJson right),
      ("denominator", expressionJson denominator),
      ("rounding", toJson (match rounding with | .Floor => "floor" | .Ceil => "ceil"))]

def entriesJson (entries : List (Nat × Nat)) : Json :=
  toJson (entries.map fun (asset, value) => (asset, toString value))

def amountJson (amount : rate.ComputedAmount Nat Nat Nat) : Json :=
  Json.mkObj [("role", toJson amount.role), ("asset", toJson amount.asset),
    ("quantity", expressionJson amount.quantity)]

def conditionJson (condition : rate.RateCondition Nat Nat Nat) : Json :=
  Json.mkObj [("name", toJson condition.name), ("left", expressionJson condition.left),
    ("right", expressionJson condition.right), ("comparison", toJson (match condition.comparison with
      | .Equal => "equal" | .GreaterThanOrEqual => "greater" | .LessThanOrEqual => "less"))]

def ruleJson (rule : Rule) : Json :=
  let staticJson := fun entries : List (Nat × basket.Basket Nat Nat) =>
    toJson (entries.map fun (role, contents) => (role, entriesJson contents.quantities))
  Json.mkObj [("consume", staticJson rule.consume), ("produce", staticJson rule.produce),
    ("preserve", staticJson rule.preserve), ("roles", toJson rule.roles), ("distinct", toJson rule.distinct),
    ("computedConsume", toJson (rule.computed_consume.val.map amountJson)),
    ("computedProduce", toJson (rule.computed_produce.val.map amountJson)),
    ("computedPreserve", toJson (rule.computed_preserve.val.map amountJson)),
    ("conditions", toJson (rule.conditions.val.map conditionJson)),
    ("computedAssets", toJson (computedReferences rule))]

def emptyRule : Rule := ⟨[], [], [], [], [], alloc.vec.Vec.new _, alloc.vec.Vec.new _,
  alloc.vec.Vec.new _, alloc.vec.Vec.new _⟩

def exampleRule (expression : Expression) (value : Nat) : Rule :=
  -- All expression balance roles in the scenarios below are 4 or 8.
  { consume := [(3, ⟨[(10, value)]⟩)], produce := [(1, ⟨[(11, value)]⟩)],
    preserve := [(2, ⟨[(12, value)]⟩)], roles := [1, 2, 3, 4, 5, 8], distinct := [(1, 3), (5, 5)],
    computed_consume := alloc.vec.Vec.from [⟨5, 21, expression⟩] (by simp; scalar_tac),
    computed_produce := alloc.vec.Vec.from [⟨3, 22, .Balance 4 88⟩] (by simp; scalar_tac),
    computed_preserve := alloc.vec.Vec.from [⟨5, 21, expression⟩] (by simp; scalar_tac),
    conditions := alloc.vec.Vec.from [⟨"éλ", expression, .Equal, .Balance 8 99⟩,
      ⟨"", .Units, .LessThanOrEqual, .Constant value⟩,
      ⟨"未知", .Parameter "unbound", .GreaterThanOrEqual, .Constant 0⟩] (by simp; scalar_tac) }

def modelCase (mode value : Nat) (expression : Expression) : Json :=
  let model : World := {
    accounts := if mode % 2 = 0 then [] else [(7, ⟨⟨[(2, value), (0, 1)]⟩⟩), (2, ⟨⟨[]⟩⟩)],
    rates := if (mode / 2) % 2 = 0 then [] else [(9, exampleRule expression value), (1, emptyRule)],
    invariants := if mode / 4 = 0 then alloc.vec.Vec.new _ else
      alloc.vec.Vec.from [⟨"éλ", [(45, (-1)#i64), (2, 1#i64)]⟩, ⟨"", []⟩] (by simp; scalar_tac) }
  Json.mkObj [("value", toJson (toString value)),
    ("accounts", toJson (model.accounts.map fun (id, holder) => (id, entriesJson holder.balances.quantities))),
    ("rates", toJson (model.rates.map fun (id, rule) => (id, ruleJson rule))),
    ("invariants", toJson (model.invariants.val.map fun invariant =>
      Json.mkObj [("name", toJson invariant.name),
        ("weights", weightsJson (Account.mapEntries IScalar.val invariant.weights))])),
    ("assets", toJson (Discovery.registerAll compare (modelReferences compare model) []))]

def models : Array Json := Id.run do
  let mut cases := #[]
  for value in [1, Quantity.u64Max, 2^128 + 7] do
    let left : Expression := .Add (.Balance 4 88) (.Parameter "未知`")
    let right : Expression := .Maximum (.Balance 8 77) (.Balance 4 88)
    let expressions : List Expression := [.Constant value, .Units, .Parameter "", .Balance 4 88,
      .Add left right, .Subtract left right, .Multiply left right, .MulDiv left (.Constant 1) right .Floor,
      .Minimum left right, .Maximum left right, .MulDiv left right (.Balance 8 66) .Floor,
      .MulDiv (.Balance 8 66) right left .Ceil]
    for expression in expressions do
      for mode in List.range 8 do
        cases := cases.push (modelCase mode value expression)
  return cases

def fixtures : Json := Json.mkObj [("lookups", toJson lookups), ("invariants", toJson invariants),
  ("models", toJson models)]

end InspectionConformance
