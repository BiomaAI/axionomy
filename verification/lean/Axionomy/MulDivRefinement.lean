import Axionomy.MulDiv
import Axionomy.AccountRefinement

/-! The extracted production `u64` mul-div widens both operands to `u128`,
multiplies, divides (adding one for a nonzero remainder when rounding up) and
narrows the quotient back to `u64`. These theorems prove, for every triple of
`u64` inputs, that every primitive step succeeds and that the result is exactly
`MulDiv.mulDiv u64Max`: only the final quotient is range-checked. -/
namespace Axionomy.MulDivRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

theorem spec_eq {α : Type} {m : Result α} {p : α → Prop} (h : WP.spec m p) :
    ∃ value, m = .ok value ∧ p value := by
  cases m <;> simp_all

theorem map_val_injective {a b : Option Std.U64}
    (h : a.map UScalar.val = b.map UScalar.val) : a = b := by
  cases a <;> cases b <;> simp_all
  exact UScalar.eq_imp _ _ h

theorem u64_bound (value : Nat) (h : value ≤ Quantity.u64Max) :
    value < 2 ^ UScalarTy.U64.numBits :=
  Nat.lt_of_le_of_lt h (by simp [Quantity.u64Max, UScalarTy.numBits])

/-- Pure `u64` view of the specified quotient. -/
def mulDiv (rounding : MulDiv.Rounding) (left right denominator : Std.U64) : Option Std.U64 :=
  if h : denominator.val ≠ 0 ∧
      MulDiv.quotient rounding left.val right.val denominator.val ≤ Quantity.u64Max then
    some (UScalar.ofNatCore (MulDiv.quotient rounding left.val right.val denominator.val)
      (u64_bound _ h.2))
  else none

theorem mulDiv_val (rounding : MulDiv.Rounding) (left right denominator : Std.U64) :
    (mulDiv rounding left right denominator).map UScalar.val =
      MulDiv.mulDiv Quantity.u64Max rounding left.val right.val denominator.val := by
  unfold mulDiv MulDiv.mulDiv Quantity.checked
  by_cases zero : denominator.val = 0
  · simp [zero]
  · by_cases fits : MulDiv.quotient rounding left.val right.val denominator.val ≤ Quantity.u64Max
    · simp [zero, fits, UScalar.ofNatCore_val_eq]
    · simp [zero, fits]

theorem widen_val (value : Std.U64) : (UScalar.cast .U128 value).val = value.val :=
  U64.cast_U128_val_eq value

theorem product_fits (left right : Std.U64) :
    (UScalar.cast .U128 left).val * (UScalar.cast .U128 right).val ≤ U128.max := by
  rw [widen_val, widen_val]
  have hl : left.val ≤ 2^64 - 1 := by have := left.hBounds; simp [UScalarTy.numBits] at this; omega
  have hr : right.val ≤ 2^64 - 1 := by have := right.hBounds; simp [UScalarTy.numBits] at this; omega
  have := Nat.mul_le_mul hl hr
  have : (2^64 - 1) * (2^64 - 1) ≤ U128.max := by simp [U128.max, U128.numBits]
  omega

/-- Narrowing returns the value exactly when it fits `u64`. -/
theorem narrow_spec (value : Std.U128) :
    ∃ result, quantity.narrow value = .ok result ∧
      result.map UScalar.val = Quantity.checked Quantity.u64Max value.val := by
  have maxVal : (UScalar.cast .U128 core.num.U64.MAX).val = Quantity.u64Max := by
    rw [widen_val]; simp [core.num.U64.MAX, U64.rMax, Quantity.u64Max, UScalar.ofNatCore_val_eq]
  have rmax : U64.rMax = Quantity.u64Max := by simp [U64.rMax, Quantity.u64Max]
  unfold quantity.narrow Quantity.checked
  by_cases fits : value.val ≤ Quantity.u64Max
  · have notGreater : ¬ value > UScalar.cast .U128 core.num.U64.MAX := by
      rw [gt_iff_lt, UScalar.lt_equiv, maxVal]; omega
    refine ⟨some (UScalar.cast .U64 value), by simp [lift, rmax, fits], ?_⟩
    simp only [Option.map_some, fits, ite_true, Option.some.injEq]
    exact UScalar.cast_val_mod_pow_of_inBounds_eq _ _ (u64_bound _ fits)
  · have greater : value > UScalar.cast .U128 core.num.U64.MAX := by
      rw [gt_iff_lt, UScalar.lt_equiv, maxVal]; omega
    exact ⟨none, by simp [lift, rmax]; omega, by simp [fits]⟩

/-- The production `Rounding` enum maps one-to-one onto the specification's. -/
def decodeRounding : quantity.Rounding → MulDiv.Rounding
  | .Floor => .floor
  | .Ceil => .ceil

theorem decodeRounding_injective {a b : quantity.Rounding}
    (h : decodeRounding a = decodeRounding b) : a = b := by
  cases a <;> cases b <;> simp_all [decodeRounding]

