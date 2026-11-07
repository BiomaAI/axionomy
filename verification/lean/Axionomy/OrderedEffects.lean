import Axionomy.TransactionAggregation

namespace Axionomy.OrderedEffects

-- These are laws of the identifier order, independently of any transaction.
-- Matching a Rust comparison function to a logical function alone does not
-- establish that it defines a lawful order.
structure OrderLaws {K : Type} (cmp : K → K → Ordering) : Prop where
  reflexive : ∀ x, cmp x x = .eq
  equal : ∀ x y, cmp x y = .eq → x = y
  transitive : ∀ x y z, cmp x y = .lt → cmp y z = .lt → cmp x z = .lt
  greater : ∀ x y, cmp x y = .gt → cmp y x = .lt

def Sorted {K V : Type} (cmp : K → K → Ordering) (entries : List (K × V)) : Prop :=
  (entries.map Prod.fst).Pairwise (fun x y => cmp x y = .lt)

theorem put_keys {K V : Type} (cmp : K → K → Ordering) (laws : OrderLaws cmp)
    (query : K) (entries : List (K × V)) (value : V) :
    ∀ key ∈ ((RateConstruction.locate cmp query entries).put value).map Prod.fst,
      key = query ∨ key ∈ entries.map Prod.fst := by
  intro key member
  obtain ⟨entry, present, same⟩ := List.mem_map.mp member
  subst key
  exact TransactionAggregation.put_valid cmp laws.equal query entries
    (fun key _ => key = query ∨ key ∈ entries.map Prod.fst) value (Or.inl rfl)
    (fun entry member => Or.inr (List.mem_map.mpr ⟨entry, member, rfl⟩)) entry present

theorem put_sorted {K V : Type} (cmp : K → K → Ordering) (laws : OrderLaws cmp)
    (query : K) (entries : List (K × V)) (value : V) (sorted : Sorted cmp entries) :
    Sorted cmp ((RateConstruction.locate cmp query entries).put value) := by
  induction entries with
  | nil => simp [Sorted, RateConstruction.locate, RateConstruction.Position.put]
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    have parts : (∀ other ∈ rest.map Prod.fst, cmp key other = .lt) ∧ Sorted cmp rest := by
      simpa [Sorted] using sorted
    cases compared : cmp query key with
    | lt =>
      have before : ∀ other ∈ ((key, old) :: rest).map Prod.fst, cmp query other = .lt := by
        intro other member
        rcases List.mem_cons.mp member with same | tail
        · simpa [same] using compared
        · exact laws.transitive query key other compared (parts.1 other tail)
      simpa [Sorted, RateConstruction.locate, compared, RateConstruction.Position.put] using And.intro before sorted
    | eq =>
      have same := laws.equal query key compared
      subst query
      simpa [Sorted, RateConstruction.locate, compared, RateConstruction.Position.put] using sorted
    | gt =>
      have after : ∀ other ∈ ((RateConstruction.locate cmp query rest).put value).map Prod.fst, cmp key other = .lt := by
        intro other member
        rcases put_keys cmp laws query rest value other member with same | tail
        · subst other; exact laws.greater query key compared
        · exact parts.1 other tail
      simpa [Sorted, RateConstruction.locate, compared, RateConstruction.Position.put] using And.intro after (ih parts.2)

theorem sorted_unique {K V : Type} (cmp : K → K → Ordering) (laws : OrderLaws cmp)
    (entries : List (K × V)) (sorted : Sorted cmp entries) : (entries.map Prod.fst).Nodup := by
  exact List.Pairwise.imp (fun {x y} less same => by
    subst y
    rw [laws.reflexive x] at less
    contradiction) sorted

theorem naturalOrder : OrderLaws (compare : Nat → Nat → Ordering) where
  reflexive := fun _ => Nat.compare_eq_eq.mpr rfl
  equal := fun _ _ => Nat.compare_eq_eq.mp
  transitive := fun _ _ _ first second => Nat.compare_eq_lt.mpr (Nat.lt_trans (Nat.compare_eq_lt.mp first) (Nat.compare_eq_lt.mp second))
  greater := fun _ _ value => Nat.compare_eq_lt.mpr (Nat.compare_eq_gt.mp value)

section Aggregation
open Transaction
variable {AccountId A RateId Role : Type} [DecidableEq A]

