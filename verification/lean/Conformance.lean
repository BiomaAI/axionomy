import Lean
import Axionomy.Exchange
import Axionomy.Rates
import Axionomy.Basket
import Axionomy.Account
import Axionomy.BasketOperations
import Axionomy.Conservation
import Axionomy.RateConstruction
import Axionomy.Evaluation
import Axionomy.Effects
import Axionomy.ModelConstruction
import Axionomy.Discovery
import Axionomy.Validation
import Axionomy.TransactionConformance
import Axionomy.BasketCollectionConformance
import Axionomy.InvariantInputConformance

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
  .mulDiv .floor (.balance () ()) .units (.parameter ())

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

structure ConservationDeltaCase where
  account : Nat
  consumed : List (Nat × String)
  produced : List (Nat × String)
  preserved : List (Nat × String)
  deriving ToJson

structure ConservationCase where
  weights : List (Nat × String)
  deltas : List ConservationDeltaCase
  conserved : Option Bool
  deriving ToJson

def conservationCase (weights : List (Nat × Int))
    (deltas : List (Conservation.Delta Nat Nat Nat)) : ConservationCase :=
  let strings := fun entries : List (Nat × Nat) =>
    entries.map (fun (asset, amount) => (asset, toString amount))
  ⟨weights.map (fun (asset, weight) => (asset, toString weight)),
    deltas.map (fun d => ⟨d.account, strings d.consumed, strings d.produced, strings d.preserved⟩),
    Conservation.conserves (fun a q => some (Conservation.contribution weights a q))
      Conservation.checkedAdd 0 deltas⟩

structure RateMergeCase where
  target : List (Nat × List (Nat × String))
  role : Nat
  assets : List (Nat × String)
  merged : Option (List (Nat × List (Nat × String)))
  overflow : Option Nat
  roles : List Nat
  deriving ToJson

def rateMergeCase (target : List (Nat × List (Nat × Nat))) (role : Nat)
    (assets : List (Nat × Nat)) : RateMergeCase :=
  let assets := Basket.canonical assets
  let strings := fun entries : List (Nat × Nat) => entries.map fun (asset, amount) => (asset, toString amount)
  let maps := fun entries : List (Nat × List (Nat × Nat)) => entries.map fun (r, b) => (r, strings b)
  let result := RateConstruction.merge (Account.naturals Quantity.u64Max) compare target role assets
  let outcome := match result with
    | .error asset => (none, some asset)
    | .ok merged => (some (maps merged), none)
  ⟨maps target, role, strings assets, outcome.1, outcome.2,
    RateConstruction.register compare role (target.map Prod.fst)⟩


/-- Plain floor division: a mul-div by the constant one. -/
def divide (left right : Evaluation.Expr Nat Nat Nat) : Evaluation.Expr Nat Nat Nat :=
  .mulDiv .floor left (.constant 1) right

/-- The binary combinators in their historical order; division is a mul-div. -/
def combinators : List (Evaluation.Expr Nat Nat Nat → Evaluation.Expr Nat Nat Nat → Evaluation.Expr Nat Nat Nat) :=
  [.binary .add, .binary .sub, .binary .mul, divide, .binary .min, .binary .max]

def expressionJson : Evaluation.Expr Nat Nat Nat → Json
  | .constant value => Json.mkObj [("kind", toJson "constant"), ("value", toJson value)]
  | .units => Json.mkObj [("kind", toJson "units")]
  | .parameter parameter => Json.mkObj [("kind", toJson "parameter"), ("name", toJson parameter)]
  | .balance role asset => Json.mkObj [("kind", toJson "balance"), ("role", toJson role), ("asset", toJson asset)]
  | .binary op left right =>
    let kind := match op with
      | .add => "add" | .sub => "subtract" | .mul => "multiply"
      | .min => "minimum" | .max => "maximum"
    Json.mkObj [("kind", toJson kind), ("left", expressionJson left), ("right", expressionJson right)]
  | .mulDiv rounding left right denominator =>
    Json.mkObj [("kind", toJson "mul_div"), ("left", expressionJson left), ("right", expressionJson right),
      ("denominator", expressionJson denominator),
      ("rounding", toJson (match rounding with | .floor => "floor" | .ceil => "ceil"))]

