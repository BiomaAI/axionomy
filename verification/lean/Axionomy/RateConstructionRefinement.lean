import Axionomy.BasketOperationsRefinement
import Axionomy.RateConstruction

namespace Axionomy.RateConstructionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement
open RateConstruction

def LawfulOrdering {K : Type} (inst : core.cmp.Ord K) (cmp : K → K → Ordering) : Prop :=
  ∀ x y, inst.cmp x y = .ok (cmp x y)

theorem u64Ordering_lawful : LawfulOrdering core.cmp.OrdU64
    (fun x y => compare x.val y.val) := by
  intro x y
  rfl

theorem u64Ordering_identity (x y : Std.U64) (equal : compare x.val y.val = .eq) : x = y := by
  exact UScalar.eq_of_val_eq (Nat.compare_eq_eq.mp equal)

def encodePosition {K V : Type} (position : Position K V) :
    alloc.collections.btree.map.entry.Entry K V alloc.alloc.Global :=
  match position.old with
  | none => .Vacant ⟨position.before, position.key, position.after⟩
  | some value => .Occupied ⟨position.before, position.key, value, position.after⟩

theorem locate_exact {K V : Type} (ordInst : core.cmp.Ord K) (cmp : K → K → Ordering)
    (lawful : LawfulOrdering ordInst cmp) (query : K) (entries : List (K × V)) :
    treeLocate ordInst.cmp query entries = .ok (encodePosition (locate cmp query entries)) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases h : cmp query key with
    | lt => simp [treeLocate, lawful query key, h, locate, encodePosition]
    | eq => simp [treeLocate, lawful query key, h, locate, encodePosition]
    | gt =>
      simp only [treeLocate, lawful query key, bind_ok, h, ih]
      cases ho : (locate cmp query rest).old <;>
        simp [locate, h, encodePosition, ho, treePrepend]

theorem register_exact {K : Type} (ordInst : core.cmp.Ord K) (cmp : K → K → Ordering)
    (lawful : LawfulOrdering ordInst cmp) (query : K) (roles : List K) :
    ∃ inserted, sortedInsert ordInst.cmp query roles = .ok (inserted, register cmp query roles) := by
  induction roles with
  | nil => exact ⟨true, rfl⟩
  | cons key rest ih =>
    cases h : cmp query key with
    | lt => exact ⟨true, by simp [sortedInsert, lawful query key, h, register]⟩
    | eq => exact ⟨false, by simp [sortedInsert, lawful query key, h, register]⟩
    | gt =>
      obtain ⟨inserted, exactResult⟩ := ih
      exact ⟨inserted, by simp [sortedInsert, lawful query key, h, exactResult, register]⟩

-- The entry API's backward function updates exactly the selected position.
theorem entry_default {K V : Type} (ordInst : core.cmp.Ord K)
    (defaultInst : core.default.Default V) (empty : V) (defaultLaw : defaultInst.default = .ok empty)
    (position : Position K V) :
    alloc.collections.btree.map.entry.Entry.or_default ordInst defaultInst
      alloc.alloc.Global.Insts.CoreAllocAllocatorClone (encodePosition position) =
      .ok (position.old.getD empty, fun value =>
        .Occupied ⟨position.before, position.key, value, position.after⟩) := by
  cases h : position.old <;>
    simp [encodePosition, h, alloc.collections.btree.map.entry.Entry.or_default, defaultLaw]

def encodeMap {Role A : Type} (entries : List (Role × List (A × Std.U64))) :
    List (Role × basket.Basket A Std.U64) := entries.map fun (role, assets) => (role, ⟨assets⟩)

theorem locate_encode {Role A : Type} (cmp : Role → Role → Ordering) (role : Role)
    (entries : List (Role × List (A × Std.U64))) :
    locate cmp role (encodeMap entries) =
      ⟨encodeMap (locate cmp role entries).before, (locate cmp role entries).key,
        (locate cmp role entries).old.map basket.Basket.mk, encodeMap (locate cmp role entries).after⟩ := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, assets⟩
    cases h : cmp role key <;> simp [encodeMap, locate, h] at *
    simp only [ih]
    simp

