import Axionomy.Transaction
import Batteries.Data.List.Basic

namespace Axionomy.Transaction

variable {AccountId A RateId Role : Type} [DecidableEq AccountId] [DecidableEq A]

def effectDelta (entry : AccountId × Effect A) : Delta AccountId A :=
  ⟨entry.1, entry.2.consume, entry.2.produce, entry.2.preserve⟩

-- Each prepared value comes from the corresponding original account, using
-- exactly that account's aggregated consumption and production.
def PreparedEntry (accounts : List (AccountId × Balances A))
    (entry : AccountId × Effect A) (prepared : AccountId × Balances A) : Prop :=
  prepared.1 = entry.1 ∧ ∃ before, Account.find? entry.1 accounts = some before ∧
    Account.prepare (Account.naturals Quantity.u64Max) before entry.2.consume entry.2.produce = .ok prepared.2

theorem prepareOne_success (accounts : List (AccountId × Balances A)) (id : AccountId)
    (effect : Effect A) (updated : Balances A)
    (success : prepareOne (RateId := RateId) (Role := Role) accounts id effect = .ok updated) :
    PreparedEntry accounts (id, effect) (id, updated) := by
  cases found : Account.find? id accounts with
  | none => simp [prepareOne, require, found] at success
  | some before =>
    cases prepared : Account.prepare (Account.naturals Quantity.u64Max) before effect.consume effect.produce with
    | error error => simp [prepareOne, require, found, prepared, liftIssue, Except.mapError] at success
    | ok result =>
      have same : result = updated := by simpa [prepareOne, require, found, prepared, liftIssue, Except.mapError] using success
      exact ⟨rfl, before, found, same ▸ prepared⟩

theorem prepare_success (accounts : List (AccountId × Balances A)) (effects : List (AccountId × Effect A))
    (accumulated result : Preparation AccountId A)
    (success : prepare (RateId := RateId) (Role := Role) accounts effects accumulated = .ok result) :
    ∃ staged, result.accounts = accumulated.accounts ++ staged ∧
      result.deltas = accumulated.deltas ++ effects.map effectDelta ∧
      List.Forall₂ (PreparedEntry accounts) effects staged := by
  induction effects generalizing accumulated with
  | nil =>
    have same : accumulated = result := by simpa [prepare] using success
    exact ⟨[], by simp [same], by simp [same], .nil⟩
  | cons entry rest ih =>
    rcases entry with ⟨id, effect⟩
    cases first : prepareOne (RateId := RateId) (Role := Role) accounts id effect with
    | error error => simp [prepare, first] at success
    | ok updated =>
      have following : prepare (RateId := RateId) (Role := Role) accounts rest
          ⟨accumulated.accounts ++ [(id, updated)], accumulated.deltas ++ [effectDelta (id, effect)]⟩ = .ok result := by
        simpa [prepare, first, effectDelta] using success
      obtain ⟨staged, values, deltas, matched⟩ := ih _ following
      refine ⟨(id, updated) :: staged, ?_, ?_, .cons (prepareOne_success accounts id effect updated first) matched⟩
      · simpa [List.append_assoc] using values
      · simpa [List.append_assoc] using deltas

theorem preparedEntry_keys (accounts : List (AccountId × Balances A))
    (effects : List (AccountId × Effect A)) (prepared : List (AccountId × Balances A))
    (matched : List.Forall₂ (PreparedEntry accounts) effects prepared) :
    prepared.map Prod.fst = effects.map Prod.fst := by
  induction matched with
  | nil => rfl
  | cons first rest ih => simp [first.1, ih]

theorem preparedEntry_existing (accounts : List (AccountId × Balances A))
    (effects : List (AccountId × Effect A)) (prepared : List (AccountId × Balances A))
    (matched : List.Forall₂ (PreparedEntry accounts) effects prepared) :
    ∀ entry ∈ prepared, entry.1 ∈ accounts.map Prod.fst := by
  induction matched with
  | nil => simp
  | @cons source staged sources remaining first rest ih =>
    intro entry member
    rcases List.mem_cons.mp member with equal | tail
    · subst entry
      rw [first.1]
      exact (find_exists_iff_key_mem source.1 accounts).mp ⟨_, first.2.choose_spec.1⟩
    · exact ih entry tail

theorem preparation_contract (accounts : List (AccountId × Balances A))
    (effects : List (AccountId × Effect A)) (result : Preparation AccountId A)
    (success : prepare (RateId := RateId) (Role := Role) accounts effects ⟨[], []⟩ = .ok result) :
    result.accounts.map Prod.fst = effects.map Prod.fst ∧
      result.deltas = effects.map effectDelta ∧
      List.Forall₂ (PreparedEntry accounts) effects result.accounts ∧
      (∀ entry ∈ result.accounts, entry.1 ∈ accounts.map Prod.fst) ∧
      ∃ committed, install accounts result.accounts = some committed := by
  obtain ⟨staged, values, deltas, matched⟩ := prepare_success accounts effects ⟨[], []⟩ result success
  simp only [List.nil_append] at values deltas
  rw [← values] at matched
  have existing := preparedEntry_existing accounts effects result.accounts matched
  exact ⟨preparedEntry_keys accounts effects result.accounts matched, deltas, matched, existing,
    install_exists accounts result.accounts existing⟩

theorem preparation_unique (accounts : List (AccountId × Balances A))
    (effects : List (AccountId × Effect A)) (result : Preparation AccountId A)
    (unique : (effects.map Prod.fst).Nodup)
    (success : prepare (RateId := RateId) (Role := Role) accounts effects ⟨[], []⟩ = .ok result) :
    (result.accounts.map Prod.fst).Nodup ∧ (result.deltas.map Conservation.Delta.account).Nodup := by
  have contract := preparation_contract accounts effects result success
  constructor
  · rw [contract.1]; exact unique
  · rw [contract.2.1]
    simpa [List.map_map, Function.comp_def, effectDelta] using unique

end Axionomy.Transaction
