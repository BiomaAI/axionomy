import Axionomy.Quantity

/-! Pure pre-snapshot expression evaluation and aggregated effects. Rust role
lookup, map iteration, and checked aggregation still need refinement proofs. -/
namespace Axionomy.Rates

inductive Op where
  | add | sub | mul | div | min | max
  deriving Repr

def operate (op : Op) (left right : Nat) : Option Nat :=
  match op with
  | .add => some (left + right)
  | .sub => Quantity.sub left right
  | .mul => some (left * right)
  | .div => Quantity.div left right
  | .min => some (min left right)
  | .max => some (max left right)

inductive Expr (Role Asset Parameter : Type) where
  | constant (value : Nat)
  | units
  | parameter (name : Parameter)
  | balance (role : Role) (asset : Asset)
  | binary (op : Op) (left right : Expr Role Asset Parameter)

structure Context (Role Asset Parameter : Type) where
  units : Nat
  parameter : Parameter → Option Nat
  balance : Role → Asset → Option Nat

def eval {Role Asset Parameter : Type} (limit : Nat) (ctx : Context Role Asset Parameter) :
    Expr Role Asset Parameter → Option Nat
  | .constant v => Quantity.checked limit v
  | .units => Quantity.checked limit ctx.units
  | .parameter name => (ctx.parameter name).bind (Quantity.checked limit)
  | .balance role asset => (ctx.balance role asset).bind (Quantity.checked limit)
  | .binary op left right => do
      let l ← eval limit ctx left
      let r ← eval limit ctx right
      let value ← operate op l r
      Quantity.checked limit value

def mathematical {Role Asset Parameter : Type} (ctx : Context Role Asset Parameter) :
    Expr Role Asset Parameter → Option Nat
  | .constant v => some v
  | .units => some ctx.units
  | .parameter name => ctx.parameter name
  | .balance role asset => ctx.balance role asset
  | .binary op left right => do
      let l ← mathematical ctx left
      let r ← mathematical ctx right
      operate op l r

theorem bind_checked {limit result : Nat} {source : Option Nat}
    (h : source.bind (Quantity.checked limit) = some result) :
    source = some result ∧ result ≤ limit := by
  cases source with
  | none => simp at h
  | some value =>
    have hs := (Quantity.checked_success limit value result).mp h
    rcases hs with ⟨bound, rfl⟩
    exact ⟨rfl, bound⟩

theorem eval_refines_mathematical {Role Asset Parameter : Type}
    (limit : Nat) (ctx : Context Role Asset Parameter) (expr : Expr Role Asset Parameter)
    (result : Nat) (h : eval limit ctx expr = some result) :
    mathematical ctx expr = some result ∧ result ≤ limit := by
  induction expr generalizing result with
  | constant v =>
    have hs := (Quantity.checked_success limit v result).mp h
    rcases hs with ⟨bound, rfl⟩
    exact ⟨rfl, bound⟩
  | units =>
    have hs := (Quantity.checked_success limit ctx.units result).mp h
    rcases hs with ⟨bound, rfl⟩
    exact ⟨rfl, bound⟩
  | parameter name => exact bind_checked h
  | balance role asset => exact bind_checked h
  | binary op left right ihl ihr =>
    cases hl : eval limit ctx left with
    | none => simp [eval, hl] at h
    | some l =>
      cases hr : eval limit ctx right with
      | none => simp [eval, hl, hr] at h
      | some r =>
        have hc : (operate op l r).bind (Quantity.checked limit) = some result := by
          simpa [eval, hl, hr] using h
        have hs := bind_checked hc
        constructor
        · simp [mathematical, (ihl l hl).1, (ihr r hr).1, hs.1]
        · exact hs.2

structure Effect where
  consume : Nat
  produce : Nat
  preserve : Nat
  deriving DecidableEq

def merge (left right : Effect) : Effect :=
  ⟨left.consume + right.consume, left.produce + right.produce, left.preserve + right.preserve⟩

def scale (units : Nat) (effect : Effect) : Effect :=
  ⟨effect.consume * units, effect.produce * units, effect.preserve⟩

theorem merge_commutative (left right : Effect) : merge left right = merge right left := by
  simp [merge, Nat.add_comm]

theorem merge_associative (a b c : Effect) : merge (merge a b) c = merge a (merge b c) := by
  simp [merge, Nat.add_assoc]

theorem preservation_unscaled (units : Nat) (effect : Effect) :
    (scale units effect).preserve = effect.preserve := by rfl

end Axionomy.Rates
