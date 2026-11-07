import Axionomy.TransactionDecode
import Axionomy.ConservationRefinement

namespace Axionomy.TransactionConservation
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

variable {AccountId A : Type} [DecidableEq A]

def weighted (invariant : rate.LinearInvariant A) (asset : A) (amount : Nat) : Option Int :=
  some (Conservation.contribution (TransactionDecode.invariant invariant).weights asset amount)

theorem weighted_function (invariant : rate.LinearInvariant A) :
    (fun asset amount => some (Conservation.contribution (TransactionDecode.invariant invariant).weights asset amount)) =
      weighted invariant := rfl

theorem contribution_decode (invariant : rate.LinearInvariant A) (asset : A) (amount : Std.U64) :
    ConservationRefinement.mathematical invariant.weights asset amount =
      Conservation.contribution (TransactionDecode.invariant invariant).weights asset amount.val := by
  unfold ConservationRefinement.mathematical Conservation.contribution TransactionDecode.invariant
  rw [← Account.map_find]
  cases Account.find? asset invariant.weights <;> rfl

theorem accumulate_decode (invariant : rate.LinearInvariant A) (entries : List (A × Std.U64)) (total : Int) :
    Conservation.accumulate (fun a q => some (ConservationRefinement.mathematical invariant.weights a q))
        Conservation.checkedAdd total entries =
      Conservation.accumulate (weighted invariant) Conservation.checkedAdd total (Account.mapEntries UScalar.val entries) := by
  induction entries generalizing total with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, value⟩
    simp only [Conservation.accumulate, Account.mapEntries_cons, weighted, contribution_decode invariant asset value]
    cases added : Conservation.checkedAdd total (Conservation.contribution (TransactionDecode.invariant invariant).weights asset value.val) with
    | none => simp [added]
    | some next => simpa [added] using ih next

theorem totals_decode (invariant : rate.LinearInvariant A)
    (deltas : List (exchange.AccountDelta AccountId A Std.U64)) (consumed produced : Int) :
    Conservation.totals (fun a q => some (ConservationRefinement.mathematical invariant.weights a q)) Conservation.checkedAdd
        consumed produced (deltas.map ConservationRefinement.decodeDelta) =
      Conservation.totals (weighted invariant) Conservation.checkedAdd consumed produced (deltas.map TransactionDecode.delta) := by
  induction deltas generalizing consumed produced with
  | nil => rfl
  | cons delta rest ih =>
    simp only [List.map_cons, Conservation.totals, ConservationRefinement.decodeDelta, TransactionDecode.delta,
      TransactionDecode.balances, accumulate_decode]
    cases hc : Conservation.accumulate (weighted invariant) Conservation.checkedAdd consumed
        (Account.mapEntries UScalar.val delta.consumed.quantities) with
    | none => simp
    | some nextC =>
      cases hp : Conservation.accumulate (weighted invariant) Conservation.checkedAdd produced
          (Account.mapEntries UScalar.val delta.produced.quantities) with
      | none => simp
      | some nextP => simpa [hc, hp] using ih nextC nextP

variable (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (hEq : LawfulEquality assetEq)

include hEq in
theorem conserves_refines (invariant : rate.LinearInvariant A) (deltas : Slice (exchange.AccountDelta AccountId A Std.U64)) :
    rate.LinearInvariant.conserves assetEq assetHash scalar invariant deltas =
      .ok (Conservation.conserves (weighted invariant) Conservation.checkedAdd 0 (deltas.val.map TransactionDecode.delta)) := by
  have called := ConservationRefinement.conserves_refines_integers assetEq assetHash hEq invariant deltas
  change rate.LinearInvariant.conserves assetEq assetHash scalar invariant deltas = _ at called
  simpa only [Conservation.conserves, totals_decode] using called

omit [DecidableEq A] in
theorem accounts_flatten (states : List (AccountId × account.Account A Std.U64)) :
    Account.mapEntries UScalar.val (states.flatMap fun entry => entry.2.balances.quantities) =
      (TransactionDecode.accounts states).flatMap Prod.snd := by
  simp [TransactionDecode.accounts, TransactionDecode.holder, TransactionDecode.balances,
    Account.mapEntries, List.flatMap_map, List.map_flatMap]

include hEq in
theorem measure_refines (assetClone : core.clone.Clone A) (invariant : rate.LinearInvariant A)
    (states : List (AccountId × account.Account A Std.U64)) :
    ∃ result, rate.LinearInvariant.measure assetEq assetHash assetClone ConservationRefinement.accountRef scalar invariant states = .ok result ∧
      result.map IScalar.val = Conservation.accumulate (weighted invariant) Conservation.checkedAdd 0
        ((TransactionDecode.accounts states).flatMap Prod.snd) := by
  obtain ⟨result, called, decoded⟩ := ConservationRefinement.measure_refines_integers assetEq assetHash hEq assetClone invariant states
  refine ⟨result, called, ?_⟩
  rw [decoded, accumulate_decode, accounts_flatten]

end Axionomy.TransactionConservation
