import Axionomy.TypedDataRefinement

namespace Axionomy.DerivedValueRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement
variable {A N H : Type}

/-- Independent law on the primitive payload comparator, rather than on the
result of any Axionomy composite comparison. -/
def LawfulPartialEquality {T : Type} [DecidableEq T] (inst : core.cmp.PartialEq T T) : Prop :=
  ∀ left right, inst.eq left right = .ok (decide (left = right))

def accepts (allowed : List Ordering) (result : Option Ordering) : Bool :=
  match result with
  | none => false
  | some order => allowed.contains order

@[simp] theorem guard_ok (condition : Prop) [Decidable condition] (value : Bool) :
    (if condition then Result.ok value else .ok false) = (Result.ok (decide condition && value)) := by
  by_cases condition <;> simp_all

theorem quantity_eq_effect (inst : core.cmp.PartialEq N N) (left right : N) :
    derived_quantity_eq inst left right = inst.eq left right := rfl

theorem quantity_clone_effect (inst : core.clone.Clone N) (value : N) :
    derived_quantity_clone inst value = inst.clone value := by
  simp [derived_quantity_clone, quantity.Quantity.Insts.CoreCloneClone.clone]

theorem quantity_partial_cmp_effect (inst : core.cmp.PartialOrd N N) (left right : N) :
    derived_quantity_partial_cmp inst left right = inst.partial_cmp left right := rfl

theorem quantity_cmp_effect (inst : core.cmp.Ord N) (left right : N) :
    derived_quantity_cmp inst left right = inst.cmp left right := rfl

/- Derived PartialOrd uses its default methods through partial_cmp, even if
its payload overrides lt/le/gt/ge. Incomparability makes all four false. -/
theorem quantity_lt_effect (inst : core.cmp.PartialOrd N N) (left right : N) :
    derived_quantity_lt inst left right = (do
      let order ← inst.partial_cmp left right
      .ok (accepts [.lt] order)) := by
  simp only [derived_quantity_lt, core.cmp.PartialOrd.lt.trait_default,
    core.cmp.PartialOrd.lt.default, core.cmp.PartialOrd.lt_body,
    quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp]
  congr 1
  funext order
  cases order with
  | none => rfl
  | some order => cases order <;> rfl

theorem quantity_le_effect (inst : core.cmp.PartialOrd N N) (left right : N) :
    derived_quantity_le inst left right = (do
      let order ← inst.partial_cmp left right
      .ok (accepts [.lt, .eq] order)) := by
  simp only [derived_quantity_le, core.cmp.PartialOrd.le.trait_default,
    core.cmp.PartialOrd.le.default, core.cmp.PartialOrd.le_body,
    quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp]
  congr 1
  funext order
  cases order with
  | none => rfl
  | some order => cases order <;> rfl

theorem quantity_gt_effect (inst : core.cmp.PartialOrd N N) (left right : N) :
    derived_quantity_gt inst left right = (do
      let order ← inst.partial_cmp left right
      .ok (accepts [.gt] order)) := by
  simp only [derived_quantity_gt, core.cmp.PartialOrd.gt.trait_default,
    core.cmp.PartialOrd.gt.default, core.cmp.PartialOrd.gt_body,
    quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp]
  congr 1
  funext order
  cases order with
  | none => rfl
  | some order => cases order <;> rfl

theorem quantity_ge_effect (inst : core.cmp.PartialOrd N N) (left right : N) :
    derived_quantity_ge inst left right = (do
      let order ← inst.partial_cmp left right
      .ok (accepts [.eq, .gt] order)) := by
  simp only [derived_quantity_ge, core.cmp.PartialOrd.ge.trait_default,
    core.cmp.PartialOrd.ge.default, core.cmp.PartialOrd.ge_body,
    quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp]
  congr 1
  funext order
  cases order with
  | none => rfl
  | some order => cases order <;> rfl

theorem quantity_hash_effect (inst : core.hash.Hash N) (hasher : core.hash.Hasher H)
    (value : N) (state : H) : derived_quantity_hash inst hasher value state = inst.hash hasher value state := rfl

theorem amount_eq_effect (assetEq : core.cmp.PartialEq A A) (numberEq : core.cmp.PartialEq N N)
    (left right : amount.AssetAmount A N) :
    derived_amount_eq assetEq numberEq left right = (do
      let same ← assetEq.eq left.asset right.asset
      if same then numberEq.eq left.quantity right.quantity else .ok false) := rfl

