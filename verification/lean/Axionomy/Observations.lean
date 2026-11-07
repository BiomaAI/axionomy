import Axionomy.OrderedEffects
import Axionomy.Transaction
import Axionomy.Discovery

namespace Axionomy.Observations

abbrev World (AccountId Asset Q : Type) := List (AccountId × List (Asset × Q))

def account {AccountId Asset Q : Type} [DecidableEq AccountId]
    (world : World AccountId Asset Q) (visible : List AccountId) (id : AccountId) : Option (List (Asset × Q)) :=
  if id ∈ visible then Account.find? id world else none

def balance {AccountId Asset Q : Type} [DecidableEq AccountId] [DecidableEq Asset]
    (zero : Q) (world : World AccountId Asset Q) (visible : List AccountId) (id : AccountId) (asset : Asset) : Option Q :=
  (account world visible id).map (fun entries => Account.balance zero entries asset)

theorem hidden_account {AccountId Asset Q : Type} [DecidableEq AccountId]
    (world : World AccountId Asset Q) (visible : List AccountId) (id : AccountId) (hidden : id ∉ visible) :
    account world visible id = none := by simp [account, hidden]

theorem missing_account {AccountId Asset Q : Type} [DecidableEq AccountId]
    (world : World AccountId Asset Q) (visible : List AccountId) (id : AccountId)
    (missing : Account.find? id world = none) : account world visible id = none := by
  simp [account, missing]

theorem empty_account_zero {AccountId Asset Q : Type} [DecidableEq AccountId] [DecidableEq Asset]
    (zero : Q) (world : World AccountId Asset Q) (visible : List AccountId) (id : AccountId) (asset : Asset)
    (shown : id ∈ visible) (empty : Account.find? id world = some []) :
    balance zero world visible id asset = some zero := by
  simp [balance, account, shown, empty, Account.balance, Account.find?]

theorem noninterference {AccountId Asset Q : Type} [DecidableEq AccountId]
    (left right : World AccountId Asset Q) (visible : List AccountId)
    (same : ∀ id ∈ visible, Account.find? id left = Account.find? id right) :
    account left visible = account right visible := by
  funext id
  by_cases shown : id ∈ visible
  · simp [account, shown, same id shown]
  · simp [account, shown]

def meets {Asset : Type} [DecidableEq Asset] (available required : List (Asset × Nat)) : Bool :=
  required.all fun entry => decide (entry.2 ≤ Account.balance 0 available entry.1)

def satisfies {AccountId Asset : Type} [DecidableEq AccountId] [DecidableEq Asset]
    (world : World AccountId Asset Nat) : World AccountId Asset Nat → Bool
  | [] => true
  | (id, required) :: rest =>
    match Account.find? id world with
    | none => false
    | some available => meets available required && satisfies world rest

theorem satisfies_iff {AccountId Asset : Type} [DecidableEq AccountId] [DecidableEq Asset]
    (world required : World AccountId Asset Nat) : satisfies world required = true ↔
      ∀ entry ∈ required, ∃ available, Account.find? entry.1 world = some available ∧
        ∀ amount ∈ entry.2, amount.2 ≤ Account.balance 0 available amount.1 := by
  induction required with
  | nil => simp [satisfies]
  | cons entry rest ih =>
    rcases entry with ⟨id, required⟩
    cases found : Account.find? id world <;> simp [satisfies, found, meets, ih]

theorem goal_monotone {AccountId Asset : Type} [DecidableEq AccountId] [DecidableEq Asset]
    (left right required : World AccountId Asset Nat)
    (extension : ∀ id available, Account.find? id left = some available →
      ∃ larger, Account.find? id right = some larger ∧ ∀ asset, Account.balance 0 available asset ≤ Account.balance 0 larger asset)
    (satisfied : satisfies left required = true) : satisfies right required = true := by
  rw [satisfies_iff] at satisfied ⊢
  intro entry member
  obtain ⟨available, found, enough⟩ := satisfied entry member
  obtain ⟨larger, present, increased⟩ := extension entry.1 available found
  exact ⟨larger, present, fun amount member => Nat.le_trans (enough amount member) (increased amount.1)⟩

