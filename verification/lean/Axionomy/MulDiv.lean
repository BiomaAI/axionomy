import Axionomy.Quantity
import Mathlib.Tactic.Linarith

/-! Full-width multiply-then-divide with an explicit rounding direction.

The product `left * right` is an unbounded natural number. Only the final
quotient must fit the numeric bound, so a result is rejected exactly when the
denominator is zero or the rounded quotient itself is out of range. This is the
`mulDiv` primitive used by fixed-point ledgers: intermediate products never
overflow. -/
namespace Axionomy.MulDiv
open Quantity

inductive Rounding where
  | floor
  | ceil
  deriving DecidableEq, Repr

/-- Exact rounded quotient of `left * right / denominator`. The ceiling adds one
exactly when the division leaves a remainder, mirroring the implementation. -/
def quotient : Rounding → Nat → Nat → Nat → Nat
  | .floor, left, right, denominator => left * right / denominator
  | .ceil, left, right, denominator =>
    left * right / denominator + if left * right % denominator = 0 then 0 else 1

def mulDiv (limit : Nat) (rounding : Rounding) (left right denominator : Nat) : Option Nat :=
  if denominator = 0 then none else checked limit (quotient rounding left right denominator)

theorem success_iff (limit : Nat) (rounding : Rounding) (left right denominator result : Nat) :
    mulDiv limit rounding left right denominator = some result ↔
      denominator ≠ 0 ∧ quotient rounding left right denominator ≤ limit ∧
        result = quotient rounding left right denominator := by
  unfold mulDiv
  by_cases zero : denominator = 0
  · simp [zero]
  · simp [zero, checked_success]

theorem zero_denominator_rejected (limit : Nat) (rounding : Rounding) (left right : Nat) :
    mulDiv limit rounding left right 0 = none := by
  simp [mulDiv]

/-- Only the final quotient is range-checked; the product may exceed the limit. -/
theorem overflow_iff (limit : Nat) (rounding : Rounding) (left right denominator : Nat)
    (nonzero : denominator ≠ 0) :
    mulDiv limit rounding left right denominator = none ↔
      limit < quotient rounding left right denominator := by
  unfold mulDiv checked
  simp [nonzero]

theorem full_width_example :
    mulDiv u64Max .floor u64Max u64Max u64Max = some u64Max ∧
      u64Max < u64Max * u64Max := by
  constructor
  · simp [mulDiv, quotient, checked, u64Max]
  · simp [u64Max]

theorem floor_bounds (left right denominator : Nat) (nonzero : denominator ≠ 0) :
    quotient .floor left right denominator * denominator ≤ left * right ∧
      left * right < (quotient .floor left right denominator + 1) * denominator := by
  have positive : 0 < denominator := Nat.pos_of_ne_zero nonzero
  simp only [quotient]
  constructor
  · exact Nat.div_mul_le_self _ _
  · rw [Nat.add_mul, Nat.one_mul]
    have := Nat.lt_div_mul_add (a := left * right) positive
    linarith [Nat.div_add_mod (left * right) denominator, Nat.mod_lt (left * right) positive,
      Nat.mul_comm denominator (left * right / denominator)]

theorem ceil_bounds (left right denominator : Nat) (nonzero : denominator ≠ 0) :
    left * right ≤ quotient .ceil left right denominator * denominator ∧
      quotient .ceil left right denominator * denominator < left * right + denominator := by
  have positive : 0 < denominator := Nat.pos_of_ne_zero nonzero
  have split := Nat.div_add_mod (left * right) denominator
  have small := Nat.mod_lt (left * right) positive
  simp only [quotient]
  by_cases exact : left * right % denominator = 0
  · simp only [exact, ite_true, Nat.add_zero]
    constructor <;> nlinarith [Nat.mul_comm denominator (left * right / denominator)]
  · simp only [exact, ite_false]
    have rem_pos : 0 < left * right % denominator := Nat.pos_of_ne_zero exact
    constructor <;> nlinarith [Nat.mul_comm denominator (left * right / denominator)]

theorem floor_le_ceil (left right denominator : Nat) :
    quotient .floor left right denominator ≤ quotient .ceil left right denominator := by
  simp only [quotient]
  omega

