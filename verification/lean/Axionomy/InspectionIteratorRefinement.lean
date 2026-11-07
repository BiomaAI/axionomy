import Axionomy.InspectionRefinement

namespace Axionomy.InspectionIteratorRefinement
open Aeneas Aeneas.Std AccountExtracted InspectionRefinement
variable {AccountId A RateId Role N M : Type}

theorem slice_yields {T : Type} (iter : core.slice.iter.Iter T) :
    IteratorContract.Yields core.slice.iter.IteratorSliceIter.next iter (iter.slice.val.drop iter.i) := by
  generalize suffix : iter.slice.val.drop iter.i = entries
  induction entries generalizing iter with
  | nil => exact .done (ConservationRefinement.next_empty iter suffix)
  | cons entry rest ih =>
    obtain ⟨called, remaining⟩ := ConservationRefinement.next_cons iter entry rest suffix
    exact .step called (ih { iter with i := iter.i + 1 } remaining)

theorem slice_hint {T : Type} (iter : core.slice.iter.Iter T) :
    core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.size_hint iter =
      exactIteratorHint (iter.slice.val.drop iter.i).length := by
  simp [core.slice.iter.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.size_hint]

theorem weights_iterator (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A)
    (value : rate.LinearInvariant A) :
    ∃ iter, rate.LinearInvariant.impl.weights eqA hashA value = .ok iter ∧
      IteratorContract.Yields rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.next
        iter value.weights ∧
      rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.size_hint iter =
        exactIteratorHint value.weights.length :=
  ⟨value.weights, InspectionRefinement.invariant_weights eqA hashA value, weights_yield _, weights_hint _⟩

theorem weights_public_hint (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A)
    (value : rate.LinearInvariant A) (fits : value.weights.length ≤ Usize.max) :
    invariant_weights_hint eqA hashA value = .ok
      (Usize.ofNatCore value.weights.length (by scalar_tac),
        some (Usize.ofNatCore value.weights.length (by scalar_tac))) := by
  simp [invariant_weights_hint, InspectionRefinement.invariant_weights, weights_hint, exact_hint _ fits]

section Models
variable (cloneId : core.clone.Clone AccountId) (eqId : core.cmp.Eq AccountId)
  (hashId : core.hash.Hash AccountId) (ordId : core.cmp.Ord AccountId)
  (cloneA : core.clone.Clone A) (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A) (ordA : core.cmp.Ord A)
  (cloneRate : core.clone.Clone RateId) (eqRate : core.cmp.Eq RateId)
  (hashRate : core.hash.Hash RateId) (ordRate : core.cmp.Ord RateId)
  (cloneRole : core.clone.Clone Role) (ordRole : core.cmp.Ord Role) (scalar : quantity.QuantityScalar N M)

theorem accounts_iterator (model : economy.Economy AccountId A RateId Role N) :
    ∃ iter, economy.Economy.impl.accounts cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok iter ∧
      IteratorContract.Yields
        economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.next
        iter model.accounts :=
  ⟨model.accounts, model_accounts cloneId eqId hashId ordId cloneA eqA hashA ordA
    cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model, accounts_yield _⟩

theorem accounts_public_next (model : economy.Economy AccountId A RateId Role N) :
    inspect_model_accounts_next cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok model.accounts.head? := by
  simp [inspect_model_accounts_next, model_accounts, account_next]

theorem accounts_public_hint (model : economy.Economy AccountId A RateId Role N)
    (fits : model.accounts.length ≤ Usize.max) :
    inspect_model_accounts_hint cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok
        (Usize.ofNatCore model.accounts.length (by scalar_tac),
          some (Usize.ofNatCore model.accounts.length (by scalar_tac))) := by
  simp [inspect_model_accounts_hint, model_accounts, accounts_hint, exact_hint _ fits]

theorem rate_ids_iterator (model : economy.Economy AccountId A RateId Role N) :
    ∃ iter, economy.Economy.rate_ids cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok iter ∧
      IteratorContract.Yields
        (indexmap.map.iter.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next (V := rate.Rate Role A N))
        iter (model.rates.map Prod.fst) :=
  ⟨model.rates.map Prod.fst, model_rate_ids cloneId eqId hashId ordId cloneA eqA hashA ordA
    cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model, keys_yield _⟩

theorem rate_ids_public_next (model : economy.Economy AccountId A RateId Role N) :
    inspect_model_rate_ids_next cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok (model.rates.map Prod.fst).head? := by
  simp only [inspect_model_rate_ids_next, model_rate_ids, bind_ok]
  cases model.rates <;>
    simp [indexmap.map.iter.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next]

theorem rate_ids_public_hint (model : economy.Economy AccountId A RateId Role N)
    (fits : model.rates.length ≤ Usize.max) :
    inspect_model_rate_ids_hint cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok
        (Usize.ofNatCore model.rates.length (by scalar_tac),
          some (Usize.ofNatCore model.rates.length (by scalar_tac))) := by
  simp [inspect_model_rate_ids_hint, model_rate_ids,
    indexmap.map.iter.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.size_hint, exact_hint _ fits]

theorem invariants_iterator (model : economy.Economy AccountId A RateId Role N) :
    ∃ iter, economy.Economy.impl.invariants cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok iter ∧
      IteratorContract.Yields core.slice.iter.IteratorSliceIter.next iter model.invariants.val := by
  refine ⟨⟨alloc.vec.Vec.deref model.invariants, 0⟩, model_invariants cloneId eqId hashId ordId
    cloneA eqA hashA ordA cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model, ?_⟩
  simpa [alloc.vec.Vec.deref] using slice_yields (⟨alloc.vec.Vec.deref model.invariants, 0⟩)

theorem invariants_public_next (model : economy.Economy AccountId A RateId Role N) :
    inspect_model_invariants_next cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok model.invariants.val.head? := by
  simp only [inspect_model_invariants_next, model_invariants, bind_ok]
  have suffix : (alloc.vec.Vec.deref model.invariants).val.drop 0 = model.invariants.val := by
    simp [alloc.vec.Vec.deref]
  cases contents : model.invariants.val with
  | nil =>
    have called := ConservationRefinement.next_empty
      (⟨alloc.vec.Vec.deref model.invariants, 0⟩) (suffix.trans contents)
    simp [called]
  | cons entry rest =>
    have called := (ConservationRefinement.next_cons
      (⟨alloc.vec.Vec.deref model.invariants, 0⟩) entry rest (suffix.trans contents)).1
    simp [called]

theorem invariants_public_hint (model : economy.Economy AccountId A RateId Role N) :
    inspect_model_invariants_hint cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok
        (Usize.ofNatCore model.invariants.val.length (by scalar_tac),
          some (Usize.ofNatCore model.invariants.val.length (by scalar_tac))) := by
  simp [inspect_model_invariants_hint, model_invariants, slice_hint, alloc.vec.Vec.deref,
    exact_hint _ model.invariants.property]

end Models
end Axionomy.InspectionIteratorRefinement