section Builders
variable {Role A : Type} [DecidableEq A]
variable (roleClone : core.clone.Clone Role) (ordInst : core.cmp.Ord Role)
variable (assetClone : core.clone.Clone A) (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
variable (cmp : Role → Role → Ordering)
variable (hRole : LawfulClone roleClone) (hOrd : LawfulOrdering ordInst cmp)
variable (hAsset : LawfulClone assetClone) (hEq : LawfulEquality eqInst)

def mergeResult (position : Position Role (List (A × Std.U64)))
    (result : Except A (List (A × Std.U64))) :
    core.result.Result Unit A × List (Role × basket.Basket A Std.U64) :=
  match result with
  | .error asset => (.Err asset, encodeMap (position.put (position.old.getD [])))
  | .ok assets => (.Ok (), encodeMap (position.put assets))

include hOrd hAsset hEq in
theorem merge_refines (target : List (Role × List (A × Std.U64)))
    (role : Role) (assets : List (A × Std.U64)) :
    rate.merge ordInst assetClone eqInst hashInst scalar (encodeMap target) role ⟨assets⟩ =
      .ok (mergeResult (locate cmp role target)
        (Account.credit arithmetic ((locate cmp role target).old.getD []) assets)) := by
  have addCall (left right : List (A × Std.U64)) :
      basket.Basket.checked_add assetClone eqInst hashInst scalar ⟨left⟩ ⟨right⟩ =
        .ok (BasketOperationsRefinement.addResult left (Account.credit arithmetic left right)) :=
    BasketOperationsRefinement.add_refines assetClone eqInst hashInst hAsset hEq left right
  cases old : (locate cmp role target).old <;>
    simp only [rate.merge, alloc.collections.btree.map.BTreeMap.entry,
      locate_exact ordInst cmp hOrd, bind_ok, locate_encode, encodePosition, old,
      Option.map_none, Option.map_some,
      alloc.collections.btree.map.entry.Entry.or_default,
      basket.Basket.Insts.CoreDefaultDefault.default, basket.Basket.new,
      indexmap.map.IndexMapKVRandomState.new, addCall]
  all_goals
    cases h : Account.credit arithmetic ((locate cmp role target).old.getD []) assets <;>
      simp_all [BasketOperationsRefinement.addResult, mergeResult, treeRestore, Position.put, encodeMap]


def decodeMap (entries : List (Role × basket.Basket A Std.U64)) :
    List (Role × List (A × Std.U64)) := entries.map fun (role, assets) => (role, assets.quantities)

omit [DecidableEq A] in
@[simp] theorem encode_decode (entries : List (Role × basket.Basket A Std.U64)) :
    encodeMap (decodeMap entries) = entries := by
  simp [encodeMap, decodeMap, List.map_map, Function.comp_def]

def consumeResult (rule : rate.Rate Role A Std.U64) (role : Role)
    (assets : List (A × Std.U64)) : core.result.Result (rate.Rate Role A Std.U64) (rate.RateError Role A) :=
  match RateConstruction.merge arithmetic cmp (decodeMap rule.consume) role assets with
  | .error asset => .Err (.BasketOverflow role asset)
  | .ok merged => .Ok { rule with consume := encodeMap merged, roles := register cmp role rule.roles }

include hRole hOrd hAsset hEq in
theorem consume_refines (rule : rate.Rate Role A Std.U64) (role : Role)
    (assets : List (A × Std.U64)) :
    rate_consume roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ =
      .ok (consumeResult cmp rule role assets) := by
  obtain ⟨inserted, registration⟩ := register_exact ordInst cmp hOrd role rule.roles
  have mergeCall := merge_refines ordInst assetClone eqInst hashInst cmp hOrd hAsset hEq
    (decodeMap rule.consume) role assets
  rw [encode_decode] at mergeCall
  simp only [rate_consume, rate.Rate.try_consume, hRole role,
    alloc.collections.btree.set.BTreeSet.insert, registration, bind_ok, mergeCall]
  cases h : Account.credit arithmetic ((locate cmp role (decodeMap rule.consume)).old.getD []) assets <;>
    simp [h, consumeResult, RateConstruction.merge, mergeResult,
      core.result.Result.map_err,
      rate.Rate.try_consume.closure.Insts.CoreOpsFunctionFnOnceTupleARateError.call_once,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
      core.convert.FromSame.from]

def produceResult (rule : rate.Rate Role A Std.U64) (role : Role)
    (assets : List (A × Std.U64)) : core.result.Result (rate.Rate Role A Std.U64) (rate.RateError Role A) :=
  match RateConstruction.merge arithmetic cmp (decodeMap rule.produce) role assets with
  | .error asset => .Err (.BasketOverflow role asset)
  | .ok merged => .Ok { rule with produce := encodeMap merged, roles := register cmp role rule.roles }

include hRole hOrd hAsset hEq in
theorem produce_refines (rule : rate.Rate Role A Std.U64) (role : Role)
    (assets : List (A × Std.U64)) :
    rate_produce roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ =
      .ok (produceResult cmp rule role assets) := by
  obtain ⟨inserted, registration⟩ := register_exact ordInst cmp hOrd role rule.roles
  have mergeCall := merge_refines ordInst assetClone eqInst hashInst cmp hOrd hAsset hEq
    (decodeMap rule.produce) role assets
  rw [encode_decode] at mergeCall
  simp only [rate_produce, rate.Rate.try_produce, hRole role,
    alloc.collections.btree.set.BTreeSet.insert, registration, bind_ok, mergeCall]
  cases h : Account.credit arithmetic ((locate cmp role (decodeMap rule.produce)).old.getD []) assets <;>
    simp [h, produceResult, RateConstruction.merge, mergeResult,
      core.result.Result.map_err,
      rate.Rate.try_produce.closure.Insts.CoreOpsFunctionFnOnceTupleARateError.call_once,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
      core.convert.FromSame.from]

def preserveResult (rule : rate.Rate Role A Std.U64) (role : Role)
    (assets : List (A × Std.U64)) : core.result.Result (rate.Rate Role A Std.U64) (rate.RateError Role A) :=
  match RateConstruction.merge arithmetic cmp (decodeMap rule.preserve) role assets with
  | .error asset => .Err (.BasketOverflow role asset)
  | .ok merged => .Ok { rule with preserve := encodeMap merged, roles := register cmp role rule.roles }

include hRole hOrd hAsset hEq in
theorem preserve_refines (rule : rate.Rate Role A Std.U64) (role : Role)
    (assets : List (A × Std.U64)) :
    rate_preserve roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ =
      .ok (preserveResult cmp rule role assets) := by
  obtain ⟨inserted, registration⟩ := register_exact ordInst cmp hOrd role rule.roles
  have mergeCall := merge_refines ordInst assetClone eqInst hashInst cmp hOrd hAsset hEq
    (decodeMap rule.preserve) role assets
  rw [encode_decode] at mergeCall
  simp only [rate_preserve, rate.Rate.try_preserve, hRole role,
    alloc.collections.btree.set.BTreeSet.insert, registration, bind_ok, mergeCall]
  cases h : Account.credit arithmetic ((locate cmp role (decodeMap rule.preserve)).old.getD []) assets <;>
    simp [h, preserveResult, RateConstruction.merge, mergeResult,
      core.result.Result.map_err,
      rate.Rate.try_preserve.closure.Insts.CoreOpsFunctionFnOnceTupleARateError.call_once,
      core.result.Result.Insts.CoreOpsTry.branch,
      core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual,
      core.convert.FromSame.from]

include hRole hOrd hAsset hEq in
theorem consume_registers_and_frames
    (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule updated : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64))
    (accepted : rate_consume roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .ok (.Ok updated)) :
    role ∈ updated.roles ∧ updated.produce = rule.produce ∧ updated.preserve = rule.preserve ∧
      updated.distinct = rule.distinct ∧ updated.computed_consume = rule.computed_consume ∧
      updated.computed_produce = rule.computed_produce ∧ updated.computed_preserve = rule.computed_preserve ∧
      updated.conditions = rule.conditions := by
  rw [consume_refines roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq] at accepted
  unfold consumeResult at accepted
  cases h : RateConstruction.merge arithmetic cmp (decodeMap rule.consume) role assets with
  | error asset => simp [h] at accepted
  | ok merged =>
    simp only [h, Result.ok.injEq, core.result.Result.Ok.injEq] at accepted
    subst updated
    exact ⟨registered cmp eqLaw role rule.roles, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

