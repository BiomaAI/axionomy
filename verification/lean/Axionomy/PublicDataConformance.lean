import Lean
import Axionomy.TypedDataRefinement

namespace PublicDataConformance
open Lean Axionomy

/-- The same independent predicate specification used by quantity_validation. -/
def validation : Array Json :=
  ([-9223372036854775808, -1, 0, 1, 9223372036854775807] : List Int).toArray.map fun value =>
    let result := TypedDataRefinement.validate (fun n : Int => decide (0 ≤ n)) value
    let accepted := match result with
      | .Ok valid => some (toString valid)
      | .Err _ => none
    Json.mkObj [("input", toJson (toString value)), ("accepted", toJson accepted)]

def quantities : List Nat := [0, 1, 2, 4294967295, 4294967296, Quantity.u64Max]

def values : Array Json := Id.run do
  let mut result := #[]
  for asset in [0, 1, Quantity.u64Max] do
    for value in quantities do
      let before := [(1, 7), (0, 3)]
      let old := Account.find? asset before
      let after := Account.store (Account.naturals Quantity.u64Max) before asset value
      result := result.push (Json.mkObj [("asset", toJson asset), ("value", toJson (toString value)),
        ("before", toJson (before.map fun (a, q) => (a, toString q))),
        ("old", toJson (old.map toString)),
        ("after", toJson (after.map fun (a, q) => (a, toString q)))])
  return result

def proposals : Array Json := Id.run do
  let mut result := #[]
  let bindingSequences : List (List (Nat × Nat)) := [[], [(0, 1)], [(1, 0), (0, 1)],
    [(0, 1), (0, 2), (1, 3)], [(Quantity.u64Max, Quantity.u64Max), (0, 0)]]
  let parameterSequences : List (List (String × Nat)) := [[], [("", 0)], [("z", 2), ("a", 1)],
    [("β", 1), ("a", 2), ("β", Quantity.u64Max)]]
  for units in quantities do
    for bindings in bindingSequences do
      for parameters in parameterSequences do
        let bound := bindings.foldl (fun state entry => (RateConstruction.locate compare entry.1 state).put entry.2) []
        let supplied := parameters.foldl (fun state entry => (RateConstruction.locate compare entry.1 state).put entry.2) []
        let strings := fun entries : List (String × Nat) => entries.map fun (label, value) => (label, toString value)
        result := result.push (Json.mkObj [("rate", toJson (7 : Nat)), ("units", toJson (toString units)),
          ("bindings", toJson bindings), ("parameters", toJson (strings parameters)),
          ("expectedBindings", toJson bound), ("expectedParameters", toJson (strings supplied))])
  return result

def fixtures : Json := Json.mkObj [("validation", toJson validation), ("values", toJson values),
  ("proposals", toJson proposals)]

end PublicDataConformance
