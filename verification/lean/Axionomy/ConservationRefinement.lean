import Axionomy.AccountRefinement
import Axionomy.Conservation

namespace Axionomy.ConservationRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

@[simp] theorem widen_value (value : Std.U64) :
    (widenU64 value).val = Int.ofNat value.val := IScalar.ofInt_val_eq _

theorem weighted_bounds (amount : Std.U64) (coefficient : Std.I64) :
    I128.min ≤ (amount.val : Int) * coefficient.val ∧
      (amount.val : Int) * coefficient.val ≤ I128.max := by
  have amountBound := amount.hBounds
  have coefficientBound := coefficient.hBounds
  norm_num [UScalarTy.numBits] at amountBound
  norm_num [IScalarTy.numBits] at coefficientBound
  have nonnegative : (0 : Int) ≤ amount.val := Int.natCast_nonneg _
  have lower := mul_nonneg nonnegative
    (show 0 ≤ coefficient.val + 9223372036854775808 by omega)
  have upper := mul_nonneg nonnegative
    (show 0 ≤ 9223372036854775807 - coefficient.val by omega)
  norm_num [I128.min, I128.max, I128.numBits]
  constructor <;> nlinarith

def weightedValue (amount : Std.U64) (coefficient : Std.I64) : Option Std.I128 :=
  I128.checked_mul (widenU64 amount) (core.convert.num.FromI128I64.from coefficient)

theorem weighted_exact (amount : Std.U64) (coefficient : Std.I64) :
    (weightedValue amount coefficient).map IScalar.val =
      some ((amount.val : Int) * coefficient.val) := by
  have spec := I128.checked_mul_bv_spec (widenU64 amount) (core.convert.num.FromI128I64.from coefficient)
  change match weightedValue amount coefficient with
    | some z => _
    | none => _ at spec
  simp only [widen_value, core.convert.num.FromI128I64.from_val_eq] at spec
  cases h : weightedValue amount coefficient with
  | none => simp only [h] at spec; exact False.elim (spec (weighted_bounds amount coefficient))
  | some result => simp only [h] at spec; simp [spec.2.2.1]

@[simp] theorem weighted_call (amount : Std.U64) (coefficient : Std.I64) :
    scalar.checked_weighted amount coefficient = .ok (weightedValue amount coefficient) := by
  simp [U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_weighted,
    I128.Insts.CoreConvertFromU64.from, weightedValue, lift]

theorem signed_add_exact (left right : Std.I128) :
    (I128.checked_add left right).map IScalar.val = Conservation.checkedAdd left.val right.val := by
  have spec := I128.checked_add_bv_spec left right
  change match I128.checked_add left right with
    | some z => _
    | none => _ at spec
  cases h : I128.checked_add left right with
  | none =>
    simp only [h] at spec
    have bound : ¬ (Conservation.i128Min ≤ left.val + right.val ∧
        left.val + right.val ≤ Conservation.i128Max) := by
      simpa [Conservation.i128Min, Conservation.i128Max, I128.min, I128.max, I128.numBits] using spec
    simp [Conservation.checkedAdd, bound]
  | some result =>
    simp only [h] at spec
    have bound : Conservation.i128Min ≤ left.val + right.val ∧
        left.val + right.val ≤ Conservation.i128Max := by
      have bounds := And.intro spec.1 spec.2.1
      simpa [Conservation.i128Min, Conservation.i128Max, I128.min, I128.max, I128.numBits] using bounds
    simp [Conservation.checkedAdd, bound, spec.2.2.1]

