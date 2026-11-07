import Lean
import Axionomy.InvariantInput

namespace Axionomy.InvariantInputConformance
open Lean

structure Case where
  entries : List (Nat × String)
  expected : Option (List (Nat × String))
  deriving ToJson

def sample (entries : List (Nat × Int)) : Case :=
  let encode := fun values : List (Nat × Int) => values.map (fun (key, value) => (key, toString value))
  ⟨encode entries, match InvariantInput.construct entries with
    | .error _ => none
    | .ok weights => some (encode weights)⟩

def fixtures : List Case := Id.run do
  let choices : List (Nat × Int) := [(0, -1), (0, 0), (0, 1), (1, -1), (1, 0), (1, 1)]
  let mut cases := [sample []]
  for first in choices do
    cases := cases ++ [sample [first]]
    for second in choices do
      cases := cases ++ [sample [first, second]]
      for third in choices do
        cases := cases ++ [sample [first, second, third]]
  return cases ++ [sample [(0, -9223372036854775808), (1, 9223372036854775807)],
    sample [(1, -9223372036854775808), (0, 0)],
    sample [(0, 0), (0, 9223372036854775807)]]

end Axionomy.InvariantInputConformance
