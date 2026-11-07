import Lean
import Axionomy.Exchange
import Axionomy.Rates
import Axionomy.Basket
import Axionomy.Account
import Axionomy.BasketOperations

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

structure BasketCase where
  entries : List (Nat × String)
  duplicate : Option Nat
  canonical : Option (List (Nat × String))
  deriving ToJson

def basketCase (entries : List (Nat × Nat)) : BasketCase :=
  let strings := entries.map (fun (asset, amount) => (asset, toString amount))
  match Basket.construct entries with
  | .error asset => ⟨strings, some asset, none⟩
  | .ok output => ⟨strings, none, some (output.map (fun (asset, amount) => (asset, toString amount)))⟩

def expression : Rates.Expr Unit Unit Unit :=
  .binary .div (.binary .mul (.balance () ()) .units) (.parameter ())

structure AccountCase where
  operation : String
  before : List (Nat × String)
  amounts : List (Nat × String)
  after : List (Nat × String)
  overflow : Option Nat
  shortfall : Option (List (Nat × String))
  deriving ToJson

def accountCase (depositing : Bool) (before amounts : List (Nat × Nat)) : AccountCase :=
  let before := Basket.canonical before
  let amounts := Basket.canonical amounts
  let ops := Account.naturals Quantity.u64Max
  let result := if depositing then Account.deposit ops before amounts else Account.withdraw ops before amounts
  let strings := fun entries : List (Nat × Nat) => entries.map (fun (asset, value) => (asset, toString value))
  let error := match result.1 with
    | .ok () => (none, none)
    | .error (.overflow asset) => (some asset, none)
    | .error (.insufficient missing) => (none, some (strings missing))
  ⟨if depositing then "deposit" else "withdraw", strings before, strings amounts,
    strings result.2, error.1, error.2⟩

structure PreparationCase where
  before : List (Nat × String)
  consumed : List (Nat × String)
  produced : List (Nat × String)
  after : List (Nat × String)
  overflow : Option Nat
  shortfall : Option (List (Nat × String))
  deriving ToJson

def preparationCase (before consumed produced : List (Nat × Nat)) : PreparationCase :=
  let before := Basket.canonical before
  let consumed := Basket.canonical consumed
  let produced := Basket.canonical produced
  let result := Account.preparedOutcome before
    (Account.prepare (Account.naturals Quantity.u64Max) before consumed produced)
  let strings := fun entries : List (Nat × Nat) => entries.map (fun (asset, value) => (asset, toString value))
  let error := match result.1 with
    | .ok () => (none, none)
    | .error (.overflow asset) => (some asset, none)
    | .error (.insufficient missing) => (none, some (strings missing))
  ⟨strings before, strings consumed, strings produced, strings result.2, error.1, error.2⟩

structure BasketOpsCase where
  before : List (Nat × String)
  other : List (Nat × String)
  units : String
  added : List (Nat × String)
  addOverflow : Option Nat
  scaled : Option (List (Nat × String))
  scaleOverflow : Option Nat
  contains : Bool
  equal : Bool
  deriving ToJson

def basketOpsCase (before other : List (Nat × Nat)) (units : Nat) : BasketOpsCase :=
  let before := Basket.canonical before
  let other := Basket.canonical other
  let ops := Account.naturals Quantity.u64Max
  let added := BasketOperations.add ops before other
  let scaled := BasketOperations.scale ops (Quantity.mul Quantity.u64Max) before units
  let strings := fun entries : List (Nat × Nat) => entries.map (fun (asset, value) => (asset, toString value))
  let addError := match added.1 with | .ok () => none | .error asset => some asset
  let scaleResult := match scaled with
    | .ok entries => (some (strings entries), none)
    | .error asset => (none, some asset)
  ⟨strings before, strings other, toString units, strings added.2, addError, scaleResult.1,
    scaleResult.2, (Account.shortfall ops before other).isEmpty, BasketOperations.equal before other⟩

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
  let alphabet := [0, 1].flatMap (fun asset => [0, 1, limit].map (fun amount => (asset, amount)))
  let mut level : List (List (Nat × Nat)) := [[]]
  let mut baskets : Array BasketCase := #[]
  for _ in [0:4] do
    for entries in level do
      baskets := baskets.push (basketCase entries)
    level := level.flatMap (fun prior => alphabet.map (fun entry => prior ++ [entry]))
  baskets := baskets.push (basketCase [(0, 0), (1, 2), (1, 0), (0, 1)])
  let mut accounts : Array AccountCase := #[]
  for first in [0, 1, limit] do
    for second in [0, 1, limit] do
      for addFirst in [0, 1, limit] do
        for addSecond in [0, 1, limit] do
          for reverse in [false, true] do
            let amounts := [(0, addFirst), (1, addSecond)]
            let amounts := if reverse then amounts.reverse else amounts
            for depositing in [false, true] do
              accounts := accounts.push (accountCase depositing [(0, first), (1, second), (2, 7)] amounts)
  for (before, amounts) in [([(0, 10), (1, limit), (2, 7)], [(0, 5), (1, 1)]),
      ([(0, 5), (1, 2), (2, 7)], [(0, 5), (1, 3), (3, 1)])] do
    for depositing in [false, true] do
      accounts := accounts.push (accountCase depositing before amounts)
  let mut preparations : Array PreparationCase := #[]
  for first in [0, 1, limit] do
    for second in [0, 1, limit] do
      for consumed in [0, 1, limit] do
        for producedFirst in [0, 1, limit] do
          for producedSecond in [0, 1, limit] do
            for reverse in [false, true] do
              let take := [(0, consumed), (1, 1)]
              let give := [(0, producedFirst), (1, producedSecond)]
              preparations := preparations.push (preparationCase [(0, first), (1, second), (2, 7)]
                (if reverse then take.reverse else take) (if reverse then give.reverse else give))
  for (before, consumed, produced) in
      [([], [], []), ([(0, limit)], [(0, limit)], [(0, limit)]),
       ([(0, 5), (1, limit)], [(0, 5)], [(0, 1), (1, 1)]),
       ([(0, 5)], [(0, 6), (3, 2)], [(0, 10), (3, 2)]),
       ([(0, 5), (2, 7)], [(0, 5)], [(3, 2), (0, 1)]),
       ([(0, limit), (1, limit)], [], [(1, 1), (0, 1)])] do
    preparations := preparations.push (preparationCase before consumed produced)
  let mut basketOps : Array BasketOpsCase := #[]
  for first in [0, 1, limit] do
    for second in [0, 1, limit] do
      for otherFirst in [0, 1, limit] do
        for otherSecond in [0, 1, limit] do
          for units in [0, 1, 2, limit] do
            for reverse in [false, true] do
              let other := [(0, otherFirst), (1, otherSecond), (2, 7)]
              basketOps := basketOps.push (basketOpsCase [(0, first), (1, second), (2, 7)]
                (if reverse then other.reverse else other) units)
  for (before, other, units) in [([], [], 0), ([(0, 1)], [], 1), ([], [(0, 1)], 1),
      ([(0, 1)], [(1, 1)], 1), ([(0, 0)], [], limit),
      ([(0, 2), (1, limit)], [(0, 1), (1, 1)], 2)] do
    basketOps := basketOps.push (basketOpsCase before other units)
  let document := Json.mkObj [("version", toJson (1 : Nat)), ("scalars", toJson scalars),
    ("exchanges", toJson exchanges), ("expressions", toJson expressions), ("baskets", toJson baskets),
    ("accounts", toJson accounts), ("preparations", toJson preparations), ("basketOps", toJson basketOps)]
  IO.println document.compress
