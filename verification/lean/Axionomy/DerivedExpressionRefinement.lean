import Axionomy.DerivedValueRefinement

namespace Axionomy.DerivedExpressionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement DerivedValueRefinement
variable {Role A N : Type}

deriving instance DecidableEq for quantity.Rounding
deriving instance DecidableEq for rate.QuantityExpression

/-- The derived `Rounding` clone and equality used inside expressions. -/
@[simp] theorem rounding_clone (value : quantity.Rounding) :
    quantity.Rounding.Insts.CoreCloneClone.clone value = .ok value := rfl

@[simp] theorem rounding_eq (left right : quantity.Rounding) :
    quantity.Rounding.Insts.CoreCmpPartialEqRounding.eq left right = .ok (decide (left = right)) := by
  cases left <;> cases right <;>
    simp [quantity.Rounding.Insts.CoreCmpPartialEqRounding.eq, quantity.Rounding.read_discriminant]

/-- A finite recursive expression retains its entire structure and values under
independently lawful primitive clones. The induction proves termination too. -/
theorem clone_expression (roleClone : core.clone.Clone Role) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (roleLaw : LawfulClone roleClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.Insts.CoreCloneClone.clone roleClone assetClone numberClone value = .ok value := by
  simp only [LawfulClone] at roleLaw assetLaw numberLaw
  induction value <;>
    simp [rate.QuantityExpression.Insts.CoreCloneClone.clone,
      quantity.Quantity.Insts.CoreCloneClone.clone, alloc.string.String.Insts.CoreCloneClone.clone,
      *]

theorem clone_public (roleClone : core.clone.Clone Role) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (roleLaw : LawfulClone roleClone)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : rate.QuantityExpression Role A N) :
    derived_expression_clone roleClone assetClone numberClone value = .ok value :=
  clone_expression roleClone assetClone numberClone roleLaw assetLaw numberLaw value

/-- Structural equality distinguishes all operators and compares every stored
payload, with no numeric algebra or total-order requirement. -/
theorem equal_expression [DecidableEq Role] [DecidableEq A] [DecidableEq N]
    (roleEq : core.cmp.PartialEq Role Role) (assetEq : core.cmp.PartialEq A A)
    (numberEq : core.cmp.PartialEq N N) (roleLaw : LawfulPartialEquality roleEq)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : rate.QuantityExpression Role A N) :
    rate.QuantityExpression.Insts.CoreCmpPartialEqQuantityExpression.eq roleEq assetEq numberEq left right =
      .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at roleLaw assetLaw numberLaw
  induction left generalizing right <;> cases right <;>
    simp [rate.QuantityExpression.Insts.CoreCmpPartialEqQuantityExpression.eq,
      quantity.Quantity.Insts.CoreCmpPartialEqQuantity.eq, rate.QuantityExpression.read_discriminant,
      alloc.string.String.Insts.CoreCmpPartialEqString.eq,
      *]

theorem equal_public [DecidableEq Role] [DecidableEq A] [DecidableEq N]
    (roleEq : core.cmp.PartialEq Role Role) (assetEq : core.cmp.PartialEq A A)
    (numberEq : core.cmp.PartialEq N N) (roleLaw : LawfulPartialEquality roleEq)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : rate.QuantityExpression Role A N) :
    derived_expression_eq roleEq assetEq numberEq left right = .ok (decide (left = right)) :=
  equal_expression roleEq assetEq numberEq roleLaw assetLaw numberLaw left right

end Axionomy.DerivedExpressionRefinement