structure EvaluationCase where
  units : String
  parameters : List (String × String)
  bindings : List (Nat × Nat)
  accounts : List (Nat × List (Nat × String))
  expression : Json
  result : Option String
  error : Option String
  conditionRight : Json
  conditions : List (Option Bool × Option String)
  deriving ToJson

def evaluationCase (units : Nat) (expression right : Evaluation.Expr Nat Nat Nat) : EvaluationCase :=
  let parameters := [("a", 3), ("z", 0), ("éλ", Quantity.u64Max)]
  let bindings := [(0, 0), (1, 1), (2, 999)]
  let accounts := [(0, [(0, Quantity.u64Max), (1, 3)]), (1, [(0, 1)])]
  let context := Evaluation.fromState compare 0 units parameters bindings accounts
  let ops := Evaluation.naturals Quantity.u64Max
  let outcome := match Evaluation.eval ops context expression with
    | .ok value => (some (toString value), none)
    | .error error => (none, some error.message)
  let conditions := [Evaluation.Comparison.equal, .greaterOrEqual, .lessOrEqual].map fun comparison =>
    match Evaluation.condition ops id context comparison expression right with
    | .ok value => (some value, none)
    | .error error => (none, some error.message)
  ⟨toString units, parameters.map (fun (key, value) => (key, toString value)), bindings,
    accounts.map (fun (key, entries) => (key, entries.map fun (asset, value) => (asset, toString value))),
    expressionJson expression, outcome.1, outcome.2, expressionJson right, conditions⟩

structure MeasureCase where
  weights : List (Nat × String)
  accounts : List (Nat × List (Nat × String))
  measured : Option String
  deriving ToJson

def measureCase (weights : List (Nat × Int)) (accounts : List (Nat × List (Nat × Nat))) : MeasureCase :=
  ⟨weights.map (fun (asset, weight) => (asset, toString weight)),
    accounts.map (fun (key, entries) => (key, entries.map fun (asset, value) => (asset, toString value))),
    (Conservation.accumulate (fun a q => some (Conservation.contribution weights a q))
      Conservation.checkedAdd 0 (accounts.flatMap Prod.snd)).map toString⟩

structure EffectsCase where
  target : List (Nat × String)
  source : List (Nat × String)
  units : String
  scaled : List (Nat × String)
  scaleError : Option Nat
  asset : Nat
  amount : String
  computed : List (Nat × String)
  computedError : Option Nat
  deriving ToJson

def effectsCase (target source : List (Nat × Nat)) (units asset amount : Nat) : EffectsCase :=
  let target := Basket.canonical target
  let source := Basket.canonical source
  let strings := fun entries : List (Nat × Nat) => entries.map fun (key, value) => (key, toString value)
  let ops := Account.naturals Quantity.u64Max
  let scaled := match Effects.scaled ops (Quantity.mul Quantity.u64Max) target source units with
    | .ok entries => (strings entries, none) | .error asset => (strings target, some asset)
  let computed := match Account.credit ops target [(asset, amount)] with
    | .ok entries => (strings entries, none) | .error asset => (strings target, some asset)
  ⟨strings target, strings source, toString units, scaled.1, scaled.2, asset, toString amount,
    computed.1, computed.2⟩


structure ModelSnapshot where
  accounts : List (Nat × Nat)
  rates : List (Nat × Nat)
  invariants : List Nat
  issues : List (Nat × Nat)
  accepted : Bool
  deriving ToJson

