import Axionomy.BasketOperations

namespace Axionomy.Effects

-- Static effects scale once, then merge atomically. Computed effects use the
-- single-quantity path and do not acquire an implicit exchange-unit multiplier.
def scaled {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (mul : Q → Q → Option Q) (target source : List (A × Q)) (units : Q) : Except A (List (A × Q)) :=
  match BasketOperations.scale ops mul source units with
  | .error asset => .error asset
  | .ok amounts => Account.credit ops target amounts

theorem scaled_map {A Q R : Type} [DecidableEq A]
    (ops : Account.Arithmetic Q) (other : Account.Arithmetic R) (value : Q → R)
    (laws : Account.Hom ops other value) (mul : Q → Q → Option Q) (otherMul : R → R → Option R)
    (mulLaw : ∀ l r, (mul l r).map value = otherMul (value l) (value r))
    (target source : List (A × Q)) (units : Q) :
    (scaled ops mul target source units).map (Account.mapEntries value) =
      scaled other otherMul (Account.mapEntries value target) (Account.mapEntries value source) (value units) := by
  have scaleLaw := BasketOperations.map_scaleInto ops other value laws mul otherMul mulLaw units source []
  simp only [Account.mapEntries_nil] at scaleLaw
  unfold scaled BasketOperations.scale
  rw [← scaleLaw]
  cases h : BasketOperations.scaleInto ops mul units source [] with
  | error asset => rfl
  | ok entries => exact Account.map_credit ops other value laws entries target

end Axionomy.Effects