theorem impl_spec (left right denominator : Std.U64) (rounding : quantity.Rounding) :
    ∃ result, U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_mul_div
        left right denominator rounding = .ok result ∧
      result.map UScalar.val =
        MulDiv.mulDiv Quantity.u64Max (decodeRounding rounding) left.val right.val denominator.val := by
  unfold U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_mul_div
  by_cases zero : denominator = 0#u64
  · have value : denominator.val = 0 := by simp [zero]
    exact ⟨none, by simp [zero], by simp [MulDiv.mulDiv, value]⟩
  · have nonzero : denominator.val ≠ 0 := fun h => zero (UScalar.eq_imp _ _ (by simp [h]))
    obtain ⟨product, hp, hpv⟩ := spec_eq (U128.mul_spec (product_fits left right))
    have divisorNonzero : (UScalar.cast .U128 denominator).val ≠ 0 := by
      rw [widen_val]; exact nonzero
    obtain ⟨q, hq, hqv⟩ := UScalar.div_spec product divisorNonzero
    rw [hpv, widen_val, widen_val, widen_val] at hqv
    cases rounding with
    | Floor =>
      obtain ⟨result, hn, hres⟩ := narrow_spec q
      refine ⟨result, by simp [zero, quantity.widen, hp, hq, hn], ?_⟩
      rw [hres, hqv]
      simp [MulDiv.mulDiv, MulDiv.quotient, nonzero, decodeRounding]
    | Ceil =>
      obtain ⟨rem, hr, hrv⟩ := spec_eq (UScalar.rem_spec product divisorNonzero)
      rw [hpv, widen_val, widen_val, widen_val] at hrv
      by_cases exact : rem = 0#u128
      · have remZero : left.val * right.val % denominator.val = 0 := by
          rw [← hrv]; simp [exact]
        obtain ⟨result, hn, hres⟩ := narrow_spec q
        refine ⟨result, by simp [zero, quantity.widen, hp, hq, hr, exact, hn], ?_⟩
        rw [hres, hqv]
        simp [MulDiv.mulDiv, MulDiv.quotient, nonzero, remZero, decodeRounding]
      · have remNonzero : left.val * right.val % denominator.val ≠ 0 := by
          intro h; apply exact; apply UScalar.eq_imp; rw [hrv, h]; simp
        have small : q.val + (1#u128).val ≤ U128.max := by
          have quotientLe := Nat.div_le_self (left.val * right.val) denominator.val
          have : (2^64 - 1) * (2^64 - 1) + 1 ≤ U128.max := by simp [U128.max, U128.numBits]
          have hl : left.val ≤ 2^64 - 1 := by
            have := left.hBounds; simp [UScalarTy.numBits] at this; omega
          have hr : right.val ≤ 2^64 - 1 := by
            have := right.hBounds; simp [UScalarTy.numBits] at this; omega
          have := Nat.mul_le_mul hl hr
          simp only [hqv]
          simp
          omega
        obtain ⟨succ, hs, hsv⟩ := spec_eq (U128.add_spec small)
        obtain ⟨result, hn, hres⟩ := narrow_spec succ
        refine ⟨result, by simp [zero, quantity.widen, hp, hq, hr, exact, hs, hn], ?_⟩
        rw [hres, hsv, hqv]
        simp [MulDiv.mulDiv, MulDiv.quotient, nonzero, remNonzero, decodeRounding]

theorem impl_exact (left right denominator : Std.U64) (rounding : quantity.Rounding) :
    U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_mul_div
      left right denominator rounding = .ok (mulDiv (decodeRounding rounding) left right denominator) := by
  obtain ⟨result, h, hv⟩ := impl_spec left right denominator rounding
  rw [h, map_val_injective (hv.trans (mulDiv_val _ left right denominator).symm)]

theorem from_checked_mulDiv (value : Option Std.U64) :
    quantity.Quantity.from_checked scalar value = .ok value := by
  cases value <;>
    simp [quantity.Quantity.from_checked, quantity.Quantity.try_from_scalar,
      U64.Insts.Account_extractionQuantityQuantityScalarI128.is_nonnegative]

@[simp] theorem mul_div_call (left right denominator : Std.U64) (rounding : quantity.Rounding) :
    quantity.Quantity.checked_mul_div scalar left right denominator rounding =
      .ok (mulDiv (decodeRounding rounding) left right denominator) := by
  simp [quantity.Quantity.checked_mul_div, scalar, impl_exact, from_checked_mulDiv]

/-- Public `Quantity<u64>::checked_mul_div`: the exact full-width quotient,
rounded as requested, or `None` exactly for a zero denominator or a quotient
outside `u64`. -/
theorem quantity_refines (left right denominator : Std.U64) (rounding : quantity.Rounding) :
    ∃ result, quantity_mul_div left right denominator rounding = .ok result ∧
      result.map UScalar.val =
        MulDiv.mulDiv Quantity.u64Max (decodeRounding rounding) left.val right.val denominator.val :=
  ⟨_, by simp [quantity_mul_div], mulDiv_val _ left right denominator⟩

end Axionomy.MulDivRefinement
