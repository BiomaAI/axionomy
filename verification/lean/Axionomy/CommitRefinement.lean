import Axionomy.Transaction
import Axionomy.ValidationCompleteRefinement

namespace Axionomy.CommitRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

variable {K V AccountId A RateId Role N : Type}

theorem mutable_lookup [DecidableEq K] (equal : K → K → Result Bool)
    (lawful : ∀ left right, equal left right = .ok (decide (left = right)))
    (query : K) (entries : List (K × V)) (value : V)
    (found : Account.find? query entries = some value) :
    ∃ restore, mapIndexMut equal query entries = .ok (value, restore) ∧
      ∀ updated, Transaction.replaceExisting query updated entries = some (restore updated) := by
  induction entries with
  | nil => simp [Account.find?] at found
  | cons entry rest ih =>
    rcases entry with ⟨key, stored⟩
    by_cases same : query = key
    · have values : stored = value := by simpa [Account.find?, same] using found
      subst stored
      refine ⟨fun updated => (key, updated) :: rest, ?_, ?_⟩
      · simp [mapIndexMut, lawful, same]
      · intro updated; simp [Transaction.replaceExisting, same]
    · have tailFound : Account.find? query rest = some value := by simpa [Account.find?, same] using found
      obtain ⟨restore, called, exact⟩ := ih tailFound
      refine ⟨fun updated => (key, stored) :: restore updated, ?_, ?_⟩
      · simp [mapIndexMut, lawful, same, called]
      · intro updated; simp [Transaction.replaceExisting, same, exact updated]

theorem index_mut_refines [DecidableEq K] (eqInst : core.cmp.Eq K) (hashInst : core.hash.Hash K)
    (lawful : LawfulEquality eqInst) (query : K) (entries : List (K × V))
    (present : query ∈ entries.map Prod.fst) :
    ∃ selected restore,
      indexmap.map.IndexMap.Insts.CoreOpsIndexIndexMutShared0QV.index_mut hashInst
        (equivalent.Equivalent.Blanket eqInst (core.borrow.Borrow.Blanket K))
        std.hash.random.RandomState.Insts.CoreHashBuildHasherDefaultHasher entries query = .ok (selected, restore) ∧
      ∀ updated, Transaction.replaceExisting query updated entries = some (restore updated) := by
  obtain ⟨value, found⟩ := (Transaction.find_exists_iff_key_mem query entries).mpr present
  obtain ⟨restore, executed, replaces⟩ := mutable_lookup eqInst.partialEqInst.eq lawful query entries value found
  refine ⟨value, restore, ?_, replaces⟩
  simpa only [indexmap.map.IndexMap.Insts.CoreOpsIndexIndexMutShared0QV.index_mut, equivalent_eq] using executed

theorem owning_next_none {T : Type} (iter : alloc.vec.into_iter.IntoIter T) (empty : iter.val = []) :
    alloc.vec.into_iter.IteratorIntoIter.next iter = .ok (none, iter) := by
  have same : iter = alloc.vec.Vec.from [] (by simp) := alloc.vec.Vec.ext _ _ (by simpa using empty)
  subst iter
  rfl

theorem owning_next_cons {T : Type} (iter : alloc.vec.into_iter.IntoIter T) (value : T) (rest : List T)
    (contents : iter.val = value :: rest) :
    ∃ remaining, alloc.vec.into_iter.IteratorIntoIter.next iter = .ok (some value, remaining) ∧ remaining.val = rest := by
  unfold alloc.vec.into_iter.IteratorIntoIter.next
  split
  · rename_i empty
    have impossible := contents.symm.trans empty
    cases impossible
  · rename_i first remaining observed
    have same := List.cons.inj (contents.symm.trans observed)
    rcases same with ⟨sameFirst, sameRest⟩
    subst first
    subst remaining
    exact ⟨_, rfl, alloc.vec.Vec.from_val _ _⟩

section Commit
variable [DecidableEq AccountId]
variable (eqInst : core.cmp.Eq AccountId) (hashInst : core.hash.Hash AccountId)
variable (lawful : LawfulEquality eqInst)

include lawful in
theorem apply_loop_refines (remaining : List (AccountId × account.Account A N))
    (iter : alloc.vec.into_iter.IntoIter (AccountId × account.Account A N)) (contents : iter.val = remaining)
    (model : economy.Economy AccountId A RateId Role N)
    (present : ∀ entry ∈ remaining, entry.1 ∈ model.accounts.map Prod.fst) :
    ∃ accounts, economy.Economy.apply_loop eqInst hashInst iter model = .ok { model with accounts := accounts } ∧
      Transaction.install model.accounts remaining = some accounts := by
  induction remaining generalizing iter model with
  | nil =>
    refine ⟨model.accounts, ?_, rfl⟩
    unfold economy.Economy.apply_loop
    rw [loop.eq_1]
    simp [economy.Economy.apply_loop.body, owning_next_none iter contents]
  | cons entry rest ih =>
    rcases entry with ⟨id, holder⟩
    obtain ⟨next, advanced, tail⟩ := owning_next_cons iter (id, holder) rest contents
    obtain ⟨selected, restore, indexed, replaces⟩ := index_mut_refines eqInst hashInst lawful id model.accounts
      (present (id, holder) (by simp))
    let updated := restore holder
    have replaced : Transaction.replaceExisting id holder model.accounts = some updated := replaces holder
    have keys := Transaction.replaceExisting_keys id holder model.accounts updated replaced
    obtain ⟨accounts, executed, installed⟩ := ih next tail { model with accounts := updated }
      (fun item member => keys ▸ present item (by simp [member]))
    have body : economy.Economy.apply_loop.body eqInst hashInst iter model =
        .ok (.cont (next, { model with accounts := updated })) := by
      unfold economy.Economy.apply_loop.body
      rw [advanced]
      simp only [bind_ok]
      change (do
        let (_, back) ← indexmap.map.IndexMap.Insts.CoreOpsIndexIndexMutShared0QV.index_mut hashInst
          (equivalent.Equivalent.Blanket eqInst (core.borrow.Borrow.Blanket AccountId))
          std.hash.random.RandomState.Insts.CoreHashBuildHasherDefaultHasher model.accounts id
        .ok (ControlFlow.cont (β := economy.Economy AccountId A RateId Role N)
          (next, { model with accounts := back holder }))) = _
      rw [indexed]
      simp [updated]
    refine ⟨accounts, ?_, ?_⟩
    · unfold economy.Economy.apply_loop
      rw [loop.eq_1]
      simp only [body, bind_ok]
      exact executed
    · simpa [Transaction.install, replaced] using installed