/-- Ordered lookup agrees with identity lookup in a valid sorted map. This is
proved from independent comparator laws, rather than assuming lookup success. -/
theorem ordered_find {K V : Type} [DecidableEq K] (cmp : K → K → Ordering)
    (laws : OrderedEffects.OrderLaws cmp) (query : K) (entries : List (K × V))
    (sorted : OrderedEffects.Sorted cmp entries) :
    Evaluation.orderedFind cmp query entries = Account.find? query entries := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    have parts : (∀ other ∈ rest.map Prod.fst, cmp key other = .lt) ∧ OrderedEffects.Sorted cmp rest := by
      simpa [OrderedEffects.Sorted] using sorted
    cases compared : cmp query key with
    | eq =>
      have same := laws.equal query key compared
      subst query
      simp [Evaluation.orderedFind, compared, Account.find?]
    | gt =>
      have different : query ≠ key := by intro same; subst query; simp [laws.reflexive] at compared
      simp [Evaluation.orderedFind, compared, Account.find?, different, ih parts.2]
    | lt =>
      have different : query ≠ key := by intro same; subst query; simp [laws.reflexive] at compared
      have missing : query ∉ rest.map Prod.fst := by
        intro member
        have impossible := laws.transitive query key query compared (parts.1 query member)
        simp [laws.reflexive] at impossible
      have empty : Account.find? query rest = none := by
        cases found : Account.find? query rest with
        | none => rfl
        | some value => exact False.elim (missing ((Transaction.find_exists_iff_key_mem query rest).mp ⟨value, found⟩))
      simp [Evaluation.orderedFind, compared, Account.find?, different, empty]

/-- Every finite visibility request becomes a sorted, duplicate-free set. -/
theorem register_sorted {K : Type} (cmp : K → K → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (key : K) (entries : List K) (sorted : entries.Pairwise (fun x y => cmp x y = .lt)) :
    (RateConstruction.register cmp key entries).Pairwise (fun x y => cmp x y = .lt) := by
  induction entries with
  | nil => simp [RateConstruction.register]
  | cons first rest ih =>
    have parts := List.pairwise_cons.mp sorted
    cases compared : cmp key first with
    | lt =>
      have before : ∀ x ∈ rest, cmp key x = .lt := fun x member =>
        laws.transitive key first x compared (parts.1 x member)
      have beforeAll : ∀ x ∈ first :: rest, cmp key x = .lt := by
        intro x member
        rcases List.mem_cons.mp member with same | tail
        · simpa [same] using compared
        · exact before x tail
      simpa only [RateConstruction.register, compared] using List.pairwise_cons.mpr ⟨beforeAll, sorted⟩
    | eq => simpa [RateConstruction.register, compared] using sorted
    | gt =>
      have after : ∀ x ∈ RateConstruction.register cmp key rest, cmp first x = .lt := by
        intro x member
        rcases (Discovery.register_mem cmp laws.equal key x rest).mp member with same | tail
        · subst x; exact laws.greater key first compared
        · exact parts.1 x tail
      simpa [RateConstruction.register, compared] using List.pairwise_cons.mpr ⟨after, ih parts.2⟩

theorem registerAll_sorted {K : Type} (cmp : K → K → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (entries accumulated : List K) (sorted : accumulated.Pairwise (fun x y => cmp x y = .lt)) :
    (Discovery.registerAll cmp entries accumulated).Pairwise (fun x y => cmp x y = .lt) := by
  induction entries generalizing accumulated with
  | nil => exact sorted
  | cons key rest ih => exact ih _ (register_sorted cmp laws key accumulated sorted)

theorem registerAll_unique {K : Type} (cmp : K → K → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (entries : List K) : (Discovery.registerAll cmp entries []).Nodup := by
  exact List.Pairwise.imp (fun {x y} less same => by
    subst y
    rw [laws.reflexive x] at less
    contradiction) (registerAll_sorted cmp laws entries [] (by simp))

end Axionomy.Observations
