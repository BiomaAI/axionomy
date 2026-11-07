import Axionomy.AccountMap

namespace Axionomy.BasketOperations

def add {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (state amounts : List (A × Q)) : Except A Unit × List (A × Q) :=
  match Account.credit ops state amounts with
  | .ok updated => (.ok (), updated)
  | .error asset => (.error asset, state)

def scaleInto {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (mul : Q → Q → Option Q) (units : Q) : List (A × Q) → List (A × Q) → Except A (List (A × Q))
  | [], scaled => .ok scaled
  | (asset, amount) :: rest, scaled =>
    match mul amount units with
    | none => .error asset
    | some value => scaleInto ops mul units rest (Account.store ops scaled asset value)

def scale {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (mul : Q → Q → Option Q) (entries : List (A × Q)) (units : Q) :=
  scaleInto ops mul units entries []

def matchesEntries {A Q : Type} [DecidableEq A] [DecidableEq Q]
    (left right : List (A × Q)) : Bool :=
  left.all fun (asset, amount) => decide (Account.find? asset right = some amount)

def equal {A Q : Type} [DecidableEq A] [DecidableEq Q] (left right : List (A × Q)) : Bool :=
  decide (left.length = right.length) && matchesEntries left right

theorem equal_iff {A Q : Type} [DecidableEq A] [DecidableEq Q] (left right : List (A × Q)) :
    equal left right = true ↔ left.length = right.length ∧
      ∀ entry ∈ left, Account.find? entry.1 right = some entry.2 := by
  simp [equal, matchesEntries]

theorem find_some_iff {A Q : Type} [DecidableEq A] (entries : List (A × Q))
    (unique : (entries.map Prod.fst).Nodup) (asset : A) (amount : Q) :
    Account.find? asset entries = some amount ↔ (asset, amount) ∈ entries := by
  induction entries with
  | nil => simp [Account.find?]
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    simp only [List.map_cons, List.nodup_cons] at unique
    by_cases equal : asset = key
    · subst key
      have absent : (asset, amount) ∉ rest := fun h => unique.1 (List.mem_map.mpr ⟨_, h, rfl⟩)
      simp [Account.find?, absent, eq_comm]
    · simp [Account.find?, equal, ih unique.2]

theorem equal_self {A Q : Type} [DecidableEq A] [DecidableEq Q] (entries : List (A × Q))
    (unique : (entries.map Prod.fst).Nodup) : equal entries entries = true := by
  rw [equal_iff]
  exact ⟨rfl, fun entry member => (find_some_iff entries unique entry.1 entry.2).mpr member⟩

theorem equal_of_permutation {A Q : Type} [DecidableEq A] [DecidableEq Q]
    (left right : List (A × Q)) (unique : (right.map Prod.fst).Nodup) (permutation : left.Perm right) :
    equal left right = true := by
  rw [equal_iff]
  exact ⟨permutation.length_eq, fun entry member =>
    (find_some_iff right unique entry.1 entry.2).mpr (permutation.subset member)⟩

theorem pairs_nodup {A Q : Type} (entries : List (A × Q))
    (unique : (entries.map Prod.fst).Nodup) : entries.Nodup := by
  induction entries with
  | nil => simp
  | cons entry rest ih =>
    simp only [List.map_cons, List.nodup_cons] at unique
    exact List.nodup_cons.mpr ⟨fun member => unique.1 (List.mem_map.mpr ⟨entry, member, rfl⟩), ih unique.2⟩

theorem equal_iff_permutation {A Q : Type} [DecidableEq A] [DecidableEq Q]
    (left right : List (A × Q)) (uniqueLeft : (left.map Prod.fst).Nodup)
    (uniqueRight : (right.map Prod.fst).Nodup) : equal left right = true ↔ left.Perm right := by
  constructor
  · intro same
    obtain ⟨lengths, agrees⟩ := (equal_iff left right).mp same
    have subset : left ⊆ right := fun entry member =>
      (find_some_iff right uniqueRight entry.1 entry.2).mp (agrees entry member)
    apply (List.perm_ext_iff_of_nodup (pairs_nodup left uniqueLeft) (pairs_nodup right uniqueRight)).mpr
    intro entry
    constructor
    · exact fun member => subset member
    · intro member
      by_cases absent : entry ∈ left
      · exact absent
      exfalso
      have unique : (entry :: left).Nodup := List.nodup_cons.mpr ⟨absent, pairs_nodup left uniqueLeft⟩
      have included : entry :: left ⊆ right := by
        intro other h
        rcases List.mem_cons.mp h with h | h
        · simpa [h] using member
        · exact subset h
      have impossible := unique.length_le_of_subset included
      simp only [List.length_cons] at impossible
      omega
  · exact equal_of_permutation left right uniqueRight

theorem add_rejection_atomic {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (state amounts : List (A × Q)) (asset : A) (rejected : (add ops state amounts).1 = .error asset) :
    (add ops state amounts).2 = state := by
  unfold add at *
  split at * <;> simp_all

theorem add_success_exact {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (state amounts updated : List (A × Q)) :
    add ops state amounts = (.ok (), updated) ↔ Account.credit ops state amounts = .ok updated := by
  unfold add
  cases Account.credit ops state amounts <;> simp

def Valid {A Q : Type} (ops : Account.Arithmetic Q) (entries : List (A × Q)) : Prop :=
  (entries.map Prod.fst).Nodup ∧ ∀ entry ∈ entries, ops.isZero entry.2 = false

theorem mem_upsert_keys {A Q : Type} [DecidableEq A] (asset other : A) (value : Q)
    (entries : List (A × Q)) :
    other ∈ (Account.upsert asset value entries).map Prod.fst ↔
      other = asset ∨ other ∈ entries.map Prod.fst := by
  induction entries with
  | nil => simp [Account.upsert]
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases equal : asset = key
    · subst key; simp [Account.upsert]
    · simp [Account.upsert, equal, ih, or_left_comm]

theorem upsert_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (asset : A) (value : Q) (entries : List (A × Q)) (valid : Valid ops entries)
    (nonzero : ops.isZero value = false) : Valid ops (Account.upsert asset value entries) := by
  induction entries with
  | nil => simp [Account.upsert, Valid, nonzero]
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    simp only [Valid, List.map_cons, List.nodup_cons, List.forall_mem_cons] at valid
    have tail := ih ⟨valid.1.2, valid.2.2⟩
    by_cases equal : asset = key
    · subst key
      simp only [Account.upsert, ↓reduceIte, Valid, List.map_cons, List.nodup_cons, List.forall_mem_cons]
      exact ⟨valid.1, nonzero, valid.2.2⟩
    · simp only [Account.upsert, equal, ↓reduceIte, Valid, List.map_cons, List.nodup_cons,
        List.forall_mem_cons]
      exact ⟨⟨by simpa [mem_upsert_keys, Ne.symm equal] using valid.1.1, tail.1⟩, valid.2.1, tail.2⟩

theorem erase_sublist {A Q : Type} [DecidableEq A] (asset : A) (entries : List (A × Q)) :
    (Account.erase asset entries).Sublist entries := by
  induction entries with
  | nil => simp [Account.erase]
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases equal : asset = key
    · simp [Account.erase, equal]
    · simpa [Account.erase, equal] using ih.cons_cons (key, value)

theorem store_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (entries : List (A × Q)) (asset : A) (value : Q) (valid : Valid ops entries) :
    Valid ops (Account.store ops entries asset value) := by
  unfold Account.store
  split
  · have sub := erase_sublist asset entries
    exact ⟨valid.1.sublist (sub.map Prod.fst), fun entry member => valid.2 entry (sub.subset member)⟩
  · exact upsert_valid ops asset value entries valid (by simpa using ‹¬ops.isZero value = true›)

theorem credit_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (amounts state updated : List (A × Q)) (valid : Valid ops state)
    (success : Account.credit ops state amounts = .ok updated) : Valid ops updated := by
  induction amounts generalizing state with
  | nil => simp only [Account.credit, Except.ok.injEq] at success; exact success ▸ valid
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    simp only [Account.credit] at success
    cases added : ops.add (Account.balance ops.zero state asset) amount with
    | none => simp [added] at success
    | some value => exact ih _ (store_valid ops state asset value valid) (by simpa [added] using success)

theorem scaleInto_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (mul : Q → Q → Option Q) (units : Q) (entries scaled updated : List (A × Q))
    (valid : Valid ops scaled) (success : scaleInto ops mul units entries scaled = .ok updated) :
    Valid ops updated := by
  induction entries generalizing scaled with
  | nil => simp only [scaleInto, Except.ok.injEq] at success; exact success ▸ valid
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    simp only [scaleInto] at success
    cases multiplied : mul amount units with
    | none => simp [multiplied] at success
    | some value => exact ih _ (store_valid ops scaled asset value valid) (by simpa [multiplied] using success)

theorem add_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (state amounts : List (A × Q)) (valid : Valid ops state) : Valid ops (add ops state amounts).2 := by
  unfold add
  cases result : Account.credit ops state amounts with
  | error asset => exact valid
  | ok updated => exact credit_valid ops amounts state updated valid result

theorem scale_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (mul : Q → Q → Option Q) (entries : List (A × Q)) (units : Q) (updated : List (A × Q))
    (success : scale ops mul entries units = .ok updated) : Valid ops updated :=
  scaleInto_valid ops mul units entries [] updated (by simp [Valid]) success

theorem debit_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (amounts state : List (A × Q)) (valid : Valid ops state) : Valid ops (Account.debit ops state amounts) := by
  induction amounts generalizing state with
  | nil => exact valid
  | cons entry rest ih => exact ih _ (store_valid ops state entry.1 _ valid)

theorem prepare_valid {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (state consumed produced updated : List (A × Q)) (valid : Valid ops state)
    (success : Account.prepare ops state consumed produced = .ok updated) : Valid ops updated := by
  have result := (Account.prepare_success_iff ops state consumed produced updated).mp success
  exact credit_valid ops produced _ updated (debit_valid ops consumed state valid) result.2

theorem shortfall_keys_sublist {A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (state required : List (A × Q)) :
    ((Account.shortfall ops state required).map Prod.fst).Sublist (required.map Prod.fst) := by
  induction required with
  | nil => simp [Account.shortfall]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    cases h : ops.lt (Account.balance ops.zero state asset) amount with
    | false => simpa [Account.shortfall, h] using ih.cons asset
    | true => simpa [Account.shortfall, h] using ih.cons_cons asset

theorem shortfall_valid {A : Type} [DecidableEq A] (limit : Nat)
    (state required : List (A × Nat)) (unique : (required.map Prod.fst).Nodup) :
    Valid (Account.naturals limit) (Account.shortfall (Account.naturals limit) state required) := by
  refine ⟨unique.sublist (shortfall_keys_sublist _ state required), ?_⟩
  intro entry member
  obtain ⟨amount, _, less, value⟩ := (Account.shortfall_exact limit state required entry).mp member
  have positive : 0 < entry.2 := by omega
  simp [Account.naturals, Nat.ne_of_gt positive]

section Conversion
variable {A Q R : Type} [DecidableEq A]
variable (ops : Account.Arithmetic Q) (other : Account.Arithmetic R) (f : Q → R)
variable (laws : Account.Hom ops other f)

include laws in
theorem map_add (state amounts : List (A × Q)) :
    ((add ops state amounts).1, Account.mapEntries f (add ops state amounts).2) =
      add other (Account.mapEntries f state) (Account.mapEntries f amounts) := by
  unfold add
  rw [← Account.map_credit ops other f laws]
  cases Account.credit ops state amounts <;> rfl

include laws in
theorem map_scaleInto (mul : Q → Q → Option Q) (otherMul : R → R → Option R)
    (mulLaw : ∀ x y, (mul x y).map f = otherMul (f x) (f y))
    (units : Q) (entries scaled : List (A × Q)) :
    (scaleInto ops mul units entries scaled).map (Account.mapEntries f) =
      scaleInto other otherMul (f units) (Account.mapEntries f entries) (Account.mapEntries f scaled) := by
  induction entries generalizing scaled with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    simp only [Account.mapEntries_cons, scaleInto, ← mulLaw]
    cases h : mul amount units with
    | none => rfl
    | some value =>
      simp only [Option.map_some]
      rw [← Account.map_store ops other f laws]
      exact ih _

end Conversion
end Axionomy.BasketOperations