def modelSnapshot (builder : ModelConstruction.Builder Nat Nat Nat Nat Nat) : ModelSnapshot :=
  ⟨builder.accounts, builder.rates, builder.invariants,
    builder.issues.map (fun issue => match issue with
      | .duplicateAccount id => (0, id) | .duplicateRate id => (1, id)),
    match builder.build with | .ok _ => true | .error _ => false⟩

structure ModelCase where
  steps : List (Nat × Nat × Nat)
  snapshots : List ModelSnapshot
  deriving ToJson

def modelCase (steps : List (Nat × Nat × Nat)) : ModelCase := Id.run do
  let mut builder : ModelConstruction.Builder Nat Nat Nat Nat Nat := .empty
  let mut snapshots := [modelSnapshot builder]
  for (kind, key, value) in steps do
    builder := match kind with
      | 0 => builder.addAccount key value
      | 1 => builder.addRate key value
      | _ => builder.addInvariant value
    snapshots := snapshots ++ [modelSnapshot builder]
  return ⟨steps, snapshots⟩

structure MetadataStep where
  kind : Nat
  role : Nat
  asset : Nat
  expression : Json
  right : Json
  label : String
  comparison : Nat
  deriving ToJson

structure MetadataSnapshot where
  roles : List Nat
  distinct : List (Nat × Nat)
  consumed : List (Nat × Nat × Json)
  produced : List (Nat × Nat × Json)
  preserved : List (Nat × Nat × Json)
  conditions : List (String × Json × Nat × Json)
  parameters : List String
  assets : List Nat
  deriving ToJson

structure MetadataCase where
  steps : List MetadataStep
  snapshots : List MetadataSnapshot
  deriving ToJson

def metadataCase (inputs : List (Nat × Nat × Nat × Evaluation.Expr Nat Nat Nat × Evaluation.Expr Nat Nat Nat)) : MetadataCase := Id.run do
  let mut roles := []
  let mut distinct := []
  let mut consumed := []
  let mut produced := []
  let mut preserved := []
  let mut conditions := []
  let mut steps := []
  let mut snapshots := [MetadataSnapshot.mk [] [] [] [] [] [] [] []]
  for (kind, role, asset, expression, right) in inputs do
    let label := "condition " ++ toString steps.length
    let comparison := steps.length % 3
    steps := steps ++ [⟨kind, role, asset, expressionJson expression, expressionJson right, label, comparison⟩]
    if kind < 3 then
      roles := Discovery.expressionRoles compare role expression roles
      if kind = 0 then consumed := consumed ++ [(role, asset, expression)]
      else if kind = 1 then produced := produced ++ [(role, asset, expression)]
      else preserved := preserved ++ [(role, asset, expression)]
    else if kind = 3 then
      roles := Discovery.conditionRoles compare expression right roles
      conditions := conditions ++ [(label, expression, comparison, right)]
    else
      roles := RateConstruction.register compare asset (RateConstruction.register compare role roles)
      distinct := RateConstruction.register (Discovery.pairCompare compare) (Discovery.orderedPair compare role asset) distinct
    let amounts := consumed ++ produced ++ preserved
    let parameterRefs := amounts.flatMap (fun (_, _, expression) => Discovery.parameters expression) ++
      conditions.flatMap (fun (_, left, _, right) => Discovery.parameters left ++ Discovery.parameters right)
    let assetRefs := amounts.flatMap (fun (_, asset, expression) => asset :: Discovery.assets expression) ++
      conditions.flatMap (fun (_, left, _, right) => Discovery.assets left ++ Discovery.assets right)
    let serialize := fun entries => entries.map fun (role, asset, expression) => (role, asset, expressionJson expression)
    snapshots := snapshots ++ [⟨roles, distinct, serialize consumed, serialize produced, serialize preserved,
      conditions.map (fun (label, left, comparison, right) => (label, expressionJson left, comparison, expressionJson right)),
      Discovery.registerAll compare parameterRefs [], assetRefs⟩]
  return ⟨steps, snapshots⟩

