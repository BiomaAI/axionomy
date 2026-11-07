import Axionomy.AccountMap
import Axionomy.BasketOperations

namespace Axionomy.Inspection
variable {K T : Type}

/-- Read-only lookup returns the first matching record, including when callers
construct an assessment containing repeated account identifiers. -/
def first (equal : K → K → Bool) (key : T → K) (query : K) (entries : List T) : Option T :=
  entries.find? (fun entry => equal (key entry) query)

theorem first_none (equal : K → K → Bool) (key : T → K) (query : K) (entries : List T) :
    first equal key query entries = none ↔ ∀ entry ∈ entries, equal (key entry) query = false := by
  induction entries with
  | nil => simp [first]
  | cons head rest ih =>
    cases compared : equal (key head) query <;>
      simp [first, List.find?, compared] at *

theorem first_prefix (equal : K → K → Bool) (key : T → K) (query : K)
    (leading : List T) (entry : T) (tail : List T)
    (earlier : ∀ value ∈ leading, equal (key value) query = false)
    (found : equal (key entry) query = true) :
    first equal key query (leading ++ entry :: tail) = some entry := by
  induction leading with
  | nil => simp [first, found]
  | cons head rest ih =>
    have skipped := earlier head (by simp)
    have restSkipped : ∀ value ∈ rest, equal (key value) query = false := by
      intro value member
      exact earlier value (List.mem_cons_of_mem head member)
    simpa [first, skipped] using ih restSkipped

theorem first_member (equal : K → K → Bool) (key : T → K) (query : K)
    (entries : List T) (entry : T) (found : first equal key query entries = some entry) :
    entry ∈ entries ∧ equal (key entry) query = true := by
  induction entries with
  | nil => simp [first] at found
  | cons head rest ih =>
    cases compared : equal (key head) query with
    | false =>
      have remaining : first equal key query rest = some entry := by simpa [first, compared] using found
      obtain ⟨present, matched⟩ := ih remaining
      exact ⟨by simp [present], matched⟩
    | true =>
      have same : head = entry := by simpa [first, compared] using found
      subst entry
      exact ⟨by simp, compared⟩

def weight [DecidableEq K] (entries : List (K × Int)) (asset : K) (value : Int) : List (K × Int) :=
  if value = 0 then Account.erase asset entries else Account.upsert asset value entries

def ValidWeights (entries : List (K × Int)) : Prop :=
  (entries.map Prod.fst).Nodup ∧ ∀ entry ∈ entries, entry.2 ≠ 0

theorem zero_removes [DecidableEq K] (entries : List (K × Int)) (asset : K) :
    weight entries asset 0 = Account.erase asset entries := by simp [weight]

theorem other_weight_unchanged [DecidableEq K] (entries : List (K × Int))
    (asset other : K) (value : Int) (different : asset ≠ other) :
    Account.find? other (weight entries asset value) = Account.find? other entries := by
  by_cases zero : value = 0
  · simp [weight, zero, Account.find_erase_other asset other entries (Ne.symm different)]
  · simp [weight, zero, Account.find_upsert_other asset other value entries (Ne.symm different)]

theorem weights_valid [DecidableEq K] (entries : List (K × Int))
    (asset : K) (value : Int) (valid : ValidWeights entries) :
    ValidWeights (weight entries asset value) := by
  by_cases zero : value = 0
  · have sub := BasketOperations.erase_sublist asset entries
    refine ⟨?_, ?_⟩
    · simpa [weight, zero] using valid.1.sublist (sub.map Prod.fst)
    · intro entry member
      apply valid.2 entry
      exact sub.subset (by simpa [weight, zero] using member)
  · unfold weight
    rw [ite_eq_right zero]
    induction entries with
    | nil => simp [Account.upsert, ValidWeights, zero]
    | cons entry rest ih =>
      rcases entry with ⟨key, old⟩
      simp only [ValidWeights, List.map_cons, List.nodup_cons, List.forall_mem_cons] at valid
      have remaining := ih ⟨valid.1.2, valid.2.2⟩
      by_cases same : asset = key
      · subst key
        simpa [Account.upsert, ValidWeights, zero] using And.intro valid.1 (And.intro zero valid.2.2)
      · simp only [Account.upsert, same, ↓reduceIte, ValidWeights, List.map_cons,
          List.nodup_cons, List.forall_mem_cons]
        exact ⟨⟨by simpa [BasketOperations.mem_upsert_keys, Ne.symm same] using valid.1.1,
          remaining.1⟩, valid.2.1, remaining.2⟩

end Axionomy.Inspection
