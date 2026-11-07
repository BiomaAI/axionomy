import Axionomy.TypedDataRefinement

namespace Axionomy.ExpressionConstructionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {Role A N : Type}

theorem constant (value : Std.U64) :
    rate.QuantityExpressionRoleAU64.constant Role A value = .ok (.Constant value) := by
  simp [rate.QuantityExpressionRoleAU64.constant, quantity.QuantityU64.new]

theorem units : rate.QuantityExpression.units Role A N = .ok .Units := rfl

theorem balance (role : Role) (asset : A) :
    rate.QuantityExpression.balance N role asset = .ok (.Balance role asset) := rfl

theorem parameter {T : Type} (conversion : core.convert.Into T String) (encode : T → String)
    (law : ∀ value, conversion.into value = .ok (encode value)) (value : T) :
    rate.QuantityExpression.parameter Role A N conversion value = .ok (.Parameter (encode value)) := by
  simp [rate.QuantityExpression.parameter, law value]

theorem add_operator (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.Insts.CoreOpsArithAddQuantityExpressionQuantityExpression.add left right =
      .ok (.Add left right) := rfl

def encode : Evaluation.Expr Role A N → rate.QuantityExpression Role A N
  | .constant value => .Constant value
  | .units => .Units
  | .parameter label => .Parameter label
  | .balance role asset => .Balance role asset
  | .binary op left right => match op with
    | .add => .Add (encode left) (encode right)
    | .sub => .Subtract (encode left) (encode right)
    | .mul => .Multiply (encode left) (encode right)
    | .div => .DivideFloor (encode left) (encode right)
    | .min => .Minimum (encode left) (encode right)
    | .max => .Maximum (encode left) (encode right)

/-- Build each node with the actual extracted public Rust constructor. -/
def construct : Evaluation.Expr Role A Std.U64 → Result (rate.QuantityExpression Role A Std.U64)
  | .constant value => rate.QuantityExpressionRoleAU64.constant Role A value
  | .units => rate.QuantityExpression.units Role A Std.U64
  | .parameter label => rate.QuantityExpression.parameter Role A Std.U64
      (core.convert.IntoFrom (core.convert.FromSame String)) label
  | .balance role asset => rate.QuantityExpression.balance Std.U64 role asset
  | .binary op left right => do
    let l ← construct left
    let r ← construct right
    match op with
    | .add => rate.QuantityExpression.plus l r
    | .sub => rate.QuantityExpression.subtract l r
    | .mul => rate.QuantityExpression.multiply l r
    | .div => rate.QuantityExpression.divide_floor l r
    | .min => rate.QuantityExpression.minimum l r
    | .max => rate.QuantityExpression.maximum l r

theorem construct_refines (expression : Evaluation.Expr Role A Std.U64) :
    construct expression = .ok (encode expression) := by
  induction expression with
  | constant value => simp [construct, constant, encode]
  | units => rfl
  | parameter label => simp [construct, rate.QuantityExpression.parameter, core.convert.IntoFrom.into, core.convert.FromSame.from, encode]
  | balance role asset => rfl
  | binary op left right ihLeft ihRight =>
    cases op <;> simp [construct, ihLeft, ihRight, encode, rate.QuantityExpression.plus,
      rate.QuantityExpression.subtract, rate.QuantityExpression.multiply, rate.QuantityExpression.divide_floor,
      rate.QuantityExpression.minimum, rate.QuantityExpression.maximum]

theorem decode_encode (expression : Evaluation.Expr Role A Std.U64) :
    EvaluationRefinement.decodeExpression (encode expression) = expression := by
  induction expression with
  | constant value => rfl
  | units => rfl
  | parameter label => rfl
  | balance role asset => rfl
  | binary op left right ihLeft ihRight =>
    cases op <;> simp [encode, EvaluationRefinement.decodeExpression, ihLeft, ihRight]

/-- Composing constructors with the already-proved evaluator preserves every
operator, operand position, identifier and checked-arithmetic failure. -/
theorem evaluate_constructed {AccountId RateId : Type} [DecidableEq AccountId] [DecidableEq A]
    (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
    (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (roleOrd : core.cmp.Ord Role)
    (roleCompare : Role → Role → Ordering) (hAccount : LawfulEquality accountEq)
    (hAsset : LawfulEquality assetEq) (hRole : LawfulOrdering roleOrd roleCompare)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64)) (expression : Evaluation.Expr Role A Std.U64) :
    (do let built ← construct expression; evaluate accountEq accountHash assetEq assetHash roleOrd built proposal accounts) =
      .ok (EvaluationRefinement.encodeResult (Evaluation.eval EvaluationRefinement.arithmetic
        (EvaluationRefinement.context roleCompare proposal accounts) expression)) := by
  simp only [construct_refines, bind_tc_ok,
    EvaluationRefinement.evaluate_refines accountEq accountHash assetEq assetHash roleOrd roleCompare
      hAccount hAsset hRole, decode_encode]

theorem plus (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.plus left right = .ok (.Add left right) := rfl

theorem subtract (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.subtract left right = .ok (.Subtract left right) := rfl

theorem multiply (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.multiply left right = .ok (.Multiply left right) := rfl

theorem divide_floor (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.divide_floor left right = .ok (.DivideFloor left right) := rfl

theorem minimum (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.minimum left right = .ok (.Minimum left right) := rfl

theorem maximum (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.maximum left right = .ok (.Maximum left right) := rfl

end Axionomy.ExpressionConstructionRefinement
