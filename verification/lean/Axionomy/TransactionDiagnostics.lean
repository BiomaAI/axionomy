import Axionomy.TransactionDecode
import Axionomy.DiagnosticRefinement

namespace Axionomy.TransactionDiagnostics
open Aeneas Aeneas.Std AccountExtracted

-- Bounds refer only to strings present in the expression, including the fixed
-- diagnostic prefix/suffix. They do not assume evaluation succeeds.
def MessageNamesFit {Role A Q : Type} (expression : Evaluation.Expr Role A Q) : Prop :=
  ∀ key ∈ Discovery.parameters expression, (Evaluation.Error.missingParameter key).message.toByteArray.size ≤ Usize.max

theorem operation_missing_impossible {Q : Type} (ops : Evaluation.Arithmetic Q) (op : Rates.Op)
    (left right : Q) (key : String) : Evaluation.operate ops op left right ≠ .error (.missingParameter key) := by
  cases op <;> simp only [Evaluation.operate]
  all_goals try (unfold Evaluation.checked; split)
  all_goals simp

theorem eval_parameter_source {Role A Q : Type} (ops : Evaluation.Arithmetic Q)
    (context : Evaluation.Context Role A Q)
    (balanceErrors : ∀ role asset key, context.balance role asset ≠ .error (.missingParameter key))
    (expression : Evaluation.Expr Role A Q) (key : String)
    (failed : Evaluation.eval ops context expression = .error (.missingParameter key)) :
    key ∈ Discovery.parameters expression := by
  induction expression with
  | constant value => simp [Evaluation.eval] at failed
  | units => simp [Evaluation.eval] at failed
  | parameter parameter =>
    cases found : context.parameter parameter with
    | none => simpa [Evaluation.eval, Evaluation.checked, found, Discovery.parameters, eq_comm] using failed
    | some value => simp [Evaluation.eval, Evaluation.checked, found] at failed
  | balance role asset => exact False.elim (balanceErrors role asset key failed)
  | binary op left right ihl ihr =>
    cases leftResult : Evaluation.eval ops context left with
    | error error =>
      have same : error = .missingParameter key := by simpa [Evaluation.eval, leftResult] using failed
      exact List.mem_append_left _ (ihl (same ▸ leftResult))
    | ok l =>
      cases rightResult : Evaluation.eval ops context right with
      | error error =>
        have same : error = .missingParameter key := by simpa [Evaluation.eval, leftResult, rightResult] using failed
        exact List.mem_append_right _ (ihr (same ▸ rightResult))
      | ok r =>
        exact False.elim (operation_missing_impossible ops op l r key
          (by simpa [Evaluation.eval, leftResult, rightResult] using failed))

theorem condition_parameter_source {Role A Q : Type} (ops : Evaluation.Arithmetic Q) (value : Q → Nat)
    (context : Evaluation.Context Role A Q)
    (balanceErrors : ∀ role asset key, context.balance role asset ≠ .error (.missingParameter key))
    (comparison : Evaluation.Comparison) (left right : Evaluation.Expr Role A Q) (key : String)
    (failed : Evaluation.condition ops value context comparison left right = .error (.missingParameter key)) :
    key ∈ Discovery.parameters left ++ Discovery.parameters right := by
  cases leftResult : Evaluation.eval ops context left with
  | error error =>
    have same : error = .missingParameter key := by simpa [Evaluation.condition, leftResult] using failed
    exact List.mem_append_left _ (eval_parameter_source ops context balanceErrors left key (same ▸ leftResult))
  | ok l =>
    cases rightResult : Evaluation.eval ops context right with
    | error error =>
      have same : error = .missingParameter key := by simpa [Evaluation.condition, leftResult, rightResult] using failed
      exact List.mem_append_right _ (eval_parameter_source ops context balanceErrors right key (same ▸ rightResult))
    | ok r => simp [Evaluation.condition, leftResult, rightResult] at failed

theorem state_balance_errors {AccountId Role A Q : Type} [DecidableEq AccountId] [DecidableEq A]
    (cmp : Role → Role → Ordering) (zero units : Q) (parameters : List (String × Q))
    (bindings : List (Role × AccountId)) (accounts : List (AccountId × List (A × Q))) :
    ∀ role asset key, (Evaluation.fromState cmp zero units parameters bindings accounts).balance role asset ≠
      .error (.missingParameter key) := by
  intro role asset key
  simp only [Evaluation.fromState]
  split <;> simp
  split <;> simp

theorem condition_error_fits {Role A Q : Type} (ops : Evaluation.Arithmetic Q) (value : Q → Nat)
    (context : Evaluation.Context Role A Q)
    (balanceErrors : ∀ role asset key, context.balance role asset ≠ .error (.missingParameter key))
    (comparison : Evaluation.Comparison) (left right : Evaluation.Expr Role A Q)
    (leftNames : MessageNamesFit left) (rightNames : MessageNamesFit right) (error : Evaluation.Error)
    (failed : Evaluation.condition ops value context comparison left right = .error error) :
    error.message.toByteArray.size ≤ Usize.max := by
  cases error with
  | missingParameter key =>
    have source := condition_parameter_source ops value context balanceErrors comparison left right key failed
    rcases List.mem_append.mp source with fromLeft | fromRight
    · exact leftNames key fromLeft
    · exact rightNames key fromRight
  | unboundRole | missingAccount | additionOverflow | subtractionUnderflow | multiplicationOverflow | divisionByZero =>
    calc
      _ ≤ 64 := by decide +kernel
      _ ≤ Usize.max := by scalar_tac

theorem expression_error_fits {Role A Q : Type} (ops : Evaluation.Arithmetic Q)
    (context : Evaluation.Context Role A Q)
    (balanceErrors : ∀ role asset key, context.balance role asset ≠ .error (.missingParameter key))
    (expression : Evaluation.Expr Role A Q) (names : MessageNamesFit expression) (error : Evaluation.Error)
    (failed : Evaluation.eval ops context expression = .error error) :
    error.message.toByteArray.size ≤ Usize.max := by
  cases error with
  | missingParameter key => exact names key (eval_parameter_source ops context balanceErrors expression key failed)
  | unboundRole | missingAccount | additionOverflow | subtractionUnderflow | multiplicationOverflow | divisionByZero =>
    calc
      _ ≤ 64 := by decide +kernel
      _ ≤ Usize.max := by scalar_tac

end Axionomy.TransactionDiagnostics
