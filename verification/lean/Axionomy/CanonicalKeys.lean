import Axionomy.CanonicalSort

namespace Axionomy.CanonicalKeys

variable {AccountId Asset Q : Type}

abbrev World := Observations.World AccountId Asset Q

def tag (id : AccountId) (balances : List (Asset × Q)) : List (AccountId × Asset × Q) :=
  balances.map fun entry => (id, entry.1, entry.2)

def entries (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) : List (AccountId × Asset × Q) :=
  world.flatMap fun entry => tag entry.1 entry.2

def visibleEntries [DecidableEq AccountId] (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q))
    (visible : List AccountId) : List (AccountId × Asset × Q) :=
  visible.flatMap fun id => tag id ((Account.find? id world).getD [])

def existing [DecidableEq AccountId] (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q))
    (visible : List AccountId) : List AccountId :=
  visible.filter fun id => (Account.find? id world).isSome

structure Key (AccountId Asset Q : Type) where
  visible : List AccountId
  existing : List AccountId
  balances : List (AccountId × Asset × Q)
  deriving DecidableEq

def observe [DecidableEq AccountId] (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering)
    (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId) : Key AccountId Asset Q :=
  ⟨visible, existing world visible, CanonicalSort.sort cmp (visibleEntries world visible)⟩

theorem existing_iff [DecidableEq AccountId]
    (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId) (id : AccountId) :
    id ∈ existing world visible ↔ id ∈ visible ∧ ∃ balances, Account.find? id world = some balances := by
  simp [existing, Option.isSome_iff_exists]

theorem existing_length [DecidableEq AccountId]
    (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId) :
    (existing world visible).length ≤ visible.length := List.length_filter_le _ _

theorem visible_member [DecidableEq AccountId]
    (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId)
    (id : AccountId) (asset : Asset) (value : Q) :
    (id, asset, value) ∈ visibleEntries world visible ↔
      id ∈ visible ∧ ∃ balances, Account.find? id world = some balances ∧ (asset, value) ∈ balances := by
  simp only [visibleEntries, List.mem_flatMap]
  constructor
  · rintro ⟨shown, member, contains⟩
    cases found : Account.find? shown world with
    | none => simp [found, tag] at contains
    | some balances =>
      obtain ⟨⟨held, amount⟩, stored, same⟩ := List.mem_map.mp (show (id, asset, value) ∈ tag shown balances by simpa [found] using contains)
      have equalities : shown = id ∧ held = asset ∧ amount = value := by simpa using same
      rcases equalities with ⟨rfl, rfl, rfl⟩
      exact ⟨member, balances, found, stored⟩
  · rintro ⟨shown, balances, found, member⟩
    exact ⟨id, shown, by simp [found, tag, member]⟩

theorem observe_member [DecidableEq AccountId]
    (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering)
    (world : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId)
    (id : AccountId) (asset : Asset) (value : Q) :
    (id, asset, value) ∈ (observe cmp world visible).balances ↔
      id ∈ visible ∧ ∃ balances, Account.find? id world = some balances ∧ (asset, value) ∈ balances := by
  exact (CanonicalSort.sort_perm cmp (visibleEntries world visible)).mem_iff.trans
    (visible_member world visible id asset value)

/-- Hidden accounts do not contribute account existence or balance entries. -/
theorem noninterference [DecidableEq AccountId]
    (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering)
    (left right : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId)
    (same : ∀ id ∈ visible, Account.find? id left = Account.find? id right) :
    observe cmp left visible = observe cmp right visible := by
  have present : existing left visible = existing right visible := by
    apply List.filter_congr
    intro id member
    rw [same id member]
  have balances : visibleEntries left visible = visibleEntries right visible := by
    apply List.flatMap_congr
    intro id member
    rw [same id member]
  simp [observe, present, balances]

/-- The account domain is part of observations, including accounts with zero entries. -/
theorem missing_distinguished [DecidableEq AccountId]
    (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering)
    (left right : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId) (id : AccountId)
    (shown : id ∈ visible) (present : Account.find? id left = some []) (absent : Account.find? id right = none) :
    observe cmp left visible ≠ observe cmp right visible := by
  intro same
  have existsLeft : id ∈ (observe cmp left visible).existing := (existing_iff left visible id).mpr ⟨shown, [], present⟩
  rw [same] at existsLeft
  obtain ⟨_, balances, found⟩ := (existing_iff right visible id).mp existsLeft
  simp [absent] at found

