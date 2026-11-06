import Std

/-! Extensional state and sparse-entry semantics. Account and asset domains
remain distinct type parameters. This does not verify IndexMap, sorting,
serialization, hashing, or allocation. -/
namespace Axionomy.State

abbrev World (Account Asset : Type) := Account → Asset → Nat

def set {Account Asset : Type} [DecidableEq Account] [DecidableEq Asset]
    (state : World Account Asset) (account : Account) (asset : Asset) (quantity : Nat) :
    World Account Asset :=
  fun a x => if a = account ∧ x = asset then quantity else state a x

theorem set_selected {Account Asset : Type} [DecidableEq Account] [DecidableEq Asset]
    (state : World Account Asset) (account : Account) (asset : Asset) (quantity : Nat) :
    set state account asset quantity account asset = quantity := by
  simp [set]

theorem set_frame {Account Asset : Type} [DecidableEq Account] [DecidableEq Asset]
    (state : World Account Asset) (account other : Account) (asset otherAsset : Asset)
    (quantity : Nat) (h : other ≠ account ∨ otherAsset ≠ asset) :
    set state account asset quantity other otherAsset = state other otherAsset := by
  rcases h with h | h <;> simp [set, h]

def encode (quantity : Nat) : Option Nat := if quantity = 0 then none else some quantity
def decode (entry : Option Nat) : Nat := entry.getD 0

theorem sparse_round_trip (quantity : Nat) : decode (encode quantity) = quantity := by
  by_cases h : quantity = 0 <;> simp [encode, decode, h]

theorem canonical_zero : encode 0 = none := by rfl

def shortfall (available required : Nat) : Nat := required - available

theorem shortfall_complete (available required : Nat) :
    shortfall available required = 0 ↔ required ≤ available := by
  unfold shortfall
  omega

def observes {Account Asset : Type} (visible : Account → Bool)
    (state : World Account Asset) (account : Account) (asset : Asset) : Option Nat :=
  if visible account then some (state account asset) else none

theorem hidden_account {Account Asset : Type} (visible : Account → Bool)
    (state : World Account Asset) (account : Account) (asset : Asset)
    (h : visible account = false) : observes visible state account asset = none := by
  simp [observes, h]

theorem observation_noninterference {Account Asset : Type} (visible : Account → Bool)
    (left right : World Account Asset)
    (h : ∀ account asset, visible account = true → left account asset = right account asset) :
    observes visible left = observes visible right := by
  funext account asset
  cases hv : visible account with
  | false => simp [observes, hv]
  | true => simp [observes, hv, h account asset hv]

def satisfies {Account Asset : Type} (state required : World Account Asset) : Prop :=
  ∀ account asset, required account asset ≤ state account asset

theorem goal_monotone {Account Asset : Type} (left right required : World Account Asset)
    (h : satisfies left required) (extension : ∀ a x, left a x ≤ right a x) :
    satisfies right required := by
  intro a x
  exact Nat.le_trans (h a x) (extension a x)

end Axionomy.State
