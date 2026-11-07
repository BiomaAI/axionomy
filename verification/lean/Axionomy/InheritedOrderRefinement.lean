import Axionomy.InheritedEqualityRefinement

namespace Axionomy.InheritedOrderRefinement
open Aeneas Aeneas.Std AccountExtracted DerivedValueRefinement
variable {N : Type}

/- Ord's inherited helpers use the core type's derived PartialOrd methods,
which use the payload partial_cmp. They do not use payload helper overrides. -/
theorem quantity_min_effect (inst : core.cmp.Ord N) (left right : N) :
    inherited_quantity_min inst left right = (do
      let less ← derived_quantity_lt inst.partialOrdInst right left
      if less then .ok right else .ok left) := by
  simp [inherited_quantity_min, core.cmp.Ord.min.trait_default, core.cmp.Ord.min.default, core.cmp.Ord.min_body, core.cmp.PartialOrd.lt.trait_default, derived_quantity_lt]

theorem quantity_max_effect (inst : core.cmp.Ord N) (left right : N) :
    inherited_quantity_max inst left right = (do
      let less ← derived_quantity_lt inst.partialOrdInst right left
      if less then .ok left else .ok right) := by
  simp [inherited_quantity_max, core.cmp.Ord.max.trait_default, core.cmp.Ord.max.default, core.cmp.Ord.max_body, core.cmp.PartialOrd.lt.trait_default, derived_quantity_lt]

theorem quantity_clamp_effect (inst : core.cmp.Ord N) (value lower upper : N) :
    inherited_quantity_clamp inst value lower upper = (do
      let valid ← derived_quantity_le inst.partialOrdInst lower upper
      massert valid
      let below ← derived_quantity_lt inst.partialOrdInst value lower
      if below then .ok lower else
        let above ← derived_quantity_gt inst.partialOrdInst value upper
        if above then .ok upper else .ok value) := by
  simp [inherited_quantity_clamp, core.cmp.Ord.clamp.trait_default, core.cmp.Ord.clamp.default, core.cmp.Ord.clamp_body, core.cmp.PartialOrd.le.trait_default, core.cmp.PartialOrd.lt.trait_default, core.cmp.PartialOrd.gt.trait_default, derived_quantity_le, derived_quantity_lt, derived_quantity_gt]

theorem quantity_ties (inst : core.cmp.Ord N) (left right : N)
    (equal : inst.partialOrdInst.partial_cmp right left = .ok (some .eq)) :
    inherited_quantity_min inst left right = .ok left ∧
    inherited_quantity_max inst left right = .ok right := by
  simp [quantity_min_effect, quantity_max_effect, quantity_lt_effect, equal, accepts]

theorem quantity_incomparable (inst : core.cmp.Ord N) (left right : N)
    (incomparable : inst.partialOrdInst.partial_cmp right left = .ok none) :
    inherited_quantity_min inst left right = .ok left ∧
    inherited_quantity_max inst left right = .ok right := by
  simp [quantity_min_effect, quantity_max_effect, quantity_lt_effect, incomparable, accepts]

theorem quantity_order_failure (inst : core.cmp.Ord N) (left right : N) (error : Error)
    (failed : inst.partialOrdInst.partial_cmp right left = .fail error) :
    inherited_quantity_min inst left right = .fail error ∧
    inherited_quantity_max inst left right = .fail error := by
  simp [quantity_min_effect, quantity_max_effect, quantity_lt_effect, failed]

theorem quantity_clamp_invalid (inst : core.cmp.Ord N) (value lower upper : N)
    (order : Option Ordering) (compared : inst.partialOrdInst.partial_cmp lower upper = .ok order)
    (invalid : accepts [.lt, .eq] order = false) :
    inherited_quantity_clamp inst value lower upper = .fail .assertionFailure := by
  simp [quantity_clamp_effect, quantity_le_effect, compared, invalid, massert]

theorem quantity_clamp_range_failure (inst : core.cmp.Ord N) (value lower upper : N) (error : Error)
    (failed : inst.partialOrdInst.partial_cmp lower upper = .fail error) :
    inherited_quantity_clamp inst value lower upper = .fail error := by
  simp [quantity_clamp_effect, quantity_le_effect, failed]

/-- A primitive comparison law. It permits distinguishable values with equal
keys, so tie selection remains observable instead of being hidden by equality. -/
def NatOrder (inst : core.cmp.Ord N) (key : N → Nat) : Prop :=
  ∀ left right, inst.partialOrdInst.partial_cmp left right = .ok (some (compare (key left) (key right)))

@[simp] theorem accepts_lt (left right : Nat) :
    accepts [.lt] (some (compare left right)) = decide (left < right) := by
  simp [accepts, compare, compareOfLessAndEq]
  split <;> simp_all