theorem staticEffects_sorted (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderLaws cmpAccount) (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (remaining : List Role) (initial result : List (AccountId × Effect A))
    (sorted : Sorted cmpAccount initial)
    (success : staticEffects cmpAccount cmpRole proposal rule remaining initial = .ok result) : Sorted cmpAccount result := by
  induction remaining generalizing initial with
  | nil => exact (Except.ok.inj success) ▸ sorted
  | cons role rest ih =>
    cases binding : Evaluation.orderedFind cmpRole role proposal.bindings with
    | none => simp [staticEffects, binding, require] at success
    | some id =>
      cases merged : mergeStatic cmpRole proposal rule role ((RateConstruction.locate cmpAccount id initial).old.getD {}) with
      | error error => simp [staticEffects, binding, require, merged] at success
      | ok updated =>
        exact ih _ (put_sorted cmpAccount laws id initial updated sorted)
          (by simpa only [staticEffects, binding, require, checked_bind_ok, merged] using success)

theorem computedEffects_sorted (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderLaws cmpAccount) (proposal : Proposal AccountId RateId Role)
    (context : Evaluation.Context Role A Nat) (direction : Direction)
    (remaining : List (Computed Role A)) (initial result : List (AccountId × Effect A))
    (sorted : Sorted cmpAccount initial)
    (success : computedEffects cmpAccount cmpRole proposal context direction remaining initial = .ok result) : Sorted cmpAccount result := by
  induction remaining generalizing initial with
  | nil => exact (Except.ok.inj success) ▸ sorted
  | cons amount rest ih =>
    cases evaluated : Evaluation.eval (Evaluation.naturals Quantity.u64Max) context amount.quantity with
    | error error => simp [computedEffects, evaluated] at success
    | ok value =>
      cases binding : Evaluation.orderedFind cmpRole amount.role proposal.bindings with
      | none => simp [computedEffects, evaluated, binding, require] at success
      | some id =>
        cases merged : mergeComputed direction amount.asset value ((RateConstruction.locate cmpAccount id initial).old.getD {}) with
        | error error => simp [computedEffects, evaluated, binding, require, merged, rateResult] at success
        | ok updated =>
          exact ih _ (put_sorted cmpAccount laws id initial updated sorted)
            (by simpa only [computedEffects, evaluated, binding, require, checked_bind_ok, merged, rateResult] using success)

theorem computedChannels_sorted (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderLaws cmpAccount) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (context : Evaluation.Context Role A Nat) (initial result : List (AccountId × Effect A))
    (sorted : Sorted cmpAccount initial)
    (success : computedChannels cmpAccount cmpRole proposal rule context initial = .ok result) : Sorted cmpAccount result := by
  cases consumed : computedEffects cmpAccount cmpRole proposal context .consume rule.computedConsume initial with
  | error error => simp [computedChannels, consumed] at success
  | ok consume =>
    have first := computedEffects_sorted cmpAccount cmpRole laws proposal context .consume rule.computedConsume initial consume sorted consumed
    cases produced : computedEffects cmpAccount cmpRole proposal context .produce rule.computedProduce consume with
    | error error => simp [computedChannels, consumed, produced] at success
    | ok produce =>
      have second := computedEffects_sorted cmpAccount cmpRole laws proposal context .produce rule.computedProduce consume produce first produced
      exact computedEffects_sorted cmpAccount cmpRole laws proposal context .preserve rule.computedPreserve produce result second
        (by simpa [computedChannels, consumed, produced] using success)

theorem aggregate_sorted (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderLaws cmpAccount) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (context : Evaluation.Context Role A Nat) (result : List (AccountId × Effect A))
    (success : aggregate cmpAccount cmpRole proposal rule context = .ok result) : Sorted cmpAccount result := by
  cases initial : staticEffects cmpAccount cmpRole proposal rule rule.roles [] with
  | error error => simp [aggregate, initial] at success
  | ok entries =>
    have first := staticEffects_sorted cmpAccount cmpRole laws proposal rule rule.roles [] entries (by simp [Sorted]) initial
    exact computedChannels_sorted cmpAccount cmpRole laws proposal rule context entries result first
      (by simpa [aggregate, initial] using success)

theorem aggregate_contract (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (laws : OrderLaws cmpAccount) (accounts : List AccountId) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (context : Evaluation.Context Role A Nat) (result : List (AccountId × Effect A))
    (boundStatic : ∀ role ∈ rule.roles, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ id ∈ accounts)
    (boundComputed : ∀ amount ∈ rule.computedConsume ++ rule.computedProduce ++ rule.computedPreserve,
      ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ id ∈ accounts)
    (success : aggregate cmpAccount cmpRole proposal rule context = .ok result) :
    TransactionAggregation.ValidEffects (· ∈ accounts) result ∧ Sorted cmpAccount result ∧
      (result.map Prod.fst).Nodup ∧ result.length ≤ accounts.length := by
  have valid := TransactionAggregation.aggregate_valid cmpAccount cmpRole laws.equal (· ∈ accounts) proposal rule context result
    boundStatic boundComputed success
  have sorted := aggregate_sorted cmpAccount cmpRole laws proposal rule context result success
  have unique := sorted_unique cmpAccount laws result sorted
  refine ⟨valid, sorted, unique, ?_⟩
  have subset : result.map Prod.fst ⊆ accounts := by
    intro id member
    obtain ⟨entry, present, same⟩ := List.mem_map.mp member
    rw [← same]
    exact (valid entry present).1
  simpa using unique.length_le_of_subset subset

end Aggregation
end Axionomy.OrderedEffects
