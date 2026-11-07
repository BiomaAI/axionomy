import Axionomy.Transaction
import Axionomy.EvaluationRefinement

namespace Axionomy.TransactionDecode
open Aeneas Aeneas.Std AccountExtracted

variable {AccountId A RateId Role : Type}

def balances (value : basket.Basket A Std.U64) : Transaction.Balances A :=
  Account.mapEntries UScalar.val value.quantities

def holder (value : account.Account A Std.U64) : Transaction.Balances A := balances value.balances

def accounts (values : List (AccountId × account.Account A Std.U64)) : List (AccountId × Transaction.Balances A) :=
  Account.mapEntries holder values

def proposal (value : exchange.Exchange RateId Role AccountId Std.U64) : Transaction.Proposal AccountId RateId Role :=
  ⟨value.rate, value.bindings, value.units.val, Account.mapEntries UScalar.val value.parameters⟩

def expression (value : rate.QuantityExpression Role A Std.U64) : Evaluation.Expr Role A Nat :=
  (EvaluationRefinement.decodeExpression value).map UScalar.val

def condition (value : rate.RateCondition Role A Std.U64) : Transaction.Condition Role A :=
  ⟨value.name, expression value.left, EvaluationRefinement.decodeComparison value.comparison, expression value.right⟩

def computed (value : rate.ComputedAmount Role A Std.U64) : Transaction.Computed Role A :=
  ⟨value.role, value.asset, expression value.quantity⟩

def rule (value : rate.Rate Role A Std.U64) : Transaction.Rule Role A :=
  ⟨value.roles, value.distinct, Account.mapEntries balances value.consume,
    Account.mapEntries balances value.produce, Account.mapEntries balances value.preserve,
    value.computed_consume.val.map computed, value.computed_produce.val.map computed,
    value.computed_preserve.val.map computed, value.conditions.val.map condition⟩

def invariant (value : rate.LinearInvariant A) : Transaction.Invariant A :=
  ⟨value.name, Account.mapEntries IScalar.val value.weights⟩

def model (value : economy.Economy AccountId A RateId Role Std.U64) : Transaction.Model AccountId A RateId Role :=
  ⟨accounts value.accounts, Account.mapEntries rule value.rates, value.invariants.val.map invariant⟩

def effect (value : economy.Effect A Std.U64) : Transaction.Effect A :=
  ⟨balances value.consume, balances value.produce, balances value.preserve⟩

def effects (values : List (AccountId × economy.Effect A Std.U64)) : List (AccountId × Transaction.Effect A) :=
  Account.mapEntries effect values

def direction : economy.ComputedDirection → Transaction.Direction
  | .Consume => .consume | .Produce => .produce | .Preserve => .preserve

def delta (value : exchange.AccountDelta AccountId A Std.U64) : Transaction.Delta AccountId A :=
  ⟨value.account, balances value.consumed, balances value.produced, balances value.preserved⟩

def shortfall (value : economy.AccountShortfall AccountId A Std.U64) : AccountId × Transaction.Balances A :=
  (value.account, balances value.missing)

def assessment (value : economy.AccountAssessment AccountId A Std.U64) : Transaction.AccountAssessment AccountId A :=
  ⟨value.account, balances value.available, balances value.required,
    balances value.consumed, balances value.produced, balances value.preserved⟩

def availability (value : economy.Availability AccountId A Std.U64) : Transaction.Availability AccountId A :=
  ⟨value.accounts.val.map assessment, value.shortfalls.val.map shortfall⟩

def preparation (value : economy.Preparation AccountId A Std.U64) : Transaction.Preparation AccountId A :=
  ⟨accounts value.accounts.val, value.deltas.val.map delta⟩

def failure : economy.ApplyError RateId Role AccountId A Std.U64 Std.I128 → Transaction.Failure AccountId A RateId Role
  | .MissingRate id => .structural (.missingRate id)
  | .MissingBinding role => .structural (.missingBinding role)
  | .UnknownBinding role => .structural (.unknownBinding role)
  | .RolesMustDiffer left right => .structural (.rolesMustDiffer left right)
  | .MissingAccount id => .structural (.missingAccount id)
  | .ZeroUnits => .structural .zeroUnits
  | .MissingParameter key => .structural (.missingParameter key)
  | .UnknownParameter key => .structural (.unknownParameter key)
  | .RateOverflow id asset => .rateOverflow id asset
  | .RateLawEvaluation id message => .lawEvaluation id message
  | .RateConditionFailed id label => .conditionFailed id label
  | .Infeasible missing => .infeasible (missing.val.map shortfall)
  | .BalanceOverflow id asset => .balanceOverflow id asset
  | .InvariantOverflow label => .invariantOverflow label
  | .InvariantViolation label before after => .invariantViolation label before.val after.val

def checked {T U : Type} (decode : T → U)
    (value : core.result.Result T (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)) :
    Transaction.Checked AccountId A RateId Role U :=
  match value with
  | .Ok result => .ok (decode result)
  | .Err error => .error (.issue (failure error))

