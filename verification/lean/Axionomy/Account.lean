import Axionomy.Quantity
import Std.Tactic

namespace Axionomy.Account

-- The specification is independent of Rust storage and can be executed with
-- natural quantities. The refinement instantiates the same operations at u64.
structure Arithmetic (Q : Type) where
  zero : Q
  isZero : Q → Bool
  add : Q → Q → Option Q
  sub : Q → Q → Q
  lt : Q → Q → Bool

def naturals (limit : Nat) : Arithmetic Nat :=
  ⟨0, (· == 0), Quantity.add limit, Nat.sub, fun x y => decide (x < y)⟩

def find? {A Q : Type} [DecidableEq A] (asset : A) : List (A × Q) → Option Q
  | [] => none
  | (key, value) :: rest => if asset = key then some value else find? asset rest

def balance {A Q : Type} [DecidableEq A] (zero : Q) (entries : List (A × Q)) (asset : A) : Q :=
  (find? asset entries).getD zero

def upsert {A Q : Type} [DecidableEq A] (asset : A) (value : Q) : List (A × Q) → List (A × Q)
  | [] => [(asset, value)]
  | (key, old) :: rest =>
    if asset = key then (key, value) :: rest else (key, old) :: upsert asset value rest

def erase {A Q : Type} [DecidableEq A] (asset : A) : List (A × Q) → List (A × Q)
  | [] => []
  | (key, value) :: rest => if asset = key then rest else (key, value) :: erase asset rest

