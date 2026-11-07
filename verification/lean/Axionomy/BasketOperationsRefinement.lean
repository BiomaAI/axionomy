import Axionomy.AccountRefinement
import Axionomy.BasketOperations

namespace Axionomy.BasketOperationsRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

theorem mul_exact (left right : Std.U64) :
    (Std.U64.checked_mul left right).map UScalar.val =
      Quantity.mul Quantity.u64Max left.val right.val := by
  have spec := Std.U64.checked_mul_bv_spec left right
  cases h : Std.U64.checked_mul left right with
  | none =>
    simp only [h] at spec
    have large : ¬ left.val * right.val ≤ Quantity.u64Max := by
      simpa [Quantity.u64Max, Std.U64.max, Std.U64.numBits] using Nat.not_le.mpr spec
    simp [Quantity.mul, Quantity.checked, large]
  | some value =>
    simp only [h] at spec
    have bounded : left.val * right.val ≤ Quantity.u64Max := by
      simpa [Quantity.u64Max, Std.U64.max, Std.U64.numBits] using spec.1
    simp [Quantity.mul, Quantity.checked, bounded, spec.2.1]

@[simp] theorem mul_call (left right : Std.U64) :
    quantity.Quantity.checked_mul scalar left right = .ok (Std.U64.checked_mul left right) := by
  simp only [quantity.Quantity.checked_mul,
    U64.Insts.Num_traitsOpsCheckedCheckedMul.checked_mul, bind_ok]
  cases h : Std.U64.checked_mul left right <;>
    simp [quantity.Quantity.from_checked, quantity.Quantity.try_from_scalar,
      U64.Insts.Account_extractionQuantityQuantityScalarI128.is_nonnegative]

