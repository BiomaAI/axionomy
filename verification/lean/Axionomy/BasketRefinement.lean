import Axionomy.Basket
import BasketExtraction

namespace Axionomy.BasketRefinement
open Aeneas Aeneas.Std BasketExtracted

-- This explicit premise requires Rust equality to be total and match asset
-- identity. Rust's Eq trait alone cannot establish that law for arbitrary code.
def LawfulEquality {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A) : Prop :=
  ∀ left right, eqInst.partialEqInst.eq left right = .ok (decide (left = right))

theorem contains_exact {A V : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hEq : LawfulEquality eqInst) (asset : A) (entries : List (A × V)) :
    mapContains eqInst.partialEqInst.eq asset entries =
      .ok (decide (asset ∈ entries.map Prod.fst)) := by
  induction entries with
  | nil => simp [mapContains]
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases h : asset = key
    · subst asset
      simp [mapContains, hEq key key]
    · simp [mapContains, hEq asset key, h, ih]

theorem insert_fresh {A V : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hEq : LawfulEquality eqInst) (asset : A) (value : V)
    (entries : List (A × V)) (fresh : asset ∉ entries.map Prod.fst) :
    mapInsert eqInst.partialEqInst.eq asset value entries =
      .ok (none, entries ++ [(asset, value)]) := by
  induction entries with
  | nil => simp [mapInsert]
  | cons entry rest ih =>
    rcases entry with ⟨key, oldValue⟩
    simp only [List.map_cons, List.mem_cons, not_or] at fresh
    simp [mapInsert, hEq asset key, fresh.1, ih fresh.2]

theorem retain_exact {A V F : Type} (keep : F → V → Result (Bool × F))
    (predicate : V → Bool) (hKeep : ∀ state value, keep state value = .ok (predicate value, state))
    (state : F) (entries : List (A × V)) :
    retainValues keep state entries = .ok (entries.filter (fun entry => predicate entry.2)) := by
  induction entries with
  | nil => simp [retainValues]
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    simp [retainValues, hKeep, ih, List.filter_cons]