include lawful in
theorem snapshot_loop_refines (remaining : List (AccountId × account.Account A N))
    (iter : core.slice.iter.Iter (AccountId × account.Account A N)) (suffix : iter.slice.val.drop iter.i = remaining)
    (accounts : List (AccountId × account.Account A N))
    (present : ∀ entry ∈ remaining, entry.1 ∈ accounts.map Prod.fst) :
    ∃ updated, economy.Economy.accounts_with_prepared_loop eqInst hashInst iter accounts = .ok updated ∧
      Transaction.install accounts remaining = some updated := by
  induction remaining generalizing iter accounts with
  | nil =>
    refine ⟨accounts, ?_, rfl⟩
    unfold economy.Economy.accounts_with_prepared_loop
    rw [loop.eq_1]
    simp [economy.Economy.accounts_with_prepared_loop.body, ConservationRefinement.next_empty iter suffix]
  | cons entry rest ih =>
    rcases entry with ⟨id, holder⟩
    obtain ⟨advanced, tail⟩ := ConservationRefinement.next_cons iter (id, holder) rest suffix
    obtain ⟨selected, restore, indexed, replaces⟩ := index_mut_refines eqInst hashInst lawful id accounts
      (present (id, holder) (by simp))
    let middle := restore holder
    have replaced : Transaction.replaceExisting id holder accounts = some middle := replaces holder
    have keys := Transaction.replaceExisting_keys id holder accounts middle replaced
    obtain ⟨updated, executed, installed⟩ := ih { iter with i := iter.i + 1 } tail middle
      (fun item member => keys ▸ present item (by simp [member]))
    have body : economy.Economy.accounts_with_prepared_loop.body eqInst hashInst iter accounts =
        .ok (.cont ({ iter with i := iter.i + 1 }, middle)) := by
      simp [economy.Economy.accounts_with_prepared_loop.body, advanced,
        alloc.sync.Arc.Insts.CoreCloneClone.clone, indexed, middle]
    refine ⟨updated, ?_, ?_⟩
    · unfold economy.Economy.accounts_with_prepared_loop
      rw [loop.eq_1]
      simp only [body, bind_ok]
      exact executed
    · simpa [Transaction.install, replaced] using installed

include lawful in
theorem apply_loop_frames (remaining : List (AccountId × account.Account A N))
    (iter : alloc.vec.into_iter.IntoIter (AccountId × account.Account A N)) (contents : iter.val = remaining)
    (model : economy.Economy AccountId A RateId Role N)
    (present : ∀ entry ∈ remaining, entry.1 ∈ model.accounts.map Prod.fst) :
    ∃ result, economy.Economy.apply_loop eqInst hashInst iter model = .ok result ∧
      result.rates = model.rates ∧ result.invariants = model.invariants ∧
      result.accounts.map Prod.fst = model.accounts.map Prod.fst ∧
      ∀ id, id ∉ remaining.map Prod.fst → Account.find? id result.accounts = Account.find? id model.accounts := by
  obtain ⟨accounts, executed, installed⟩ := apply_loop_refines eqInst hashInst lawful remaining iter contents model present
  exact ⟨_, executed, rfl, rfl, Transaction.install_keys _ _ _ installed,
    fun id untouched => Transaction.install_find_other id _ _ _ untouched installed⟩

include lawful in
theorem snapshot_refines {M : Type} (accountClone : core.clone.Clone AccountId) (accountOrd : core.cmp.Ord AccountId)
    (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
    (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role) (number : quantity.QuantityScalar N M)
    (hClone : LawfulClone accountClone) (model : economy.Economy AccountId A RateId Role N)
    (prepared : alloc.vec.Vec (AccountId × account.Account A N))
    (present : ∀ entry ∈ prepared.val, entry.1 ∈ model.accounts.map Prod.fst) :
    ∃ result, economy.Economy.accounts_with_prepared accountClone eqInst hashInst accountOrd
        assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd number model prepared = .ok result ∧
      Transaction.install model.accounts prepared.val = some result := by
  have arcClone : LawfulClone (alloc.sync.Arc.Insts.CoreCloneClone (account.Account A N) alloc.alloc.Global.Insts.CoreAllocAllocatorClone) := by
    intro value; rfl
  have cloned := clone_exact accountClone _ hClone arcClone model.accounts
  obtain ⟨result, called, installed⟩ := snapshot_loop_refines eqInst hashInst lawful prepared.val
    ⟨prepared.slice, 0⟩ (by rfl) model.accounts present
  refine ⟨result, ?_, installed⟩
  simpa [economy.Economy.accounts_with_prepared, indexmap.map.IndexMap.Insts.CoreCloneClone.clone, cloned,
    SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter, core.slice.Slice.iter] using called

end Commit
end Axionomy.CommitRefinement
