import Axionomy.DerivedRecordRefinement
import Axionomy.ExecutionRefinement

namespace Axionomy.DerivedCollectionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement DerivedValueRefinement
variable {K V T A N Role AccountId RateId : Type}

theorem equal_entries [DecidableEq K] [DecidableEq V]
    (keyEq : core.cmp.PartialEq K K) (valueEq : core.cmp.PartialEq V V)
    (keyLaw : LawfulPartialEquality keyEq) (valueLaw : LawfulPartialEquality valueEq)
    (left right : List (K × V)) : equalMapEntries keyEq valueEq left right = .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at keyLaw valueLaw
  induction left generalizing right with
  | nil => cases right <;> simp [equalMapEntries]
  | cons entry rest ih =>
    cases entry with | mk key value =>
      cases right with
      | nil => simp [equalMapEntries]
      | cons other others =>
        cases other
        simp [equalMapEntries, keyLaw, valueLaw, ih]

theorem map_eq [DecidableEq K] [DecidableEq V]
    (keyEq : core.cmp.PartialEq K K) (valueEq : core.cmp.PartialEq V V)
    (keyLaw : LawfulPartialEquality keyEq) (valueLaw : LawfulPartialEquality valueEq)
    (allocator : core.alloc.AllocatorClone A) (left right : List (K × V)) :
    alloc.collections.btree.map.BTreeMap.Insts.CoreCmpPartialEqBTreeMap.eq keyEq valueEq allocator left right =
      .ok (decide (left = right)) := by
  simp only [alloc.collections.btree.map.BTreeMap.Insts.CoreCmpPartialEqBTreeMap.eq,
    equal_entries keyEq valueEq keyLaw valueLaw]
  by_cases lengths : left.length = right.length
  · simp [lengths]
  · have different : left ≠ right := fun same => lengths (congrArg List.length same)
    simp [lengths, different]

theorem zip_eq [DecidableEq T] (equal : core.cmp.PartialEq T T) (law : LawfulPartialEquality equal)
    (left right : List T) (lengths : left.length = right.length) :
    List.allM (fun pair => equal.eq pair.1 pair.2) (left.zip right) = .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at law
  induction left generalizing right with
  | nil =>
    cases right with
    | nil => rfl
    | cons _ _ => simp at lengths
  | cons value rest ih =>
    cases right with
    | nil => simp at lengths
    | cons other others =>
      have tailLength : rest.length = others.length := by simpa using lengths
      simp [law value other, ih others tailLength, pure]

theorem vec_eq [DecidableEq T] (equal : core.cmp.PartialEq T T) (law : LawfulPartialEquality equal)
    (left right : alloc.vec.Vec T) :
    alloc.vec.partial_eq.PartialEqVec.eq equal left right = .ok (decide (left.val = right.val)) := by
  simp only [alloc.vec.partial_eq.PartialEqVec.eq]
  by_cases lengths : left.val.length = right.val.length
  · simp [lengths, zip_eq equal law left.val right.val lengths]
  · have different : left.val ≠ right.val := fun same => lengths (congrArg List.length same)
    simp [lengths, different]

theorem vec_clone (inst : core.clone.Clone T) (law : LawfulClone inst) (value : alloc.vec.Vec T) :
    alloc.vec.CloneVec.clone inst value = .ok value := by
  have specification := Slice.clone_spec (s := value.slice) (clone := inst.clone) (fun x _ => law x)
  have copied : Slice.clone inst.clone value.slice = .ok value.slice := by
    cases result : Slice.clone inst.clone value.slice <;> simp_all
  simp [alloc.vec.CloneVec.clone, copied]

end Axionomy.DerivedCollectionRefinement
