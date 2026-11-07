import Axionomy.TransactionDiagnostics
import Axionomy.ConservationRefinement

namespace Axionomy.ConditionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement TransactionDiagnostics

variable {AccountId A RateId Role : Type}

def NamesFit (value : rate.RateCondition Role A Std.U64) : Prop :=
  value.name.toByteArray.size ≤ Usize.max ∧
    MessageNamesFit (TransactionDecode.expression value.left) ∧ MessageNamesFit (TransactionDecode.expression value.right)

def context [DecidableEq AccountId] [DecidableEq A] (cmp : Role → Role → Ordering)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (accounts : List (AccountId × account.Account A Std.U64)) :
    Evaluation.Context Role A Nat :=
  Evaluation.fromState cmp 0 (TransactionDecode.proposal proposal).units
    (TransactionDecode.proposal proposal).parameters (TransactionDecode.proposal proposal).bindings (TransactionDecode.accounts accounts)

section Conditions
variable [DecidableEq AccountId] [DecidableEq A]
variable (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
variable (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (rateClone : core.clone.Clone RateId) (roleOrd : core.cmp.Ord Role) (cmp : Role → Role → Ordering)
variable (hAccount : LawfulEquality accountEq) (hAsset : LawfulEquality assetEq)
variable (hRate : LawfulClone rateClone) (hRole : RateConstructionRefinement.LawfulOrdering roleOrd cmp)

include hAccount hAsset hRate hRole in
theorem conditions_loop_refines (remaining : List (rate.RateCondition Role A Std.U64))
    (iter : core.slice.iter.Iter (rate.RateCondition Role A Std.U64)) (suffix : iter.slice.val.drop iter.i = remaining)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (fits : ∀ value ∈ remaining, NamesFit value) :
    ∃ result, economy.Economy.check_conditions_loop accountEq accountHash assetEq assetHash rateClone roleOrd scalar iter model proposal = .ok result ∧
      TransactionDecode.checked id result = Transaction.liftIssue
        (Transaction.checkConditions proposal.rate (context cmp proposal model.accounts) (remaining.map TransactionDecode.condition)) := by
  induction remaining generalizing iter with
  | nil =>
    refine ⟨.Ok (), ?_, rfl⟩
    unfold economy.Economy.check_conditions_loop
    rw [loop.eq_1]
    simp [economy.Economy.check_conditions_loop.body, ConservationRefinement.next_empty iter suffix]
  | cons condition rest ih =>
    obtain ⟨advance, tail⟩ := ConservationRefinement.next_cons iter condition rest suffix
    have evaluated := EvaluationRefinement.condition_refines_naturals accountEq accountHash assetEq assetHash roleOrd cmp
      hAccount hAsset hRole proposal model.accounts condition
    rw [TransactionDecode.context_refines] at evaluated
    change economy.evaluate_condition accountEq accountHash assetEq assetHash roleOrd scalar condition proposal model.accounts =
      .ok (EvaluationRefinement.encodeResult (Evaluation.condition (Evaluation.naturals Quantity.u64Max) id
        (context cmp proposal model.accounts) (EvaluationRefinement.decodeComparison condition.comparison)
        (TransactionDecode.expression condition.left) (TransactionDecode.expression condition.right))) at evaluated
    cases outcome : Evaluation.condition (Evaluation.naturals Quantity.u64Max) id (context cmp proposal model.accounts)
        (EvaluationRefinement.decodeComparison condition.comparison) (TransactionDecode.expression condition.left)
        (TransactionDecode.expression condition.right) with
    | error error =>
      have errorFits := condition_error_fits (Evaluation.naturals Quantity.u64Max) id (context cmp proposal model.accounts)
        (state_balance_errors cmp 0 _ _ _ _) (EvaluationRefinement.decodeComparison condition.comparison)
        (TransactionDecode.expression condition.left) (TransactionDecode.expression condition.right)
        (fits condition (by simp)).2.1 (fits condition (by simp)).2.2 error outcome
      have message := DiagnosticRefinement.message_refines error errorFits
      have body : economy.Economy.check_conditions_loop.body accountEq accountHash assetEq assetHash rateClone roleOrd scalar
          model proposal iter = .ok (.done (.Err (.RateLawEvaluation proposal.rate error.message))) := by
        simp [economy.Economy.check_conditions_loop.body, advance, evaluated, outcome,
          EvaluationRefinement.encodeResult, exchange.Exchange.impl.rate, hRate proposal.rate,
          expression_message] at message ⊢
        simp [message]
      refine ⟨.Err (.RateLawEvaluation proposal.rate error.message), ?_, ?_⟩
      · unfold economy.Economy.check_conditions_loop
        rw [loop.eq_1]
        simp [body]
      · simp [TransactionDecode.checked, TransactionDecode.failure, Transaction.checkConditions,
          Transaction.conditionResult, TransactionDecode.condition, outcome, Transaction.liftIssue]
    | ok satisfied =>
      cases satisfied with
      | false =>
        have labelFits := (fits condition (by simp)).1
        have body : economy.Economy.check_conditions_loop.body accountEq accountHash assetEq assetHash rateClone roleOrd scalar
            model proposal iter = .ok (.done (.Err (.RateConditionFailed proposal.rate condition.name))) := by
          simp [economy.Economy.check_conditions_loop.body, advance, evaluated, outcome,
            EvaluationRefinement.encodeResult, exchange.Exchange.impl.rate, hRate proposal.rate,
            rate.RateCondition.impl.name, DiagnosticRefinement.deref_exact condition.name labelFits,
            Str.Insts.AllocBorrowToOwnedString.to_owned]
        refine ⟨.Err (.RateConditionFailed proposal.rate condition.name), ?_, ?_⟩
        · unfold economy.Economy.check_conditions_loop
          rw [loop.eq_1]
          simp [body]
        · simp [TransactionDecode.checked, TransactionDecode.failure, Transaction.checkConditions,
            Transaction.conditionResult, TransactionDecode.condition, outcome, Transaction.liftIssue]
      | true =>
        obtain ⟨result, called, decoded⟩ := ih { iter with i := iter.i + 1 } tail (fun value member => fits value (by simp [member]))
        have body : economy.Economy.check_conditions_loop.body accountEq accountHash assetEq assetHash rateClone roleOrd scalar
            model proposal iter = .ok (.cont { iter with i := iter.i + 1 }) := by
          simp [economy.Economy.check_conditions_loop.body, advance, evaluated, outcome, EvaluationRefinement.encodeResult]
        refine ⟨result, ?_, ?_⟩
        · unfold economy.Economy.check_conditions_loop
          rw [loop.eq_1]
          simp only [body, bind_ok]
          exact called
        · simpa [Transaction.checkConditions, Transaction.conditionResult, TransactionDecode.condition, outcome] using decoded

include hAccount hAsset hRate hRole in
theorem conditions_refines (accountClone : core.clone.Clone AccountId) (accountOrd : core.cmp.Ord AccountId)
    (assetClone : core.clone.Clone A) (assetOrd : core.cmp.Ord A)
    (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (model : economy.Economy AccountId A RateId Role Std.U64)
    (rule : rate.Rate Role A Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (fits : ∀ value ∈ rule.conditions.val, NamesFit value) :
    ∃ result, economy.Economy.check_conditions accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model rule proposal = .ok result ∧
      TransactionDecode.checked id result = Transaction.liftIssue
        (Transaction.checkConditions proposal.rate (context cmp proposal model.accounts) (TransactionDecode.rule rule).conditions) := by
  obtain ⟨result, called, decoded⟩ := conditions_loop_refines accountEq accountHash assetEq assetHash rateClone roleOrd cmp
    hAccount hAsset hRate hRole rule.conditions.val ⟨alloc.vec.Vec.deref rule.conditions, 0⟩
    (by simp [alloc.vec.Vec.deref]) model proposal fits
  refine ⟨result, ?_, decoded⟩
  simpa [economy.Economy.check_conditions, rate.Rate.impl.conditions,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, core.slice.Slice.iter,
    alloc.vec.Vec.deref] using called

end Conditions
end Axionomy.ConditionRefinement
