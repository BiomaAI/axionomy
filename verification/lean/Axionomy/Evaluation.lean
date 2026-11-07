import Axionomy.Rates
import Axionomy.Account
import Axionomy.MulDiv

namespace Axionomy.Evaluation

inductive Error where
  | missingParameter (name : String)
  | unboundRole
  | missingAccount
  | additionOverflow
  | subtractionUnderflow
  | multiplicationOverflow
  | divisionByZero
  deriving DecidableEq, Repr

def Error.message : Error → String
  | .missingParameter name => "missing parameter `" ++ name ++ "`"
  | .unboundRole => "expression references an unbound role"
  | .missingAccount => "expression references a missing account"
  | .additionOverflow => "addition overflow"
  | .subtractionUnderflow => "subtraction underflow"
  | .multiplicationOverflow => "multiplication overflow"
  | .divisionByZero => "division by zero"

structure Arithmetic (Q : Type) where
  add : Q → Q → Option Q
  sub : Q → Q → Option Q
  mul : Q → Q → Option Q
  div : Q → Q → Option Q
  min : Q → Q → Q
  max : Q → Q → Q
  isZero : Q → Bool
  mulDiv : MulDiv.Rounding → Q → Q → Q → Option Q

def naturals (limit : Nat) : Arithmetic Nat :=
  ⟨Quantity.add limit, Quantity.sub, Quantity.mul limit, Quantity.div, min, max,
    fun value => decide (value = 0), MulDiv.mulDiv limit⟩

def checked {Q : Type} (error : Error) : Option Q → Except Error Q
  | none => .error error
  | some value => .ok value

def combine {Q : Type} (operation : Q → Q → Except Error Q)
    (left right : Except Error Q) : Except Error Q :=
  match left with
  | .error error => .error error
  | .ok l => match right with
    | .error error => .error error
    | .ok r => operation l r

def operate {Q : Type} (ops : Arithmetic Q) : Rates.Op → Q → Q → Except Error Q
  | .add, l, r => checked .additionOverflow (ops.add l r)
  | .sub, l, r => checked .subtractionUnderflow (ops.sub l r)
  | .mul, l, r => checked .multiplicationOverflow (ops.mul l r)
  | .div, l, r => checked .divisionByZero (ops.div l r)
  | .min, l, r => .ok (ops.min l r)
  | .max, l, r => .ok (ops.max l r)

-- A zero denominator is reported before the full-width quotient is attempted;
-- a quotient outside the backend reports the multiplication overflow.
def operateMulDiv {Q : Type} (ops : Arithmetic Q) (rounding : MulDiv.Rounding)
    (l r d : Q) : Except Error Q :=
  if ops.isZero d then .error .divisionByZero
  else checked .multiplicationOverflow (ops.mulDiv rounding l r d)

inductive Expr (Role Asset Q : Type) where
  | constant (value : Q)
  | units
  | parameter (name : String)
  | balance (role : Role) (asset : Asset)
  | binary (op : Rates.Op) (left right : Expr Role Asset Q)
  | mulDiv (rounding : MulDiv.Rounding) (left right denominator : Expr Role Asset Q)

structure Context (Role Asset Q : Type) where
  units : Q
  parameter : String → Option Q
  balance : Role → Asset → Except Error Q

def orderedFind {K V : Type} (cmp : K → K → Ordering) (query : K) : List (K × V) → Option V
  | [] => none
  | (key, value) :: rest => match cmp query key with
    | .lt => none
    | .eq => some value
    | .gt => orderedFind cmp query rest

def fromState {AccountId Role Asset Q : Type} [DecidableEq AccountId] [DecidableEq Asset]
    (roleCompare : Role → Role → Ordering) (zero units : Q)
    (parameters : List (String × Q)) (bindings : List (Role × AccountId))
    (accounts : List (AccountId × List (Asset × Q))) : Context Role Asset Q where
  units := units
  parameter := fun name => orderedFind compare name parameters
  balance := fun role asset =>
    match orderedFind roleCompare role bindings with
    | none => .error .unboundRole
    | some id => match Account.find? id accounts with
      | none => .error .missingAccount
      | some entries => .ok (Account.balance zero entries asset)

