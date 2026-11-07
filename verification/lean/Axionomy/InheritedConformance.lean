import Lean
import Axionomy.InheritedHashRefinement
import Axionomy.DerivedConformance

namespace InheritedConformance
open Lean Axionomy Aeneas Aeneas.Std
open DerivedConformance

def selectMin (left right : Nat) : Nat := if right < left then right else left
def selectMax (left right : Nat) : Nat := if right < left then left else right

def orderCases : List Json := numbers.flatMap fun left => numbers.map fun right =>
  Json.mkObj [("left", toJson (toString left)), ("right", toJson (toString right)),
    ("min", toJson (toString (selectMin left right))), ("max", toJson (toString (selectMax left right)))]

def clampCases : List Json := numbers.flatMap fun value => numbers.flatMap fun lower => numbers.map fun upper =>
  let result := if lower ≤ upper then some (if value < lower then lower else if upper < value then upper else value) else none
  Json.mkObj [("value", toJson (toString value)), ("lower", toJson (toString lower)),
    ("upper", toJson (toString upper)), ("result", toJson (result.map toString))]

def bytes (value : Nat) : List Nat := (wordBytes value).map UScalar.val
def keyChunks (value : KeyFields) : List (List Nat) :=
  [bytes value.1.length] ++ value.1.map bytes ++
  [bytes value.2.1.length] ++ value.2.1.map bytes ++
  [bytes value.2.2.length] ++ value.2.2.flatMap fun (account, asset, number) =>
    [bytes account, bytes asset, bytes number]

def selections : List (List Nat) := [[], [0], [1], [0, 1], [1, 0], [1, 1], [0, 2, 5], [5, 4, 3, 2, 1, 0]]

def hashCases : List Json := selections.map fun selected =>
  let values := selected.filterMap fun i => numbers[i]?
  let selectedKeys := selected.filterMap fun i => keys[i]?
  Json.mkObj [("indices", toJson selected),
    ("numbers", toJson (values.map toString)),
    ("quantityChunks", toJson (values.map bytes)),
    ("amountChunks", toJson (values.flatMap fun value => [bytes 7, bytes value])),
    ("keyChunks", toJson (selectedKeys.flatMap keyChunks))]

def taggedOrder : List Json := [0, 5, 9].flatMap fun left => [0, 5, 9].map fun right =>
  Json.mkObj [("left", toJson left), ("right", toJson right),
    ("minTag", toJson (if right < left then 2 else 1 : Nat)),
    ("maxTag", toJson (if right < left then 1 else 2 : Nat))]

def taggedClamp : List Json := [0, 5, 9].flatMap fun value => [0, 5, 9].flatMap fun lower => [0, 5, 9].map fun upper =>
  let tag : Option Nat := if lower ≤ upper then some (if value < lower then 2 else if upper < value then 3 else 1) else none
  Json.mkObj [("value", toJson value), ("lower", toJson lower), ("upper", toJson upper), ("tag", toJson tag)]

def fixtures : Json := Json.mkObj [
  ("order", toJson orderCases), ("clamp", toJson clampCases), ("hashSlices", toJson hashCases),
  ("taggedOrder", toJson taggedOrder), ("taggedClamp", toJson taggedClamp)]

end InheritedConformance
