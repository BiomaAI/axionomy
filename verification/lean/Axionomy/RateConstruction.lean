import Axionomy.BasketOperations

namespace Axionomy.RateConstruction

-- A sorted-map position records both the retained key and the unchanged frame.
structure Position (K V : Type) where
  before : List (K × V)
  key : K
  old : Option V
  after : List (K × V)

def Position.restore {K V : Type} (position : Position K V) : List (K × V) :=
  position.before ++ (position.old.toList.map (position.key, ·)) ++ position.after

def Position.put {K V : Type} (position : Position K V) (value : V) : List (K × V) :=
  position.before ++ (position.key, value) :: position.after

def locate {K V : Type} (cmp : K → K → Ordering) (query : K) : List (K × V) → Position K V
  | [] => ⟨[], query, none, []⟩
  | (key, value) :: rest =>
    match cmp query key with
    | .lt => ⟨[], query, none, (key, value) :: rest⟩
    | .eq => ⟨[], key, some value, rest⟩
    | .gt => let position := locate cmp query rest
             { position with before := (key, value) :: position.before }

theorem locate_restore {K V : Type} (cmp : K → K → Ordering) (query : K) (entries : List (K × V)) :
    (locate cmp query entries).restore = entries := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases h : cmp query key with
    | lt => simp [locate, h, Position.restore]
    | eq => simp [locate, h, Position.restore]
    | gt =>
      simp only [locate, h]
      change (key, value) :: (locate cmp query rest).restore = (key, value) :: rest
      rw [ih]

theorem locate_key {K V : Type} (cmp : K → K → Ordering)
    (eqLaw : ∀ x y, cmp x y = .eq → x = y) (query : K) (entries : List (K × V)) :
    (locate cmp query entries).key = query := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases h : cmp query key with
    | lt => simp [locate, h]
    | eq => simp only [locate, h]; exact (eqLaw query key h).symm
    | gt => simp [locate, h, ih]

def register {K : Type} (cmp : K → K → Ordering) (query : K) : List K → List K
  | [] => [query]
  | key :: rest => match cmp query key with
    | .lt => query :: key :: rest
    | .eq => key :: rest
    | .gt => key :: register cmp query rest

theorem registered {K : Type} (cmp : K → K → Ordering)
    (eqLaw : ∀ x y, cmp x y = .eq → x = y) (query : K) (roles : List K) :
    query ∈ register cmp query roles := by
  induction roles with
  | nil => simp [register]
  | cons key rest ih =>
    cases h : cmp query key with
    | lt => simp [register, h]
    | eq => simp only [register, h]; exact List.mem_cons.mpr (Or.inl (eqLaw query key h))
    | gt => simp [register, h, ih]

def merge {Role A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (cmp : Role → Role → Ordering) (target : List (Role × List (A × Q)))
    (role : Role) (assets : List (A × Q)) : Except A (List (Role × List (A × Q))) :=
  let position := locate cmp role target
  match Account.credit ops (position.old.getD []) assets with
  | .error asset => .error asset
  | .ok merged => .ok (position.put merged)

theorem merge_success {Role A Q : Type} [DecidableEq A] (ops : Account.Arithmetic Q)
    (cmp : Role → Role → Ordering) (target : List (Role × List (A × Q)))
    (role : Role) (assets : List (A × Q)) (result : List (Role × List (A × Q))) :
    merge ops cmp target role assets = .ok result ↔
      ∃ merged, Account.credit ops ((locate cmp role target).old.getD []) assets = .ok merged ∧
        result = (locate cmp role target).put merged := by
  unfold merge
  cases h : Account.credit ops ((locate cmp role target).old.getD []) assets <;> simp [h, eq_comm]

end Axionomy.RateConstruction
