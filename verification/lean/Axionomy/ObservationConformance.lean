import Lean
import Axionomy.FingerprintRefinement
import Axionomy.GoalRefinement
import Axionomy.ViewConstructionRefinement

namespace ObservationConformance
open Lean Axionomy Aeneas Aeneas.Std

abbrev World := Observations.World Nat Nat Nat

def balances (entries : List (Nat × Nat)) : Json := toJson (entries.map fun (asset, value) => (asset, toString value))
def accounts (world : World) : Json := toJson (world.map fun (id, entries) => (id, balances entries))
def triples (entries : List (Nat × Nat × Nat)) : Json :=
  toJson (entries.map fun (id, asset, value) => Json.arr #[toJson id, toJson asset, toJson (toString value)])

def word (value : Nat) : Std.U64 := ⟨BitVec.ofNat 64 value⟩
def cmp : (Nat × Nat × Nat) → (Nat × Nat × Nat) → Ordering := KeyOrdering.lex compare (KeyOrdering.lex compare compare)

def fingerprint (entries : List (Nat × Nat × Nat)) : String :=
  let values := entries.map fun (id, asset, amount) => (id, asset, word amount)
  if fits : values.length ≤ U64.max then
    toString (FingerprintRefinement.run 14695981039346656037#u64
      (FingerprintRefinement.bytes
        (FingerprintRefinement.tripleBytes (FingerprintRefinement.numberBytes ∘ word) (FingerprintRefinement.numberBytes ∘ word))
        values fits)).val
  else "length overflow"

def scenario (before : World) (requested : List Nat) (requirements : World) : Json :=
  let world := before.map fun (id, entries) => (id, Basket.canonical entries)
  let goal := requirements.foldl (fun state entry => (RateConstruction.locate compare entry.1 state).put (Basket.canonical entry.2)) []
  let visible := Discovery.registerAll compare requested []
  let observed := CanonicalKeys.observe cmp world visible
  let stateKey := CanonicalSort.sort cmp (CanonicalKeys.entries world)
  let queries := [0, 1, 2, 3, Quantity.u64Max].flatMap fun id => [0, 1, 2, Quantity.u64Max].map fun asset =>
    Json.mkObj [("account", toJson id), ("asset", toJson asset),
      ("balance", toJson (toString (Account.balance 0 ((Account.find? id world).getD []) asset))),
      ("observed", toJson ((Observations.balance 0 world visible id asset).map toString))]
  Json.mkObj [("accounts", accounts before), ("requested", toJson requested), ("goal", accounts requirements),
    ("expected", Json.mkObj [("matches", toJson (Observations.satisfies world goal)),
      ("requirements", accounts goal),
      ("goalAssets", toJson (Discovery.registerAll compare (goal.flatMap fun entry => entry.2.map Prod.fst) [])),
      ("visible", toJson observed.visible), ("existing", toJson observed.existing),
      ("observationKey", triples observed.balances), ("stateKey", triples stateKey),
      ("fingerprint", toJson (fingerprint stateKey)), ("queries", toJson queries)])]

def fixtures : Array Json := Id.run do
  let limit := Quantity.u64Max
  let worlds : List World := [[], [(0, [])], [(1, [])], [(1, []), (0, [])],
    [(0, [(0, 0), (1, 0)])], [(0, [(1, 2), (0, 1)]), (1, [(2, 3)])],
    [(1, [(2, 3)]), (0, [(0, 1), (1, 2)])],
    [(0, [(0, limit), (2, 1)]), (2, []), (1, [(1, limit)])],
    [(limit, [(limit, limit)]), (0, [(0, 1)])]]
  let goals : List World := [[], [(0, [])], [(3, [])], [(0, [(0, 1)])],
    [(0, [(0, limit)])], [(0, [(0, limit)]), (0, [])],
    [(1, [(2, 3)]), (0, [(1, 2), (0, 1)])], [(limit, [(limit, limit)])]]
  let mut result := #[]
  for world in worlds do
    for visible in [[], [0], [1], [0, 3], [3, 2, 1, 0], [1, 0, 1, 3, 0], [limit, 0, limit]] do
      for goal in goals do
        result := result.push (scenario world visible goal)
  return result

end ObservationConformance
