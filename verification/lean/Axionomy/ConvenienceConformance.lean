import Lean
import Axionomy.ConvenienceRefinement
import Axionomy.CallbackRefinement

namespace ConvenienceConformance
open Lean Axionomy

def literals : Array Json := Id.run do
  let atoms : List (Nat × Nat) := [(0, 0), (0, 1), (1, 0), (1, 7), (2, Quantity.u64Max)]
  let mut all := [[]]
  let mut layer := [[]]
  for _ in [0:4] do
    layer := layer.flatMap fun rest => atoms.map fun entry => entry :: rest
    all := all ++ layer
  let strings := fun entries : List (Nat × Nat) => entries.map fun (asset, value) => (asset, toString value)
  return all.toArray.map fun entries =>
    let result := BasketCollection.construct (fun value : Nat => decide (value = 0)) [] entries
    let expected := match result with | .error _ => none | .ok contents => some (strings contents)
    Json.mkObj [("entries", toJson (strings entries)), ("expected", toJson expected)]

def callbacks : Array Json := Id.run do
  let mut cases := #[]
  let before : List (String × Nat) := [("!", 7), ("z", 3)]
  let left : Evaluation.Expr Nat Nat Nat := .balance 5 7
  let right : Evaluation.Expr Nat Nat Nat := .binary .add (.parameter "param") (.balance 2 9)
  for input in ["", "a", "β", "未知🚚"] do
    for fails in [false, true] do
      for value in [0, 1, Quantity.u64Max] do
        for comparison in ["equal", "greater", "less"] do
          let label := input ++ "!"
          let parameters := (RateConstruction.locate compare label before).put value
          cases := cases.push (Json.mkObj [("input", toJson input), ("fails", toJson fails),
            ("label", toJson label), ("value", toJson (toString value)),
            ("comparison", toJson comparison),
            ("parameters", toJson (parameters.map fun (key, value) => (key, toString value))),
            ("roles", toJson (Discovery.conditionRoles compare left right [1, 8]))])
  return cases

def fixtures : Json := Json.mkObj [("literals", toJson literals), ("callbacks", toJson callbacks)]

end ConvenienceConformance
