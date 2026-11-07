import Axionomy.Quantity
import QuantityExtraction

namespace Axionomy.QuantityRefinement
open Aeneas Aeneas.Std

-- Equality to `ok` proves termination without a panic as well as the exact
-- Option result. `none` is a checked rejection, not an execution failure.
def Refines (actual : Result (Option Std.U64)) (expected : Option Nat) : Prop :=
  ∃ result, actual = Result.ok result ∧ result.map UScalar.val = expected

theorem checked_add_refines (left right : Std.U64) :
    Refines (Extracted.checked_add left right)
      (Quantity.add Quantity.u64Max left.val right.val) := by
  by_cases h : left.val + right.val ≤ Quantity.u64Max
  · have hsmall : left.val + right.val < 2^64 := by
      unfold Quantity.u64Max at h
      omega
    have hno : ¬ left.val + right.val > UScalar.max .U64 := by
      simpa only [UScalar.max, UScalarTy.numBits, Quantity.u64Max] using Nat.not_lt.mpr h
    have hover := UScalar.overflowing_add_eq left right
    rw [ite_eq_right hno] at hover
    have hop : Std.U64.add left right =
        Result.ok (UScalar.ofNatCore (ty := .U64) (left.val + right.val) hsmall) := by
      simp only [UScalar.add, UScalar.tryMk, UScalar.tryMkOpt, UScalar.check_bounds,
        UScalarTy.numBits, hsmall, decide_true, _root_.dite_true, Result.ofOption]
    refine ⟨some (UScalar.ofNatCore (ty := .U64) (left.val + right.val) hsmall), ?_, ?_⟩
    · simp [Extracted.checked_add, Extracted.core.intrinsics.add_with_overflow,
        Extracted.u64Intrinsic, Std.U64.overflowing_add, hover.2, hop]
    · simp [Quantity.add, Quantity.checked, h, UScalar.ofNatCore_val_eq]
  · have hlarge : left.val + right.val > UScalar.max .U64 := by
      simpa only [UScalar.max, UScalarTy.numBits, Quantity.u64Max] using Nat.lt_of_not_ge h
    have hover := UScalar.overflowing_add_eq left right
    rw [ite_eq_left hlarge] at hover
    refine ⟨none, ?_, ?_⟩
    · simp [Extracted.checked_add, Extracted.core.intrinsics.add_with_overflow,
        Extracted.u64Intrinsic, Std.U64.overflowing_add, hover.2,
        Extracted.core.intrinsics.cold_path]
    · simp [Quantity.add, Quantity.checked, h]

theorem checked_mul_refines (left right : Std.U64) :
    Refines (Extracted.checked_mul left right)
      (Quantity.mul Quantity.u64Max left.val right.val) := by
  by_cases h : left.val * right.val ≤ Quantity.u64Max
  · have hno : ¬ left.val * right.val > UScalar.max .U64 := by
      simpa only [UScalar.max, UScalarTy.numBits, Quantity.u64Max] using Nat.not_lt.mpr h
    have hover := UScalar.overflowing_mul_eq left right
    rw [ite_eq_right hno] at hover
    refine ⟨some (UScalar.overflowing_mul left right).1, ?_, ?_⟩
    · simp [Extracted.checked_mul, Extracted.core.intrinsics.mul_with_overflow,
        Extracted.u64Intrinsic, Std.U64.overflowing_mul, hover.2]
    · simp [Quantity.mul, Quantity.checked, h, hover.1]
  · have hlarge : left.val * right.val > UScalar.max .U64 := by
      simpa only [UScalar.max, UScalarTy.numBits, Quantity.u64Max] using Nat.lt_of_not_ge h
    have hover := UScalar.overflowing_mul_eq left right
    rw [ite_eq_left hlarge] at hover
    refine ⟨none, ?_, ?_⟩
    · simp [Extracted.checked_mul, Extracted.core.intrinsics.mul_with_overflow,
        Extracted.u64Intrinsic, Std.U64.overflowing_mul, hover.2,
        Extracted.core.intrinsics.cold_path]
    · simp [Quantity.mul, Quantity.checked, h]

theorem checked_sub_refines (left right : Std.U64) :
    Refines (Extracted.checked_sub left right) (Quantity.sub left.val right.val) := by
  by_cases h : right.val ≤ left.val
  · refine ⟨some ⟨BitVec.ofNat 64 (left.val - right.val)⟩, ?_, ?_⟩
    · simp [Extracted.checked_sub, UScalar.sub, Nat.not_lt.mpr h]
    · have hbound : left.val - right.val < 2^64 :=
        Nat.lt_of_le_of_lt (Nat.sub_le _ _) left.hBounds
      have hval : (⟨BitVec.ofNat 64 (left.val - right.val)⟩ : Std.U64).val =
          left.val - right.val := by
        change (left.val - right.val) % 2^64 = left.val - right.val
        exact Nat.mod_eq_of_lt hbound
      simp [Quantity.sub, h, hval]
  · refine ⟨none, ?_, ?_⟩
    · simp [Extracted.checked_sub, Nat.lt_of_not_ge h]
    · simp [Quantity.sub, h]

theorem checked_div_refines (left right : Std.U64) :
    Refines (Extracted.checked_div left right) (Quantity.div left.val right.val) := by
  by_cases h : right.val = 0
  · have hz : right = 0#u64 := UScalar.eq_of_val_eq h
    subst right
    refine ⟨none, ?_, by simp [Quantity.div]⟩
    unfold Extracted.checked_div
    split
    · simp [Extracted.core.intrinsics.cold_path]
    · rename_i hne
      exact False.elim (hne rfl)
  · obtain ⟨result, hop, hval⟩ := UScalar.div_spec left h
    have hop' : UScalar.div left right = Result.ok result := hop
    have hzval : (⟨0#64⟩ : Std.U64).val = 0 := rfl
    refine ⟨some result, ?_, ?_⟩
    · unfold Extracted.checked_div
      split <;> simp_all [Std.U64.div]
    · simp [Quantity.div, h, hval]

end Axionomy.QuantityRefinement
