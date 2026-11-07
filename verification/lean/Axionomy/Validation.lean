import Axionomy.Discovery

namespace Axionomy.Validation

inductive Issue (AccountId RateId Role : Type) where
  | zeroUnits
  | missingRate (rate : RateId)
  | missingBinding (role : Role)
  | unknownBinding (role : Role)
  | missingParameter (parameter : String)
  | unknownParameter (parameter : String)
  | rolesMustDiffer (left right : Role)
  | missingAccount (account : AccountId)
  deriving DecidableEq, Repr

structure Metadata (Role : Type) where
  roles : List Role
  parameters : List String
  distinct : List (Role × Role)

variable {AccountId RateId Role Q Rule : Type}

def missingBinding (binding : Role → Option AccountId) (role : Role) : Option (Issue AccountId RateId Role) :=
  if (binding role).isSome then none else some (.missingBinding role)

def unknownBinding [DecidableEq Role] (roles : List Role) (role : Role) : Option (Issue AccountId RateId Role) :=
  if role ∈ roles then none else some (.unknownBinding role)

def missingParameter (parameters : List (String × Q)) (parameter : String) : Option (Issue AccountId RateId Role) :=
  if (Evaluation.orderedFind compare parameter parameters).isSome then none else some (.missingParameter parameter)

def containsName (names : List String) (parameter : String) : Bool :=
  (Evaluation.orderedFind compare parameter (names.map fun key => (key, ()))).isSome

def unknownParameter (names : List String) (parameter : String) : Option (Issue AccountId RateId Role) :=
  if containsName names parameter then none else some (.unknownParameter parameter)

def distinctIssue [DecidableEq AccountId] (binding : Role → Option AccountId) (pair : Role × Role) :
    Option (Issue AccountId RateId Role) :=
  match binding pair.1, binding pair.2 with
  | some left, some right => if left = right then some (.rolesMustDiffer pair.1 pair.2) else none
  | _, _ => none

def missingAccount (present : AccountId → Bool) (binding : Role → Option AccountId) (role : Role) : Option AccountId :=
  match binding role with
  | none => none
  | some account => if present account then none else some account

def missingAccounts (cmp : AccountId → AccountId → Ordering) (present : AccountId → Bool)
    (binding : Role → Option AccountId) (roles : List Role) : List AccountId :=
  Discovery.registerAll cmp (roles.filterMap (missingAccount present binding)) []

def zeroIssues (units : Nat) : List (Issue AccountId RateId Role) :=
  if units = 0 then [.zeroUnits] else []

