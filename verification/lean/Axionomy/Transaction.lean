import Axionomy.Validation
import Axionomy.Effects
import Axionomy.Conservation

namespace Axionomy.Transaction

abbrev Balances (A : Type) := List (A × Nat)

structure Proposal (AccountId RateId Role : Type) where
  rate : RateId
  bindings : List (Role × AccountId)
  units : Nat
  parameters : List (String × Nat)

structure Computed (Role A : Type) where
  role : Role
  asset : A
  quantity : Evaluation.Expr Role A Nat

structure Condition (Role A : Type) where
  label : String
  left : Evaluation.Expr Role A Nat
  comparison : Evaluation.Comparison
  right : Evaluation.Expr Role A Nat

structure Rule (Role A : Type) where
  roles : List Role
  distinct : List (Role × Role)
  consume : List (Role × Balances A)
  produce : List (Role × Balances A)
  preserve : List (Role × Balances A)
  computedConsume : List (Computed Role A)
  computedProduce : List (Computed Role A)
  computedPreserve : List (Computed Role A)
  conditions : List (Condition Role A)

def Rule.parameters {Role A : Type} (rule : Rule Role A) : List String :=
  Discovery.registerAll compare
    ((rule.computedConsume ++ rule.computedProduce ++ rule.computedPreserve).flatMap
      (fun amount => Discovery.parameters amount.quantity) ++ rule.conditions.flatMap
      (fun condition => Discovery.parameters condition.left ++ Discovery.parameters condition.right)) []

def Rule.metadata {Role A : Type} (rule : Rule Role A) : Validation.Metadata Role :=
  ⟨rule.roles, rule.parameters, rule.distinct⟩

structure Invariant (A : Type) where
  label : String
  weights : List (A × Int)

structure Model (AccountId A RateId Role : Type) where
  accounts : List (AccountId × Balances A)
  rates : List (RateId × Rule Role A)
  invariants : List (Invariant A)

structure Effect (A : Type) where
  consume : Balances A := []
  produce : Balances A := []
  preserve : Balances A := []

inductive Direction where
  | consume | produce | preserve
  deriving DecidableEq

structure AccountAssessment (AccountId A : Type) where
  account : AccountId
  available : Balances A
  required : Balances A
  consumed : Balances A
  produced : Balances A
  preserved : Balances A

abbrev Shortfalls (AccountId A : Type) := List (AccountId × Balances A)
abbrev Delta (AccountId A : Type) := Conservation.Delta AccountId A Nat

inductive Failure (AccountId A RateId Role : Type) where
  | structural (issue : Validation.Issue AccountId RateId Role)
  | rateOverflow (rate : RateId) (asset : A)
  | lawEvaluation (rate : RateId) (message : String)
  | conditionFailed (rate : RateId) (label : String)
  | infeasible (shortfalls : Shortfalls AccountId A)
  | balanceOverflow (account : AccountId) (asset : A)
  | invariantOverflow (label : String)
  | invariantViolation (label : String) (before after : Int)

-- A violated internal lookup precondition is distinguished from a handled
-- application error. Implementation proofs must rule it out for valid models.
inductive Fault (E : Type) where
  | panic
  | issue (error : E)

abbrev Checked (AccountId A RateId Role T : Type) := Except (Fault (Failure AccountId A RateId Role)) T

@[simp] theorem checked_bind_ok {AccountId A RateId Role T U : Type} (value : T)
    (next : T → Checked AccountId A RateId Role U) :
    ((.ok value : Checked AccountId A RateId Role T) >>= next) = next value := rfl

@[simp] theorem checked_bind_error {AccountId A RateId Role T U : Type} (error : Fault (Failure AccountId A RateId Role))
    (next : T → Checked AccountId A RateId Role U) :
    ((.error error : Checked AccountId A RateId Role T) >>= next) = .error error := rfl

structure Availability (AccountId A : Type) where
  accounts : List (AccountAssessment AccountId A)
  shortfalls : Shortfalls AccountId A

structure Preparation (AccountId A : Type) where
  accounts : List (AccountId × Balances A)
  deltas : List (Delta AccountId A)