structure ValidationIssueCase where
  kind : String
  ids : List Nat := []
  parameter : String := ""
  deriving ToJson

def validationIssue (issue : Validation.Issue Nat Nat Nat) : ValidationIssueCase :=
  match issue with
  | .zeroUnits => ⟨"zeroUnits", [], ""⟩
  | .missingRate id => ⟨"missingRate", [id], ""⟩
  | .missingBinding role => ⟨"missingBinding", [role], ""⟩
  | .unknownBinding role => ⟨"unknownBinding", [role], ""⟩
  | .missingParameter parameter => ⟨"missingParameter", [], parameter⟩
  | .unknownParameter parameter => ⟨"unknownParameter", [], parameter⟩
  | .rolesMustDiffer left right => ⟨"rolesMustDiffer", [left, right], ""⟩
  | .missingAccount id => ⟨"missingAccount", [id], ""⟩

structure ValidationCase where
  roles : List Nat
  requiredParameters : List String
  distinct : List (Nat × Nat)
  units : String
  rate : Nat
  bindings : List (Nat × Nat)
  parameters : List (String × String)
  accounts : List Nat
  accepted : Bool
  issues : List ValidationIssueCase
  deriving ToJson

def validationCase (roles : List Nat) (requiredParameters : List String) (distinct : List (Nat × Nat))
    (units rate : Nat) (bindings : List (Nat × Nat)) (parameters : List (String × Nat))
    (accounts : List Nat) : ValidationCase :=
  let roles := Discovery.registerAll compare (roles ++ distinct.flatMap (fun (l, r) => [l, r])) []
  let names := Discovery.registerAll compare requiredParameters []
  let pairs := Discovery.registerAll (Discovery.pairCompare compare)
    (distinct.map fun (l, r) => Discovery.orderedPair compare l r) []
  let bindings := bindings.mergeSort (fun l r => l.1 ≤ r.1)
  let parameters := parameters.mergeSort (fun l r => l.1 ≤ r.1)
  let metadata : Validation.Metadata Nat := ⟨roles, names, pairs⟩
  let result := Validation.validate compare compare id units rate [(0, metadata), (2, ⟨[], [], []⟩)]
    bindings parameters (fun id => decide (id ∈ accounts))
  let (accepted, issues) := match result with
    | .ok _ => (true, [])
    | .error issues => (false, issues.map validationIssue)
  ⟨roles, names, pairs, toString units, rate, bindings,
    parameters.map (fun (key, value) => (key, toString value)), accounts, accepted, issues⟩

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
  let mut conservation : Array ConservationCase := #[]
  let coefficients : List Int := [-(2^63), -1, 0, 1, 2^63 - 1]
  for firstWeight in coefficients do
    for secondWeight in coefficients do
      for first in [0, 1, limit] do
        for second in [0, 1, limit] do
          for same in [false, true] do
            let consumed := Basket.canonical [(0, first), (1, second), (2, limit)]
            let produced := if same then consumed else Basket.canonical [(0, second), (1, first)]
            conservation := conservation.push (conservationCase [(0, firstWeight), (1, secondWeight)]
              [⟨0, consumed, produced, [(3, limit)]⟩])
  conservation := conservation.push (conservationCase [] [])
  for sign in [(1 : Int), -1] do
    let weights := [(0, sign * (2^63 - 1)), (1, sign * (2^63 - 1)), (2, -sign * (2^63 - 1))]
    for order in [[0, 1, 2], [0, 2, 1], [2, 0, 1]] do
      let entries := order.map (fun asset => (asset, limit))
      conservation := conservation.push (conservationCase weights [⟨0, entries, entries, []⟩])
      conservation := conservation.push (conservationCase weights
        (entries.map fun (asset, amount) => ⟨asset, [(asset, amount)], [(asset, amount)], []⟩))
  -- Negative products have a slightly larger magnitude at i64::MIN.
  conservation := conservation.push (conservationCase [(0, -(2^63)), (1, -(2^63))]
    [⟨0, [(0, limit)], [], []⟩, ⟨1, [(1, limit)], [], []⟩])
  let mut rateMerges : Array RateMergeCase := #[]
  for target in [[], [(0, [(0, 1)]), (2, [(1, limit)])]] do
    for role in [0, 1, 2, 3] do
      for assets in [[], [(0, 0)], [(0, 1)], [(1, 1)], [(0, limit)], [(1, limit)], [(0, 1), (1, 1)]] do
        rateMerges := rateMerges.push (rateMergeCase target role assets)
  let atoms : List (Evaluation.Expr Nat Nat Nat) :=
    [.constant 0, .constant 1, .constant limit, .units, .parameter "a", .parameter "éλ",
     .parameter "", .parameter "未知`", .balance 0 0, .balance 0 99,
     .balance 1 0, .balance 2 0, .balance 3 0]
  let mut evaluation : Array EvaluationCase := #[]
  for units in [0, 2] do
    for left in atoms do
      evaluation := evaluation.push (evaluationCase units left (.parameter "missing condition"))
      for right in atoms do
        for combine in combinators do
          evaluation := evaluation.push (evaluationCase units (combine left right) .units)
      for combine in combinators do
        evaluation := evaluation.push (evaluationCase units
          (combine (.binary .sub (.balance 0 0) left) (divide (.parameter "éλ") .units))
          (.balance 3 0))
      -- Full-width products, exact/inexact quotients, zero denominators,
      -- quotient overflow and an error in each operand position.
      let mulDivOperands : List (Evaluation.Expr Nat Nat Nat × Evaluation.Expr Nat Nat Nat) :=
        [(.constant limit, .constant limit), (.parameter "a", .constant 2), (.balance 0 0, .units),
         (.constant 1, .parameter "z"), (.constant 2, .constant 1), (.parameter "", .constant 1),
         (.constant 1, .balance 3 0), (.units, .balance 1 0)]
      for rounding in [MulDiv.Rounding.floor, .ceil] do
        for (right, denominator) in mulDivOperands do
          evaluation := evaluation.push (evaluationCase units (.mulDiv rounding left right denominator)
            (.parameter "a"))
        evaluation := evaluation.push (evaluationCase units
          (.binary .add (.mulDiv rounding (.balance 0 0) left (.parameter "éλ")) .units)
          (.mulDiv rounding left (.constant 3) (.constant 2)))
  let mut measurements : Array MeasureCase := #[]
  for firstWeight in coefficients do
    for secondWeight in coefficients do
      for first in [0, 1, limit] do
        for second in [0, 1, limit] do
          let weights := [(0, firstWeight), (1, secondWeight)]
          for split in [false, true] do
            measurements := measurements.push (measureCase weights
              (if split then [(0, Basket.canonical [(0, first)]), (1, Basket.canonical [(1, second)]), (2, [])]
               else [(0, Basket.canonical [(0, first), (1, second), (2, limit)]), (1, [])]))
  measurements := measurements.push (measureCase [] [])
  for sign in [(1 : Int), -1] do
    let weights := [(0, sign * (2^63 - 1)), (1, sign * (2^63 - 1)), (2, -sign * (2^63 - 1))]
    for order in [[0, 1, 2], [0, 2, 1], [2, 0, 1]] do
      let entries := order.map (fun asset => (asset, limit))
      measurements := measurements.push (measureCase weights [(0, entries)])
      measurements := measurements.push (measureCase weights (entries.map fun (asset, amount) => (asset, [(asset, amount)])))
  let mut effects : Array EffectsCase := #[]
  for before in [0, 1, limit] do
    for added in [0, 1, limit] do
      for units in [0, 1, 2, limit] do
        for asset in [0, 2] do
          effects := effects.push (effectsCase [(0, before), (1, limit), (3, 7)]
            [(0, added), (1, 1)] units asset added)
  effects := effects.push (effectsCase [] [] 0 0 0)
  let mut models : Array ModelCase := #[modelCase []]
  let options : List (Nat × Nat × Nat) := [(0, 0, 1), (0, 0, 9), (0, 1, 3),
    (1, 0, 2), (1, 0, 8), (1, 2, 0), (2, 0, 5)]
  for first in options do
    for second in options do
      for third in options do
        models := models.push (modelCase [first, second, third])
  models := models.push (modelCase [(0, 0, 1), (1, 0, 2), (1, 0, 7), (0, 0, 8), (1, 0, 9), (2, 0, 4), (0, 1, 3)])
  let mut metadata : Array MetadataCase := #[metadataCase []]
  let mut discoveryExpressions := atoms
  for combine in combinators do
    discoveryExpressions := discoveryExpressions ++
      [combine (.binary .add (.balance 2 3) (.parameter "éλ"))
        (.binary .max (.balance 0 99) (.binary .sub (.parameter "a") (.balance 2 3)))]
  for rounding in [MulDiv.Rounding.floor, .ceil] do
    discoveryExpressions := discoveryExpressions ++
      [.mulDiv rounding (.balance 2 3) (.parameter "éλ") (.binary .sub (.parameter "a") (.balance 0 99))]
  for expression in discoveryExpressions do
    for kind in [0, 1, 2, 3] do
      metadata := metadata.push (metadataCase [
        (4, 4, 1, .units, .units), (kind, 3, 5, expression, .parameter "未知`"),
        (0, 2, 7, .parameter "éλ", .units), (1, 1, 6, .balance 0 99, .units),
        (2, 2, 7, expression, .units), (3, 0, 0, expression, .binary .add (.parameter "") (.parameter "éλ")),
        (4, 1, 4, .units, .units), (4, 2, 2, .units, .units)])
  let mut validation : Array ValidationCase := #[]
  -- Structural errors accumulate in category order, except a missing rate
  -- stops validation after the units check. Aliased missing accounts deduplicate.
  for units in [0, 1, limit] do
    for rule in [0, 1, 2] do
      for bindings in [[], [(0, 0)], [(1, 1), (0, 0)], [(0, 0), (1, 0)],
          [(0, 9), (1, 9)], [(1, 8), (0, 9)], [(2, 7), (0, 0), (1, 1)]] do
        for parameters in [[], [("éλ", 1)], [("éλ", limit), ("", 0)],
            [("z", 3), ("未知`", 4), ("", 0)]] do
          for availableAccounts in [[], [0], [1, 0]] do
            validation := validation.push (validationCase [1, 0] ["éλ", "", "éλ"] [(1, 0)]
              units rule bindings parameters availableAccounts)
  for roles in [[], [0], [2, 0, 1]] do
    for pairs in [[], [(0, 0)], [(1, 0), (0, 1), (2, 0)]] do
      validation := validation.push (validationCase roles [] pairs 1 0
        (roles.map fun role => (role, role)) [] roles)
  let document := Json.mkObj [("version", toJson (1 : Nat)), ("scalars", toJson scalars),
    ("exchanges", toJson exchanges), ("expressions", toJson expressions), ("baskets", toJson baskets),
    ("accounts", toJson accounts), ("preparations", toJson preparations), ("basketOps", toJson basketOps),
    ("conservation", toJson conservation), ("rateMerges", toJson rateMerges),
    ("evaluation", toJson evaluation), ("measurements", toJson measurements), ("effects", toJson effects),
    ("models", toJson models), ("metadata", toJson metadata), ("validation", toJson validation),
    ("transactions", toJson (TransactionConformance.fixtures expressionJson)),
    ("executions", toJson (TransactionConformance.executionFixtures expressionJson)),
    ("basketCollections", toJson BasketCollectionConformance.fixtures),
    ("invariantInputs", toJson InvariantInputConformance.fixtures)]
  IO.println document.compress
