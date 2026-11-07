import Axionomy.AnalysisRefinement
import Axionomy.CommitRefinement

namespace Axionomy.FilteringRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

def applicableAssessment : Transaction.Assessment AccountId A RateId Role → Bool
  | .applicable _ _ => true
  | _ => false

def selected [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Transaction.Model AccountId A RateId Role) (proposal : Transaction.Proposal AccountId RateId Role) : Option Bool :=
  (Transaction.analyze cmpAccount cmpRole model proposal).map (fun analysis => applicableAssessment analysis.assessment)

section Filtering
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

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem is_applicable_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok result ∧
      selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) = some result := by
  obtain ⟨analysis, executed, decoded⟩ := AnalysisRefinement.analyze_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd laws model proposal parameterFits rulesReady invariantFits accountCapacity
  refine ⟨applicableAssessment (TransactionDecode.exchangeAssessment analysis.assessment), ?_, ?_⟩
  · simp only [economy.Economy.is_applicable, economy.Economy.assess, executed, bind_ok]
    cases analysis.assessment <;> rfl
  · simp [selected, decoded, TransactionDecode.analysis]

omit [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
theorem filter_loop (choose : exchange.Exchange RateId Role AccountId Std.U64 → Bool)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (remaining : List (exchange.Exchange RateId Role AccountId Std.U64))
    (iter : alloc.vec.into_iter.IntoIter (exchange.Exchange RateId Role AccountId Std.U64))
    (contents : iter.val = remaining)
    (accumulated : alloc.vec.Vec (exchange.Exchange RateId Role AccountId Std.U64))
    (capacity : accumulated.val.length + remaining.length ≤ Usize.max)
    (evaluate : ∀ proposal ∈ remaining, economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      model proposal = .ok (choose proposal)) :
    ∃ result, economy.Economy.applicable_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar
      (core.iter.traits.collect.IntoIteratorVec _) iter model accumulated = .ok result ∧
      result.val = accumulated.val ++ remaining.filter choose := by
  induction remaining generalizing iter accumulated with
  | nil =>
    refine ⟨accumulated, ?_, by simp⟩
    unfold economy.Economy.applicable_loop
    rw [loop.eq_1]
    simp [economy.Economy.applicable_loop.body,
      CommitRefinement.owning_next_none iter contents]
  | cons proposal rest ih =>
    obtain ⟨next, advanced, tail⟩ := CommitRefinement.owning_next_cons iter proposal rest contents
    have called := evaluate proposal (by simp)
    have following := fun proposal member => evaluate proposal (List.mem_cons_of_mem _ member)
    cases keep : choose proposal with
    | false =>
      obtain ⟨result, executed, values⟩ := ih next tail accumulated (by simp at capacity; omega) following
      refine ⟨result, ?_, ?_⟩
      · unfold economy.Economy.applicable_loop
        rw [loop.eq_1]
        simp only [economy.Economy.applicable_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
          called, keep, Bool.false_eq_true, ↓reduceIte]
        exact executed
      · simpa [keep] using values
    | true =>
      obtain ⟨grown, pushed, grows⟩ := ModelConstructionRefinement.push_call accumulated proposal (by simp at capacity; omega)
      obtain ⟨result, executed, values⟩ := ih next tail grown (by simp [grows] at *; omega) following
      refine ⟨result, ?_, ?_⟩
      · unfold economy.Economy.applicable_loop
        rw [loop.eq_1]
        simp only [economy.Economy.applicable_loop.body, advanced, bind_ok, Aeneas.Std.uncurry,
          called, keep, ↓reduceIte, pushed]
        exact executed
      · simpa [grows, keep, List.append_assoc] using values

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem applicable_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (candidates : alloc.vec.Vec (exchange.Exchange RateId Role AccountId Std.U64))
    (parameterFits : ∀ proposal ∈ candidates.val, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ candidates.val, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ result, model_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd model candidates = .ok result ∧
      result.val = candidates.val.filter (fun proposal =>
        (selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal)).getD false) ∧
      ∀ proposal ∈ candidates.val, ∃ accepted,
        selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal) = some accepted := by
  let choose := fun proposal => (selected cmpAccount cmpRole (TransactionDecode.model model) (TransactionDecode.proposal proposal)).getD false
  have total (proposal) (member : proposal ∈ candidates.val) := is_applicable_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd laws model proposal (parameterFits proposal member) (rulesReady proposal member) invariantFits accountCapacity
  have evaluate (proposal) (member : proposal ∈ candidates.val) : economy.Economy.is_applicable accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok (choose proposal) := by
    obtain ⟨result, called, specified⟩ := total proposal member
    simpa [choose, specified] using called
  obtain ⟨result, called, specified⟩ := filter_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd choose model candidates.val candidates rfl
    (alloc.vec.Vec.new _) (by simp [alloc.vec.Vec.new]) evaluate
  refine ⟨result, ?_, ?_, ?_⟩
  · unfold model_applicable economy.Economy.applicable
    simp only [core.iter.traits.collect.IntoIteratorVec, alloc.vec.IntoIteratorVec.into_iter]
    dsimp only [alloc.vec.into_iter.IntoIter]
    rw [bind_ok]
    exact called
  · simpa [alloc.vec.Vec.new] using specified
  · intro proposal member
    obtain ⟨result, _, specified⟩ := total proposal member
    exact ⟨result, specified⟩

end Filtering
end Axionomy.FilteringRefinement