/-- Equal produced keys recover each stored asset lookup. No assumption about
list iteration order or fingerprint collision freedom is made. -/
theorem stored_lookup_equal [DecidableEq AccountId] [DecidableEq Asset]
    (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering)
    (left right : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId)
    (same : observe cmp left visible = observe cmp right visible)
    (id : AccountId) (shown : id ∈ visible) (asset : Asset) (leftBalances rightBalances : List (Asset × Q))
    (leftFound : Account.find? id left = some leftBalances) (rightFound : Account.find? id right = some rightBalances)
    (leftUnique : (leftBalances.map Prod.fst).Nodup) (rightUnique : (rightBalances.map Prod.fst).Nodup) :
    Account.find? asset leftBalances = Account.find? asset rightBalances := by
  have members (value : Q) : (asset, value) ∈ leftBalances ↔ (asset, value) ∈ rightBalances := by
    have equalMembers := congrArg (fun key : Key AccountId Asset Q => (id, asset, value) ∈ key.balances) same
    simpa [observe_member, shown, leftFound, rightFound] using (Iff.of_eq equalMembers)
  cases leftValue : Account.find? asset leftBalances with
  | none =>
    cases rightValue : Account.find? asset rightBalances with
    | none => rfl
    | some value =>
      have found := (BasketOperations.find_some_iff leftBalances leftUnique asset value).mpr
        ((members value).mpr ((BasketOperations.find_some_iff rightBalances rightUnique asset value).mp rightValue))
      simp [leftValue] at found
  | some value =>
    exact ((BasketOperations.find_some_iff rightBalances rightUnique asset value).mpr
      ((members value).mp ((BasketOperations.find_some_iff leftBalances leftUnique asset value).mp leftValue))).symm

theorem queries_equal [DecidableEq AccountId] [DecidableEq Asset]
    (zero : Q) (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering)
    (left right : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (leftVisible rightVisible : List AccountId)
    (same : observe cmp left leftVisible = observe cmp right rightVisible)
    (leftUnique : ∀ id balances, Account.find? id left = some balances → (balances.map Prod.fst).Nodup)
    (rightUnique : ∀ id balances, Account.find? id right = some balances → (balances.map Prod.fst).Nodup) :
    Observations.balance zero left leftVisible = Observations.balance zero right rightVisible := by
  have visibleSame : leftVisible = rightVisible := congrArg Key.visible same
  subst rightVisible
  funext id asset
  by_cases shown : id ∈ leftVisible
  · have existence : (∃ balances, Account.find? id left = some balances) ↔
        ∃ balances, Account.find? id right = some balances := by
      have equalMembers := congrArg (fun key : Key AccountId Asset Q => id ∈ key.existing) same
      simpa [observe, existing_iff, shown] using Iff.of_eq equalMembers
    cases leftFound : Account.find? id left with
    | none =>
      have missing : Account.find? id right = none := by
        cases found : Account.find? id right with
        | none => rfl
        | some balances =>
          obtain ⟨other, impossible⟩ := existence.mpr ⟨balances, found⟩
          simp [leftFound] at impossible
      simp [Observations.balance, Observations.account, shown, leftFound, missing]
    | some leftBalances =>
      obtain ⟨rightBalances, rightFound⟩ := existence.mp ⟨leftBalances, leftFound⟩
      have lookups := stored_lookup_equal cmp left right leftVisible same id shown asset leftBalances rightBalances
        leftFound rightFound (leftUnique id leftBalances leftFound) (rightUnique id rightBalances rightFound)
      simp [Observations.balance, Observations.account, shown, leftFound, rightFound, Account.balance, lookups]
  · simp [Observations.balance, Observations.account, shown]

/-- Reordering balances within accounts does not change a canonical observation.
Account existence is checked separately, including empty accounts. -/
theorem reordered [DecidableEq AccountId]
    (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (left right : World (AccountId := AccountId) (Asset := Asset) (Q := Q)) (visible : List AccountId)
    (presence : ∀ id ∈ visible, (Account.find? id left).isSome = (Account.find? id right).isSome)
    (balances : ∀ id ∈ visible, ((Account.find? id left).getD []).Perm ((Account.find? id right).getD [])) :
    observe cmp left visible = observe cmp right visible := by
  have present : existing left visible = existing right visible := by
    exact List.filter_congr presence
  have entries : (visibleEntries left visible).Perm (visibleEntries right visible) := by
    apply List.Perm.flatMap_left
    intro id member
    exact (balances id member).map (fun entry => (id, entry.1, entry.2))
  simp [observe, present, CanonicalSort.sort_canonical cmp laws _ _ entries]

theorem state_reordered
    (cmp : (AccountId × Asset × Q) → (AccountId × Asset × Q) → Ordering) (laws : OrderedEffects.OrderLaws cmp)
    (left right : World (AccountId := AccountId) (Asset := Asset) (Q := Q))
    (same : (entries left).Perm (entries right)) :
    CanonicalSort.sort cmp (entries left) = CanonicalSort.sort cmp (entries right) :=
  CanonicalSort.sort_canonical cmp laws _ _ same

end Axionomy.CanonicalKeys
