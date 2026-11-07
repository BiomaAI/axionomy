import Axionomy.TransactionConservation
import Axionomy.PreparationRefinement
import Axionomy.DiagnosticRefinement

namespace Axionomy.InvariantRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement TransactionConservation

variable {AccountId A RateId Role : Type} [DecidableEq AccountId] [DecidableEq A]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq) (hAssetEq : LawfulEquality assetEq)

include hAccountClone hAccountEq hAssetEq in
theorem invariants_loop_refines (remaining : List (rate.LinearInvariant A))
    (iter : core.slice.iter.Iter (rate.LinearInvariant A)) (suffix : iter.slice.val.drop iter.i = remaining)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (deltas : Slice (exchange.AccountDelta AccountId A Std.U64))
    (prepared : alloc.vec.Vec (AccountId × account.Account A Std.U64))
    (present : ∀ entry ∈ prepared.val, entry.1 ∈ model.accounts.map Prod.fst)
    (fits : ∀ invariant ∈ remaining, invariant.name.toByteArray.size ≤ Usize.max) :
    ∃ result, economy.Economy.check_invariants_loop accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
        iter model.accounts model.rates model.invariants deltas prepared = .ok result ∧
      TransactionDecode.checked id result = Transaction.checkInvariants (TransactionDecode.accounts model.accounts)
        ⟨TransactionDecode.accounts prepared.val, deltas.val.map TransactionDecode.delta⟩ (remaining.map TransactionDecode.invariant) := by
  induction remaining generalizing iter with
  | nil =>
    refine ⟨.Ok (), ?_, rfl⟩
    unfold economy.Economy.check_invariants_loop
    rw [loop.eq_1]
    simp [economy.Economy.check_invariants_loop.body, ConservationRefinement.next_empty iter suffix]
  | cons invariant rest ih =>
    obtain ⟨advance, tail⟩ := ConservationRefinement.next_cons iter invariant rest suffix
    have labelFits := fits invariant (by simp)
    have conserved := conserves_refines assetEq assetHash hAssetEq invariant deltas
    cases check : Conservation.conserves (weighted invariant) Conservation.checkedAdd 0 (deltas.val.map TransactionDecode.delta) with
    | none =>
      have body : economy.Economy.check_invariants_loop.body accountClone accountEq accountHash accountOrd
          assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
          model.accounts model.rates model.invariants deltas prepared iter = .ok (.done (.Err (.InvariantOverflow invariant.name))) := by
        simp [economy.Economy.check_invariants_loop.body, advance, conserved, check, rate.LinearInvariant.impl.name,
          DiagnosticRefinement.deref_exact invariant.name labelFits, Str.Insts.AllocBorrowToOwnedString.to_owned]
      refine ⟨.Err (.InvariantOverflow invariant.name), ?_, ?_⟩
      · unfold economy.Economy.check_invariants_loop
        rw [loop.eq_1]
        simp [body]
      · simp only [List.map_cons, Transaction.checkInvariants, weighted_function invariant, check]
        rfl
    | some accepted =>
      cases accepted with
      | true =>
        obtain ⟨result, called, decoded⟩ := ih { iter with i := iter.i + 1 } tail (fun inv member => fits inv (by simp [member]))
        have body : economy.Economy.check_invariants_loop.body accountClone accountEq accountHash accountOrd
            assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
            model.accounts model.rates model.invariants deltas prepared iter = .ok (.cont { iter with i := iter.i + 1 }) := by
          simp [economy.Economy.check_invariants_loop.body, advance, conserved, check]
        refine ⟨result, ?_, ?_⟩
        · unfold economy.Economy.check_invariants_loop
          rw [loop.eq_1]
          simp only [body, bind_ok]
          exact called
        · simpa only [List.map_cons, Transaction.checkInvariants, weighted_function invariant, check] using decoded
      | false =>
        obtain ⟨beforeResult, beforeCall, beforeValue⟩ := measure_refines assetEq assetHash hAssetEq assetClone invariant model.accounts
        cases beforeResult with
        | none =>
          have body : economy.Economy.check_invariants_loop.body accountClone accountEq accountHash accountOrd
              assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
              model.accounts model.rates model.invariants deltas prepared iter = .ok (.done (.Err (.InvariantOverflow invariant.name))) := by
            simp [economy.Economy.check_invariants_loop.body, advance, conserved, check, beforeCall,
              rate.LinearInvariant.impl.name, DiagnosticRefinement.deref_exact invariant.name labelFits,
              Str.Insts.AllocBorrowToOwnedString.to_owned]
          refine ⟨.Err (.InvariantOverflow invariant.name), ?_, ?_⟩
          · unfold economy.Economy.check_invariants_loop
            rw [loop.eq_1]
            simp [body]
          · simp only [List.map_cons, Transaction.checkInvariants, weighted_function invariant, check, ← beforeValue, Option.map_none,
              Transaction.checked_bind_error]
            rfl
        | some before =>
          obtain ⟨updated, snapshotCall, installed⟩ := CommitRefinement.snapshot_refines accountEq accountHash hAccountEq
            accountClone accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd
            roleClone roleOrd scalar hAccountClone model prepared present
          have mapped := TransactionDecode.install_map TransactionDecode.holder model.accounts prepared.val
          rw [installed] at mapped
          have mappedInstall : Transaction.install (TransactionDecode.accounts model.accounts) (TransactionDecode.accounts prepared.val) =
              some (TransactionDecode.accounts updated) := mapped.symm
          obtain ⟨afterResult, afterCall, afterValue⟩ := measure_refines assetEq assetHash hAssetEq assetClone invariant updated
          cases afterResult with
          | none =>
            have body : economy.Economy.check_invariants_loop.body accountClone accountEq accountHash accountOrd
                assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
                model.accounts model.rates model.invariants deltas prepared iter = .ok (.done (.Err (.InvariantOverflow invariant.name))) := by
              simp [economy.Economy.check_invariants_loop.body, advance, conserved, check, beforeCall, snapshotCall, afterCall,
                rate.LinearInvariant.impl.name, DiagnosticRefinement.deref_exact invariant.name labelFits,
                Str.Insts.AllocBorrowToOwnedString.to_owned]
            refine ⟨.Err (.InvariantOverflow invariant.name), ?_, ?_⟩
            · unfold economy.Economy.check_invariants_loop
              rw [loop.eq_1]
              simp [body]
            · simp only [List.map_cons, Transaction.checkInvariants, weighted_function invariant, check, ← beforeValue, Option.map_some,
                Transaction.checked_bind_ok, mappedInstall, Transaction.require, ← afterValue, Option.map_none,
                Transaction.checked_bind_error]
              rfl
          | some after =>
            have body : economy.Economy.check_invariants_loop.body accountClone accountEq accountHash accountOrd
                assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
                model.accounts model.rates model.invariants deltas prepared iter =
                .ok (.done (.Err (.InvariantViolation invariant.name before after))) := by
              simp [economy.Economy.check_invariants_loop.body, advance, conserved, check, beforeCall, snapshotCall, afterCall,
                rate.LinearInvariant.impl.name, DiagnosticRefinement.deref_exact invariant.name labelFits,
                Str.Insts.AllocBorrowToOwnedString.to_owned]
            refine ⟨.Err (.InvariantViolation invariant.name before after), ?_, ?_⟩
            · unfold economy.Economy.check_invariants_loop
              rw [loop.eq_1]
              simp [body]
            · simp only [List.map_cons, Transaction.checkInvariants, weighted_function invariant, check, ← beforeValue, Option.map_some,
                Transaction.checked_bind_ok, mappedInstall, Transaction.require, ← afterValue]
              rfl