section Updates
variable {A : Type} [DecidableEq A]
variable (cloneInst : core.clone.Clone A) (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
variable (hClone : LawfulClone cloneInst) (hEq : LawfulEquality eqInst)

-- The original snapshot is explicitly returned on every overflow branch.
def addResult (original : List (A × Std.U64)) (result : Except A (List (A × Std.U64))) :
    core.result.Result Unit A × basket.Basket A Std.U64 :=
  match result with
  | .ok updated => (.Ok (), ⟨updated⟩)
  | .error asset => (.Err asset, ⟨original⟩)

include hClone hEq in
theorem add_body (original remaining updated : List (A × Std.U64)) :
    basket.Basket.checked_add_loop.body cloneInst eqInst hashInst scalar ⟨original⟩ remaining ⟨updated⟩ =
      .ok (match remaining with
        | [] => .done (.Ok (), ⟨updated⟩)
        | (asset, amount) :: rest =>
          match arithmetic.add (Account.balance arithmetic.zero updated asset) amount with
          | none => .done (.Err asset, ⟨original⟩)
          | some value => .cont (rest, ⟨Account.store arithmetic updated asset value⟩)) := by
  cases remaining with
  | nil => simp [basket.Basket.checked_add_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, amount⟩
    simp only [basket.Basket.checked_add_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, bind_ok,
      quantity_call eqInst hashInst hEq, add_call]
    cases h : arithmetic.add (Account.balance arithmetic.zero updated asset) amount <;>
      simp [hClone asset, store_call eqInst hashInst hEq, h]

include hClone hEq in
theorem add_loop (original remaining updated : List (A × Std.U64)) :
    basket.Basket.checked_add_loop cloneInst eqInst hashInst scalar remaining ⟨original⟩ ⟨updated⟩ =
      .ok (addResult original (Account.credit arithmetic updated remaining)) := by
  induction remaining generalizing updated with
  | nil =>
    unfold basket.Basket.checked_add_loop
    rw [loop.eq_1]
    simp [add_body cloneInst eqInst hashInst hClone hEq, bind_ok, Account.credit, addResult]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    unfold basket.Basket.checked_add_loop
    rw [loop.eq_1]
    simp only [add_body cloneInst eqInst hashInst hClone hEq original ((asset, amount) :: rest) updated]
    cases h : arithmetic.add (Account.balance arithmetic.zero updated asset) amount with
    | none => simp [h, Account.credit, addResult]
    | some value =>
      simp only [bind_ok]
      change basket.Basket.checked_add_loop cloneInst eqInst hashInst scalar rest ⟨original⟩
        ⟨Account.store arithmetic updated asset value⟩ = _
      rw [ih]
      simp [Account.credit, h]


include hClone hEq in
theorem add_refines (state amounts : List (A × Std.U64)) :
    basket_add cloneInst eqInst hashInst ⟨state⟩ ⟨amounts⟩ =
      .ok (addResult state (Account.credit arithmetic state amounts)) := by
  simp [basket_add, basket.Basket.checked_add, clone_call cloneInst hClone,
    basket.Basket.iter, indexmap.map.IndexMap.iter, add_loop cloneInst eqInst hashInst hClone hEq]

include hClone hEq in
theorem contains_refines (state required : List (A × Std.U64))
    (unique : (required.map Prod.fst).Nodup) :
    basket_contains cloneInst eqInst hashInst ⟨state⟩ ⟨required⟩ =
      .ok (Account.shortfall arithmetic state required).isEmpty := by
  simp [basket_contains, basket.Basket.contains,
    shortfall_refines cloneInst eqInst hashInst hClone hEq state required unique,
    basket.Basket.is_empty, indexmap.map.IndexMap.is_empty]

def scaleResult (result : Except A (List (A × Std.U64))) : core.result.Result (basket.Basket A Std.U64) A :=
  match result with
  | .ok entries => .Ok ⟨entries⟩
  | .error asset => .Err asset

include hClone hEq in
theorem scale_body (units : Std.U64) (remaining scaled : List (A × Std.U64)) :
    basket.Basket.checked_scale_loop.body cloneInst eqInst hashInst scalar units remaining ⟨scaled⟩ =
      .ok (match remaining with
        | [] => .done (.Ok ⟨scaled⟩)
        | (asset, amount) :: rest =>
          match Std.U64.checked_mul amount units with
          | none => .done (.Err asset)
          | some value => .cont (rest, ⟨Account.store arithmetic scaled asset value⟩)) := by
  cases remaining with
  | nil => simp [basket.Basket.checked_scale_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, amount⟩
    simp only [basket.Basket.checked_scale_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, bind_ok, mul_call]
    cases h : Std.U64.checked_mul amount units <;>
      simp [hClone asset, store_call eqInst hashInst hEq, h]

include hClone hEq in
theorem scale_loop (units : Std.U64) (remaining scaled : List (A × Std.U64)) :
    basket.Basket.checked_scale_loop cloneInst eqInst hashInst scalar remaining units ⟨scaled⟩ =
      .ok (scaleResult (BasketOperations.scaleInto arithmetic Std.U64.checked_mul units remaining scaled)) := by
  induction remaining generalizing scaled with
  | nil =>
    unfold basket.Basket.checked_scale_loop
    rw [loop.eq_1]
    simp [scale_body cloneInst eqInst hashInst hClone hEq, bind_ok, BasketOperations.scaleInto, scaleResult]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    unfold basket.Basket.checked_scale_loop
    rw [loop.eq_1]
    simp only [scale_body cloneInst eqInst hashInst hClone hEq units ((asset, amount) :: rest) scaled]
    cases h : Std.U64.checked_mul amount units with
    | none => simp [h, BasketOperations.scaleInto, scaleResult]
    | some value =>
      simp only [bind_ok]
      change basket.Basket.checked_scale_loop cloneInst eqInst hashInst scalar rest units
        ⟨Account.store arithmetic scaled asset value⟩ = _
      rw [ih]
      simp [BasketOperations.scaleInto, h]

include hClone hEq in
theorem scale_refines (entries : List (A × Std.U64)) (units : Std.U64) :
    basket_scale cloneInst eqInst hashInst ⟨entries⟩ units =
      .ok (scaleResult (BasketOperations.scale arithmetic Std.U64.checked_mul entries units)) := by
  simp [basket_scale, basket.Basket.checked_scale, basket.Basket.new,
    indexmap.map.IndexMapKVRandomState.new, basket.Basket.iter, indexmap.map.IndexMap.iter,
    scale_loop cloneInst eqInst hashInst hClone hEq, BasketOperations.scale]

def decodeAdd (result : core.result.Result Unit A × basket.Basket A Std.U64) :
    Except A Unit × List (A × Nat) :=
  (match result.1 with | .Ok () => .ok () | .Err asset => .error asset,
    Account.mapEntries UScalar.val result.2.quantities)

def decodeScale (result : core.result.Result (basket.Basket A Std.U64) A) : Except A (List (A × Nat)) :=
  match result with
  | .Ok entries => .ok (Account.mapEntries UScalar.val entries.quantities)
  | .Err asset => .error asset

include hClone hEq in
theorem add_refines_naturals (state amounts : List (A × Std.U64)) :
    ∃ result, basket_add cloneInst eqInst hashInst ⟨state⟩ ⟨amounts⟩ = .ok result ∧
      decodeAdd result = BasketOperations.add (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val state) (Account.mapEntries UScalar.val amounts) := by
  refine ⟨addResult state (Account.credit arithmetic state amounts),
    add_refines cloneInst eqInst hashInst hClone hEq state amounts, ?_⟩
  rw [← BasketOperations.map_add arithmetic _ _ natural_hom]
  cases h : Account.credit arithmetic state amounts <;> simp [decodeAdd, addResult, BasketOperations.add, h]

include hClone hEq in
theorem scale_refines_naturals (entries : List (A × Std.U64)) (units : Std.U64) :
    ∃ result, basket_scale cloneInst eqInst hashInst ⟨entries⟩ units = .ok result ∧
      decodeScale result = BasketOperations.scale (Account.naturals Quantity.u64Max)
        (Quantity.mul Quantity.u64Max) (Account.mapEntries UScalar.val entries) units.val := by
  refine ⟨scaleResult (BasketOperations.scale arithmetic Std.U64.checked_mul entries units),
    scale_refines cloneInst eqInst hashInst hClone hEq entries units, ?_⟩
  unfold BasketOperations.scale
  rw [← Account.mapEntries_nil (@UScalar.val .U64),
    ← BasketOperations.map_scaleInto arithmetic _ _ natural_hom _ _ mul_exact]
  cases BasketOperations.scaleInto arithmetic Std.U64.checked_mul units entries [] <;> rfl

include hClone hEq in
theorem contains_refines_naturals (state required : List (A × Std.U64))
    (unique : (required.map Prod.fst).Nodup) :
    basket_contains cloneInst eqInst hashInst ⟨state⟩ ⟨required⟩ = .ok true ↔
      ∀ entry ∈ required, entry.2.val ≤ (Account.balance arithmetic.zero state entry.1).val := by
  rw [contains_refines cloneInst eqInst hashInst hClone hEq state required unique]
  simp only [Result.ok.injEq, List.isEmpty_iff, Account.shortfall_empty_iff]
  simp [arithmetic]

end Updates

section Equality
variable {A : Type} [DecidableEq A]
variable (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A) (lawful : LawfulEquality eqInst)

theorem option_eq_call (left : Option Std.U64) (right : Std.U64) :
    core.option.Option.Insts.CoreCmpPartialEqOption.eq
      (core.cmp.PartialEqShared (quantity.Quantity.Insts.CoreCmpPartialEqQuantity core.cmp.PartialEqU64))
      left (some right) = .ok (decide (left = some right)) := by
  cases left <;> simp [core.option.Option.Insts.CoreCmpPartialEqOption.eq,
    core.cmp.impls.PartialEqShared.eq, quantity.Quantity.Insts.CoreCmpPartialEqQuantity.eq]

include lawful in
theorem equal_body (remaining right : List (A × Std.U64)) :
    basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body eqInst hashInst core.cmp.PartialEqU64
      ⟨right⟩ remaining = .ok (match remaining with
        | [] => .done true
        | (asset, amount) :: rest => if Account.find? asset right = some amount then .cont rest else .done false) := by
  cases remaining with
  | nil => simp [basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, amount⟩
    simp [basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
      indexmap.map.IndexMap.get, equivalent_eq, get_exact eqInst lawful, option_eq_call]
    split <;> rfl

include lawful in
theorem equal_loop (remaining right : List (A × Std.U64)) :
    basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop eqInst hashInst core.cmp.PartialEqU64
      remaining ⟨right⟩ = .ok (BasketOperations.matchesEntries remaining right) := by
  induction remaining with
  | nil =>
    unfold basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop
    rw [loop.eq_1]
    simp [equal_body eqInst hashInst lawful, BasketOperations.matchesEntries]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    unfold basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop
    rw [loop.eq_1]
    simp only [equal_body eqInst hashInst lawful ((asset, amount) :: rest) right]
    by_cases agrees : Account.find? asset right = some amount
    · simp only [agrees, ↓reduceIte, bind_ok]
      change basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop eqInst hashInst core.cmp.PartialEqU64 rest ⟨right⟩ = _
      rw [ih]
      simp [BasketOperations.matchesEntries, agrees]
    · simp [agrees, BasketOperations.matchesEntries]

include lawful in
theorem equal_refines (left right : List (A × Std.U64))
    (leftBound : left.length < 2^UScalarTy.Usize.numBits)
    (rightBound : right.length < 2^UScalarTy.Usize.numBits) :
    basket_equal eqInst hashInst ⟨left⟩ ⟨right⟩ = .ok (BasketOperations.equal left right) := by
  simp only [basket_equal, basket.Basket.Insts.CoreCmpPartialEqBasket.eq, basket.Basket.len,
    indexmap.map.IndexMap.len, leftBound, rightBound, ↓reduceDIte, bind_ok]
  have lengthEqual : Std.Usize.ofNatCore left.length leftBound = Std.Usize.ofNatCore right.length rightBound ↔
      left.length = right.length := by
    constructor
    · intro h; simpa using congrArg UScalar.val h
    · intro h; apply UScalar.eq_of_val_eq; simpa using h
  by_cases lengths : left.length = right.length
  · simp [basket.Basket.iter, indexmap.map.IndexMap.iter,
      equal_loop eqInst hashInst lawful, BasketOperations.equal, lengths]
  · simp [lengthEqual, lengths, BasketOperations.equal]

include lawful in
theorem equal_iff_permutation (left right : List (A × Std.U64))
    (leftBound : left.length < 2^UScalarTy.Usize.numBits)
    (rightBound : right.length < 2^UScalarTy.Usize.numBits)
    (uniqueLeft : (left.map Prod.fst).Nodup) (uniqueRight : (right.map Prod.fst).Nodup) :
    basket_equal eqInst hashInst ⟨left⟩ ⟨right⟩ = .ok true ↔ left.Perm right := by
  rw [equal_refines eqInst hashInst lawful left right leftBound rightBound]
  simp only [Result.ok.injEq, BasketOperations.equal_iff_permutation left right uniqueLeft uniqueRight]

end Equality
end Axionomy.BasketOperationsRefinement
