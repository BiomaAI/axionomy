import Axionomy.InvariantInput
import Axionomy.BasketCollectionRefinement

namespace Axionomy.InvariantInputRefinement
set_option backward.isDefEq.respectTransparency false
set_option backward.isDefEq.respectTransparency.types false
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

variable {A : Type} [DecidableEq A]
variable (equal : core.cmp.Eq A) (hash : core.hash.Hash A)
variable (lawful : LawfulEquality equal)

def canonical (entries : List (A × Std.I64)) : List (A × Std.I64) :=
  entries.filter (fun entry => entry.2 != 0#i64)

include lawful in
theorem normalize_body (remaining accumulated : List (A × Std.I64)) :
    rate.LinearInvariant.from_serialized_weights_loop0_loop0.body equal hash remaining accumulated =
      .ok (match remaining with
        | [] => .done accumulated
        | (asset, weight) :: rest => .cont (rest,
            if weight = 0#i64 then accumulated else Account.upsert asset weight accumulated)) := by
  cases remaining with
  | nil => simp [rate.LinearInvariant.from_serialized_weights_loop0_loop0.body,
      indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next]
  | cons entry rest =>
    rcases entry with ⟨asset, weight⟩
    by_cases zero : weight = 0#i64
    · simp [rate.LinearInvariant.from_serialized_weights_loop0_loop0.body,
        indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next, zero]
    · have valueNonzero : weight.val ≠ 0 := fun h => zero (IScalar.eq_of_val_eq h)
      simp [rate.LinearInvariant.from_serialized_weights_loop0_loop0.body,
        indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next,
        zero, valueNonzero, indexmap.map.IndexMap.insert, insert_exact equal lawful]

include lawful in
theorem normalize_loop (remaining accumulated : List (A × Std.I64))
    (unique : ((accumulated ++ remaining).map Prod.fst).Nodup) :
    rate.LinearInvariant.from_serialized_weights_loop0_loop0 equal hash remaining accumulated =
      .ok (accumulated ++ canonical remaining) := by
  induction remaining generalizing accumulated with
  | nil =>
    unfold rate.LinearInvariant.from_serialized_weights_loop0_loop0
    rw [loop.eq_1]
    simp [rate.LinearInvariant.from_serialized_weights_loop0_loop0.body,
      indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next, canonical]
  | cons entry rest ih =>
    rcases entry with ⟨asset, weight⟩
    have fresh : asset ∉ accumulated.map Prod.fst := by
      have split := List.nodup_append.mp (by simpa using unique)
      intro member
      exact split.2.2 asset member asset (by simp) rfl
    have nextUnique : (((accumulated ++ [(asset, weight)]) ++ rest).map Prod.fst).Nodup := by
      simpa [List.append_assoc] using unique
    have tailUnique : ((accumulated ++ rest).map Prod.fst).Nodup := by
      simp only [List.map_append, List.map_cons, List.nodup_append, List.nodup_cons] at unique ⊢
      exact ⟨unique.1, unique.2.1.2, fun x hx y hy => unique.2.2 x hx y (by simp [hy])⟩
    unfold rate.LinearInvariant.from_serialized_weights_loop0_loop0
    rw [loop.eq_1]
    dsimp only
    rw [normalize_body equal hash lawful]
    by_cases zero : weight = 0#i64
    · simp only [zero, ↓reduceIte, bind_ok]
      change rate.LinearInvariant.from_serialized_weights_loop0_loop0 equal hash rest accumulated = _
      rw [ih accumulated tailUnique]
      simp [canonical]
    · simp only [zero, ↓reduceIte, Account.upsert_fresh asset weight accumulated fresh, bind_ok]
      change rate.LinearInvariant.from_serialized_weights_loop0_loop0 equal hash rest
        (accumulated ++ [(asset, weight)]) = _
      rw [ih _ nextUnique]
      simp [canonical, zero, List.append_assoc]

def expected (label : String) (seen remaining : List (A × Std.I64)) :
    core.result.Result (rate.LinearInvariant A) A :=
  match Basket.firstDuplicate (seen.map Prod.fst) (remaining.map Prod.fst) with
  | some asset => .Err asset
  | none => .Ok ⟨label, canonical (seen ++ remaining)⟩

include lawful in
theorem validation_step (label : String) (asset : A) (weight : Std.I64)
    (rest : List (A × Std.I64)) (bound : ((asset, weight) :: rest).length ≤ Usize.max)
    (seen : List (A × Std.I64)) :
    rate.LinearInvariant.from_serialized_weights_loop0.body equal hash [] label
      (BasketRefinement.iteratorFrom ((asset, weight) :: rest) bound) seen =
      .ok (if asset ∈ seen.map Prod.fst then .done (.Err asset)
        else .cont (BasketRefinement.iteratorFrom rest (Nat.le_of_succ_le bound), seen ++ [(asset, weight)])) := by
  by_cases present : asset ∈ seen.map Prod.fst
  · simp [rate.LinearInvariant.from_serialized_weights_loop0.body, BasketRefinement.next_cons,
      indexmap.map.IndexMap.contains_key, indexmap.map.IndexMap.get,
      equivalent_eq, get_exact equal lawful, BasketCollectionRefinement.found_isSome, present]
  · simp [rate.LinearInvariant.from_serialized_weights_loop0.body, BasketRefinement.next_cons,
      indexmap.map.IndexMap.contains_key, indexmap.map.IndexMap.get,
      equivalent_eq, get_exact equal lawful, BasketCollectionRefinement.found_isSome,
      present, indexmap.map.IndexMap.insert, BasketRefinement.insert_fresh equal lawful _ _ _ present]

include lawful in
theorem validation_loop (label : String) (remaining : List (A × Std.I64))
    (bound : remaining.length ≤ Usize.max) (seen : List (A × Std.I64))
    (unique : (seen.map Prod.fst).Nodup) :
    rate.LinearInvariant.from_serialized_weights_loop0 equal hash []
      (BasketRefinement.iteratorFrom remaining bound) label seen = .ok (expected label seen remaining) := by
  induction remaining generalizing seen with
  | nil =>
    unfold rate.LinearInvariant.from_serialized_weights_loop0
    rw [loop.eq_1]
    simp [rate.LinearInvariant.from_serialized_weights_loop0.body, BasketRefinement.next_nil,
      indexmap.map.IndexMap.Insts.CoreIterTraitsCollectIntoIteratorPairIntoIter.into_iter,
      normalize_loop equal hash lawful seen [] (by simpa using unique), expected, Basket.firstDuplicate]
  | cons entry rest ih =>
    rcases entry with ⟨asset, weight⟩
    unfold rate.LinearInvariant.from_serialized_weights_loop0
    rw [loop.eq_1]
    dsimp only
    rw [validation_step equal hash lawful]
    by_cases present : asset ∈ seen.map Prod.fst
    · simp [present, expected, Basket.firstDuplicate]
    · have nextUnique : ((seen ++ [(asset, weight)]).map Prod.fst).Nodup := by
        simp only [List.map_append, List.map_cons, List.map_nil]
        refine List.nodup_append.mpr ⟨unique, by simp, ?_⟩
        intro x hx y hy same
        have yeq : y = asset := by simpa using hy
        exact present ((same.trans yeq) ▸ hx)
      simp only [present, ↓reduceIte, bind_ok]
      change rate.LinearInvariant.from_serialized_weights_loop0 equal hash []
        (BasketRefinement.iteratorFrom rest (Nat.le_of_succ_le bound)) label (seen ++ [(asset, weight)]) = _
      rw [ih _ _ nextUnique]
      simp [expected, Basket.firstDuplicate, present, List.map_append, List.append_assoc]

include lawful in
theorem construct_exact (label : String) (entries : alloc.vec.Vec (A × Std.I64)) :
    invariant_from_serialized_weights equal hash label entries = .ok (expected label [] entries.val) := by
  simpa [invariant_from_serialized_weights, rate.LinearInvariant.from_serialized_weights,
    indexmap.map.IndexMapKVRandomState.new, alloc.vec.IntoIteratorVec.into_iter,
    BasketRefinement.iteratorFrom, bind_ok] using
    validation_loop equal hash lawful label entries.val entries.property [] (by simp)

def decodeEntries (entries : List (A × Std.I64)) : List (A × Int) :=
  entries.map (fun (asset, weight) => (asset, weight.val))

def decode : core.result.Result (rate.LinearInvariant A) A → Except A (List (A × Int))
  | .Ok invariant => .ok (decodeEntries invariant.weights)
  | .Err asset => .error asset

omit [DecidableEq A] in
theorem decode_canonical (entries : List (A × Std.I64)) :
    decodeEntries (canonical entries) = InvariantInput.canonical (decodeEntries entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, weight⟩
    have zero : weight = 0#i64 ↔ weight.val = 0 := by
      constructor
      · intro same; subst weight; rfl
      · intro h; exact IScalar.eq_of_val_eq h
    by_cases empty : weight = 0#i64
    · simpa [decodeEntries, canonical, InvariantInput.canonical, empty] using ih
    · simpa [decodeEntries, canonical, InvariantInput.canonical, empty, mt zero.mpr empty] using
        congrArg (List.cons (asset, weight.val)) ih

theorem decode_expected (label : String) (entries : List (A × Std.I64)) :
    decode (expected label [] entries) = InvariantInput.construct (decodeEntries entries) := by
  have keys : (decodeEntries entries).map Prod.fst = entries.map Prod.fst := by
    simp [decodeEntries, List.map_map]
  unfold expected InvariantInput.construct
  rw [keys]
  cases duplicate : Basket.firstDuplicate [] (entries.map Prod.fst) <;>
    simp [decode, duplicate, decode_canonical]

include lawful in
theorem construct_refines (label : String) (entries : alloc.vec.Vec (A × Std.I64)) :
    ∃ result, invariant_from_serialized_weights equal hash label entries = .ok result ∧
      decode result = InvariantInput.construct (decodeEntries entries.val) :=
  ⟨expected label [] entries.val, construct_exact equal hash lawful label entries, decode_expected label entries.val⟩

include lawful in
theorem duplicate_rejected (label : String) (entries : alloc.vec.Vec (A × Std.I64)) (asset : A)
    (duplicate : Basket.firstDuplicate [] (entries.val.map Prod.fst) = some asset) :
    invariant_from_serialized_weights equal hash label entries = .ok (.Err asset) := by
  rw [construct_exact equal hash lawful]
  simp [expected, duplicate]

include lawful in
theorem accepted_fields (label : String) (entries : alloc.vec.Vec (A × Std.I64))
    (unique : (entries.val.map Prod.fst).Nodup) :
    invariant_from_serialized_weights equal hash label entries = .ok (.Ok ⟨label, canonical entries.val⟩) := by
  have duplicate := (Basket.firstDuplicate_none_iff (entries.val.map Prod.fst) []).mpr (by simpa using unique)
  rw [construct_exact equal hash lawful]
  simp [expected, duplicate]

include lawful in
theorem accepted_valid (label : String) (entries : alloc.vec.Vec (A × Std.I64))
    (invariant : rate.LinearInvariant A)
    (accepted : invariant_from_serialized_weights equal hash label entries = .ok (.Ok invariant)) :
    invariant.name = label ∧ Inspection.ValidWeights (decodeEntries invariant.weights) := by
  obtain ⟨result, executed, decoded⟩ := construct_refines equal hash lawful label entries
  have same : result = .Ok invariant := by simpa [accepted] using executed.symm
  subst result
  refine ⟨?_, InvariantInput.successful_weights_valid _ _ decoded.symm⟩
  rw [construct_exact equal hash lawful] at accepted
  unfold expected at accepted
  cases duplicate : Basket.firstDuplicate [] (entries.val.map Prod.fst) with
  | some asset => simp [duplicate] at accepted
  | none =>
    simp only [List.map_nil, duplicate, List.nil_append, Result.ok.injEq,
      core.result.Result.Ok.injEq] at accepted
    exact congrArg rate.LinearInvariant.name accepted.symm

end Axionomy.InvariantInputRefinement