def store {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (entries : List (A × Q)) (asset : A) (value : Q) : List (A × Q) :=
  if ops.isZero value then erase asset entries else upsert asset value entries

def credit {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state : List (A × Q)) : List (A × Q) → Except A (List (A × Q))
  | [] => .ok state
  | (asset, amount) :: rest =>
    match ops.add (balance ops.zero state asset) amount with
    | none => .error asset
    | some updated => credit ops (store ops state asset updated) rest

def debit {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state : List (A × Q)) : List (A × Q) → List (A × Q)
  | [] => state
  | (asset, amount) :: rest =>
    debit ops (store ops state asset (ops.sub (balance ops.zero state asset) amount)) rest

def shortfall {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state : List (A × Q)) (required : List (A × Q)) : List (A × Q) :=
  required.filterMap fun (asset, amount) =>
    let available := balance ops.zero state asset
    if ops.lt available amount then some (asset, ops.sub amount available) else none

inductive UpdateError (A Q : Type) where
  | overflow : A → UpdateError A Q
  | insufficient : List (A × Q) → UpdateError A Q
  deriving Repr, DecidableEq

def deposit {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state amounts : List (A × Q)) : Except (UpdateError A Q) Unit × List (A × Q) :=
  match credit ops state amounts with
  | .ok updated => (.ok (), updated)
  | .error asset => (.error (.overflow asset), state)

def withdraw {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state amounts : List (A × Q)) : Except (UpdateError A Q) Unit × List (A × Q) :=
  let missing := shortfall ops state amounts
  if missing.isEmpty then (.ok (), debit ops state amounts)
  else (.error (.insufficient missing), state)

-- Preparing returns a new account or an error; it never publishes an
-- intermediate withdrawal. Consumption is checked before production.
def prepare {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state consumed produced : List (A × Q)) : Except (UpdateError A Q) (List (A × Q)) :=
  match withdraw ops state consumed with
  | (.error error, _) => .error error
  | (.ok (), debited) =>
    match deposit ops debited produced with
    | (.error error, _) => .error error
    | (.ok (), updated) => .ok updated

def preparedOutcome {A Q : Type} (state : List (A × Q))
    (result : Except (UpdateError A Q) (List (A × Q))) :
    Except (UpdateError A Q) Unit × List (A × Q) :=
  match result with
  | .error error => (.error error, state)
  | .ok updated => (.ok (), updated)

theorem prepare_insufficient {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state consumed produced : List (A × Q))
    (missing : shortfall ops state consumed ≠ []) :
    prepare ops state consumed produced = .error (.insufficient (shortfall ops state consumed)) := by
  simp [prepare, withdraw, List.isEmpty_iff, missing]

theorem prepare_success_iff {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state consumed produced updated : List (A × Q)) :
    prepare ops state consumed produced = .ok updated ↔
      shortfall ops state consumed = [] ∧ credit ops (debit ops state consumed) produced = .ok updated := by
  by_cases empty : shortfall ops state consumed = []
  · simp only [prepare, withdraw, empty, List.isEmpty_nil, ↓reduceIte, deposit]
    cases credit ops (debit ops state consumed) produced <;> simp
  · simp [prepare_insufficient ops state consumed produced empty, empty]

theorem prepare_late_overflow {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state consumed produced : List (A × Q)) (asset : A)
    (enough : shortfall ops state consumed = [])
    (overflow : credit ops (debit ops state consumed) produced = .error asset) :
    prepare ops state consumed produced = .error (.overflow asset) := by
  simp [prepare, withdraw, enough, deposit, overflow]

theorem prepared_rejection_atomic {A Q : Type} (state : List (A × Q))
    (result : Except (UpdateError A Q) (List (A × Q))) (error : UpdateError A Q)
    (rejected : (preparedOutcome state result).1 = .error error) :
    (preparedOutcome state result).2 = state := by
  cases result <;> simp_all [preparedOutcome]

theorem deposit_rejection_atomic {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state amounts : List (A × Q)) (error : UpdateError A Q)
    (rejected : (deposit ops state amounts).1 = .error error) :
    (deposit ops state amounts).2 = state := by
  unfold deposit at *
  split at * <;> simp_all

theorem withdrawal_rejection_atomic {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state amounts : List (A × Q)) (error : UpdateError A Q)
    (rejected : (withdraw ops state amounts).1 = .error error) :
    (withdraw ops state amounts).2 = state := by
  simp only [withdraw] at *
  split at * <;> simp_all

theorem shortfall_empty_iff {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state required : List (A × Q)) :
    shortfall ops state required = [] ↔
      ∀ entry ∈ required, ops.lt (balance ops.zero state entry.1) entry.2 = false := by
  induction required with
  | nil => simp [shortfall]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    cases h : ops.lt (balance ops.zero state asset) amount <;> simp_all [shortfall]

theorem shortfall_exact {A : Type} [DecidableEq A] (limit : Nat)
    (state required : List (A × Nat)) (entry : A × Nat) :
    entry ∈ shortfall (naturals limit) state required ↔
      ∃ amount, (entry.1, amount) ∈ required ∧
        balance 0 state entry.1 < amount ∧ entry.2 = amount - balance 0 state entry.1 := by
  rcases entry with ⟨key, value⟩
  simp only [shortfall, List.mem_filterMap]
  constructor
  · rintro ⟨⟨asset, amount⟩, member, result⟩
    simp only [naturals, decide_eq_true_eq] at result
    split at result
    · simp only [Option.some.injEq, Prod.mk.injEq] at result
      exact ⟨amount, by simpa [← result.1] using member, by simpa [← result.1] using ‹_ < _›,
        by simpa [← result.1] using result.2.symm⟩
    · contradiction
  · rintro ⟨amount, member, less, value⟩
    exact ⟨(key, amount), member, by simp [naturals, less, value]⟩

theorem find_absent {A Q : Type} [DecidableEq A] (asset : A) (entries : List (A × Q))
    (absent : asset ∉ entries.map Prod.fst) : find? asset entries = none := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    simp only [List.map_cons, List.mem_cons, not_or] at absent
    simp [find?, absent.1, ih absent.2]

theorem upsert_fresh {A Q : Type} [DecidableEq A] (asset : A) (value : Q)
    (entries : List (A × Q)) (absent : asset ∉ entries.map Prod.fst) :
    upsert asset value entries = entries ++ [(asset, value)] := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    simp only [List.map_cons, List.mem_cons, not_or] at absent
    simp [upsert, absent.1, ih absent.2]

theorem find_upsert_other {A Q : Type} [DecidableEq A] (asset other : A) (value : Q)
    (entries : List (A × Q)) (different : other ≠ asset) :
    find? other (upsert asset value entries) = find? other entries := by
  induction entries with
  | nil => simp [upsert, find?, different]
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases equal : asset = key
    · subst key; simp [upsert, find?, different]
    · simp [upsert, find?, equal, ih]

theorem find_erase_other {A Q : Type} [DecidableEq A] (asset other : A)
    (entries : List (A × Q)) (different : other ≠ asset) :
    find? other (erase asset entries) = find? other entries := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases equal : asset = key
    · subst key; simp [erase, find?, different]
    · simp [erase, find?, equal, ih]

theorem store_frame {A Q : Type} [DecidableEq A] (ops : Arithmetic Q)
    (state : List (A × Q)) (asset other : A) (value : Q) (different : other ≠ asset) :
    balance ops.zero (store ops state asset value) other = balance ops.zero state other := by
  unfold store balance
  split <;> simp [find_upsert_other _ _ _ _ different, find_erase_other _ _ _ different]

end Axionomy.Account
