import Axionomy.FilteringRefinement
import Axionomy.IteratorContract

namespace Axionomy.IteratorFilteringRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement FilteringRefinement
variable {AccountId A RateId Role : Type}

section Filtering
variable {Source Iter : Type}
variable (into : core.iter.traits.collect.IntoIterator Source (exchange.Exchange RateId Role AccountId Std.U64) Iter)
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetClone : LawfulClone assetClone)
variable (hAssetEq : LawfulEquality assetEq) (hRateClone : LawfulClone rateClone) (hRateEq : LawfulEquality rateEq)
variable (hRoleClone : LawfulClone roleClone) (hRoleEq : LawfulEquality roleOrd.eqInst) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

omit [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
theorem filter_loop (choose : exchange.Exchange RateId Role AccountId Std.U64 → Bool)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (remaining : List (exchange.Exchange RateId Role AccountId Std.U64))
    (iter : Iter)
    (yields : IteratorContract.Yields into.iteratorInst.next iter remaining)
    (accumulated : alloc.vec.Vec (exchange.Exchange RateId Role AccountId Std.U64))
    (capacity : accumulated.val.length + remaining.length ≤ Usize.max)
    (evaluate : ∀ proposal ∈ remaining, economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      model proposal = .ok (choose proposal)) :
    ∃ result, economy.Economy.applicable_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      into iter model accumulated = .ok result ∧
      result.val = accumulated.val ++ remaining.filter choose := by
  induction yields generalizing accumulated with
  | done advanced =>
    refine ⟨accumulated, ?_, by simp⟩
    unfold economy.Economy.applicable_loop
    rw [loop.eq_1]
    simp [economy.Economy.applicable_loop.body,
      advanced]
  | @step state after proposal rest advanced tail ih =>
    have called := evaluate proposal (by simp)
    have following := fun proposal member => evaluate proposal (List.mem_cons_of_mem _ member)
    cases keep : choose proposal with
    | false =>
      obtain ⟨result, executed, values⟩ := ih accumulated (by simp at capacity; omega) following
      refine ⟨result, ?_, ?_⟩
      · unfold economy.Economy.applicable_loop
        rw [loop.eq_1]
        simp only [economy.Economy.applicable_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
          called, keep, Bool.false_eq_true, ↓reduceIte]
        exact executed
      · simpa [keep] using values
    | true =>
      obtain ⟨grown, pushed, grows⟩ := ModelConstructionRefinement.push_call accumulated proposal (by simp at capacity; omega)
      obtain ⟨result, executed, values⟩ := ih grown (by simp [grows] at *; omega) following
      refine ⟨result, ?_, ?_⟩
      · unfold economy.Economy.applicable_loop
        rw [loop.eq_1]
        simp only [economy.Economy.applicable_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
          called, keep, ↓reduceIte, pushed]
        exact executed
      · simpa [grows, keep, List.append_assoc] using values

omit [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
theorem filter_loop_failure (error : Error) (choose : exchange.Exchange RateId Role AccountId Std.U64 → Bool)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (remaining : List (exchange.Exchange RateId Role AccountId Std.U64))
    (iter : Iter)
    (fails : IteratorContract.FailsAfter into.iteratorInst.next error iter remaining)
    (accumulated : alloc.vec.Vec (exchange.Exchange RateId Role AccountId Std.U64))
    (capacity : accumulated.val.length + remaining.length ≤ Usize.max)
    (evaluate : ∀ proposal ∈ remaining, economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      model proposal = .ok (choose proposal)) :
    economy.Economy.applicable_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      into iter model accumulated = .fail error := by
  induction fails generalizing accumulated with
  | stopped advanced =>
    unfold economy.Economy.applicable_loop
    rw [loop.eq_1]
    simp [economy.Economy.applicable_loop.body,
      advanced]
  | @step state after proposal rest advanced tail ih =>
    have called := evaluate proposal (by simp)
    have following := fun proposal member => evaluate proposal (List.mem_cons_of_mem _ member)
    cases keep : choose proposal with
    | false =>
      have executed := ih accumulated (by simp at capacity; omega) following
      unfold economy.Economy.applicable_loop
      rw [loop.eq_1]
      simp only [economy.Economy.applicable_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
        called, keep, Bool.false_eq_true, ↓reduceIte]
      exact executed
    | true =>
      obtain ⟨grown, pushed, grows⟩ := ModelConstructionRefinement.push_call accumulated proposal (by simp at capacity; omega)
      have executed := ih grown (by simp [grows] at *; omega) following
      unfold economy.Economy.applicable_loop
      rw [loop.eq_1]
      simp only [economy.Economy.applicable_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
        called, keep, ↓reduceIte, pushed]
      exact executed

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem applicable_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (candidates : Source) (iter : Iter)
    (started : into.into_iter candidates = .ok iter)
    (values : List (exchange.Exchange RateId Role AccountId Std.U64))
    (yields : IteratorContract.Yields into.iteratorInst.next iter values)
    (capacity : values.length ≤ Usize.max)
    (parameterFits : ∀ proposal ∈ values, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ values, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar into model candidates = .ok result ∧
      result.val = values.filter (fun proposal =>
        (selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal)).getD false) ∧
      ∀ proposal ∈ values, ∃ accepted,
        selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) = some accepted := by
  let choose := fun proposal => (selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal)).getD false
  have total (proposal) (member : proposal ∈ values) := is_applicable_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd laws model proposal (parameterFits proposal member) (rulesReady proposal member) invariantFits accountCapacity
  have evaluate (proposal) (member : proposal ∈ values) : economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok (choose proposal) := by
    obtain ⟨result, called, specified⟩ := total proposal member
    simpa [choose, specified] using called
  obtain ⟨result, called, specified⟩ := filter_loop into accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd choose model values iter yields
    (alloc.vec.Vec.new _) (by simpa [alloc.vec.Vec.new] using capacity) evaluate
  refine ⟨result, ?_, ?_, ?_⟩
  · unfold economy.Economy.applicable
    rw [started, bind_ok]
    exact called
  · simpa [alloc.vec.Vec.new] using specified
  · intro proposal member
    obtain ⟨result, _, specified⟩ := total proposal member
    exact ⟨result, specified⟩

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem applicable_failure (error : Error) (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (candidates : Source) (iter : Iter)
    (started : into.into_iter candidates = .ok iter)
    (values : List (exchange.Exchange RateId Role AccountId Std.U64))
    (fails : IteratorContract.FailsAfter into.iteratorInst.next error iter values)
    (capacity : values.length ≤ Usize.max)
    (parameterFits : ∀ proposal ∈ values, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ values, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    economy.Economy.applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar into model candidates = .fail error := by
  let choose := fun proposal => (selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal)).getD false
  have total (proposal) (member : proposal ∈ values) := is_applicable_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd laws model proposal (parameterFits proposal member) (rulesReady proposal member) invariantFits accountCapacity
  have evaluate (proposal) (member : proposal ∈ values) : economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok (choose proposal) := by
    obtain ⟨result, called, specified⟩ := total proposal member
    simpa [choose, specified] using called
  have called := filter_loop_failure into accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd error choose model values iter fails
    (alloc.vec.Vec.new _) (by simpa [alloc.vec.Vec.new] using capacity) evaluate
  unfold economy.Economy.applicable
  rw [started, bind_ok]
  exact called

omit [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
theorem applicable_start_failure (error : Error)
    (model : economy.Economy AccountId A RateId Role Std.U64) (candidates : Source)
    (failed : into.into_iter candidates = .fail error) :
    economy.Economy.applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar into model candidates = .fail error := by
  simp [economy.Economy.applicable, failed]

end Filtering
end Axionomy.IteratorFilteringRefinement
