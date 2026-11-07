import Axionomy.AnalysisRefinement
import Axionomy.TransactionSafety
import Axionomy.TransactionReceipts

namespace Axionomy.ApplyRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

def result (value : core.result.Result (exchange.Receipt RateId Role AccountId A Std.U64)
    (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)) :
    Except (Transaction.Failure AccountId A RateId Role) (Transaction.Receipt AccountId A RateId Role) :=
  match value with
  | .Ok receipt => .ok (TransactionDecode.receipt receipt)
  | .Err error => .error (TransactionDecode.failure error)

def outcome (value : core.result.Result (exchange.Receipt RateId Role AccountId A Std.U64)
    (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128) × economy.Economy AccountId A RateId Role Std.U64) :=
  (result value.1, TransactionDecode.model value.2)

def Frame [DecidableEq AccountId] (before : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (applied : core.result.Result (exchange.Receipt RateId Role AccountId A Std.U64)
      (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128) × economy.Economy AccountId A RateId Role Std.U64) : Prop :=
  applied.2.rates = before.rates ∧ applied.2.invariants = before.invariants ∧
    applied.2.accounts.map Prod.fst = before.accounts.map Prod.fst ∧
    match applied.1 with
    | .Err _ => applied.2 = before
    | .Ok receipt => receipt.proposal = proposal ∧
        ∀ id, id ∉ receipt.deltas.val.map exchange.AccountDelta.account →
          Account.find? id applied.2.accounts = Account.find? id before.accounts

section Apply
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetClone : LawfulClone assetClone)
variable (hAssetEq : LawfulEquality assetEq) (hRateClone : LawfulClone rateClone) (hRateEq : LawfulEquality rateEq)
variable (hRoleClone : LawfulClone roleClone) (hRoleEq : LawfulEquality roleOrd.eqInst) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem apply_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ applied, economy.Economy.apply accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        model proposal = .ok applied ∧
      Transaction.apply cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) = some (outcome applied) ∧
      Frame model proposal applied := by
  obtain ⟨analysis, analyzed, decoded⟩ := AnalysisRefinement.analyze_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws model proposal parameterFits rulesReady invariantFits accountCapacity
  have ready := Transaction.analyze_ready cmpAccount cmpRole laws (TransactionDecode.model model) (TransactionDecode.proposal proposal)
    (TransactionDecode.analysis analysis) decoded
  cases assessed : analysis.assessment with
  | Invalid issues =>
    have nonempty : issues.val ≠ [] := by
      simpa [Transaction.AnalysisReady, TransactionDecode.analysis, TransactionDecode.exchangeAssessment, assessed] using ready
    cases entries : issues.val with
    | nil => exact False.elim (nonempty entries)
    | cons error rest =>
      obtain ⟨remaining, advanced, _⟩ := CommitRefinement.owning_next_cons issues error rest entries
      refine ⟨(.Err error, model), ?_, ?_, by simp [Frame]⟩
      · simp [economy.Economy.apply, analyzed, assessed, alloc.vec.IntoIteratorVec.into_iter, core.option.Option.expect, bind_ok]
        dsimp only [alloc.vec.into_iter.IntoIter]
        rw [bind_ok, advanced]
        dsimp only [alloc.vec.into_iter.IntoIter]
        simp [bind_ok, Aeneas.Std.uncurry]
      · simp [Transaction.apply, decoded, Transaction.applyAnalysis, TransactionDecode.analysis, assessed,
          TransactionDecode.exchangeAssessment, entries, outcome, result]
  | Infeasible accounts missing =>
    refine ⟨(.Err (.Infeasible missing), model), ?_, ?_, by simp [Frame]⟩
    · simp [economy.Economy.apply, analyzed, assessed]
    · simp [Transaction.apply, decoded, Transaction.applyAnalysis, TransactionDecode.analysis, assessed,
        TransactionDecode.exchangeAssessment, outcome, result, TransactionDecode.failure]
  | Applicable accounts deltas =>
    simp only [Transaction.AnalysisReady, TransactionDecode.analysis, TransactionDecode.exchangeAssessment, assessed] at ready
    cases staged : analysis.prepared_accounts with
    | none =>
      simp [staged] at ready
    | some prepared =>
      obtain ⟨effects, values, valuesEqual, deltaEqual, matched, _, present⟩ := ready
      have same : TransactionDecode.accounts prepared.val = values := by
        simpa [TransactionDecode.analysis, staged] using valuesEqual
      have rawPresent : ∀ entry ∈ prepared.val, entry.1 ∈ model.accounts.map Prod.fst := by
        intro entry member
        have mapped : (entry.1, TransactionDecode.holder entry.2) ∈ values := by
          rw [← same]
          exact List.mem_map.mpr ⟨entry, member, rfl⟩
        simpa [TransactionDecode.model, TransactionDecode.accounts_keys] using present _ mapped
      obtain ⟨committed, commitCall, installed⟩ := CommitRefinement.apply_loop_refines accountEq accountHash hAccountEq
        prepared.val prepared rfl model rawPresent
      have installedValue : Transaction.install (TransactionDecode.accounts model.accounts) (TransactionDecode.accounts prepared.val) =
          some (TransactionDecode.accounts committed) := by
        rw [TransactionDecode.accounts, TransactionDecode.accounts, ← TransactionDecode.install_map, installed]
        rfl
      refine ⟨(.Ok ⟨proposal, deltas⟩, { model with accounts := committed }), ?_, ?_, ?_⟩
      · simp [economy.Economy.apply, analyzed, assessed, staged, core.option.Option.expect,
          alloc.vec.IntoIteratorVec.into_iter, exchange.Receipt.new, bind_ok]
        dsimp only [alloc.vec.into_iter.IntoIter]
        rw [bind_ok, commitCall]
        simp [bind_ok]
      · unfold Transaction.apply
        rw [decoded]
        simp [Transaction.applyAnalysis, TransactionDecode.analysis, assessed, staged,
          TransactionDecode.exchangeAssessment, installedValue, outcome, result, TransactionDecode.receipt, TransactionDecode.model]
      · have stagedKeys : prepared.val.map Prod.fst = effects.map Prod.fst := by
          rw [← TransactionDecode.accounts_keys prepared.val, same]
          exact Transaction.preparedEntry_keys _ effects values matched
        have deltaKeys : deltas.val.map exchange.AccountDelta.account = effects.map Prod.fst := by
          have keys := congrArg (List.map Conservation.Delta.account) deltaEqual
          simpa [List.map_map, Function.comp_def, TransactionDecode.delta, Transaction.effectDelta] using keys
        exact ⟨rfl, rfl, Transaction.install_keys _ _ _ installed, rfl,
          fun id untouched => Transaction.install_find_other id _ _ _
            (by simpa [stagedKeys, deltaKeys] using untouched) installed⟩

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem accepted_receipt_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max)
    (receipt : exchange.Receipt RateId Role AccountId A Std.U64)
    (after : economy.Economy AccountId A RateId Role Std.U64)
    (success : economy.Economy.apply accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok (.Ok receipt, after)) :
    receipt.proposal = proposal ∧
      Transaction.ReceiptChanges (TransactionDecode.model model) (TransactionDecode.model after) (TransactionDecode.receipt receipt) := by
  obtain ⟨applied, executed, specified, frame⟩ := apply_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd laws model proposal parameterFits rulesReady invariantFits accountCapacity
  have same : applied = (.Ok receipt, after) := Result.ok_injective (executed.symm.trans success)
  subst applied
  have changes := Transaction.apply_receipt_changes cmpAccount cmpRole laws (TransactionDecode.model model)
    (TransactionDecode.proposal proposal) (TransactionDecode.receipt receipt) (TransactionDecode.model after)
    (by simpa only [outcome, result] using specified)
  exact ⟨frame.2.2.2.1, changes.2⟩

end Apply
end Axionomy.ApplyRefinement