def exchangeAssessment : economy.ExchangeAssessment AccountId A RateId Role Std.U64 Std.I128 →
    Transaction.Assessment AccountId A RateId Role
  | .Applicable assessed deltas => .applicable (assessed.val.map assessment) (deltas.val.map delta)
  | .Infeasible assessed missing => .infeasible (assessed.val.map assessment) (missing.val.map shortfall)
  | .Invalid issues => .invalid (issues.val.map failure)

def analysis (value : economy.Analysis AccountId A RateId Role Std.U64 Std.I128) : Transaction.Analysis AccountId A RateId Role :=
  ⟨exchangeAssessment value.assessment, value.prepared_accounts.map (fun prepared => accounts prepared.val)⟩

def receipt (value : exchange.Receipt RateId Role AccountId A Std.U64) : Transaction.Receipt AccountId A RateId Role :=
  ⟨proposal value.proposal, value.deltas.val.map delta⟩

theorem checked_not_panic {T U : Type} (decode : T → U)
    (value : core.result.Result T (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)) :
    checked decode value ≠ .error .panic := by cases value <;> simp [checked]

@[simp] theorem accounts_keys (values : List (AccountId × account.Account A Std.U64)) :
    (accounts values).map Prod.fst = values.map Prod.fst := by simp [accounts, Account.mapEntries, List.map_map]

@[simp] theorem effects_keys (values : List (AccountId × economy.Effect A Std.U64)) :
    (effects values).map Prod.fst = values.map Prod.fst := by simp [effects, Account.mapEntries, List.map_map]

theorem orderedFind_map {K V W : Type} (cmp : K → K → Ordering) (f : V → W) (key : K) (values : List (K × V)) :
    (Evaluation.orderedFind cmp key values).map f = Evaluation.orderedFind cmp key (Account.mapEntries f values) := by
  induction values with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨stored, value⟩
    cases compared : cmp key stored <;> simp [Evaluation.orderedFind, compared, ih]

theorem map_fromState {K R S Q T : Type} [DecidableEq K] [DecidableEq S]
    (cmp : R → R → Ordering) (f : Q → T) (zero units : Q)
    (parameters : List (String × Q)) (bindings : List (R × K)) (states : List (K × List (S × Q))) :
    (Evaluation.fromState cmp zero units parameters bindings states).map f =
      Evaluation.fromState cmp (f zero) (f units) (Account.mapEntries f parameters) bindings (Account.mapEntries (Account.mapEntries f) states) := by
  unfold Evaluation.Context.map Evaluation.fromState
  congr 1
  · funext key; exact orderedFind_map compare f key parameters
  · funext role asset
    cases bound : Evaluation.orderedFind cmp role bindings with
    | none => simp [bound, Evaluation.mapResult]
    | some id =>
      simp only [bound]
      rw [← Account.map_find]
      cases found : Account.find? id states with
      | none => simp [Evaluation.mapResult]
      | some entries =>
        simp only [Option.map_some, Evaluation.mapResult]
        exact congrArg Except.ok (Account.map_balance f zero entries asset)

theorem context_refines [DecidableEq AccountId] [DecidableEq A] (cmp : Role → Role → Ordering)
    (value : exchange.Exchange RateId Role AccountId Std.U64) (states : List (AccountId × account.Account A Std.U64)) :
    (EvaluationRefinement.context cmp value states).map UScalar.val =
      Evaluation.fromState cmp 0 (proposal value).units (proposal value).parameters (proposal value).bindings (accounts states) := by
  rw [EvaluationRefinement.context, map_fromState]
  congr 1
  simp [Account.mapEntries, List.map_map, Function.comp_def, accounts, holder, balances]

theorem replaceExisting_map {K V W : Type} [DecidableEq K] (f : V → W) (query : K) (value : V)
    (entries : List (K × V)) :
    (Transaction.replaceExisting query value entries).map (Account.mapEntries f) =
      Transaction.replaceExisting query (f value) (Account.mapEntries f entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, stored⟩
    by_cases same : query = key
    · simp [Transaction.replaceExisting, same]
    · simp only [Transaction.replaceExisting, Account.mapEntries_cons, ite_eq_right same]
      rw [← ih]
      cases Transaction.replaceExisting query value rest <;> rfl

theorem install_map {K V W : Type} [DecidableEq K] (f : V → W) (entries prepared : List (K × V)) :
    (Transaction.install entries prepared).map (Account.mapEntries f) =
      Transaction.install (Account.mapEntries f entries) (Account.mapEntries f prepared) := by
  induction prepared generalizing entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    simp only [Transaction.install, Account.mapEntries_cons, ← replaceExisting_map]
    cases replaced : Transaction.replaceExisting key value entries with
    | none => simp
    | some middle => simpa using ih middle

end Axionomy.TransactionDecode
