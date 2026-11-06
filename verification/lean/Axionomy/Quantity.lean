import Std

/-! Exact arithmetic specification. A finite bound models u64's range; this
does not assume that arbitrary Rust QuantityScalar implementations obey it. -/
namespace Axionomy.Quantity

def u64Max : Nat := 2^64 - 1

def checked (limit value : Nat) : Option Nat :=
  if value ≤ limit then some value else none

theorem checked_success (limit value result : Nat) :
    checked limit value = some result ↔ value ≤ limit ∧ result = value := by
  by_cases h : value ≤ limit <;> simp [checked, h, eq_comm]

def add (limit left right : Nat) : Option Nat := checked limit (left + right)
def mul (limit left right : Nat) : Option Nat := checked limit (left * right)
def sub (left right : Nat) : Option Nat :=
  if right ≤ left then some (left - right) else none
def div (left right : Nat) : Option Nat :=
  if right = 0 then none else some (left / right)

theorem add_exact (limit left right result : Nat) (h : add limit left right = some result) :
    result = left + right ∧ result ≤ limit := by
  have hs := (checked_success limit (left + right) result).mp h
  omega

theorem mul_exact (limit left right result : Nat) (h : mul limit left right = some result) :
    result = left * right ∧ result ≤ limit := by
  have hs := (checked_success limit (left * right) result).mp h
  omega

theorem sub_exact (left right result : Nat) (h : sub left right = some result) :
    right ≤ left ∧ result + right = left := by
  unfold sub at h
  split at h <;> simp_all <;> omega

theorem div_exact (left right result : Nat) (h : div left right = some result) :
    right ≠ 0 ∧ result = left / right := by
  unfold div at h
  split at h <;> simp_all [eq_comm]

theorem underflow_rejected (left right : Nat) (h : left < right) : sub left right = none := by
  simp [sub, Nat.not_le_of_lt h]

theorem division_by_zero_rejected (left : Nat) : div left 0 = none := by
  simp [div]

-- Asset identity stays in the type; quantities never erase their asset domain.
structure Amount (Asset : Type) where
  asset : Asset
  quantity : Nat

def parts {Asset : Type} (amount : Amount Asset) : Asset × Nat :=
  (amount.asset, amount.quantity)

theorem amount_round_trip {Asset : Type} (amount : Amount Asset) :
    Amount.mk (parts amount).1 (parts amount).2 = amount := by
  rfl

end Axionomy.Quantity