-- Both operands read the same immutable context. The first failing operand
-- wins; arithmetic is attempted only after both operands succeed.
def eval {Role Asset Q : Type} (ops : Arithmetic Q) (context : Context Role Asset Q) :
    Expr Role Asset Q → Except Error Q
  | .constant value => .ok value
  | .units => .ok context.units
  | .parameter name => checked (.missingParameter name) (context.parameter name)
  | .balance role asset => context.balance role asset
  | .binary op left right =>
    match eval ops context left with
    | .error error => .error error
    | .ok l => match eval ops context right with
      | .error error => .error error
      | .ok r => operate ops op l r
  | .mulDiv rounding left right denominator =>
    match eval ops context left with
    | .error error => .error error
    | .ok l => match eval ops context right with
      | .error error => .error error
      | .ok r => match eval ops context denominator with
        | .error error => .error error
        | .ok d => operateMulDiv ops rounding l r d

inductive Comparison where
  | equal | greaterOrEqual | lessOrEqual
  deriving DecidableEq, Repr

def Comparison.test : Comparison → Nat → Nat → Bool
  | .equal, l, r => decide (l = r)
  | .greaterOrEqual, l, r => decide (r ≤ l)
  | .lessOrEqual, l, r => decide (l ≤ r)

def condition {Role Asset Q : Type} (ops : Arithmetic Q) (value : Q → Nat)
    (context : Context Role Asset Q) (comparison : Comparison)
    (left right : Expr Role Asset Q) : Except Error Bool :=
  match eval ops context left with
  | .error error => .error error
  | .ok l => match eval ops context right with
    | .error error => .error error
    | .ok r => .ok (comparison.test (value l) (value r))

theorem left_error_first {Role Asset Q : Type} (ops : Arithmetic Q)
    (context : Context Role Asset Q) (op : Rates.Op) (left right : Expr Role Asset Q) (error : Error)
    (failed : eval ops context left = .error error) :
    eval ops context (.binary op left right) = .error error := by
  simp [eval, failed]

theorem right_error_after_left {Role Asset Q : Type} (ops : Arithmetic Q)
    (context : Context Role Asset Q) (op : Rates.Op) (left right : Expr Role Asset Q)
    (value : Q) (error : Error) (succeeded : eval ops context left = .ok value)
    (failed : eval ops context right = .error error) :
    eval ops context (.binary op left right) = .error error := by
  simp [eval, succeeded, failed]

theorem mulDiv_operand_error_first {Role Asset Q : Type} (ops : Arithmetic Q)
    (context : Context Role Asset Q) (rounding : MulDiv.Rounding)
    (left right denominator : Expr Role Asset Q) (error : Error)
    (failed : eval ops context left = .error error ∨
      (∃ l, eval ops context left = .ok l ∧ eval ops context right = .error error) ∨
      (∃ l r, eval ops context left = .ok l ∧ eval ops context right = .ok r ∧
        eval ops context denominator = .error error)) :
    eval ops context (.mulDiv rounding left right denominator) = .error error := by
  rcases failed with failed | ⟨l, hl, failed⟩ | ⟨l, r, hl, hr, failed⟩ <;> simp [eval, *]

theorem mulDiv_zero_denominator {Role Asset Q : Type} (ops : Arithmetic Q)
    (context : Context Role Asset Q) (rounding : MulDiv.Rounding)
    (left right denominator : Expr Role Asset Q) (l r d : Q)
    (hl : eval ops context left = .ok l) (hr : eval ops context right = .ok r)
    (hd : eval ops context denominator = .ok d) (zero : ops.isZero d = true) :
    eval ops context (.mulDiv rounding left right denominator) = .error .divisionByZero := by
  simp [eval, hl, hr, hd, operateMulDiv, zero]

def mapResult {Q R : Type} (value : Q → R) : Except Error Q → Except Error R
  | .error error => .error error
  | .ok result => .ok (value result)

def Expr.map {Role Asset Q R : Type} (value : Q → R) : Expr Role Asset Q → Expr Role Asset R
  | .constant q => .constant (value q)
  | .units => .units
  | .parameter name => .parameter name
  | .balance role asset => .balance role asset
  | .binary op left right => .binary op (left.map value) (right.map value)
  | .mulDiv rounding left right denominator =>
    .mulDiv rounding (left.map value) (right.map value) (denominator.map value)