def issues [DecidableEq AccountId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (units : Nat) (metadata : Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool) : List (Issue AccountId RateId Role) :=
  let binding := fun role => Evaluation.orderedFind cmpRole role bindings
  zeroIssues units ++ metadata.roles.filterMap (missingBinding binding) ++
    (bindings.map Prod.fst).filterMap (unknownBinding metadata.roles) ++
    metadata.parameters.filterMap (missingParameter parameters) ++
    (parameters.map Prod.fst).filterMap (unknownParameter metadata.parameters) ++
    metadata.distinct.filterMap (distinctIssue binding) ++
    (missingAccounts cmpAccount present binding metadata.roles).map Issue.missingAccount

def validate [DecidableEq AccountId] [DecidableEq Role] [DecidableEq RateId]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (describe : Rule → Metadata Role) (units : Nat) (rate : RateId) (rules : List (RateId × Rule))
    (bindings : List (Role × AccountId)) (parameters : List (String × Q)) (present : AccountId → Bool) :
    Except (List (Issue AccountId RateId Role)) Rule :=
  match Account.find? rate rules with
  | none => .error (zeroIssues units ++ [.missingRate rate])
  | some rule =>
    let failures := issues cmpAccount cmpRole units (describe rule) bindings parameters present
    if failures.isEmpty then .ok rule else .error failures

theorem missing_binding_none (binding : Role → Option AccountId) (role : Role) :
    (missingBinding (RateId := RateId) binding role) = none ↔ ∃ account, binding role = some account := by
  cases found : binding role <;> simp [missingBinding, found]

theorem unknown_binding_none [DecidableEq Role] (roles : List Role) (role : Role) :
    (unknownBinding (AccountId := AccountId) (RateId := RateId) roles role) = none ↔ role ∈ roles := by
  simp [unknownBinding]

theorem missing_account_none (present : AccountId → Bool) (binding : Role → Option AccountId) (role : Role) :
    missingAccount present binding role = none ↔ ∀ account, binding role = some account → present account = true := by
  cases found : binding role with
  | none => simp [missingAccount, found]
  | some account => simp [missingAccount, found]

theorem valid_units [DecidableEq AccountId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (units : Nat) (metadata : Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool)
    (clean : issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present = []) :
    units ≠ 0 := by
  intro zero
  simp [issues, zeroIssues, zero] at clean

theorem accepted_rule [DecidableEq AccountId] [DecidableEq Role] [DecidableEq RateId]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (describe : Rule → Metadata Role) (units : Nat) (rate : RateId) (rules : List (RateId × Rule))
    (bindings : List (Role × AccountId)) (parameters : List (String × Q)) (present : AccountId → Bool)
    (selected : Rule)
    (accepted : validate cmpAccount cmpRole describe units rate rules bindings parameters present = .ok selected) :
    Account.find? rate rules = some selected ∧
      issues (RateId := RateId) cmpAccount cmpRole units (describe selected) bindings parameters present = [] := by
  unfold validate at accepted
  cases found : Account.find? rate rules with
  | none => simp [found] at accepted
  | some rule =>
    by_cases clean : issues (RateId := RateId) cmpAccount cmpRole units (describe rule) bindings parameters present = []
    · have same : rule = selected := by simpa [found, clean] using accepted
      subst selected
      exact ⟨rfl, clean⟩
    · simp [found, clean] at accepted

theorem required_binding [DecidableEq AccountId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (units : Nat) (metadata : Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool)
    (clean : issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present = [])
    (role : Role) (member : role ∈ metadata.roles) :
    ∃ account, Evaluation.orderedFind cmpRole role bindings = some account := by
  cases found : Evaluation.orderedFind cmpRole role bindings with
  | some account => exact ⟨account, rfl⟩
  | none =>
    have reported : Issue.missingBinding (AccountId := AccountId) (RateId := RateId) role ∈
        metadata.roles.filterMap (missingBinding (fun key => Evaluation.orderedFind cmpRole key bindings)) :=
      List.mem_filterMap.mpr ⟨role, member, by simp [missingBinding, found]⟩
    have bad : Issue.missingBinding role ∈ issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present := by
      simp only [issues, List.mem_append]
      simp [reported]
    simp [clean] at bad

theorem required_account [DecidableEq AccountId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (identity : ∀ x y, cmpAccount x y = .eq → x = y)
    (units : Nat) (metadata : Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool)
    (clean : issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present = [])
    (role : Role) (member : role ∈ metadata.roles) (account : AccountId)
    (bound : Evaluation.orderedFind cmpRole role bindings = some account) : present account = true := by
  by_cases foundAccount : present account = true
  · exact foundAccount
  have missing : missingAccount present (fun key => Evaluation.orderedFind cmpRole key bindings) role = some account := by
    simp [missingAccount, bound, foundAccount]
  have reported : account ∈ missingAccounts cmpAccount present
      (fun key => Evaluation.orderedFind cmpRole key bindings) metadata.roles := by
    rw [missingAccounts, Discovery.registerAll_mem cmpAccount identity]
    exact Or.inl (List.mem_filterMap.mpr ⟨role, member, missing⟩)
  have bad : Issue.missingAccount account ∈ issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present := by
    simp only [issues, List.mem_append]
    exact Or.inr (List.mem_map.mpr ⟨account, reported, rfl⟩)
  simp [clean] at bad

theorem rejected_nonempty [DecidableEq AccountId] [DecidableEq Role] [DecidableEq RateId]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (describe : Rule → Metadata Role) (units : Nat) (rate : RateId) (rules : List (RateId × Rule))
    (bindings : List (Role × AccountId)) (parameters : List (String × Q)) (present : AccountId → Bool)
    (failures : List (Issue AccountId RateId Role))
    (rejected : validate cmpAccount cmpRole describe units rate rules bindings parameters present = .error failures) :
    failures ≠ [] := by
  intro empty
  subst failures
  unfold validate at rejected
  cases found : Account.find? rate rules with
  | none => simp [found] at rejected
  | some rule =>
    by_cases clean : issues (RateId := RateId) cmpAccount cmpRole units (describe rule) bindings parameters present = []
    · simp [found, clean] at rejected
    · simp [found, clean] at rejected

theorem required_parameter [DecidableEq AccountId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (units : Nat) (metadata : Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool)
    (clean : issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present = [])
    (parameter : String) (member : parameter ∈ metadata.parameters) :
    ∃ value, Evaluation.orderedFind compare parameter parameters = some value := by
  cases found : Evaluation.orderedFind compare parameter parameters with
  | some value => exact ⟨value, rfl⟩
  | none =>
    have reported : Issue.missingParameter (AccountId := AccountId) (RateId := RateId) (Role := Role) parameter ∈
        metadata.parameters.filterMap (missingParameter parameters) :=
      List.mem_filterMap.mpr ⟨parameter, member, by simp [missingParameter, found]⟩
    have bad : Issue.missingParameter parameter ∈ issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present := by
      simp only [issues, List.mem_append]
      simp [reported]
    simp [clean] at bad

theorem bound_accounts_differ [DecidableEq AccountId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (units : Nat) (metadata : Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool)
    (clean : issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present = [])
    (left right : Role) (member : (left, right) ∈ metadata.distinct) (first second : AccountId)
    (boundLeft : Evaluation.orderedFind cmpRole left bindings = some first)
    (boundRight : Evaluation.orderedFind cmpRole right bindings = some second) : first ≠ second := by
  intro same
  have reported : Issue.rolesMustDiffer (AccountId := AccountId) (RateId := RateId) left right ∈
      metadata.distinct.filterMap (distinctIssue (fun key => Evaluation.orderedFind cmpRole key bindings)) :=
    List.mem_filterMap.mpr ⟨(left, right), member, by simp [distinctIssue, boundLeft, boundRight, same]⟩
  have bad : Issue.rolesMustDiffer left right ∈ issues (RateId := RateId) cmpAccount cmpRole units metadata bindings parameters present := by
    simp only [issues, List.mem_append]
    simp [reported]
  simp [clean] at bad

end Axionomy.Validation
