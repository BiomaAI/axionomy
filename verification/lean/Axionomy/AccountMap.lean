import Axionomy.Account

namespace Axionomy.Account

def mapEntries {A Q R : Type} (f : Q → R) (entries : List (A × Q)) : List (A × R) :=
  entries.map fun (asset, value) => (asset, f value)

@[simp] theorem mapEntries_nil {A Q R : Type} (f : Q → R) :
    mapEntries (A := A) f [] = [] := rfl
@[simp] theorem mapEntries_cons {A Q R : Type} (f : Q → R) (entry : A × Q) (rest : List (A × Q)) :
    mapEntries f (entry :: rest) = (entry.1, f entry.2) :: mapEntries f rest := rfl

structure Hom {Q R : Type} (ops : Arithmetic Q) (other : Arithmetic R) (f : Q → R) : Prop where
  zero : f ops.zero = other.zero
  isZero : ∀ x, other.isZero (f x) = ops.isZero x
  add : ∀ x y, (ops.add x y).map f = other.add (f x) (f y)
  sub : ∀ x y, f (ops.sub x y) = other.sub (f x) (f y)
  lt : ∀ x y, other.lt (f x) (f y) = ops.lt x y

theorem map_find {A Q R : Type} [DecidableEq A] (f : Q → R) (entries : List (A × Q)) (asset : A) :
    (find? asset entries).map f = find? asset (mapEntries f entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases h : asset = key <;> simp [mapEntries, find?, h, ih]

theorem map_balance {A Q R : Type} [DecidableEq A] (f : Q → R) (zero : Q)
    (entries : List (A × Q)) (asset : A) :
    f (balance zero entries asset) = balance (f zero) (mapEntries f entries) asset := by
  unfold balance
  rw [← map_find]
  cases find? asset entries <;> rfl

theorem map_upsert {A Q R : Type} [DecidableEq A] (f : Q → R) (entries : List (A × Q))
    (asset : A) (value : Q) :
    mapEntries f (upsert asset value entries) = upsert asset (f value) (mapEntries f entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases h : asset = key <;> simp [upsert, h, ih]

theorem map_erase {A Q R : Type} [DecidableEq A] (f : Q → R) (entries : List (A × Q)) (asset : A) :
    mapEntries f (erase asset entries) = erase asset (mapEntries f entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases h : asset = key <;> simp [erase, h, ih]

section Mapping
variable {A Q R : Type} [DecidableEq A]
variable (ops : Arithmetic Q) (other : Arithmetic R) (f : Q → R) (laws : Hom ops other f)

include laws in
theorem map_store (entries : List (A × Q)) (asset : A) (value : Q) :
    mapEntries f (store ops entries asset value) =
      store other (mapEntries f entries) asset (f value) := by
  unfold store
  rw [laws.isZero]
  split <;> simp [map_erase, map_upsert]

include laws in
theorem map_credit (amounts state : List (A × Q)) :
    (credit ops state amounts).map (mapEntries f) =
      credit other (mapEntries f state) (mapEntries f amounts) := by
  induction amounts generalizing state with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    have values := laws.add (balance ops.zero state asset) amount
    rw [map_balance f ops.zero state asset, laws.zero] at values
    simp only [mapEntries_cons, credit]
    rw [← values]
    cases added : ops.add (balance ops.zero state asset) amount with
    | none => rfl
    | some value =>
      simp only [Option.map_some]
      rw [← map_store ops other f laws]
      exact ih _

include laws in
theorem map_debit (amounts state : List (A × Q)) :
    mapEntries f (debit ops state amounts) =
      debit other (mapEntries f state) (mapEntries f amounts) := by
  induction amounts generalizing state with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    simp only [mapEntries_cons, debit]
    rw [ih, map_store ops other f laws, laws.sub, map_balance f ops.zero state asset, laws.zero]

include laws in
theorem map_shortfall (amounts state : List (A × Q)) :
    mapEntries f (shortfall ops state amounts) =
      shortfall other (mapEntries f state) (mapEntries f amounts) := by
  induction amounts with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    have less := laws.lt (balance ops.zero state asset) amount
    rw [map_balance f ops.zero state asset, laws.zero] at less
    simp only [mapEntries_cons, shortfall, List.filterMap_cons]
    rw [less]
    cases h : ops.lt (balance ops.zero state asset) amount with
    | false => exact ih
    | true =>
      simp only [↓reduceIte, mapEntries_cons]
      rw [laws.sub, map_balance f ops.zero state asset, laws.zero]
      exact congrArg (List.cons (asset, other.sub (f amount) (balance other.zero (mapEntries f state) asset))) ih

def mapError (error : UpdateError A Q) : UpdateError A R :=
  match error with
  | .overflow asset => .overflow asset
  | .insufficient missing => .insufficient (mapEntries f missing)

def mapOutcome (outcome : Except (UpdateError A Q) Unit × List (A × Q)) :
    Except (UpdateError A R) Unit × List (A × R) :=
  (outcome.1.mapError (mapError f), mapEntries f outcome.2)

include laws in
theorem map_deposit (state amounts : List (A × Q)) :
    mapOutcome f (deposit ops state amounts) =
      deposit other (mapEntries f state) (mapEntries f amounts) := by
  unfold deposit
  rw [← map_credit ops other f laws]
  cases credit ops state amounts <;> rfl

include laws in
theorem map_withdraw (state amounts : List (A × Q)) :
    mapOutcome f (withdraw ops state amounts) =
      withdraw other (mapEntries f state) (mapEntries f amounts) := by
  simp only [withdraw, ← map_shortfall ops other f laws]
  have empty : (mapEntries f (shortfall ops state amounts)).isEmpty = (shortfall ops state amounts).isEmpty := by
    simp [mapEntries]
  rw [empty]
  split <;> simp [mapOutcome, Except.mapError, mapError, map_debit ops other f laws]

def mapPrepared (result : Except (UpdateError A Q) (List (A × Q))) :
    Except (UpdateError A R) (List (A × R)) :=
  (result.map (mapEntries f)).mapError (mapError f)

include laws in
theorem map_prepare (state consumed produced : List (A × Q)) :
    mapPrepared f (prepare ops state consumed produced) =
      prepare other (mapEntries f state) (mapEntries f consumed) (mapEntries f produced) := by
  unfold prepare
  rw [← map_withdraw ops other f laws]
  cases withdrawal : withdraw ops state consumed with
  | mk status debited =>
    cases status with
    | error error => rfl
    | ok value =>
      cases value
      simp only [mapOutcome, Except.mapError]
      rw [← map_deposit ops other f laws]
      cases deposit ops debited produced with
      | mk status updated => cases status <;> rfl

end Mapping
end Axionomy.Account
