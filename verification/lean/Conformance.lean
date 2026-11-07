import Lean
import Axionomy.Exchange
import Axionomy.Rates

/-! Deterministic executable examples from the SAME definitions used in the
proofs. Rust runs these examples through the public API. Agreement on these
finite cases is a regression bridge, not a universal Rust refinement proof. -/
open Lean Axionomy

structure ScalarCase where
  left : String
  right : String
  add : Option String
  sub : Option String
  mul : Option String
  div : Option String
  deriving ToJson

structure ExchangeCase where
  before : String
  consumed : String
  produced : String
  preserved : String
  units : String
  conserve : Bool
  accepted : Bool
  after : String
  deriving ToJson

structure ExpressionCase where
  balance : String
  units : String
  denominator : String
  minimum : String
  evaluated : Option String
  accepted : Bool
  sourceAfter : String
  sinkAfter : String
  deriving ToJson

def expression : Rates.Expr Unit Unit Unit :=
  .binary .div (.binary .mul (.balance () ()) .units) (.parameter ())

def main : IO Unit := do
  let limit := Quantity.u64Max
  let boundaries := [0, 1, 2, 3, limit - 1, limit]
  let mut scalars : Array ScalarCase := #[]
  for left in boundaries do
    for right in boundaries do
      scalars := scalars.push ⟨toString left, toString right,
        (Quantity.add limit left right).map toString, (Quantity.sub left right).map toString,
        (Quantity.mul limit left right).map toString, (Quantity.div left right).map toString⟩
  let mut exchanges : Array ExchangeCase := #[]
  for before in [0, 1, 3, 6, limit] do
    for consumed in [0, 1, 2] do
      for produced in [0, 1, 2] do
        for preserved in [0, 1] do
          for units in [0, 1, 2] do
            for conserve in [false, true] do
              let result := Exchange.staticOutcome before consumed produced preserved units conserve
              exchanges := exchanges.push ⟨toString before, toString consumed, toString produced,
                toString preserved, toString units, conserve, result.1, toString result.2⟩
  -- Scaling overflow, consume+preserve overflow, and maximum valid quantities.
  for (before, consumed, produced, preserved, units) in
      [(limit, limit, 0, 0, 2), (0, 0, limit, 0, 2), (limit, limit, 0, 1, 1),
       (limit, limit, limit, 0, 1)] do
    let result := Exchange.staticOutcome before consumed produced preserved units false
    exchanges := exchanges.push ⟨toString before, toString consumed, toString produced,
      toString preserved, toString units, false, result.1, toString result.2⟩
  let mut expressions : Array ExpressionCase := #[]
  for balance in [0, 1, 10, limit] do
    for units in [1, 2] do
      for denominator in [0, 1, 3] do
        for minimum in [0, 1] do
          let ctx : Rates.Context Unit Unit Unit :=
            ⟨units, fun _ => some denominator, fun _ _ => some balance⟩
          let evaluated := Rates.eval limit ctx expression
          let accepted := match evaluated with
            | none => false
            | some value => decide (minimum ≤ value ∧ value ≤ balance)
          let value := if accepted then evaluated.getD 0 else 0
          expressions := expressions.push ⟨toString balance, toString units, toString denominator,
            toString minimum, evaluated.map toString, accepted, toString (balance - value),
            toString value⟩
  let document := Json.mkObj [("version", toJson (1 : Nat)), ("scalars", toJson scalars),
    ("exchanges", toJson exchanges), ("expressions", toJson expressions)]
  IO.println document.compress