theorem ceil_le_floor_succ (left right denominator : Nat) :
    quotient .ceil left right denominator ≤ quotient .floor left right denominator + 1 := by
  simp only [quotient]
  split <;> omega

theorem ceil_eq_floor_iff (left right denominator : Nat) :
    quotient .ceil left right denominator = quotient .floor left right denominator ↔
      left * right % denominator = 0 := by
  simp only [quotient]
  split <;> simp_all

theorem floor_mono_left (left left' right denominator : Nat) (h : left ≤ left') :
    quotient .floor left right denominator ≤ quotient .floor left' right denominator :=
  Nat.div_le_div_right (Nat.mul_le_mul_right right h)

theorem floor_mono_right (left right right' denominator : Nat) (h : right ≤ right') :
    quotient .floor left right denominator ≤ quotient .floor left right' denominator :=
  Nat.div_le_div_right (Nat.mul_le_mul_left left h)

theorem floor_anti_denominator (left right denominator denominator' : Nat)
    (positive : 0 < denominator) (h : denominator ≤ denominator') :
    quotient .floor left right denominator' ≤ quotient .floor left right denominator :=
  Nat.div_le_div_left h positive

/-- Ceiling division is `⌈p / d⌉ = ⌊(p + d - 1) / d⌋`, so it inherits monotonicity. -/
theorem ceil_closed_form (left right denominator : Nat) (nonzero : denominator ≠ 0) :
    quotient .ceil left right denominator = (left * right + denominator - 1) / denominator := by
  have positive : 0 < denominator := Nat.pos_of_ne_zero nonzero
  have bounds := ceil_bounds left right denominator nonzero
  symm
  rw [Nat.div_eq_iff positive]
  constructor <;> omega

theorem ceil_mono_left (left left' right denominator : Nat) (nonzero : denominator ≠ 0)
    (h : left ≤ left') :
    quotient .ceil left right denominator ≤ quotient .ceil left' right denominator := by
  rw [ceil_closed_form _ _ _ nonzero, ceil_closed_form _ _ _ nonzero]
  exact Nat.div_le_div_right (by have := Nat.mul_le_mul_right right h; omega)

theorem ceil_mono_right (left right right' denominator : Nat) (nonzero : denominator ≠ 0)
    (h : right ≤ right') :
    quotient .ceil left right denominator ≤ quotient .ceil left right' denominator := by
  rw [ceil_closed_form _ _ _ nonzero, ceil_closed_form _ _ _ nonzero]
  exact Nat.div_le_div_right (by have := Nat.mul_le_mul_left left h; omega)

theorem ceil_anti_denominator (left right denominator denominator' : Nat)
    (positive : 0 < denominator) (h : denominator ≤ denominator') :
    quotient .ceil left right denominator' ≤ quotient .ceil left right denominator := by
  have nonzero : denominator ≠ 0 := Nat.pos_iff_ne_zero.mp positive
  have nonzero' : denominator' ≠ 0 := by omega
  have small := (ceil_bounds left right denominator nonzero).1
  -- The larger denominator's ceiling also covers the product.
  by_contra greater
  have strict : quotient .ceil left right denominator + 1 ≤ quotient .ceil left right denominator' := by
    omega
  have large := (ceil_bounds left right denominator' nonzero').2
  have : (quotient .ceil left right denominator + 1) * denominator ≤
      quotient .ceil left right denominator' * denominator' :=
    Nat.mul_le_mul strict h
  nlinarith

/-- Floor division by a unit factor is plain natural-number division. -/
theorem floor_unit (numerator denominator : Nat) :
    quotient .floor numerator 1 denominator = numerator / denominator := by
  simp [quotient]

theorem mulDiv_floor_unit (limit numerator denominator : Nat) (bounded : numerator ≤ limit) :
    mulDiv limit .floor numerator 1 denominator = div numerator denominator := by
  unfold mulDiv div checked
  by_cases zero : denominator = 0
  · simp [zero]
  · simp [zero, quotient, Nat.le_trans (Nat.div_le_self numerator denominator) bounded]

end Axionomy.MulDiv
