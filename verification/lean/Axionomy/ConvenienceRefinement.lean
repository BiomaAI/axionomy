import Axionomy.RateConstructionRefinement
import Axionomy.BasketPublicRefinement

namespace Axionomy.ConvenienceRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement
variable {Role A T F : Type}

/-- Convenience constructors discard the typed error and raise a Rust panic.
The underlying checked constructor's independent specification chooses the arm. -/
def unwrap {E V : Type} : core.result.Result V E → Result V
  | .Ok value => .ok value
  | .Err _ => .fail .panic

theorem unwrap_panics_iff {E V : Type} (result : core.result.Result V E) :
    unwrap result = .fail .panic ↔ ∃ error, result = .Err error := by
  cases result <;> simp [unwrap]

/-- The failure criterion is the bounded natural-number credit specification,
not a premise that the Rust checked builder reports an error. -/
theorem merge_overflow_naturals [DecidableEq A] (cmp : Role → Role → Ordering)
    (target : List (Role × List (A × Std.U64))) (role : Role) (assets : List (A × Std.U64)) :
    (∃ asset, RateConstruction.merge arithmetic cmp target role assets = .error asset) ↔
      ∃ asset, Account.credit (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val ((RateConstruction.locate cmp role target).old.getD []))
        (Account.mapEntries UScalar.val assets) = .error asset := by
  have mapped := Account.map_credit arithmetic (Account.naturals Quantity.u64Max) UScalar.val
    natural_hom assets ((RateConstruction.locate cmp role target).old.getD [])
  unfold RateConstruction.merge
  rw [← mapped]
  cases result : Account.credit arithmetic ((RateConstruction.locate cmp role target).old.getD []) assets <;>
    simp [result, Except.map]

