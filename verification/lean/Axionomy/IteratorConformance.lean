import Lean
import Axionomy.IteratorContract
import Axionomy.Discovery

namespace IteratorConformance
open Lean Axionomy

def fixtures : Array Json := Id.run do
  let scripts : List (List (Option Nat)) := [[], [none], [some 1],
    [some 2, some 1, some 2], [none, some 7], [some 2, none, some 1],
    [some 0, some 0, none, some 9], [some 9, some 1, some 0, some 1]]
  let mut result := #[]
  for initial in [[], [7], [1, 2]] do
    for script in scripts do
      let values := IteratorContract.takeUntilNone script
      result := result.push (Json.mkObj [("initial", toJson initial), ("script", toJson script),
        ("expectedTrace", toJson (initial ++ values)),
        ("expectedVisible", toJson (Discovery.registerAll compare values [])),
        ("calls", toJson (values.length + 1))])
  return result

end IteratorConformance