include hAccountClone hAccountEq hAssetEq in
theorem invariants_refines (model : economy.Economy AccountId A RateId Role Std.U64)
    (deltas : Slice (exchange.AccountDelta AccountId A Std.U64))
    (prepared : alloc.vec.Vec (AccountId × account.Account A Std.U64))
    (present : ∀ entry ∈ prepared.val, entry.1 ∈ model.accounts.map Prod.fst)
    (fits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max) :
    ∃ result, economy.Economy.check_invariants accountClone accountEq accountHash accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model deltas prepared = .ok result ∧
      TransactionDecode.checked id result = Transaction.checkInvariants (TransactionDecode.accounts model.accounts)
        ⟨TransactionDecode.accounts prepared.val, deltas.val.map TransactionDecode.delta⟩ (model.invariants.val.map TransactionDecode.invariant) := by
  obtain ⟨result, called, decoded⟩ := invariants_loop_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd
    hAccountClone hAccountEq hAssetEq model.invariants.val ⟨alloc.vec.Vec.deref model.invariants, 0⟩
    (by simp [alloc.vec.Vec.deref]) model deltas prepared present fits
  refine ⟨result, ?_, decoded⟩
  simpa [economy.Economy.check_invariants, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, core.slice.Slice.iter] using called

end Axionomy.InvariantRefinement