theorem amount_eq [DecidableEq A] [DecidableEq N]
    (assetEq : core.cmp.PartialEq A A) (numberEq : core.cmp.PartialEq N N)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : amount.AssetAmount A N) :
    derived_amount_eq assetEq numberEq left right =
      .ok (decide (left.asset = right.asset ∧ left.quantity = right.quantity)) := by
  simp only [LawfulPartialEquality] at assetLaw numberLaw
  by_cases same : left.asset = right.asset <;>
    simp [amount_eq_effect, assetLaw, numberLaw, same]

theorem amount_clone_effect (assetClone : core.clone.Clone A) (numberClone : core.clone.Clone N)
    (value : amount.AssetAmount A N) : derived_amount_clone assetClone numberClone value = (do
      let asset ← assetClone.clone value.asset
      let quantity ← numberClone.clone value.quantity
      .ok ⟨asset, quantity⟩) := by
  simp [derived_amount_clone, amount.AssetAmount.Insts.CoreCloneClone.clone,
    quantity.Quantity.Insts.CoreCloneClone.clone]

theorem amount_clone (assetClone : core.clone.Clone A) (numberClone : core.clone.Clone N)
    (assetLaw : LawfulClone assetClone) (numberLaw : LawfulClone numberClone)
    (value : amount.AssetAmount A N) : derived_amount_clone assetClone numberClone value = .ok value := by
  simp [amount_clone_effect, assetLaw value.asset, numberLaw value.quantity]

theorem amount_hash_effect (assetHash : core.hash.Hash A) (numberHash : core.hash.Hash N)
    (hasher : core.hash.Hasher H) (value : amount.AssetAmount A N) (state : H) :
    derived_amount_hash assetHash numberHash hasher value state = (do
      let next ← assetHash.hash hasher value.asset state
      numberHash.hash hasher value.quantity next) := rfl

theorem quantity_incomparable (inst : core.cmp.PartialOrd N N) (left right : N)
    (incomparable : inst.partial_cmp left right = .ok none) :
    derived_quantity_lt inst left right = .ok false ∧
    derived_quantity_le inst left right = .ok false ∧
    derived_quantity_gt inst left right = .ok false ∧
    derived_quantity_ge inst left right = .ok false := by
  simp [quantity_lt_effect, quantity_le_effect, quantity_gt_effect, quantity_ge_effect,
    incomparable, accepts]

theorem quantity_order_failure (inst : core.cmp.PartialOrd N N) (left right : N) (error : Error)
    (failed : inst.partial_cmp left right = .fail error) :
    derived_quantity_lt inst left right = .fail error ∧
    derived_quantity_le inst left right = .fail error ∧
    derived_quantity_gt inst left right = .fail error ∧
    derived_quantity_ge inst left right = .fail error := by
  simp [quantity_lt_effect, quantity_le_effect, quantity_gt_effect, quantity_ge_effect, failed]

theorem amount_unequal_asset (assetEq : core.cmp.PartialEq A A) (numberEq : core.cmp.PartialEq N N)
    (left right : amount.AssetAmount A N) (different : assetEq.eq left.asset right.asset = .ok false) :
    derived_amount_eq assetEq numberEq left right = .ok false := by
  simp [amount_eq_effect, different]

theorem amount_comparison_failure (assetEq : core.cmp.PartialEq A A) (numberEq : core.cmp.PartialEq N N)
    (left right : amount.AssetAmount A N) (error : Error)
    (failed : assetEq.eq left.asset right.asset = .fail error) :
    derived_amount_eq assetEq numberEq left right = .fail error := by
  simp [amount_eq_effect, failed]

theorem amount_clone_failure (assetClone : core.clone.Clone A) (numberClone : core.clone.Clone N)
    (value : amount.AssetAmount A N) (error : Error) (failed : assetClone.clone value.asset = .fail error) :
    derived_amount_clone assetClone numberClone value = .fail error := by
  simp [amount_clone_effect, failed]

theorem amount_hash_failure (assetHash : core.hash.Hash A) (numberHash : core.hash.Hash N)
    (hasher : core.hash.Hasher H) (value : amount.AssetAmount A N) (state : H) (error : Error)
    (failed : assetHash.hash hasher value.asset state = .fail error) :
    derived_amount_hash assetHash numberHash hasher value state = .fail error := by
  simp [amount_hash_effect, failed]

end Axionomy.DerivedValueRefinement
