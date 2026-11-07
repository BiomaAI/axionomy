import Axionomy.InspectionCollectionRefinement

namespace Axionomy.InspectionModelAssetsRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement
open InspectionAssetsRefinement InspectionCollectionRefinement ConservationRefinement
variable {AccountId Role A RateId N M : Type}

def accountAssets (holder : account.Account A N) : List A := holder.balances.quantities.map Prod.fst
def invariantAssets (invariant : rate.LinearInvariant A) : List A := invariant.weights.map Prod.fst

def modelReferences (cmp : Role → Role → Ordering)
    (model : economy.Economy AccountId A RateId Role N) : List A :=
  (model.accounts.map Prod.snd).flatMap accountAssets ++
  (model.rates.map Prod.snd).flatMap (rateReferences cmp) ++
  model.invariants.val.flatMap invariantAssets

section Loops
variable (assetOrd : core.cmp.Ord A) (cmpA : A → A → Ordering)
  (lawA : LawfulOrdering assetOrd cmpA)

include lawA in
theorem accounts_loop (remaining : List (account.Account A N)) (collected : List A) :
    economy.Economy.asset_keys_loop0 (AccountId := AccountId) assetOrd remaining collected =
      .ok (Discovery.registerAll cmpA (remaining.flatMap accountAssets) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold economy.Economy.asset_keys_loop0
    rw [loop.eq_1]
    simp [economy.Economy.asset_keys_loop0.body,
      indexmap.map.iter.Values.Insts.CoreIterTraitsIteratorIteratorSharedAV.next, Discovery.registerAll]
  | cons holder rest ih =>
    have body : economy.Economy.asset_keys_loop0.body (AccountId := AccountId) assetOrd (holder :: rest) collected =
        .ok (.cont (rest, Discovery.registerAll cmpA (accountAssets holder) collected)) := by
      simp [economy.Economy.asset_keys_loop0.body,
        indexmap.map.iter.Values.Insts.CoreIterTraitsIteratorIteratorSharedAV.next,
        alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref, account.Account.impl.balances,
        basket_assets assetOrd cmpA lawA, accountAssets]
    unfold economy.Economy.asset_keys_loop0
    rw [loop.eq_1]
    simp only [body, bind_ok]
    change economy.Economy.asset_keys_loop0 (AccountId := AccountId) assetOrd rest
      (Discovery.registerAll cmpA (accountAssets holder) collected) = _
    simpa [Discovery.registerAll_append] using ih (Discovery.registerAll cmpA (accountAssets holder) collected)

variable (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)

include lawA in
theorem invariants_loop (remaining : List (rate.LinearInvariant A))
    (iter : core.slice.iter.Iter (rate.LinearInvariant A)) (suffix : iter.slice.val.drop iter.i = remaining)
    (collected : List A) :
    economy.Economy.asset_keys_loop2 assetEq assetHash assetOrd iter collected =
      .ok (Discovery.registerAll cmpA (remaining.flatMap invariantAssets) collected) := by
  induction remaining generalizing iter collected with
  | nil =>
    unfold economy.Economy.asset_keys_loop2
    rw [loop.eq_1]
    simp [economy.Economy.asset_keys_loop2.body, next_empty iter suffix, Discovery.registerAll]
  | cons invariant rest ih =>
    obtain ⟨advanced, tail⟩ := next_cons iter invariant rest suffix
    have body : economy.Economy.asset_keys_loop2.body assetEq assetHash assetOrd iter collected =
        .ok (.cont ({ iter with i := iter.i + 1 }, Discovery.registerAll cmpA (invariantAssets invariant) collected)) := by
      simp [economy.Economy.asset_keys_loop2.body, advanced, invariantAssets,
        invariant_assets assetOrd cmpA lawA assetEq assetHash]
    unfold economy.Economy.asset_keys_loop2
    rw [loop.eq_1]
    simp only [body, bind_ok]
    change economy.Economy.asset_keys_loop2 assetEq assetHash assetOrd { iter with i := iter.i + 1 }
      (Discovery.registerAll cmpA (invariantAssets invariant) collected) = _
    simpa [Discovery.registerAll_append] using
      ih { iter with i := iter.i + 1 } tail (Discovery.registerAll cmpA (invariantAssets invariant) collected)

variable (assetClone : core.clone.Clone A) (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
  (scalar : quantity.QuantityScalar N M) (cmpR : Role → Role → Ordering) (lawR : LawfulOrdering roleOrd cmpR)

include lawA lawR in
theorem rates_loop (remaining : List (rate.Rate Role A N))
    (fits : ∀ rule ∈ remaining, (computedReferences rule).length ≤ Usize.max) (collected : List A) :
    economy.Economy.asset_keys_loop1 (RateId := RateId) assetClone assetEq assetHash assetOrd
      roleClone roleOrd scalar remaining collected =
      .ok (Discovery.registerAll cmpA (remaining.flatMap (rateReferences cmpR)) collected) := by
  induction remaining generalizing collected with
  | nil =>
    unfold economy.Economy.asset_keys_loop1
    rw [loop.eq_1]
    simp [economy.Economy.asset_keys_loop1.body,
      indexmap.map.iter.Values.Insts.CoreIterTraitsIteratorIteratorSharedAV.next, Discovery.registerAll]
  | cons rule rest ih =>
    have body : economy.Economy.asset_keys_loop1.body (RateId := RateId) assetClone assetEq assetHash assetOrd
        roleClone roleOrd scalar (rule :: rest) collected =
        .ok (.cont (rest, Discovery.registerAll cmpA (rateReferences cmpR rule) collected)) := by
      simp [economy.Economy.asset_keys_loop1.body,
        indexmap.map.iter.Values.Insts.CoreIterTraitsIteratorIteratorSharedAV.next,
        rate_assets roleClone roleOrd assetClone assetEq assetHash scalar assetOrd cmpA lawA cmpR lawR
          rule (fits rule (by simp))]
    unfold economy.Economy.asset_keys_loop1
    rw [loop.eq_1]
    simp only [body, bind_ok]
    change economy.Economy.asset_keys_loop1 (RateId := RateId) assetClone assetEq assetHash assetOrd
      roleClone roleOrd scalar rest (Discovery.registerAll cmpA (rateReferences cmpR rule) collected) = _
    simpa [Discovery.registerAll_append] using
      ih (fun value member => fits value (List.mem_cons_of_mem rule member))
        (Discovery.registerAll cmpA (rateReferences cmpR rule) collected)

end Loops

section Model
variable (cloneId : core.clone.Clone AccountId) (eqId : core.cmp.Eq AccountId)
  (hashId : core.hash.Hash AccountId) (ordId : core.cmp.Ord AccountId)
  (cloneA : core.clone.Clone A) (eqA : core.cmp.Eq A) (hashA : core.hash.Hash A) (ordA : core.cmp.Ord A)
  (cloneRate : core.clone.Clone RateId) (eqRate : core.cmp.Eq RateId)
  (hashRate : core.hash.Hash RateId) (ordRate : core.cmp.Ord RateId)
  (cloneRole : core.clone.Clone Role) (ordRole : core.cmp.Ord Role) (scalar : quantity.QuantityScalar N M)
  (cmpA : A → A → Ordering) (lawA : LawfulOrdering ordA cmpA)
  (cmpR : Role → Role → Ordering) (lawR : LawfulOrdering ordRole cmpR)

include lawA lawR in
theorem model_assets (model : economy.Economy AccountId A RateId Role N)
    (fits : ∀ rule ∈ model.rates.map Prod.snd, (computedReferences rule).length ≤ Usize.max) :
    economy.Economy.asset_keys cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model =
      .ok (Discovery.registerAll cmpA (modelReferences cmpR model) []) := by
  have invariantSuffix : (alloc.vec.Vec.deref model.invariants).val.drop 0 = model.invariants.val := by
    simp [alloc.vec.Vec.deref]
  simp [economy.Economy.asset_keys, alloc.collections.btree.set.BTreeSetTGlobal.new,
    indexmap.map.IndexMap.values, alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref,
    accounts_loop (AccountId := AccountId) ordA cmpA lawA,
    rates_loop (RateId := RateId) ordA cmpA lawA eqA hashA cloneA cloneRole ordRole scalar cmpR lawR _ fits,
    core.slice.Slice.iter,
    invariants_loop ordA cmpA lawA eqA hashA model.invariants.val
      ⟨alloc.vec.Vec.deref model.invariants, 0⟩ invariantSuffix,
    modelReferences, Discovery.registerAll_append]

include lawA lawR in
theorem model_assets_contract (laws : OrderedEffects.OrderLaws cmpA)
    (model : economy.Economy AccountId A RateId Role N)
    (fits : ∀ rule ∈ model.rates.map Prod.snd, (computedReferences rule).length ≤ Usize.max) :
    ∃ result, economy.Economy.asset_keys cloneId eqId hashId ordId cloneA eqA hashA ordA
      cloneRate eqRate hashRate ordRate cloneRole ordRole scalar model = .ok result ∧
      (∀ asset, asset ∈ result ↔ asset ∈ modelReferences cmpR model) ∧
      result.Pairwise (fun left right => cmpA left right = .lt) ∧ result.Nodup ∧
      result.length ≤ (modelReferences cmpR model).length := by
  refine ⟨_, model_assets cloneId eqId hashId ordId cloneA eqA hashA ordA cloneRate eqRate hashRate ordRate
    cloneRole ordRole scalar cmpA lawA cmpR lawR model fits, ?_, ?_, ?_, ?_⟩
  · intro asset
    simpa using Discovery.registerAll_mem cmpA laws.equal asset (modelReferences cmpR model) []
  · exact Observations.registerAll_sorted cmpA laws _ [] (by simp)
  · exact Observations.registerAll_unique cmpA laws _
  · simpa using Discovery.registerAll_length_le cmpA (modelReferences cmpR model) []

end Model
end Axionomy.InspectionModelAssetsRefinement
