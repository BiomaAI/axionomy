import Lean
import Axionomy.DerivedAccountEqualityRefinement

namespace DerivedConformance
open Lean Axionomy Aeneas Aeneas.Std AccountExtracted

abbrev Expr := rate.QuantityExpression Nat Nat Nat

def atoms : List Expr := [.Constant 0, .Constant Quantity.u64Max, .Units, .Parameter "", .Parameter "β",
  .Balance 0 1, .Balance 1 0, .Constant 0]

def expressions : List Expr := atoms ++
  ([rate.QuantityExpression.Add, rate.QuantityExpression.Subtract, rate.QuantityExpression.Multiply, rate.QuantityExpression.Minimum, rate.QuantityExpression.Maximum].flatMap fun op =>
    [op (.Constant 0) .Units, op .Units (.Constant 0), op (.Balance 0 1) (.Parameter "β")]) ++
  [.Add (.Multiply .Units (.Constant 3)) (.Parameter "β"),
   .Add (.Multiply .Units (.Constant 4)) (.Parameter "β"),
   .Add (.Multiply .Units (.Constant 3)) (.Parameter "γ"),
   .Maximum (.Minimum (.Balance 0 1) .Units) (.MulDiv (.Constant 7) (.Constant 1) (.Constant 2) .Floor)] ++
  ([quantity.Rounding.Floor, quantity.Rounding.Ceil].flatMap fun rounding =>
    [.MulDiv (.Constant 7) .Units (.Constant 2) rounding, .MulDiv .Units (.Constant 7) (.Constant 2) rounding,
     .MulDiv (.Constant 7) .Units (.Balance 0 1) rounding,
     .MulDiv (.Balance 0 1) (.Parameter "β") .Units rounding])

def binaryJson (kind : String) (left right : Json) : Json :=
  Json.mkObj [("kind", toJson kind), ("left", left), ("right", right)]

def expressionJson : Expr → Json
  | .Constant value => Json.mkObj [("kind", toJson "constant"), ("value", toJson (toString value))]
  | .Units => Json.mkObj [("kind", toJson "units")]
  | .Parameter key => Json.mkObj [("kind", toJson "parameter"), ("name", toJson key)]
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

def equalPairs {T : Type} [DecidableEq T] (values : List T) : List (Nat × Nat × Bool) :=
  values.zipIdx.flatMap fun (left, i) => values.zipIdx.map fun (right, j) => (i, j, decide (left = right))

abbrev Proposal := exchange.Exchange Nat Nat Nat Nat

def proposals : List Proposal :=
  [⟨0, [], 0, []⟩, ⟨1, [], 0, []⟩, ⟨0, [], 1, []⟩,
   ⟨0, [(0, 1)], 1, []⟩, ⟨0, [(0, 2)], 1, []⟩,
   ⟨0, [(0, 1)], 1, [("β", 7)]⟩, ⟨0, [(0, 1)], 1, [("β", 8)]⟩,
   ⟨0, [(0, 1)], 1, [("γ", 7)]⟩, ⟨0, [(0, 1), (1, 2)], Quantity.u64Max, [("", 0), ("β", 7)]⟩,
   ⟨0, [], 0, []⟩]

def proposalJson (value : Proposal) : Json := Json.mkObj [
  ("rate", toJson value.rate), ("bindings", toJson value.bindings), ("units", toJson (toString value.units)),
  ("parameters", toJson (value.parameters.map fun (key, number) => (key, toString number)))]

def traces : List (List Nat) := [[], [0], [9], [0, 1], [1, 0], [0, 0], [0, 1, 2], [0, 1, 3]]

abbrev KeyFields := List Nat × List Nat × List (Nat × Nat × Nat)

def keys : List KeyFields := [([], [], []), ([0], [], []), ([0], [0], []),
  ([0], [0], [(0, 1, 7)]), ([0], [0], [(0, 1, 8)]), ([0], [0], [(0, 2, 7)]),
  ([0, 1], [0, 1], [(0, 1, 7)]), ([0, 1], [0, 1], [(1, 1, 7)]),
  ([0, 1], [0], [(0, 1, 7)]), ([0], [0], [(0, 1, 7)]),
  ([0, 1], [0, 1], [(0, 1, 7), (1, 2, Quantity.u64Max)])]

def word (value : Nat) : Std.U64 := UScalar.ofNatCore (value % 2^64) (Nat.mod_lt _ (by decide))

def wordBytes (value : Nat) : List Std.U8 := FingerprintRefinement.numberBytes (word value)

def sequenceBytes {T : Type} (encode : T → List Std.U8) (values : List T) : List Std.U8 :=
  wordBytes values.length ++ values.flatMap encode

def keyBytes (value : KeyFields) : List Std.U8 :=
  sequenceBytes wordBytes value.1 ++ sequenceBytes wordBytes value.2.1 ++
    sequenceBytes (DerivedKeyRefinement.balanceBytes wordBytes wordBytes wordBytes) value.2.2

def keyJson (value : KeyFields) : Json :=
  let bytes := (keyBytes value).map UScalar.val
  Json.mkObj [("visible", toJson value.1), ("existing", toJson value.2.1),
    ("balances", toJson (value.2.2.map fun (account, asset, number) => (account, asset, toString number))),
    ("bytes", toJson bytes), ("hash", toJson (toString (FingerprintRefinement.fnv 14695981039346656037 bytes)))]

def numbers : List Nat := [0, 1, 2, 4294967295, 4294967296, Quantity.u64Max]

def numberPairs : List Json := numbers.flatMap fun left => numbers.map fun right =>
  let order := compare left right
  let ordering := match order with | .lt => -1 | .eq => 0 | .gt => 1
  Json.mkObj [("left", toJson (toString left)), ("right", toJson (toString right)), ("cmp", toJson ordering),
    ("eq", toJson (decide (left = right))),
    ("predicates", toJson ([ [.lt], [.lt, .eq], [.gt], [.eq, .gt] ].map fun allowed =>
      DerivedValueRefinement.accepts allowed (some order))),
    ("bytes", toJson ((wordBytes left).map UScalar.val))]

def baskets : List (List (Nat × Nat)) := [[], [(0, 1)], [(1, 1)], [(0, 2)],
  [(0, 1), (1, 2)], [(1, 2), (0, 1)], [(0, Quantity.u64Max)]]

def basketPairs : List (Nat × Nat × Bool) := baskets.zipIdx.flatMap fun (left, i) =>
  baskets.zipIdx.map fun (right, j) => (i, j, DerivedAccountEqualityRefinement.basketEquality left right)

def fixtures : Json := Json.mkObj [
  ("expressions", toJson (expressions.map expressionJson)), ("expressionPairs", toJson (equalPairs expressions)),
  ("proposals", toJson (proposals.map proposalJson)), ("proposalPairs", toJson (equalPairs proposals)),
  ("traces", toJson traces),
  ("tracePairs", toJson (equalPairs (traces.map fun trace => trace.filterMap fun i => proposals[i]?))),
  ("keys", toJson (keys.map keyJson)), ("keyPairs", toJson (equalPairs keys)),
  ("numbers", toJson numberPairs),
  ("baskets", toJson (baskets.map fun entries => entries.map fun (asset, number) => (asset, toString number))),
  ("basketPairs", toJson basketPairs)]

end DerivedConformance
