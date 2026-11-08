import Axionomy.Evaluation
import Axionomy.RateConstructionRefinement
import Axionomy.MulDivRefinement

namespace Axionomy.EvaluationRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

def arithmetic : Evaluation.Arithmetic Std.U64 :=
  ⟨U64.checked_add, U64.checked_sub, U64.checked_mul,
    fun l r => if l.val ≤ r.val then l else r,
    fun l r => if l.val ≤ r.val then r else l,
    fun value => decide (value = 0#u64), MulDivRefinement.mulDiv⟩

theorem sub_exact (left right : Std.U64) :
    (U64.checked_sub left right).map UScalar.val = Quantity.sub left.val right.val := by
  have spec := U64.checked_sub_bv_spec left right
  cases h : U64.checked_sub left right with
  | none =>
    simp only [h] at spec
    simp [Quantity.sub, Nat.not_le_of_gt spec]
  | some result =>
    simp only [h] at spec
    simp [Quantity.sub, spec.1, spec.2.1]

theorem natural_hom : Evaluation.Hom arithmetic (Evaluation.naturals Quantity.u64Max) UScalar.val where
  add := AccountRefinement.add_exact
  sub := sub_exact
  mul := BasketOperationsRefinement.mul_exact
  min := by intro l r; by_cases h : l.val ≤ r.val <;> simp [arithmetic, Evaluation.naturals, h, Nat.min_def]
  max := by intro l r; by_cases h : l.val ≤ r.val <;> simp [arithmetic, Evaluation.naturals, h, Nat.max_def]
  isZero := by intro q; simp [arithmetic, Evaluation.naturals, UScalar.eq_equiv]
  mulDiv := MulDivRefinement.mulDiv_val

@[simp] theorem sub_call (left right : Std.U64) :
    quantity.Quantity.checked_sub scalar left right = .ok (U64.checked_sub left right) := by
  simp only [quantity.Quantity.checked_sub, U64.Insts.Num_traitsOpsCheckedCheckedSub.checked_sub, bind_ok]
  cases h : U64.checked_sub left right <;>
    simp [quantity.Quantity.from_checked, quantity.Quantity.try_from_scalar,
      U64.Insts.Account_extractionQuantityQuantityScalarI128.is_nonnegative]

def encodeError : Evaluation.Error → economy.ExpressionError
  | .missingParameter parameter => .MissingParameter parameter
  | .unboundRole => .UnboundRole
  | .missingAccount => .MissingAccount
  | .additionOverflow => .AdditionOverflow
  | .subtractionUnderflow => .SubtractionUnderflow
  | .multiplicationOverflow => .MultiplicationOverflow
  | .divisionByZero => .DivisionByZero

def encodeResult {Q : Type} (result : Except Evaluation.Error Q) :
    core.result.Result Q economy.ExpressionError :=
  match result with
  | .error error => .Err (encodeError error)
  | .ok value => .Ok value

def decodeExpression {Role A : Type} : rate.QuantityExpression Role A Std.U64 → Evaluation.Expr Role A Std.U64
  | .Constant value => .constant value
  | .Units => .units
  | .Parameter parameter => .parameter parameter
  | .Balance role asset => .balance role asset
  | .Add left right => .binary .add (decodeExpression left) (decodeExpression right)
  | .Subtract left right => .binary .sub (decodeExpression left) (decodeExpression right)
  | .Multiply left right => .binary .mul (decodeExpression left) (decodeExpression right)
  | .Minimum left right => .binary .min (decodeExpression left) (decodeExpression right)
  | .Maximum left right => .binary .max (decodeExpression left) (decodeExpression right)
  | .MulDiv left right denominator rounding =>
    .mulDiv (MulDivRefinement.decodeRounding rounding) (decodeExpression left)
      (decodeExpression right) (decodeExpression denominator)

open Evaluation (orderedFind)

theorem tree_get_exact {K V : Type} (ordInst : core.cmp.Ord K) (cmp : K → K → Ordering)
    (lawful : RateConstructionRefinement.LawfulOrdering ordInst cmp) (query : K) (entries : List (K × V)) :
    treeGet (core.borrow.Borrow.Blanket K).borrow ordInst.cmp query entries =
      .ok (orderedFind cmp query entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases h : cmp query key <;>
      simp [treeGet, core.borrow.Borrow.Blanket.borrow, lawful query key, h, orderedFind, ih]

@[simp] theorem quantity_less (left right : Std.U64) :
    core.cmp.PartialOrd.lt.default
      (quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp core.cmp.PartialOrdU64)
      left right = .ok (decide (left.val < right.val)) := AccountRefinement.lt_call left right

@[simp] theorem min_call (left right : Std.U64) :
    core.cmp.min (quantity.Quantity.Insts.CoreCmpOrd core.cmp.OrdU64) left right =
      .ok (arithmetic.min left right) := by
  simp only [core.cmp.min, core.cmp.Ord.min.default, core.cmp.Ord.min_body, quantity_less, bind_ok]
  by_cases h : left.val ≤ right.val
  · simp [arithmetic, h, Nat.not_lt.mpr h]
  · simp [arithmetic, h, Nat.lt_of_not_ge h]

@[simp] theorem max_call (left right : Std.U64) :
    core.cmp.max (quantity.Quantity.Insts.CoreCmpOrd core.cmp.OrdU64) left right =
      .ok (arithmetic.max left right) := by
  simp only [core.cmp.max, core.cmp.Ord.max.default, core.cmp.Ord.max_body, quantity_less, bind_ok]
  by_cases h : left.val ≤ right.val
  · simp [arithmetic, h, Nat.not_lt.mpr h]
  · simp [arithmetic, h, Nat.lt_of_not_ge h]

section Interpreter
variable {AccountId Role A RateId : Type} [DecidableEq AccountId] [DecidableEq A]
variable (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
variable (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (roleOrd : core.cmp.Ord Role)
variable (roleCompare : Role → Role → Ordering)
variable (hAccount : LawfulEquality accountEq) (hAsset : LawfulEquality assetEq)
variable (hRole : RateConstructionRefinement.LawfulOrdering roleOrd roleCompare)

def context (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64)) : Evaluation.Context Role A Std.U64 :=
  Evaluation.fromState roleCompare 0#u64 proposal.units proposal.parameters proposal.bindings
    (Account.mapEntries (fun state => state.balances.quantities) accounts)

omit [DecidableEq AccountId] [DecidableEq A] in
theorem binary_call {F : Type} (operationInst : core.ops.function.FnOnce F (Std.U64 × Std.U64) (Option Std.U64))
    (operation : F) (mathematical : Std.U64 → Std.U64 → Option Std.U64)
    (operationLaw : ∀ l r, operationInst.call_once operation (l, r) = .ok (mathematical l r))
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (left right : rate.QuantityExpression Role A Std.U64)
    (leftResult rightResult : Except Evaluation.Error Std.U64) (error : Evaluation.Error)
    (hl : economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar left proposal accounts = .ok (encodeResult leftResult))
    (hr : economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar right proposal accounts = .ok (encodeResult rightResult)) :
    economy.evaluate_binary accountEq accountHash assetEq assetHash roleOrd scalar operationInst
      left right proposal accounts (encodeError error) operation =
      .ok (encodeResult (Evaluation.combine
        (fun l r => Evaluation.checked error (mathematical l r)) leftResult rightResult)) := by
  rw [economy.evaluate_binary.eq_def]
  cases leftResult <;> cases rightResult <;>
    simp [Evaluation.combine, hl, hr, encodeResult, operationLaw, core.option.Option.ok_or,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
      core.convert.FromSame.from]
  split <;> simp_all [Evaluation.checked]

def mulDivResult (rounding : MulDiv.Rounding)
    (left right denominator : Except Evaluation.Error Std.U64) : Except Evaluation.Error Std.U64 :=
  match left with
  | .error error => .error error
  | .ok l => match right with
    | .error error => .error error
    | .ok r => match denominator with
      | .error error => .error error
      | .ok d => Evaluation.operateMulDiv arithmetic rounding l r d

omit [DecidableEq AccountId] [DecidableEq A] in
theorem mulDiv_call (rounding : quantity.Rounding)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (left right denominator : rate.QuantityExpression Role A Std.U64)
    (leftResult rightResult denominatorResult : Except Evaluation.Error Std.U64)
    (hl : economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar left proposal accounts = .ok (encodeResult leftResult))
    (hr : economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar right proposal accounts = .ok (encodeResult rightResult))
    (hd : economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar denominator proposal accounts = .ok (encodeResult denominatorResult)) :
    economy.evaluate_mul_div accountEq accountHash assetEq assetHash roleOrd scalar
      left right denominator rounding proposal accounts =
      .ok (encodeResult (mulDivResult (MulDivRefinement.decodeRounding rounding)
        leftResult rightResult denominatorResult)) := by
  rw [economy.evaluate_mul_div.eq_def]
  cases leftResult <;> cases rightResult <;> cases denominatorResult <;>
    simp [mulDivResult, hl, hr, hd, encodeResult, core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
      core.convert.FromSame.from]
  rename_i l r d
  by_cases zero : d = 0#u64
  · simp [zero, Evaluation.operateMulDiv, arithmetic, AccountRefinement.arithmetic, encodeError]
  · simp [zero, Evaluation.operateMulDiv, arithmetic, AccountRefinement.arithmetic,
      core.option.Option.ok_or]
    split <;> simp_all [Evaluation.checked, encodeResult, encodeError]

include hAccount hAsset hRole in
theorem eval_impl (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (expression : rate.QuantityExpression Role A Std.U64) :
    economy.evaluate_expression accountEq accountHash assetEq assetHash roleOrd scalar expression proposal accounts =
      .ok (encodeResult (Evaluation.eval arithmetic (context roleCompare proposal accounts)
        (decodeExpression expression))) := by
  induction expression with
  | Constant value =>
    rw [economy.evaluate_expression.eq_def]
    simp [quantity.Quantity.Insts.CoreCloneClone.clone, decodeExpression, Evaluation.eval, encodeResult]
  | Units =>
    rw [economy.evaluate_expression.eq_def]
    simp [exchange.Exchange.impl.units, quantity.Quantity.Insts.CoreCloneClone.clone,
      decodeExpression, Evaluation.eval, encodeResult, context, Evaluation.fromState]
  | Parameter parameter =>
    rw [economy.evaluate_expression.eq_def]
    have stringOrdering : RateConstructionRefinement.LawfulOrdering alloc.string.String.Insts.CoreCmpOrd compare := by
      intro x y; rfl
    simp only [exchange.Exchange.impl.parameters, bind_ok, alloc.collections.btree.map.BTreeMap.get,
      tree_get_exact alloc.string.String.Insts.CoreCmpOrd compare stringOrdering]
    cases h : orderedFind compare parameter proposal.parameters <;>
      simp [h, core.option.OptionShared0T.cloned, quantity.Quantity.Insts.CoreCloneClone.clone,
        core.option.Option.ok_or_else,
        economy.evaluate_expression.closure.Insts.CoreOpsFunctionFnOnceTupleExpressionError.call_once,
        alloc.string.String.Insts.CoreCloneClone.clone, context, Evaluation.fromState,
        Evaluation.eval, decodeExpression, Evaluation.checked, encodeResult, encodeError]
  | Balance role asset =>
    rw [economy.evaluate_expression.eq_def]
    simp only [exchange.Exchange.impl.bindings, bind_ok, alloc.collections.btree.map.BTreeMap.get,
      tree_get_exact roleOrd roleCompare hRole]
    cases hb : orderedFind roleCompare role proposal.bindings with
    | none =>
      simp [hb, core.option.Option.ok_or, core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
        core.convert.FromSame.from, context, Evaluation.fromState, Evaluation.eval,
        decodeExpression, encodeResult, encodeError]
    | some id =>
      simp only [core.option.Option.ok_or, core.result.Result.Insts.CoreOpsTry.branch,
        bind_ok, indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccount]
      cases ha : Account.find? id accounts <;>
        simp [ha, hb,
          core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
          core.convert.FromSame.from, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
          account.Account.balance, quantity_call assetEq assetHash hAsset,
          context, Evaluation.fromState, Evaluation.eval, decodeExpression,
          encodeResult, encodeError, ← Account.map_find, AccountRefinement.arithmetic]
  | Add left right ihl ihr =>
    rw [economy.evaluate_expression.eq_def]
    let inst := economy.evaluate_expression.closure_1.Insts.CoreOpsFunctionFnOncePairShared0QuantityShared1QuantityOptionQuantity
      RateId accountEq accountHash assetEq assetHash roleOrd scalar
    have operationLaw (l r : Std.U64) : inst.call_once () (l, r) = .ok (U64.checked_add l r) := by
      simp [inst, economy.evaluate_expression.closure_1.Insts.CoreOpsFunctionFnOncePairShared0QuantityShared1QuantityOptionQuantity.call_once,
        AccountRefinement.add_call, AccountRefinement.arithmetic]
    have step := binary_call accountEq accountHash assetEq assetHash roleOrd inst () U64.checked_add
      operationLaw proposal accounts left right _ _ .additionOverflow ihl ihr
    cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression left) <;>
      cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression right) <;>
      simpa [inst, decodeExpression, Evaluation.eval, Evaluation.operate, Evaluation.combine, arithmetic, encodeError, hl, hr] using step
  | Subtract left right ihl ihr =>
    rw [economy.evaluate_expression.eq_def]
    let inst := economy.evaluate_expression.closure_2.Insts.CoreOpsFunctionFnOncePairShared0QuantityShared1QuantityOptionQuantity
      RateId accountEq accountHash assetEq assetHash roleOrd scalar
    have operationLaw (l r : Std.U64) : inst.call_once () (l, r) = .ok (U64.checked_sub l r) := by
      simp [inst, economy.evaluate_expression.closure_2.Insts.CoreOpsFunctionFnOncePairShared0QuantityShared1QuantityOptionQuantity.call_once,
        sub_call]
    have step := binary_call accountEq accountHash assetEq assetHash roleOrd inst () U64.checked_sub
      operationLaw proposal accounts left right _ _ .subtractionUnderflow ihl ihr
    cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression left) <;>
      cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression right) <;>
      simpa [inst, decodeExpression, Evaluation.eval, Evaluation.operate, Evaluation.combine, arithmetic, encodeError, hl, hr] using step
  | Multiply left right ihl ihr =>
    rw [economy.evaluate_expression.eq_def]
    let inst := economy.evaluate_expression.closure_3.Insts.CoreOpsFunctionFnOncePairShared0QuantityShared1QuantityOptionQuantity
      RateId accountEq accountHash assetEq assetHash roleOrd scalar
    have operationLaw (l r : Std.U64) : inst.call_once () (l, r) = .ok (U64.checked_mul l r) := by
      simp [inst, economy.evaluate_expression.closure_3.Insts.CoreOpsFunctionFnOncePairShared0QuantityShared1QuantityOptionQuantity.call_once,
        BasketOperationsRefinement.mul_call]
    have step := binary_call accountEq accountHash assetEq assetHash roleOrd inst () U64.checked_mul
      operationLaw proposal accounts left right _ _ .multiplicationOverflow ihl ihr
    cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression left) <;>
      cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression right) <;>
      simpa [inst, decodeExpression, Evaluation.eval, Evaluation.operate, Evaluation.combine, arithmetic, encodeError, hl, hr] using step
  | Minimum left right ihl ihr =>
    rw [economy.evaluate_expression.eq_def]
    simp only [ihl, ihr]
    cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression left) <;>
      cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression right) <;>
      simp [hl, hr, encodeResult, core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
        core.convert.FromSame.from, Evaluation.eval, decodeExpression, Evaluation.operate]
    rename_i l r
    by_cases h : l.val ≤ r.val
    · simp [core.cmp.Ord.min.default, core.cmp.Ord.min_body, arithmetic, h, Nat.not_lt.mpr h]
    · simp [core.cmp.Ord.min.default, core.cmp.Ord.min_body, arithmetic, h, Nat.lt_of_not_ge h]
  | Maximum left right ihl ihr =>
    rw [economy.evaluate_expression.eq_def]
    simp only [ihl, ihr]
    cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression left) <;>
      cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression right) <;>
      simp [hl, hr, encodeResult, core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
        core.convert.FromSame.from, Evaluation.eval, decodeExpression, Evaluation.operate]
    rename_i l r
    by_cases h : l.val ≤ r.val
    · simp [core.cmp.Ord.max.default, core.cmp.Ord.max_body, arithmetic, h, Nat.not_lt.mpr h]
    · simp [core.cmp.Ord.max.default, core.cmp.Ord.max_body, arithmetic, h, Nat.lt_of_not_ge h]
  | MulDiv left right denominator rounding ihl ihr ihd =>
    rw [economy.evaluate_expression.eq_def]
    have step := mulDiv_call accountEq accountHash assetEq assetHash roleOrd rounding
      proposal accounts left right denominator _ _ _ ihl ihr ihd
    cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression left) <;>
      cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression right) <;>
      cases hd : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression denominator) <;>
      simpa [decodeExpression, Evaluation.eval, mulDivResult, hl, hr, hd] using step

