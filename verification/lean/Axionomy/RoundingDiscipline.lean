import Axionomy.MulDiv

/-! Rounding against the caller.

Fixed-point ledgers stay solvent by choosing each rounding direction so that
rounding dust always remains with the pool or protocol: amounts a participant
pays round up, amounts a participant receives round down. These theorems state
that discipline for the `MulDiv` quotients and for the share-vault pattern used
by the lending reference model. They are model-level laws over exact naturals;
the kernel's own `MulDivFloor`/`MulDivCeil` refinement is proved separately. -/
namespace Axionomy.RoundingDiscipline
open MulDiv

/-- A floor-rounded payout never exceeds the exact value it represents. -/
theorem payout_never_exceeds_exact (amount numerator denominator : Nat)
    (nonzero : denominator ≠ 0) :
    quotient .floor amount numerator denominator * denominator ≤ amount * numerator :=
  (floor_bounds amount numerator denominator nonzero).1

/-- A ceiling-rounded charge never falls below the exact value it represents. -/
theorem charge_never_below_exact (amount numerator denominator : Nat)
    (nonzero : denominator ≠ 0) :
    amount * numerator ≤ quotient .ceil amount numerator denominator * denominator :=
  (ceil_bounds amount numerator denominator nonzero).1

/-- Charging the ceiling and paying out the floor of one exact ratio leaves the
counterparty with between zero and one atom of dust; it is never short. -/
theorem counterparty_keeps_dust (amount numerator denominator : Nat) :
    quotient .floor amount numerator denominator ≤ quotient .ceil amount numerator denominator ∧
      quotient .ceil amount numerator denominator -
        quotient .floor amount numerator denominator ≤ 1 := by
  have low := floor_le_ceil amount numerator denominator
  have high := ceil_le_floor_succ amount numerator denominator
  omega

/-- Shares minted by a deposit: `⌊assets · totalShares / totalAssets⌋`. -/
def depositShares (totalAssets totalShares assets : Nat) : Nat :=
  quotient .floor assets totalShares totalAssets

/-- Assets paid out when redeeming: `⌊shares · totalAssets / totalShares⌋`. -/
def redeemAssets (totalAssets totalShares shares : Nat) : Nat :=
  quotient .floor shares totalAssets totalShares

/-- Assets charged to mint shares: `⌈shares · totalAssets / totalShares⌉`. -/
def mintCost (totalAssets totalShares shares : Nat) : Nat :=
  quotient .ceil shares totalAssets totalShares

/-- Shares burned to withdraw assets: `⌈assets · totalShares / totalAssets⌉`. -/
def withdrawShares (totalAssets totalShares assets : Nat) : Nat :=
  quotient .ceil assets totalShares totalAssets

/-- Depositing and immediately redeeming the minted shares never returns more
than the deposit, for every vault state (including empty ones). -/
theorem deposit_redeem_never_profits (totalAssets totalShares assets : Nat) :
    let shares := depositShares totalAssets totalShares assets
    redeemAssets (totalAssets + assets) (totalShares + shares) shares ≤ assets := by
  intro shares
  have minted : shares * totalAssets ≤ assets * totalShares :=
    Nat.div_mul_le_self (assets * totalShares) totalAssets
  apply Nat.div_le_of_le_mul
  rw [Nat.mul_add, Nat.add_mul]
  have swap := Nat.mul_comm assets totalShares
  omega

/-- Paying the rounded-up mint cost and immediately redeeming never returns
more than was paid. -/
theorem mint_redeem_never_profits (totalAssets totalShares shares : Nat)
    (nonzero : totalShares ≠ 0) :
    let cost := mintCost totalAssets totalShares shares
    redeemAssets (totalAssets + cost) (totalShares + shares) shares ≤ cost := by
  intro cost
  have charged : shares * totalAssets ≤ cost * totalShares :=
    charge_never_below_exact shares totalAssets totalShares nonzero
  apply Nat.div_le_of_le_mul
  rw [Nat.mul_add, Nat.add_mul]
  have swap := Nat.mul_comm cost totalShares
  omega

/-- Withdrawing burns at least the exact share value of the withdrawn assets. -/
theorem withdraw_burns_enough (totalAssets totalShares assets : Nat)
    (nonzero : totalAssets ≠ 0) :
    assets * totalShares ≤ withdrawShares totalAssets totalShares assets * totalAssets :=
  charge_never_below_exact assets totalShares totalAssets nonzero

/-- A two-way proportional split that floors one share and assigns the explicit
remainder to the other conserves the total exactly. -/
theorem split_conserves (total weight totalWeight : Nat) (bounded : weight ≤ totalWeight) :
    let first := quotient .floor total weight totalWeight
    first ≤ total ∧ first + (total - first) = total := by
  intro first
  have fits : first ≤ total := by
    by_cases zero : totalWeight = 0
    · simp [first, quotient, zero]
    · have positive : 0 < totalWeight := Nat.pos_of_ne_zero zero
      apply Nat.div_le_of_le_mul
      have scaled := Nat.mul_le_mul_left total bounded
      have swap := Nat.mul_comm totalWeight total
      omega
  exact ⟨fits, by omega⟩

end Axionomy.RoundingDiscipline
