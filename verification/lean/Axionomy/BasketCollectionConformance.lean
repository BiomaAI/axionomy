import Lean
import Axionomy.BasketCollection
import Axionomy.Quantity

namespace BasketCollectionConformance
open Lean Axionomy

/-- Number of next calls before the first duplicate, or through the terminating
None when there is no duplicate. Used as finite integration evidence. -/
def pulls (seen : List Nat) : List (Nat × Nat) → Nat
  | [] => 1
  | (asset, _) :: rest => if asset ∈ seen then 1 else 1 + pulls (seen ++ [asset]) rest

def fixtures : Array Json := Id.run do
  let atoms : List (Nat × Nat) := [(0, 0), (0, 1), (1, Quantity.u64Max), (1, 0)]
  let mut entries := [[]]
  let mut layer := [[]]
  for _ in [0:3] do
    layer := layer.flatMap fun tail => atoms.map fun entry => entry :: tail
    entries := entries ++ layer
  let encode := fun values : List (Nat × Nat) => values.map fun (asset, value) => (asset, toString value)
  return entries.toArray.map fun values =>
    let result := BasketCollection.construct (fun value : Nat => decide (value = 0)) [] values
    let (accepted, duplicate) := match result with
      | .ok entries => (some (encode entries), none)
      | .error asset => (none, some asset)
    Json.mkObj [("entries", toJson (encode values)), ("accepted", toJson accepted),
      ("duplicate", toJson duplicate), ("pulls", toJson (pulls [] values))]

end BasketCollectionConformance