@[simp] theorem accepts_le (left right : Nat) :
    accepts [.lt, .eq] (some (compare left right)) = decide (left ≤ right) := by
  simp [accepts, compare, compareOfLessAndEq]
  split <;> (try split) <;> simp_all <;> omega

@[simp] theorem accepts_gt (left right : Nat) :
    accepts [.gt] (some (compare left right)) = decide (right < left) := by
  simp [accepts, compare, compareOfLessAndEq]
  split <;> (try split) <;> simp_all <;> omega

theorem quantity_min (inst : core.cmp.Ord N) (key : N → Nat) (law : NatOrder inst key)
    (left right : N) :
    inherited_quantity_min inst left right = .ok (if key right < key left then right else left) := by
  simp [quantity_min_effect, quantity_lt_effect, law right] at *
  split <;> rfl

theorem quantity_max (inst : core.cmp.Ord N) (key : N → Nat) (law : NatOrder inst key)
    (left right : N) :
    inherited_quantity_max inst left right = .ok (if key right < key left then left else right) := by
  simp [quantity_max_effect, quantity_lt_effect, law right] at *
  split <;> rfl

def clampBy {T : Type} (key : T → Nat) (value lower upper : T) : Result T :=
  if key lower ≤ key upper then
    .ok (if key value < key lower then lower else if key upper < key value then upper else value)
  else .fail .assertionFailure

theorem quantity_clamp (inst : core.cmp.Ord N) (key : N → Nat) (law : NatOrder inst key)
    (value lower upper : N) :
    inherited_quantity_clamp inst value lower upper = clampBy key value lower upper := by
  simp only [NatOrder] at law
  simp [quantity_clamp_effect, quantity_le_effect, quantity_lt_effect, quantity_gt_effect,
    law, clampBy, massert]
  split_ifs <;> simp_all

theorem fingerprint_min_effect (left right : economy.StateFingerprint) :
    inherited_fingerprint_min left right = (do
      let less ← derived_fingerprint_lt right left
      if less then .ok right else .ok left) := by
  simp [inherited_fingerprint_min, core.cmp.Ord.min.trait_default, core.cmp.Ord.min.default, core.cmp.Ord.min_body, core.cmp.PartialOrd.lt.trait_default, derived_fingerprint_lt]

theorem fingerprint_max_effect (left right : economy.StateFingerprint) :
    inherited_fingerprint_max left right = (do
      let less ← derived_fingerprint_lt right left
      if less then .ok left else .ok right) := by
  simp [inherited_fingerprint_max, core.cmp.Ord.max.trait_default, core.cmp.Ord.max.default, core.cmp.Ord.max_body, core.cmp.PartialOrd.lt.trait_default, derived_fingerprint_lt]

theorem fingerprint_clamp_effect (value lower upper : economy.StateFingerprint) :
    inherited_fingerprint_clamp value lower upper = (do
      let valid ← derived_fingerprint_le lower upper
      massert valid
      let below ← derived_fingerprint_lt value lower
      if below then .ok lower else
        let above ← derived_fingerprint_gt value upper
        if above then .ok upper else .ok value) := by
  simp [inherited_fingerprint_clamp, core.cmp.Ord.clamp.trait_default, core.cmp.Ord.clamp.default, core.cmp.Ord.clamp_body, core.cmp.PartialOrd.le.trait_default, core.cmp.PartialOrd.lt.trait_default, core.cmp.PartialOrd.gt.trait_default, derived_fingerprint_le, derived_fingerprint_lt, derived_fingerprint_gt]

theorem fingerprint_min (left right : economy.StateFingerprint) :
    inherited_fingerprint_min left right = .ok (if right.val < left.val then right else left) := by
  simp [fingerprint_min_effect, DerivedKeyRefinement.fingerprint_lt]
  split <;> rfl

theorem fingerprint_max (left right : economy.StateFingerprint) :
    inherited_fingerprint_max left right = .ok (if right.val < left.val then left else right) := by
  simp [fingerprint_max_effect, DerivedKeyRefinement.fingerprint_lt]
  split <;> rfl

theorem fingerprint_clamp (value lower upper : economy.StateFingerprint) :
    inherited_fingerprint_clamp value lower upper = clampBy UScalar.val value lower upper := by
  simp [fingerprint_clamp_effect, DerivedKeyRefinement.fingerprint_le,
    DerivedKeyRefinement.fingerprint_lt, DerivedKeyRefinement.fingerprint_gt, clampBy, massert]
  split_ifs <;> simp_all

end Axionomy.InheritedOrderRefinement
