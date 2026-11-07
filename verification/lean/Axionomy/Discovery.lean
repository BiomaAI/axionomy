import Axionomy.Evaluation
import Axionomy.RateConstruction

namespace Axionomy.Discovery

variable {Role A Q : Type}

def roles : Evaluation.Expr Role A Q → List Role
  | .balance role _ => [role]
  | .binary _ left right => roles left ++ roles right
  | .mulDiv _ left right denominator => roles left ++ roles right ++ roles denominator
  | _ => []

def assets : Evaluation.Expr Role A Q → List A
  | .balance _ asset => [asset]
  | .binary _ left right => assets left ++ assets right
  | .mulDiv _ left right denominator => assets left ++ assets right ++ assets denominator
  | _ => []

def parameters : Evaluation.Expr Role A Q → List String
  | .parameter parameter => [parameter]
  | .binary _ left right => parameters left ++ parameters right
  | .mulDiv _ left right denominator => parameters left ++ parameters right ++ parameters denominator
  | _ => []

def registerAll {K : Type} (cmp : K → K → Ordering) (entries accumulated : List K) : List K :=
  entries.foldl (fun state key => RateConstruction.register cmp key state) accumulated

theorem registerAll_append {K : Type} (cmp : K → K → Ordering) (left right accumulated : List K) :
    registerAll cmp (left ++ right) accumulated = registerAll cmp right (registerAll cmp left accumulated) := by
  simp [registerAll, List.foldl_append]

theorem register_mem {K : Type} (cmp : K → K → Ordering)
    (identity : ∀ x y, cmp x y = .eq → x = y) (key query : K) (entries : List K) :
    query ∈ RateConstruction.register cmp key entries ↔ query = key ∨ query ∈ entries := by
  induction entries with
  | nil => simp [RateConstruction.register]
  | cons first rest ih =>
    cases h : cmp key first with
    | lt => simp [RateConstruction.register, h]
    | eq =>
      have same := identity key first h
      subst key
      simp [RateConstruction.register, h]
    | gt => simp [RateConstruction.register, h, ih, or_left_comm]

theorem registerAll_mem {K : Type} (cmp : K → K → Ordering)
    (identity : ∀ x y, cmp x y = .eq → x = y) (query : K) (entries accumulated : List K) :
    query ∈ registerAll cmp entries accumulated ↔ query ∈ entries ∨ query ∈ accumulated := by
  induction entries generalizing accumulated with
  | nil => simp [registerAll]
  | cons key rest ih =>
    simp only [registerAll, List.foldl_cons]
    rw [show rest.foldl (fun state key => RateConstruction.register cmp key state)
      (RateConstruction.register cmp key accumulated) =
      registerAll cmp rest (RateConstruction.register cmp key accumulated) from rfl,
      ih, register_mem cmp identity]
    simp [or_left_comm, or_assoc]

def expressionRoles (cmp : Role → Role → Ordering) (target : Role)
    (expression : Evaluation.Expr Role A Q) (accumulated : List Role) : List Role :=
  registerAll cmp (registerAll cmp (roles expression) []) (RateConstruction.register cmp target accumulated)

def conditionRoles (cmp : Role → Role → Ordering) (left right : Evaluation.Expr Role A Q)
    (accumulated : List Role) : List Role :=
  registerAll cmp (registerAll cmp (roles left ++ roles right) []) accumulated

theorem expressionRoles_complete (cmp : Role → Role → Ordering)
    (identity : ∀ x y, cmp x y = .eq → x = y) (target query : Role)
    (expression : Evaluation.Expr Role A Q) (accumulated : List Role) :
    query ∈ expressionRoles cmp target expression accumulated ↔
      query = target ∨ query ∈ roles expression ∨ query ∈ accumulated := by
  simp [expressionRoles, registerAll_mem cmp identity, register_mem cmp identity, or_left_comm]

theorem conditionRoles_complete (cmp : Role → Role → Ordering)
    (identity : ∀ x y, cmp x y = .eq → x = y) (query : Role)
    (left right : Evaluation.Expr Role A Q) (accumulated : List Role) :
    query ∈ conditionRoles cmp left right accumulated ↔
      query ∈ roles left ∨ query ∈ roles right ∨ query ∈ accumulated := by
  simp [conditionRoles, registerAll_mem cmp identity, or_assoc]

def pairCompare (cmp : Role → Role → Ordering) (left right : Role × Role) : Ordering :=
  match cmp left.1 right.1 with
  | .eq => cmp left.2 right.2
  | order => order

def orderedPair (cmp : Role → Role → Ordering) (left right : Role) : Role × Role :=
  if cmp left right = .gt then (right, left) else (left, right)

theorem register_length_le {K : Type} (cmp : K → K → Ordering) (key : K) (entries : List K) :
    (RateConstruction.register cmp key entries).length ≤ entries.length + 1 := by
  induction entries with
  | nil => simp [RateConstruction.register]
  | cons first rest ih =>
    cases order : cmp key first <;> simp [RateConstruction.register, order] <;> omega

theorem registerAll_length_le {K : Type} (cmp : K → K → Ordering) (entries accumulated : List K) :
    (registerAll cmp entries accumulated).length ≤ accumulated.length + entries.length := by
  induction entries generalizing accumulated with
  | nil => simp [registerAll]
  | cons key rest ih =>
    have bound := ih (RateConstruction.register cmp key accumulated)
    have step := register_length_le cmp key accumulated
    simp only [registerAll, List.foldl_cons, List.length_cons] at *
    omega

end Axionomy.Discovery