include hRole hOrd hAsset hEq in
theorem produce_registers_and_frames
    (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule updated : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64))
    (accepted : rate_produce roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .ok (.Ok updated)) :
    role ∈ updated.roles ∧ updated.consume = rule.consume ∧ updated.preserve = rule.preserve ∧
      updated.distinct = rule.distinct ∧ updated.computed_consume = rule.computed_consume ∧
      updated.computed_produce = rule.computed_produce ∧ updated.computed_preserve = rule.computed_preserve ∧
      updated.conditions = rule.conditions := by
  rw [produce_refines roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq] at accepted
  unfold produceResult at accepted
  cases h : RateConstruction.merge arithmetic cmp (decodeMap rule.produce) role assets with
  | error asset => simp [h] at accepted
  | ok merged =>
    simp only [h, Result.ok.injEq, core.result.Result.Ok.injEq] at accepted
    subst updated
    exact ⟨registered cmp eqLaw role rule.roles, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

include hRole hOrd hAsset hEq in
theorem preserve_registers_and_frames
    (eqLaw : ∀ x y, cmp x y = .eq → x = y)
    (rule updated : rate.Rate Role A Std.U64) (role : Role) (assets : List (A × Std.U64))
    (accepted : rate_preserve roleClone ordInst assetClone eqInst hashInst rule role ⟨assets⟩ = .ok (.Ok updated)) :
    role ∈ updated.roles ∧ updated.consume = rule.consume ∧ updated.produce = rule.produce ∧
      updated.distinct = rule.distinct ∧ updated.computed_consume = rule.computed_consume ∧
      updated.computed_produce = rule.computed_produce ∧ updated.computed_preserve = rule.computed_preserve ∧
      updated.conditions = rule.conditions := by
  rw [preserve_refines roleClone ordInst assetClone eqInst hashInst cmp hRole hOrd hAsset hEq] at accepted
  unfold preserveResult at accepted
  cases h : RateConstruction.merge arithmetic cmp (decodeMap rule.preserve) role assets with
  | error asset => simp [h] at accepted
  | ok merged =>
    simp only [h, Result.ok.injEq, core.result.Result.Ok.injEq] at accepted
    subst updated
    exact ⟨registered cmp eqLaw role rule.roles, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩


end Builders
end Axionomy.RateConstructionRefinement