inductive Assessment (AccountId A RateId Role : Type) where
  | applicable (accounts : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
  | infeasible (accounts : List (AccountAssessment AccountId A)) (shortfalls : Shortfalls AccountId A)
  | invalid (issues : List (Failure AccountId A RateId Role))

structure Analysis (AccountId A RateId Role : Type) where
  assessment : Assessment AccountId A RateId Role
  prepared : Option (List (AccountId × Balances A))

structure Receipt (AccountId A RateId Role : Type) where
  proposal : Proposal AccountId RateId Role
  deltas : List (Delta AccountId A)

variable {AccountId A RateId Role : Type}

def invalid (issues : List (Failure AccountId A RateId Role)) : Analysis AccountId A RateId Role :=
  ⟨.invalid issues, none⟩

def conditionResult (rate : RateId) (context : Evaluation.Context Role A Nat) (condition : Condition Role A) :
    Except (Failure AccountId A RateId Role) Unit :=
  match Evaluation.condition (Evaluation.naturals Quantity.u64Max) id context
      condition.comparison condition.left condition.right with
  | .error error => .error (.lawEvaluation rate error.message)
  | .ok false => .error (.conditionFailed rate condition.label)
  | .ok true => .ok ()

def checkConditions (rate : RateId) (context : Evaluation.Context Role A Nat) :
    List (Condition Role A) → Except (Failure AccountId A RateId Role) Unit
  | [] => .ok ()
  | condition :: rest => match conditionResult rate context condition with
    | .error error => .error error
    | .ok () => checkConditions rate context rest

def liftIssue {T : Type} (result : Except (Failure AccountId A RateId Role) T) : Checked AccountId A RateId Role T :=
  match result with | .ok value => .ok value | .error error => .error (.issue error)

def require {T : Type} (value : Option T) : Checked AccountId A RateId Role T :=
  match value with | some value => .ok value | none => .error .panic

def rateResult {T : Type} (rate : RateId) (value : Except A T) : Checked AccountId A RateId Role T :=
  match value with | .ok result => .ok result | .error asset => .error (.issue (.rateOverflow rate asset))

def staticScaled [DecidableEq A] (rate : RateId) (units : Nat) (target : Balances A) :
    Option (Balances A) → Checked AccountId A RateId Role (Balances A)
  | none => .ok target
  | some source => rateResult rate (Effects.scaled (Account.naturals Quantity.u64Max) (Quantity.mul Quantity.u64Max) target source units)

def staticPreserved [DecidableEq A] (rate : RateId) (target : Balances A) :
    Option (Balances A) → Checked AccountId A RateId Role (Balances A)
  | none => .ok target
  | some source => rateResult rate (Account.credit (Account.naturals Quantity.u64Max) target source)

def mergeStatic [DecidableEq A] (cmpRole : Role → Role → Ordering) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (role : Role) (effect : Effect A) : Checked AccountId A RateId Role (Effect A) := do
  let consumed ← staticScaled proposal.rate proposal.units effect.consume (Evaluation.orderedFind cmpRole role rule.consume)
  let produced ← staticScaled proposal.rate proposal.units effect.produce (Evaluation.orderedFind cmpRole role rule.produce)
  let preserved ← staticPreserved proposal.rate effect.preserve (Evaluation.orderedFind cmpRole role rule.preserve)
  return ⟨consumed, produced, preserved⟩

def staticEffects [DecidableEq A] (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) :
    List Role → List (AccountId × Effect A) → Checked AccountId A RateId Role (List (AccountId × Effect A))
  | [], accumulated => .ok accumulated
  | role :: rest, accumulated => do
    let account ← require (Evaluation.orderedFind cmpRole role proposal.bindings)
    let position := RateConstruction.locate cmpAccount account accumulated
    let updated ← mergeStatic cmpRole proposal rule role (position.old.getD {})
    staticEffects cmpAccount cmpRole proposal rule rest (position.put updated)

def mergeComputed [DecidableEq A] (direction : Direction) (asset : A) (value : Nat) (effect : Effect A) : Except A (Effect A) :=
  let merge := fun target => Account.credit (Account.naturals Quantity.u64Max) target [(asset, value)]
  match direction with
  | .consume => (merge effect.consume).map fun result => { effect with consume := result }
  | .produce => (merge effect.produce).map fun result => { effect with produce := result }
  | .preserve => (merge effect.preserve).map fun result => { effect with preserve := result }

def computedEffects [DecidableEq A] (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (proposal : Proposal AccountId RateId Role) (context : Evaluation.Context Role A Nat) (direction : Direction) :
    List (Computed Role A) → List (AccountId × Effect A) → Checked AccountId A RateId Role (List (AccountId × Effect A))
  | [], accumulated => .ok accumulated
  | amount :: rest, accumulated => do
    let value ← match Evaluation.eval (Evaluation.naturals Quantity.u64Max) context amount.quantity with
      | .ok value => .ok value
      | .error error => .error (.issue (.lawEvaluation proposal.rate error.message))
    let account ← require (Evaluation.orderedFind cmpRole amount.role proposal.bindings)
    let position := RateConstruction.locate cmpAccount account accumulated
    let updated ← rateResult proposal.rate (mergeComputed direction amount.asset value (position.old.getD {}))
    computedEffects cmpAccount cmpRole proposal context direction rest (position.put updated)

def computedChannels [DecidableEq A] (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (context : Evaluation.Context Role A Nat)
    (effects : List (AccountId × Effect A)) :
    Checked AccountId A RateId Role (List (AccountId × Effect A)) := do
  let effects ← computedEffects cmpAccount cmpRole proposal context .consume rule.computedConsume effects
  let effects ← computedEffects cmpAccount cmpRole proposal context .produce rule.computedProduce effects
  computedEffects cmpAccount cmpRole proposal context .preserve rule.computedPreserve effects

def aggregate [DecidableEq A] (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (context : Evaluation.Context Role A Nat) :
    Checked AccountId A RateId Role (List (AccountId × Effect A)) := do
  let effects ← staticEffects cmpAccount cmpRole proposal rule rule.roles []
  computedChannels cmpAccount cmpRole proposal rule context effects

def relevant [DecidableEq A] (balances required : Balances A) : Balances A :=
  required.foldl (fun result (asset, _) =>
    Account.store (Account.naturals Quantity.u64Max) result asset (Account.balance 0 balances asset)) []

def preflight [DecidableEq AccountId] [DecidableEq A] (rate : RateId) (accounts : List (AccountId × Balances A)) :
    List (AccountId × Effect A) → Availability AccountId A → Checked AccountId A RateId Role (Availability AccountId A)
  | [], accumulated => .ok accumulated
  | (id, effect) :: rest, accumulated => do
    let balances ← require (Account.find? id accounts)
    let required ← rateResult rate (Account.credit (Account.naturals Quantity.u64Max) effect.consume effect.preserve)
    let missing := Account.shortfall (Account.naturals Quantity.u64Max) balances required
    let assessed := AccountAssessment.mk id (relevant balances required) required effect.consume effect.produce effect.preserve
    preflight rate accounts rest ⟨accumulated.accounts ++ [assessed],
      accumulated.shortfalls ++ if missing.isEmpty then [] else [(id, missing)]⟩

def accountFailure (id : AccountId) : Account.UpdateError A Nat → Failure AccountId A RateId Role
  | .overflow asset => .balanceOverflow id asset
  | .insufficient missing => .infeasible [(id, missing)]

def prepareOne [DecidableEq AccountId] [DecidableEq A] (accounts : List (AccountId × Balances A))
    (id : AccountId) (effect : Effect A) : Checked AccountId A RateId Role (Balances A) := do
  let balances ← require (Account.find? id accounts)
  liftIssue ((Account.prepare (Account.naturals Quantity.u64Max) balances effect.consume effect.produce).mapError (accountFailure id))

def prepare [DecidableEq AccountId] [DecidableEq A] (accounts : List (AccountId × Balances A)) :
    List (AccountId × Effect A) → Preparation AccountId A → Checked AccountId A RateId Role (Preparation AccountId A)
  | [], accumulated => .ok accumulated
  | (id, effect) :: rest, accumulated => do
    let updated ← prepareOne accounts id effect
    prepare accounts rest ⟨accumulated.accounts ++ [(id, updated)],
      accumulated.deltas ++ [⟨id, effect.consume, effect.produce, effect.preserve⟩]⟩

-- Replace an existing value while retaining its original key and map order.
def replaceExisting {K V : Type} [DecidableEq K] (query : K) (value : V) : List (K × V) → Option (List (K × V))
  | [] => none
  | (key, old) :: rest => if query = key then some ((key, value) :: rest)
      else (replaceExisting query value rest).map ((key, old) :: ·)

def install {K V : Type} [DecidableEq K] : List (K × V) → List (K × V) → Option (List (K × V))
  | accounts, [] => some accounts
  | accounts, (id, balances) :: rest => do
    let updated ← replaceExisting id balances accounts
    install updated rest

def checkInvariants [DecidableEq AccountId] [DecidableEq A] (accounts : List (AccountId × Balances A))
    (prepared : Preparation AccountId A) : List (Invariant A) → Checked AccountId A RateId Role Unit
  | [] => .ok ()
  | invariant :: rest =>
    let weighted := fun asset value => some (Conservation.contribution invariant.weights asset value)
    match Conservation.conserves weighted Conservation.checkedAdd 0 prepared.deltas with
    | none => .error (.issue (.invariantOverflow invariant.label))
    | some true => checkInvariants accounts prepared rest
    | some false => do
      let before ← match Conservation.accumulate weighted Conservation.checkedAdd 0 (accounts.flatMap Prod.snd) with
        | none => .error (.issue (.invariantOverflow invariant.label))
        | some value => .ok value
      let updated ← require (install accounts prepared.accounts)
      let after ← match Conservation.accumulate weighted Conservation.checkedAdd 0 (updated.flatMap Prod.snd) with
        | none => .error (.issue (.invariantOverflow invariant.label))
        | some value => .ok value
      .error (.issue (.invariantViolation invariant.label before after))

def stages [DecidableEq AccountId] [DecidableEq A] (cmpAccount : AccountId → AccountId → Ordering)
    (cmpRole : Role → Role → Ordering) (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) : Checked AccountId A RateId Role (Analysis AccountId A RateId Role) := do
  let context := Evaluation.fromState cmpRole 0 proposal.units proposal.parameters proposal.bindings model.accounts
  liftIssue (checkConditions proposal.rate context rule.conditions)
  let effects ← aggregate cmpAccount cmpRole proposal rule context
  let availability ← preflight proposal.rate model.accounts effects ⟨[], []⟩
  if availability.shortfalls.isEmpty then
    let prepared ← prepare model.accounts effects ⟨[], []⟩
    checkInvariants model.accounts prepared model.invariants
    return ⟨.applicable availability.accounts prepared.deltas, some prepared.accounts⟩
  else return ⟨.infeasible availability.accounts availability.shortfalls, none⟩

def resolveStages (result : Checked AccountId A RateId Role (Analysis AccountId A RateId Role)) :
    Option (Analysis AccountId A RateId Role) :=
  match result with
  | .ok result => some result
  | .error (.issue error) => some (invalid [error])
  | .error .panic => none

def analyze [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role) :
    Option (Analysis AccountId A RateId Role) :=
  match Validation.validate cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
  | .error issues => some (invalid (issues.map Failure.structural))
  | .ok rule => resolveStages (stages cmpAccount cmpRole model proposal rule)

def applyAnalysis [DecidableEq AccountId] (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (analysis : Analysis AccountId A RateId Role) :
    Option (Except (Failure AccountId A RateId Role) (Receipt AccountId A RateId Role) × Model AccountId A RateId Role) :=
  match analysis.assessment with
  | .invalid issues => issues.head?.map fun error => (.error error, model)
  | .infeasible _ missing => some (.error (.infeasible missing), model)
  | .applicable _ deltas => do
    let prepared ← analysis.prepared
    let accounts ← install model.accounts prepared
    some (.ok ⟨proposal, deltas⟩, { model with accounts := accounts })

def apply [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role) :
    Option (Except (Failure AccountId A RateId Role) (Receipt AccountId A RateId Role) × Model AccountId A RateId Role) := do
  let analysis ← analyze cmpAccount cmpRole model proposal
  applyAnalysis model proposal analysis

theorem replaceExisting_keys {K V : Type} [DecidableEq K] (key : K) (value : V) (entries updated : List (K × V))
    (replaced : replaceExisting key value entries = some updated) : updated.map Prod.fst = entries.map Prod.fst := by
  induction entries generalizing updated with
  | nil => simp [replaceExisting] at replaced
  | cons entry rest ih =>
    rcases entry with ⟨stored, old⟩
    by_cases same : key = stored
    · simp [replaceExisting, same] at replaced
      subst updated
      rfl
    · cases remaining : replaceExisting key value rest with
      | none => simp [replaceExisting, same, remaining] at replaced
      | some result =>
        simp [replaceExisting, same, remaining] at replaced
        subst updated
        simp [ih result remaining]

theorem install_keys {K V : Type} [DecidableEq K] (accounts prepared updated : List (K × V))
    (installed : install accounts prepared = some updated) : updated.map Prod.fst = accounts.map Prod.fst := by
  induction prepared generalizing accounts with
  | nil => simp [install] at installed; subst updated; rfl
  | cons entry rest ih =>
    rcases entry with ⟨id, balances⟩
    cases replaced : replaceExisting id balances accounts with
    | none => simp [install, replaced] at installed
    | some middle =>
      have following : install middle rest = some updated := by simpa [install, replaced] using installed
      exact (ih middle following).trans (replaceExisting_keys id balances accounts middle replaced)

theorem replaceExisting_find_other {K V : Type} [DecidableEq K] (changed query : K) (value : V)
    (different : query ≠ changed) (entries updated : List (K × V))
    (replaced : replaceExisting changed value entries = some updated) :
    Account.find? query updated = Account.find? query entries := by
  induction entries generalizing updated with
  | nil => simp [replaceExisting] at replaced
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases same : changed = key
    · simp [replaceExisting, same] at replaced
      subst updated
      have untouched : query ≠ key := same ▸ different
      simp [Account.find?, untouched]
    · cases remaining : replaceExisting changed value rest with
      | none => simp [replaceExisting, same, remaining] at replaced
      | some result =>
        simp [replaceExisting, same, remaining] at replaced
        subst updated
        by_cases hit : query = key <;> simp [Account.find?, hit, ih result remaining]

theorem install_find_other {K V : Type} [DecidableEq K] (query : K) (accounts prepared updated : List (K × V))
    (untouched : query ∉ prepared.map Prod.fst) (installed : install accounts prepared = some updated) :
    Account.find? query updated = Account.find? query accounts := by
  induction prepared generalizing accounts with
  | nil => simp [install] at installed; subst updated; rfl
  | cons entry rest ih =>
    rcases entry with ⟨id, value⟩
    have separate : query ≠ id ∧ query ∉ rest.map Prod.fst := by simpa using untouched
    cases replaced : replaceExisting id value accounts with
    | none => simp [install, replaced] at installed
    | some middle =>
      have following : install middle rest = some updated := by simpa [install, replaced] using installed
      exact (ih middle separate.2 following).trans
        (replaceExisting_find_other id query value separate.1 accounts middle replaced)

theorem find_exists_iff_key_mem {K V : Type} [DecidableEq K] (query : K) (entries : List (K × V)) :
    (∃ value, Account.find? query entries = some value) ↔ query ∈ entries.map Prod.fst := by
  induction entries with
  | nil => simp [Account.find?]
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases same : query = key
    · simp [Account.find?, same]
    · simp [Account.find?, same, ih]

theorem replaceExisting_exists {K V : Type} [DecidableEq K] (query : K) (value : V) (entries : List (K × V))
    (present : query ∈ entries.map Prod.fst) : ∃ updated, replaceExisting query value entries = some updated := by
  induction entries with
  | nil => simp at present
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases same : query = key
    · exact ⟨(key, value) :: rest, by simp [replaceExisting, same]⟩
    · obtain ⟨updated, replaced⟩ := ih (by simpa [same] using present)
      exact ⟨(key, old) :: updated, by simp [replaceExisting, same, replaced]⟩

theorem install_exists {K V : Type} [DecidableEq K] (accounts prepared : List (K × V))
    (present : ∀ entry ∈ prepared, entry.1 ∈ accounts.map Prod.fst) :
    ∃ updated, install accounts prepared = some updated := by
  induction prepared generalizing accounts with
  | nil => exact ⟨accounts, rfl⟩
  | cons entry rest ih =>
    obtain ⟨middle, replaced⟩ := replaceExisting_exists entry.1 entry.2 accounts (present entry (by simp))
    have keys := replaceExisting_keys entry.1 entry.2 accounts middle replaced
    obtain ⟨updated, installed⟩ := ih middle (fun item member => keys ▸ present item (by simp [member]))
    exact ⟨updated, by simpa [install, replaced] using installed⟩

theorem applyAnalysis_rejection_atomic [DecidableEq AccountId] (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (analysis : Analysis AccountId A RateId Role)
    (error : Failure AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (rejected : applyAnalysis model proposal analysis = some (.error error, after)) : after = model := by
  cases assessed : analysis.assessment with
  | invalid issues =>
    cases first : issues.head? with
    | none => simp [applyAnalysis, assessed, first] at rejected
    | some issue => exact (by simpa [applyAnalysis, assessed, first] using rejected : issue = error ∧ model = after).2.symm
  | infeasible accounts missing =>
    exact (by simpa [applyAnalysis, assessed] using rejected : Failure.infeasible missing = error ∧ model = after).2.symm
  | applicable accounts deltas =>
    cases prepared : analysis.prepared with
    | none => simp [applyAnalysis, assessed, prepared] at rejected
    | some values =>
      cases installed : install model.accounts values <;> simp [applyAnalysis, assessed, prepared, installed] at rejected

end Axionomy.Transaction
