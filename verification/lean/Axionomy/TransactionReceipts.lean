import Axionomy.TransactionSafety

namespace Axionomy.Transaction
variable {AccountId A RateId Role : Type}

theorem replaceExisting_find_selected {K V : Type} [DecidableEq K]
    (query : K) (value : V) (entries updated : List (K × V))
    (replaced : replaceExisting query value entries = some updated) :
    Account.find? query updated = some value := by
  induction entries generalizing updated with
  | nil => simp [replaceExisting] at replaced
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases same : query = key
    · simp [replaceExisting, same] at replaced
      subst updated
      simp [Account.find?, same]
    · cases tail : replaceExisting query value rest with
      | none => simp [replaceExisting, same, tail] at replaced
      | some values =>
        simp [replaceExisting, same, tail] at replaced
        subst updated
        simpa [Account.find?, same] using ih values tail

theorem install_find_staged {K V : Type} [DecidableEq K]
    (accounts prepared updated : List (K × V))
    (unique : (prepared.map Prod.fst).Nodup)
    (installed : install accounts prepared = some updated) :
    ∀ entry ∈ prepared, Account.find? entry.1 updated = some entry.2 := by
  induction prepared generalizing accounts with
  | nil => simp
  | cons entry rest ih =>
    have distinct : entry.1 ∉ rest.map Prod.fst ∧ (rest.map Prod.fst).Nodup := by simpa using unique
    cases replaced : replaceExisting entry.1 entry.2 accounts with
    | none => simp [install, replaced] at installed
    | some middle =>
      have following : install middle rest = some updated := by simpa [install, replaced] using installed
      intro value member
      rcases List.mem_cons.mp member with same | later
      · subst value
        exact (install_find_other entry.1 middle rest updated distinct.1 following).trans
          (replaceExisting_find_selected entry.1 entry.2 accounts middle replaced)
      · exact ih middle distinct.2 following value later

theorem matched_effect [DecidableEq AccountId] [DecidableEq A]
    (accounts : List (AccountId × Balances A)) (effects : List (AccountId × Effect A))
    (prepared : List (AccountId × Balances A))
    (matched : Forall₂ (PreparedEntry accounts) effects prepared) :
    ∀ entry ∈ effects, ∃ value ∈ prepared, PreparedEntry accounts entry value := by
  induction matched with
  | nil => simp
  | cons related following ih =>
    intro entry member
    rcases List.mem_cons.mp member with same | later
    · subst entry
      exact ⟨_, by simp, related⟩
    · obtain ⟨value, member, relation⟩ := ih entry later
      exact ⟨value, List.mem_cons_of_mem _ member, relation⟩

-- A receipt identifies unique accounts and exactly the successful account
-- preparation whose balance is now observable in the committed model.
def ReceiptChanges [DecidableEq AccountId] [DecidableEq A]
    (before after : Model AccountId A RateId Role) (receipt : Receipt AccountId A RateId Role) : Prop :=
  (receipt.deltas.map Conservation.Delta.account).Nodup ∧
    ∀ delta ∈ receipt.deltas, ∃ original committed,
      Account.find? delta.account before.accounts = some original ∧
      Account.find? delta.account after.accounts = some committed ∧
      Account.prepare (Account.naturals Quantity.u64Max) original delta.consumed delta.produced = .ok committed

theorem apply_receipt_changes [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderedEffects.OrderLaws cmpAccount) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (receipt : Receipt AccountId A RateId Role)
    (after : Model AccountId A RateId Role)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    receipt.proposal = proposal ∧ ReceiptChanges model after receipt := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at success
  | some analysis =>
    have ready := analyze_ready cmpAccount cmpRole laws model proposal analysis analyzed
    cases assessed : analysis.assessment with
    | invalid issues =>
      cases first : issues.head? <;> simp [apply, analyzed, applyAnalysis, assessed, first] at success
    | infeasible accounts missing => simp [apply, analyzed, applyAnalysis, assessed] at success
    | applicable accounts deltas =>
      simp only [AnalysisReady, assessed] at ready
      obtain ⟨effects, staged, preparation, exactDeltas, matched, unique, _⟩ := ready
      cases installed : install model.accounts staged with
      | none => simp [apply, analyzed, applyAnalysis, assessed, preparation, installed] at success
      | some committed =>
        have same : (⟨proposal, deltas⟩ : Receipt AccountId A RateId Role) = receipt ∧
            { model with accounts := committed } = after := by
          simpa [apply, analyzed, applyAnalysis, assessed, preparation, installed] using success
        rcases same with ⟨rfl, rfl⟩
        refine ⟨rfl, ?_, ?_⟩
        · have keys := preparedEntry_keys model.accounts effects staged matched
          simpa [exactDeltas, List.map_map, Function.comp_def, effectDelta, keys] using unique
        · intro delta member
          rw [exactDeltas] at member
          obtain ⟨effect, effectMember, rfl⟩ := List.mem_map.mp member
          obtain ⟨value, stagedMember, related⟩ := matched_effect model.accounts effects staged matched effect effectMember
          obtain ⟨sameKey, original, found, prepared⟩ := related
          refine ⟨original, value.2, found, ?_, prepared⟩
          simpa [effectDelta, sameKey] using install_find_staged model.accounts staged committed unique installed value stagedMember

end Axionomy.Transaction