def expected {A : Type} [DecidableEq A]
    (seen remaining : List (A × Std.U64)) :
    core.result.Result (basket.Basket A Std.U64) (basket.BasketError A) :=
  match Basket.firstDuplicate (seen.map Prod.fst) (remaining.map Prod.fst) with
  | some asset => .Err (.DuplicateAsset asset)
  | none => .Ok ⟨(seen ++ remaining).filter (fun entry => !(decide (entry.2 = 0#u64)))⟩

abbrev iterator (A : Type) := core.iter.traits.collect.IntoIteratorVec (A × Std.U64)

abbrev keep {A : Type} (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A) :=
  basket.construct_entries.closure.Insts.CoreOpsFunctionFnMutTupleShared0QuantityBool
    eqInst hashInst U64.Insts.Num_traitsIdentitiesZero (iterator A)

theorem retain_nonzero {A : Type} (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
    (entries : List (A × Std.U64)) :
    basket.retain_quantities (keep eqInst hashInst) entries () =
      .ok (entries.filter (fun entry => !(decide (entry.2 = 0#u64)))) := by
  unfold basket.retain_quantities
  apply retain_exact (keep eqInst hashInst).call_mut (fun value => !(decide (value = 0#u64)))
  intro state value
  simp [
    basket.construct_entries.closure.Insts.CoreOpsFunctionFnMutTupleShared0QuantityBool.call_mut,
    quantity.Quantity.scalar_is_zero,
    U64.Insts.Num_traitsIdentitiesZero.is_zero]

def iteratorFrom {T : Type} (entries : List T) (bound : entries.length ≤ Usize.max) :
    alloc.vec.into_iter.IntoIter T := alloc.vec.Vec.from entries bound

theorem next_nil {T : Type} (bound : ([] : List T).length ≤ Usize.max) :
    alloc.vec.into_iter.IteratorIntoIter.next (iteratorFrom [] bound) =
      .ok (none, iteratorFrom [] bound) := rfl

theorem next_cons {T : Type} (entry : T) (rest : List T)
    (bound : (entry :: rest).length ≤ Usize.max) :
    alloc.vec.into_iter.IteratorIntoIter.next (iteratorFrom (entry :: rest) bound) =
      .ok (some entry, iteratorFrom rest (Nat.le_of_succ_le bound)) := by
  unfold alloc.vec.into_iter.IteratorIntoIter.next
  split
  · rename_i h
    have impossible := (alloc.vec.Vec.from_val (entry :: rest) bound).symm.trans h
    contradiction
  · rename_i head tail h
    have same := (alloc.vec.Vec.from_val (entry :: rest) bound).symm.trans h
    obtain ⟨rfl, rfl⟩ := List.cons.inj same
    rfl

theorem body_nil {A : Type} (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
    (bound : ([] : List (A × Std.U64)).length ≤ Usize.max) (seen : List (A × Std.U64)) :
    basket.construct_entries_loop.body eqInst hashInst U64.Insts.Num_traitsIdentitiesZero
      (iterator A) (iteratorFrom [] bound) seen =
      .ok (.done (.Ok ⟨seen.filter (fun entry => !(decide (entry.2 = 0#u64)))⟩)) := by
  unfold basket.construct_entries_loop.body
  simp only [iterator, core.iter.traits.collect.IntoIteratorVec,
    core.iter.traits.iterator.IteratorVecIntoIter, next_nil, bind_ok]
  rw [retain_nonzero]
  simp only [bind_ok]
  rfl

theorem body_cons {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (hEq : LawfulEquality eqInst)
    (asset : A) (quantity : Std.U64) (rest : List (A × Std.U64))
    (bound : ((asset, quantity) :: rest).length ≤ Usize.max) (seen : List (A × Std.U64)) :
    basket.construct_entries_loop.body eqInst hashInst U64.Insts.Num_traitsIdentitiesZero
      (iterator A) (iteratorFrom ((asset, quantity) :: rest) bound) seen =
      .ok (if asset ∈ seen.map Prod.fst then .done (.Err (.DuplicateAsset asset))
        else .cont (iteratorFrom rest (Nat.le_of_succ_le bound), seen ++ [(asset, quantity)])) := by
  unfold basket.construct_entries_loop.body
  simp only [iterator, core.iter.traits.collect.IntoIteratorVec,
    core.iter.traits.iterator.IteratorVecIntoIter, next_cons, bind_ok]
  have equiv : equivalent.Equivalent.Blanket.equivalent eqInst (core.borrow.Borrow.Blanket A) =
      eqInst.partialEqInst.eq := by
    funext left right
    simp [equivalent.Equivalent.Blanket.equivalent,
      core.borrow.Borrow.Blanket.borrow]
  simp only [indexmap.map.IndexMap.contains_key, equiv,
    contains_exact eqInst hEq, bind_ok]
  by_cases present : asset ∈ seen.map Prod.fst
  · simp [present]
  · simp [present, indexmap.map.IndexMap.insert, insert_fresh eqInst hEq _ _ _ present]

theorem loop_refines {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (hEq : LawfulEquality eqInst)
    (remaining : List (A × Std.U64)) (bound : remaining.length ≤ Usize.max)
    (seen : List (A × Std.U64)) :
    basket.construct_entries_loop eqInst hashInst U64.Insts.Num_traitsIdentitiesZero
      (iterator A) (iteratorFrom remaining bound) seen = .ok (expected seen remaining) := by
  induction remaining generalizing seen with
  | nil =>
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp [body_nil, bind_ok, expected, Basket.firstDuplicate]
  | cons entry rest ih =>
    rcases entry with ⟨asset, quantity⟩
    unfold basket.construct_entries_loop
    rw [loop.eq_1]
    simp only [body_cons eqInst hashInst hEq]
    by_cases present : asset ∈ seen.map Prod.fst
    · simp [present, expected, Basket.firstDuplicate]
    · simp only [present, ↓reduceIte, bind_ok]
      change basket.construct_entries_loop eqInst hashInst U64.Insts.Num_traitsIdentitiesZero
        (iterator A) (iteratorFrom rest (Nat.le_of_succ_le bound)) (seen ++ [(asset, quantity)]) = _
      rw [ih]
      simp [expected, Basket.firstDuplicate, present, List.map_append, List.append_assoc]

theorem construct_exact {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (hEq : LawfulEquality eqInst)
    (entries : alloc.vec.Vec (A × Std.U64)) :
    BasketExtracted.construct eqInst hashInst entries = .ok (expected [] entries.val) := by
  have hIter : alloc.vec.IntoIteratorVec.into_iter entries =
      .ok (iteratorFrom entries.val entries.property) := by
    simp [alloc.vec.IntoIteratorVec.into_iter, iteratorFrom]
  unfold BasketExtracted.construct basket.construct_entries
  simp only [indexmap.map.IndexMapKVRandomState.new, core.iter.traits.collect.IntoIteratorVec,
    hIter, bind_ok]
  exact loop_refines eqInst hashInst hEq entries.val entries.property []

def decodeEntries {A : Type} (entries : List (A × Std.U64)) : List (A × Nat) :=
  entries.map (fun (asset, quantity) => (asset, quantity.val))

def decode {A : Type}
    (result : core.result.Result (basket.Basket A Std.U64) (basket.BasketError A)) :
    Except A (List (A × Nat)) :=
  match result with
  | .Ok output => .ok (decodeEntries output.quantities)
  | .Err (.DuplicateAsset asset) => .error asset

theorem decode_canonical {A : Type} (entries : List (A × Std.U64)) :
    decodeEntries (entries.filter (fun entry => !(decide (entry.2 = 0#u64)))) =
      Basket.canonical (decodeEntries entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, quantity⟩
    by_cases zero : quantity.val = 0
    · have same : quantity = 0#u64 := UScalar.eq_of_val_eq zero
      simp [decodeEntries, Basket.canonical, same] at ih ⊢
      exact ih
    · have nonzero : quantity ≠ 0#u64 := by
        intro h
        subst quantity
        exact zero rfl
      simpa [decodeEntries, Basket.canonical, List.filter_cons, zero, nonzero] using
        congrArg (List.cons (asset, quantity.val)) ih

theorem decode_expected {A : Type} [DecidableEq A] (entries : List (A × Std.U64)) :
    decode (expected [] entries) = Basket.construct (decodeEntries entries) := by
  have keys : (decodeEntries entries).map Prod.fst = entries.map Prod.fst := by
    simp [decodeEntries, List.map_map]
  unfold expected Basket.construct
  rw [keys]
  cases hdup : Basket.firstDuplicate [] (entries.map Prod.fst) <;>
    simp [decode, hdup, decode_canonical]

-- Successful Result.ok also proves termination without an execution failure.
-- Rust-level duplicate errors are distinct from panic/divergence in Result.
theorem construct_refines {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (hEq : LawfulEquality eqInst)
    (entries : alloc.vec.Vec (A × Std.U64)) :
    ∃ result, BasketExtracted.construct eqInst hashInst entries = .ok result ∧
      decode result = Basket.construct (decodeEntries entries.val) :=
  ⟨expected [] entries.val, construct_exact eqInst hashInst hEq entries, decode_expected entries.val⟩

theorem duplicate_rejected {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (hEq : LawfulEquality eqInst)
    (entries : alloc.vec.Vec (A × Std.U64)) (asset : A)
    (duplicate : Basket.firstDuplicate [] (entries.val.map Prod.fst) = some asset) :
    BasketExtracted.construct eqInst hashInst entries = .ok (.Err (.DuplicateAsset asset)) := by
  rw [construct_exact eqInst hashInst hEq]
  simp [expected, duplicate]

-- A concrete equality dictionary witnesses the satisfiability of the premise.
def exactEquality (A : Type) [DecidableEq A] : core.cmp.Eq A :=
  { partialEqInst := { eq := fun left right => .ok (decide (left = right)) } }

theorem exactEquality_lawful (A : Type) [DecidableEq A] :
    LawfulEquality (exactEquality A) := by
  intro left right
  rfl

end Axionomy.BasketRefinement
