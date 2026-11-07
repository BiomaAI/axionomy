import Axionomy.BasketOperationsRefinement
import Axionomy.Effects

namespace Axionomy.EffectsRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement BasketOperationsRefinement Effects

section Updates
variable {A : Type} [DecidableEq A]
variable (cloneInst : core.clone.Clone A) (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
variable (hClone : LawfulClone cloneInst) (hEq : LawfulEquality eqInst)

include hClone hEq in
theorem scaled_refines (target source : List (A × Std.U64)) (units : Std.U64) :
    merge_effects cloneInst eqInst hashInst ⟨target⟩ ⟨source⟩ units =
      .ok (addResult target (scaled arithmetic U64.checked_mul target source units)) := by
  have hs := scale_refines cloneInst eqInst hashInst hClone hEq source units
  change basket.Basket.checked_scale cloneInst eqInst hashInst scalar ⟨source⟩ units = _ at hs
  have ha (entries) := add_refines cloneInst eqInst hashInst hClone hEq target entries
  simp only [basket_add] at ha
  simp only [merge_effects, economy.merge_scaled, hs, bind_ok]
  cases h : BasketOperations.scale arithmetic U64.checked_mul source units <;>
    simp [scaleResult, core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
      core.convert.FromSame.from, scaled, h, addResult, ha]

include hEq in
theorem amount_refines (target : List (A × Std.U64)) (asset : A) (amount : Std.U64) :
    merge_amount cloneInst eqInst hashInst ⟨target⟩ asset amount =
      .ok (addResult target (Account.credit arithmetic target [(asset, amount)])) := by
  simp only [merge_amount, economy.merge_quantity, quantity_call eqInst hashInst hEq, add_call, bind_ok]
  cases h : arithmetic.add (Account.balance arithmetic.zero target asset) amount <;>
    simp [h, store_call eqInst hashInst hEq, Account.credit, addResult]

include hClone hEq in
theorem scaled_refines_naturals (target source : List (A × Std.U64)) (units : Std.U64) :
    ∃ result, merge_effects cloneInst eqInst hashInst ⟨target⟩ ⟨source⟩ units = .ok result ∧
      decodeAdd result =
        match scaled (Account.naturals Quantity.u64Max) (Quantity.mul Quantity.u64Max)
          (Account.mapEntries UScalar.val target) (Account.mapEntries UScalar.val source) units.val with
        | .error asset => (.error asset, Account.mapEntries UScalar.val target)
        | .ok updated => (.ok (), updated) := by
  refine ⟨_, scaled_refines cloneInst eqInst hashInst hClone hEq target source units, ?_⟩
  rw [← scaled_map arithmetic _ UScalar.val natural_hom U64.checked_mul _ mul_exact]
  cases scaled arithmetic U64.checked_mul target source units <;> rfl

include hEq in
theorem amount_refines_naturals (target : List (A × Std.U64)) (asset : A) (amount : Std.U64) :
    ∃ result, merge_amount cloneInst eqInst hashInst ⟨target⟩ asset amount = .ok result ∧
      decodeAdd result = BasketOperations.add (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val target) [(asset, amount.val)] := by
  refine ⟨_, amount_refines cloneInst eqInst hashInst hEq target asset amount, ?_⟩
  rw [show [(asset, amount.val)] = Account.mapEntries UScalar.val [(asset, amount)] from rfl,
    ← BasketOperations.map_add arithmetic _ UScalar.val natural_hom]
  cases h : Account.credit arithmetic target [(asset, amount)] <;>
    simp [decodeAdd, addResult, BasketOperations.add, h]

end Updates
end Axionomy.EffectsRefinement
