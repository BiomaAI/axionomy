import Lean
import Axionomy.BasketSortedRefinement
import Axionomy.Quantity

namespace BasketSortedConformance
open Lean Axionomy

def fixtures : Array Json := Id.run do
  let atoms : List (Nat × Nat) := [(0, 0), (1, 1), (2, 7), (3, Quantity.u64Max), (4, 2)]
  let mut inputs := [[]]
  let mut layer := [[]]
  for _ in [0:5] do
    layer := layer.flatMap fun tail =>
      (atoms.filter fun entry => !(tail.map Prod.fst).contains entry.1).map fun entry => entry :: tail
    inputs := inputs ++ layer
  let encode := fun values : List (Nat × Nat) => values.map fun (asset, value) => (asset, toString value)
  return inputs.toArray.map fun values =>
    let canonical := BasketCollection.canonical (fun value : Nat => decide (value = 0)) values
    Json.mkObj [("entries", toJson (encode values)),
      ("ascending", toJson (encode (BasketOrdering.sorted compare canonical))),
      ("descending", toJson (encode (BasketOrdering.sorted (fun left right => compare right left) canonical)))]

end BasketSortedConformance