def Context.map {Role Asset Q R : Type} (value : Q → R) (context : Context Role Asset Q) :
    Context Role Asset R :=
  ⟨value context.units, fun name => (context.parameter name).map value,
    fun role asset => mapResult value (context.balance role asset)⟩

structure Hom {Q R : Type} (left : Arithmetic Q) (right : Arithmetic R) (value : Q → R) : Prop where
  add : ∀ l r, (left.add l r).map value = right.add (value l) (value r)
  sub : ∀ l r, (left.sub l r).map value = right.sub (value l) (value r)
  mul : ∀ l r, (left.mul l r).map value = right.mul (value l) (value r)
  div : ∀ l r, (left.div l r).map value = right.div (value l) (value r)
  min : ∀ l r, value (left.min l r) = right.min (value l) (value r)
  max : ∀ l r, value (left.max l r) = right.max (value l) (value r)
  isZero : ∀ q, left.isZero q = right.isZero (value q)
  mulDiv : ∀ rounding l r d,
    (left.mulDiv rounding l r d).map value = right.mulDiv rounding (value l) (value r) (value d)

theorem map_checked {Q R : Type} (value : Q → R) (error : Error) (result : Option Q) :
    mapResult value (checked error result) = checked error (result.map value) := by
  cases result <;> rfl

theorem map_operate {Q R : Type} (left : Arithmetic Q) (right : Arithmetic R)
    (value : Q → R) (hom : Hom left right value) (op : Rates.Op) (l r : Q) :
    mapResult value (operate left op l r) = operate right op (value l) (value r) := by
  cases op with
  | add => simp only [operate, map_checked, hom.add]
  | sub => simp only [operate, map_checked, hom.sub]
  | mul => simp only [operate, map_checked, hom.mul]
  | div => simp only [operate, map_checked, hom.div]
  | min => simp [operate, mapResult, hom.min]
  | max => simp [operate, mapResult, hom.max]

theorem map_operateMulDiv {Q R : Type} (left : Arithmetic Q) (right : Arithmetic R)
    (value : Q → R) (hom : Hom left right value) (rounding : MulDiv.Rounding) (l r d : Q) :
    mapResult value (operateMulDiv left rounding l r d) =
      operateMulDiv right rounding (value l) (value r) (value d) := by
  simp only [operateMulDiv, ← hom.isZero]
  split
  · rfl
  · simp only [map_checked, hom.mulDiv]

theorem map_eval {Role Asset Q R : Type} (left : Arithmetic Q) (right : Arithmetic R)
    (value : Q → R) (hom : Hom left right value)
    (context : Context Role Asset Q) (expression : Expr Role Asset Q) :
    mapResult value (eval left context expression) =
      eval right (context.map value) (expression.map value) := by
  induction expression with
  | constant q => rfl
  | units => rfl
  | parameter name => simp [eval, Expr.map, Context.map, map_checked]
  | balance role asset => rfl
  | binary op l r ihl ihr =>
    simp only [eval, Expr.map, ← ihl, ← ihr]
    cases hl : eval left context l <;> cases hr : eval left context r <;>
      first | rfl | exact map_operate left right value hom op _ _
  | mulDiv rounding l r d ihl ihr ihd =>
    simp only [eval, Expr.map, ← ihl, ← ihr, ← ihd]
    cases hl : eval left context l <;> cases hr : eval left context r <;>
      cases hd : eval left context d <;>
      first | rfl | exact map_operateMulDiv left right value hom rounding _ _ _

theorem map_condition {Role Asset Q : Type} (ops : Arithmetic Q) (other : Arithmetic Nat)
    (value : Q → Nat) (hom : Hom ops other value) (context : Context Role Asset Q)
    (comparison : Comparison) (left right : Expr Role Asset Q) :
    condition ops value context comparison left right =
      condition other id (context.map value) comparison (left.map value) (right.map value) := by
  simp only [condition, ← map_eval ops other value hom]
  cases eval ops context left <;> cases eval ops context right <;> rfl

end Axionomy.Evaluation