include hAccount hAsset hRole in
theorem evaluate_refines (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (expression : rate.QuantityExpression Role A Std.U64) :
    evaluate accountEq accountHash assetEq assetHash roleOrd expression proposal accounts =
      .ok (encodeResult (Evaluation.eval arithmetic (context roleCompare proposal accounts)
        (decodeExpression expression))) :=
  eval_impl accountEq accountHash assetEq assetHash roleOrd roleCompare hAccount hAsset hRole proposal accounts expression

include hAccount hAsset hRole in
theorem evaluate_refines_naturals (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (expression : rate.QuantityExpression Role A Std.U64) :
    ∃ result, evaluate accountEq accountHash assetEq assetHash roleOrd expression proposal accounts =
      .ok (encodeResult result) ∧
      Evaluation.mapResult UScalar.val result =
        Evaluation.eval (Evaluation.naturals Quantity.u64Max)
          ((context roleCompare proposal accounts).map UScalar.val)
          ((decodeExpression expression).map UScalar.val) := by
  refine ⟨_, evaluate_refines accountEq accountHash assetEq assetHash roleOrd roleCompare
    hAccount hAsset hRole proposal accounts expression, ?_⟩
  exact Evaluation.map_eval arithmetic (Evaluation.naturals Quantity.u64Max) UScalar.val
    natural_hom _ _

def decodeComparison : rate.QuantityComparison → Evaluation.Comparison
  | .Equal => .equal
  | .GreaterThanOrEqual => .greaterOrEqual
  | .LessThanOrEqual => .lessOrEqual

include hAccount hAsset hRole in
theorem condition_refines (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (condition : rate.RateCondition Role A Std.U64) :
    evaluate_condition accountEq accountHash assetEq assetHash roleOrd condition proposal accounts =
      .ok (encodeResult (Evaluation.condition arithmetic UScalar.val
        (context roleCompare proposal accounts) (decodeComparison condition.comparison)
        (decodeExpression condition.left) (decodeExpression condition.right))) := by
  simp only [evaluate_condition, economy.evaluate_condition, rate.RateCondition.impl.left,
    rate.RateCondition.impl.right, rate.RateCondition.impl.comparison, bind_ok,
    eval_impl accountEq accountHash assetEq assetHash roleOrd roleCompare hAccount hAsset hRole]
  cases hl : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression condition.left) <;>
    cases hr : Evaluation.eval arithmetic (context roleCompare proposal accounts) (decodeExpression condition.right) <;>
    simp [hl, hr, encodeResult, Evaluation.condition, core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
  rename_i l r
  have eqVal : l = r ↔ l.val = r.val := ⟨congrArg UScalar.val, UScalar.eq_of_val_eq⟩
  cases condition.comparison <;>
    simp [decodeComparison, Evaluation.Comparison.test, eqVal,
      quantity.Quantity.Insts.CoreCmpPartialEqQuantity.eq,
      core.cmp.PartialOrd.ge.trait_default, core.cmp.PartialOrd.le.trait_default,
      core.cmp.PartialOrd.ge.default, core.cmp.PartialOrd.le.default,
      core.cmp.PartialOrd.ge_body, core.cmp.PartialOrd.le_body,
      core.cmp.impls.PartialOrdU64.partial_cmp, Nat.compare_eq_lt,
      Nat.compare_eq_gt,
      quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp]
  all_goals
    cases hc : compare l.val r.val <;>
      simp_all [Nat.compare_eq_lt, Nat.compare_eq_gt] <;> omega

include hAccount hAsset hRole in
theorem condition_refines_naturals (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (accounts : List (AccountId × account.Account A Std.U64))
    (condition : rate.RateCondition Role A Std.U64) :
    evaluate_condition accountEq accountHash assetEq assetHash roleOrd condition proposal accounts =
      .ok (encodeResult (Evaluation.condition (Evaluation.naturals Quantity.u64Max) id
        ((context roleCompare proposal accounts).map UScalar.val) (decodeComparison condition.comparison)
        ((decodeExpression condition.left).map UScalar.val)
        ((decodeExpression condition.right).map UScalar.val))) := by
  rw [← Evaluation.map_condition arithmetic _ UScalar.val natural_hom]
  exact condition_refines accountEq accountHash assetEq assetHash roleOrd roleCompare
    hAccount hAsset hRole proposal accounts condition

end Interpreter
end Axionomy.EvaluationRefinement