section Rates
variable [DecidableEq A]
variable (roleClone : core.clone.Clone Role) (ordInst : core.cmp.Ord Role)
  (assetClone : core.clone.Clone A) (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
  (cmp : Role → Role → Ordering) (hRole : LawfulClone roleClone) (hOrd : LawfulOrdering ordInst cmp)
  (hAsset : LawfulClone assetClone) (hEq : LawfulEquality eqInst)

include hRole hOrd hAsset hEq in
theorem consume_refines (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_consume_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ =
      unwrap (consumeResult cmp rule role assets) := by
  have checked := RateConstructionRefinement.consume_refines
    roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq rule role assets
  change rate.Rate.try_consume roleClone ordInst assetClone eqInst hashInst scalar rule role ⟨assets⟩ = _ at checked
  simp only [rate_consume_or_panic, rate.Rate.impl.consume, checked, bind_ok]
  cases consumeResult cmp rule role assets <;> rfl

include hRole hOrd hAsset hEq in
theorem produce_refines (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_produce_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ =
      unwrap (produceResult cmp rule role assets) := by
  have checked := RateConstructionRefinement.produce_refines
    roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq rule role assets
  change rate.Rate.try_produce roleClone ordInst assetClone eqInst hashInst scalar rule role ⟨assets⟩ = _ at checked
  simp only [rate_produce_or_panic, rate.Rate.impl.produce, checked, bind_ok]
  cases produceResult cmp rule role assets <;> rfl

include hRole hOrd hAsset hEq in
theorem preserve_refines (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_preserve_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ =
      unwrap (preserveResult cmp rule role assets) := by
  have checked := RateConstructionRefinement.preserve_refines
    roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq rule role assets
  change rate.Rate.try_preserve roleClone ordInst assetClone eqInst hashInst scalar rule role ⟨assets⟩ = _ at checked
  simp only [rate_preserve_or_panic, rate.Rate.impl.preserve, checked, bind_ok]
  cases preserveResult cmp rule role assets <;> rfl

include hRole hOrd hAsset hEq in
theorem consume_panics_iff (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_consume_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .fail .panic ↔
      ∃ asset, RateConstruction.merge arithmetic cmp (decodeMap rule.consume) role assets = .error asset := by
  rw [consume_refines roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq]
  cases merged : RateConstruction.merge arithmetic cmp (decodeMap rule.consume) role assets <;>
    simp [consumeResult, merged, unwrap]

include hRole hOrd hAsset hEq in
theorem produce_panics_iff (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_produce_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .fail .panic ↔
      ∃ asset, RateConstruction.merge arithmetic cmp (decodeMap rule.produce) role assets = .error asset := by
  rw [produce_refines roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq]
  cases merged : RateConstruction.merge arithmetic cmp (decodeMap rule.produce) role assets <;>
    simp [produceResult, merged, unwrap]

include hRole hOrd hAsset hEq in
theorem preserve_panics_iff (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_preserve_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .fail .panic ↔
      ∃ asset, RateConstruction.merge arithmetic cmp (decodeMap rule.preserve) role assets = .error asset := by
  rw [preserve_refines roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq]
  cases merged : RateConstruction.merge arithmetic cmp (decodeMap rule.preserve) role assets <;>
    simp [preserveResult, merged, unwrap]

include hRole hOrd hAsset hEq in
theorem consume_panics_naturals (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_consume_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .fail .panic ↔
      ∃ asset, Account.credit (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val ((RateConstruction.locate cmp role (decodeMap rule.consume)).old.getD []))
        (Account.mapEntries UScalar.val assets) = .error asset := by
  rw [consume_panics_iff roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq,
    merge_overflow_naturals]

include hRole hOrd hAsset hEq in
theorem produce_panics_naturals (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_produce_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .fail .panic ↔
      ∃ asset, Account.credit (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val ((RateConstruction.locate cmp role (decodeMap rule.produce)).old.getD []))
        (Account.mapEntries UScalar.val assets) = .error asset := by
  rw [produce_panics_iff roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq,
    merge_overflow_naturals]

include hRole hOrd hAsset hEq in
theorem preserve_panics_naturals (rule : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64)) :
    rate_preserve_or_panic roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .fail .panic ↔
      ∃ asset, Account.credit (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val ((RateConstruction.locate cmp role (decodeMap rule.preserve)).old.getD []))
        (Account.mapEntries UScalar.val assets) = .error asset := by
  rw [preserve_panics_iff roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq,
    merge_overflow_naturals]

end Rates

theorem map_identity (call : F → T → Result (T × F))
    (law : ∀ state value, call state value = .ok (value, state)) (state : F) (entries : List T) :
    mapArrayEntries call state entries = .ok (⟨entries, rfl⟩, state) := by
  induction entries generalizing state with
  | nil => rfl
  | cons entry rest ih => simp [mapArrayEntries, law, ih]

theorem array_identity {length : Usize} (callback : core.ops.function.FnMut F T T)
    (law : ∀ state value, callback.call_mut state value = .ok (value, state))
    (entries : Array T length) (state : F) : core.array.Array.map callback entries state = .ok entries := by
  simp [core.array.Array.map, map_identity callback.call_mut law]

theorem literal_conversion {length : Usize} (equal : core.cmp.Eq A) (hash : core.hash.Hash A)
    (entries : Array (A × Std.U64) length) :
    core.array.Array.map
      (rate.basket.closure.Insts.CoreOpsFunctionFnMutTuplePairAU64PairAQuantityU64 length equal hash)
      entries () = .ok entries := by
  apply array_identity
  intro state value
  rcases value with ⟨asset, amount⟩
  simp [rate.basket.closure.Insts.CoreOpsFunctionFnMutTuplePairAU64PairAQuantityU64.call_mut,
    quantity.QuantityU64.new]

theorem literal_refines [DecidableEq A] {length : Usize}
    (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (law : LawfulEquality equal)
    (entries : Array (A × Std.U64) length) :
    basket_literal equal hash entries = BasketCollectionRefinement.collected
      (BasketCollection.construct (fun value => decide (value = 0#u64)) [] entries.val) := by
  simp only [basket_literal, rate.basket, literal_conversion, bind_ok, core.convert.IntoFrom.into]
  exact BasketPublicRefinement.array_refines equal hash law scalar _ BasketPublicRefinement.u64_zero_law entries

theorem literal_panics_iff [DecidableEq A] {length : Usize}
    (equal : core.cmp.Eq A) (hash : core.hash.Hash A) (law : LawfulEquality equal)
    (entries : Array (A × Std.U64) length) :
    basket_literal equal hash entries = .fail .panic ↔ ¬ (entries.val.map Prod.fst).Nodup := by
  rw [literal_refines equal hash law]
  exact BasketCollectionRefinement.collected_panics_iff _ entries.val

end Axionomy.ConvenienceRefinement