section Accumulation
variable {A : Type} [DecidableEq A]
variable (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
variable (hEq : LawfulEquality eqInst)

def contribution (weights : List (A × Std.I64)) (asset : A) (amount : Std.U64) : Option Std.I128 :=
  weightedValue amount ((Account.find? asset weights).getD 0#i64)

include hEq in
theorem accumulate_body (invariant : rate.LinearInvariant A)
    (remaining : List (A × Std.U64)) (total : Std.I128) :
    rate.LinearInvariant.checked_add_basket_measure_loop.body eqInst hashInst scalar invariant remaining total =
      .ok (match remaining with
        | [] => .done (some total)
        | (asset, amount) :: rest =>
          match contribution invariant.weights asset amount with
          | none => .done none
          | some term => match I128.checked_add total term with
            | none => .done none
            | some next => .cont (rest, next)) := by
  cases remaining with
  | nil => simp [rate.LinearInvariant.checked_add_basket_measure_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, amount⟩
    have unwrap (choice : Option Std.I64) :
        core.option.Option.unwrap_or choice 0#i64 = choice.getD 0#i64 := by
      cases choice <;> rfl
    have weightedImpl (q : Std.U64) (c : Std.I64) :
        U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_weighted q c =
          .ok (weightedValue q c) := weighted_call q c
    simp only [rate.LinearInvariant.checked_add_basket_measure_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
      bind_ok, indexmap.map.IndexMap.get, equivalent_eq, get_exact eqInst hEq,
      unwrap, lift, quantity.Quantity.as_scalar, weightedImpl,
      I128.Insts.Num_traitsOpsCheckedCheckedAdd.checked_add]
    cases hw : weightedValue amount ((Account.find? asset invariant.weights).getD 0#i64) with
    | none => simp [contribution, hw,
        core.option.Option.Insts.CoreOpsTry_traitTry.branch,
        core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual]
    | some term =>
      cases ha : I128.checked_add total term <;>
        simp [contribution, hw, ha,
          core.option.Option.Insts.CoreOpsTry_traitTry.branch,
          core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual]

include hEq in
theorem accumulate_loop (invariant : rate.LinearInvariant A)
    (remaining : List (A × Std.U64)) (total : Std.I128) :
    rate.LinearInvariant.checked_add_basket_measure_loop eqInst hashInst scalar remaining invariant total =
      .ok (Conservation.accumulate (contribution invariant.weights) I128.checked_add total remaining) := by
  induction remaining generalizing total with
  | nil =>
    unfold rate.LinearInvariant.checked_add_basket_measure_loop
    rw [loop.eq_1]
    simp [accumulate_body eqInst hashInst hEq, Conservation.accumulate]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    unfold rate.LinearInvariant.checked_add_basket_measure_loop
    rw [loop.eq_1]
    simp only [accumulate_body eqInst hashInst hEq invariant ((asset, amount) :: rest) total]
    cases hw : contribution invariant.weights asset amount with
    | none => simp [hw, Conservation.accumulate]
    | some term =>
      cases ha : I128.checked_add total term with
      | none => simp [hw, ha, Conservation.accumulate]
      | some next =>
        simp only [ha, bind_ok]
        change rate.LinearInvariant.checked_add_basket_measure_loop eqInst hashInst scalar rest invariant next = _
        rw [ih]
        simp [Conservation.accumulate, hw, ha]

include hEq in
@[simp] theorem accumulate_call (invariant : rate.LinearInvariant A)
    (entries : List (A × Std.U64)) (total : Std.I128) :
    rate.LinearInvariant.checked_add_basket_measure eqInst hashInst scalar invariant total ⟨entries⟩ =
      .ok (Conservation.accumulate (contribution invariant.weights) I128.checked_add total entries) := by
  simp [rate.LinearInvariant.checked_add_basket_measure, basket.Basket.iter,
    indexmap.map.IndexMap.iter, accumulate_loop eqInst hashInst hEq]

def decodeDelta {AccountId : Type} (delta : exchange.AccountDelta AccountId A Std.U64) :
    Conservation.Delta AccountId A Std.U64 :=
  ⟨delta.account, delta.consumed.quantities, delta.produced.quantities, delta.preserved.quantities⟩

def check (result : Option (Std.I128 × Std.I128)) : Option Bool :=
  result.map fun (consumed, produced) => decide (consumed = produced)

theorem next_empty {T : Type} (iter : core.slice.iter.Iter T)
    (empty : iter.slice.val.drop iter.i = []) :
    core.slice.iter.IteratorSliceIter.next iter = .ok (none, iter) := by
  have h : iter.slice.val.length ≤ iter.i := List.drop_eq_nil_iff.mp empty
  simp [core.slice.iter.IteratorSliceIter.next, Slice.len, Nat.not_lt.mpr h]

theorem next_cons {T : Type} (iter : core.slice.iter.Iter T) (head : T) (rest : List T)
    (remaining : iter.slice.val.drop iter.i = head :: rest) :
    core.slice.iter.IteratorSliceIter.next iter = .ok (some head, { iter with i := iter.i + 1 }) ∧
      iter.slice.val.drop (iter.i + 1) = rest := by
  have h : iter.i < iter.slice.val.length := by
    by_contra outside
    have empty := List.drop_eq_nil_iff.mpr (Nat.le_of_not_gt outside)
    rw [empty] at remaining
    contradiction
  have split := List.drop_eq_getElem_cons h
  rw [remaining] at split
  have equal := List.cons.inj split
  constructor
  · simp only [core.slice.iter.IteratorSliceIter.next, Slice.len_val, h, _root_.dite_true]
    congr 2
    exact congrArg some equal.1.symm
  · exact equal.2.symm

omit [DecidableEq A] in
theorem conserves_body_empty {AccountId : Type} (invariant : rate.LinearInvariant A)
    (iter : core.slice.iter.Iter (exchange.AccountDelta AccountId A Std.U64))
    (empty : iter.slice.val.drop iter.i = []) (consumed produced : Std.I128) :
    rate.LinearInvariant.conserves_loop.body eqInst hashInst scalar invariant iter consumed produced =
      .ok (.done (some (decide (consumed = produced)))) := by
  simp [rate.LinearInvariant.conserves_loop.body, next_empty iter empty]

include hEq in
theorem conserves_body_cons {AccountId : Type} (invariant : rate.LinearInvariant A)
    (iter : core.slice.iter.Iter (exchange.AccountDelta AccountId A Std.U64))
    (delta : exchange.AccountDelta AccountId A Std.U64)
    (rest : List (exchange.AccountDelta AccountId A Std.U64))
    (remaining : iter.slice.val.drop iter.i = delta :: rest) (consumed produced : Std.I128) :
    rate.LinearInvariant.conserves_loop.body eqInst hashInst scalar invariant iter consumed produced =
      .ok (match Conservation.accumulate (contribution invariant.weights) I128.checked_add consumed delta.consumed.quantities with
        | none => .done none
        | some nextC => match Conservation.accumulate (contribution invariant.weights) I128.checked_add produced delta.produced.quantities with
          | none => .done none
          | some nextP => .cont ({ iter with i := iter.i + 1 }, nextC, nextP)) := by
  simp only [rate.LinearInvariant.conserves_loop.body, (next_cons iter delta rest remaining).1,
    bind_ok, exchange.AccountDelta.impl.consumed, exchange.AccountDelta.impl.produced,
    accumulate_call eqInst hashInst hEq]
  cases hc : Conservation.accumulate (contribution invariant.weights) I128.checked_add consumed delta.consumed.quantities with
  | none => simp [hc, core.option.Option.Insts.CoreOpsTry_traitTry.branch,
      core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual]
  | some nextC =>
    cases hp : Conservation.accumulate (contribution invariant.weights) I128.checked_add produced delta.produced.quantities <;>
      simp [hc, hp, core.option.Option.Insts.CoreOpsTry_traitTry.branch,
        core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual]

include hEq in
theorem conserves_loop {AccountId : Type} (invariant : rate.LinearInvariant A)
    (remaining : List (exchange.AccountDelta AccountId A Std.U64))
    (iter : core.slice.iter.Iter (exchange.AccountDelta AccountId A Std.U64))
    (suffix : iter.slice.val.drop iter.i = remaining) (consumed produced : Std.I128) :
    rate.LinearInvariant.conserves_loop eqInst hashInst scalar iter invariant consumed produced =
      .ok (check (Conservation.totals (contribution invariant.weights) I128.checked_add
        consumed produced (remaining.map decodeDelta))) := by
  induction remaining generalizing iter consumed produced with
  | nil =>
    unfold rate.LinearInvariant.conserves_loop
    rw [loop.eq_1]
    simp [conserves_body_empty eqInst hashInst invariant iter suffix, Conservation.totals, check]
  | cons delta rest ih =>
    unfold rate.LinearInvariant.conserves_loop
    rw [loop.eq_1]
    simp only [conserves_body_cons eqInst hashInst hEq invariant iter delta rest suffix consumed produced]
    cases hc : Conservation.accumulate (contribution invariant.weights) I128.checked_add consumed delta.consumed.quantities with
    | none => simp [hc, Conservation.totals, decodeDelta, check]
    | some nextC =>
      cases hp : Conservation.accumulate (contribution invariant.weights) I128.checked_add produced delta.produced.quantities with
      | none => simp [hp, Conservation.totals, decodeDelta, check, hc]
      | some nextP =>
        simp only [bind_ok]
        change rate.LinearInvariant.conserves_loop eqInst hashInst scalar
          { iter with i := iter.i + 1 } invariant nextC nextP = _
        rw [ih _ (next_cons iter delta rest suffix).2]
        simp [Conservation.totals, decodeDelta, hc, hp]

include hEq in
theorem conserves_refines {AccountId : Type} (invariant : rate.LinearInvariant A)
    (deltas : Slice (exchange.AccountDelta AccountId A Std.U64)) :
    invariant_conserves eqInst hashInst invariant deltas =
      .ok (Conservation.conserves (contribution invariant.weights) I128.checked_add
        0#i128 (deltas.val.map decodeDelta)) := by
  simp [invariant_conserves, rate.LinearInvariant.conserves,
    I128.Insts.Num_traitsIdentitiesZero.zero,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
    conserves_loop eqInst hashInst hEq invariant deltas.val ⟨deltas, 0⟩ (by simp),
    Conservation.conserves, check]

def mathematical (weights : List (A × Std.I64)) (asset : A) (amount : Std.U64) : Int :=
  (amount.val : Int) * ((Account.find? asset weights).getD 0#i64).val

theorem contribution_exact (weights : List (A × Std.I64)) (asset : A) (amount : Std.U64) :
    (contribution weights asset amount).map IScalar.val = some (mathematical weights asset amount) :=
  weighted_exact amount _

include hEq in
theorem conserves_refines_integers {AccountId : Type} (invariant : rate.LinearInvariant A)
    (deltas : Slice (exchange.AccountDelta AccountId A Std.U64)) :
    invariant_conserves eqInst hashInst invariant deltas =
      .ok (Conservation.conserves (fun a q => some (mathematical invariant.weights a q))
        Conservation.checkedAdd 0 (deltas.val.map decodeDelta)) := by
  rw [conserves_refines eqInst hashInst hEq]
  unfold Conservation.conserves
  have mapped := Conservation.map_totals (contribution invariant.weights) I128.checked_add
    (fun a q => some (mathematical invariant.weights a q)) Conservation.checkedAdd
    IScalar.val (contribution_exact invariant.weights) signed_add_exact
    (deltas.val.map decodeDelta) 0#i128 0#i128
  change _ = Conservation.totals _ _ 0 0 _ at mapped
  rw [← mapped]
  cases ht : Conservation.totals (contribution invariant.weights) I128.checked_add 0#i128 0#i128 (deltas.val.map decodeDelta) with
  | none => rfl
  | some pair =>
    have equal : pair.1 = pair.2 ↔ pair.1.val = pair.2.val :=
      ⟨congrArg IScalar.val, IScalar.eq_of_val_eq⟩
    simp [equal]

include hEq in
theorem accepted_conserves {AccountId : Type} (invariant : rate.LinearInvariant A)
    (deltas : Slice (exchange.AccountDelta AccountId A Std.U64))
    (accepted : invariant_conserves eqInst hashInst invariant deltas = .ok (some true)) :
    (deltas.val.map fun d => Conservation.measure (mathematical invariant.weights) d.consumed.quantities).sum =
      (deltas.val.map fun d => Conservation.measure (mathematical invariant.weights) d.produced.quantities).sum := by
  rw [conserves_refines_integers eqInst hashInst hEq] at accepted
  simp only [Result.ok.injEq] at accepted
  have sound := Conservation.conservation_sound
    (fun a q => some (mathematical invariant.weights a q)) Conservation.checkedAdd
    id (mathematical invariant.weights)
    (by intro a q m h; simpa [eq_comm] using h)
    (by intro x y z h; exact (Conservation.checkedAdd_exact x y z).mp h |>.1)
    0 (deltas.val.map decodeDelta) accepted
  simpa [List.map_map, Function.comp_def, decodeDelta] using sound

end Accumulation
end Axionomy.ConservationRefinement
