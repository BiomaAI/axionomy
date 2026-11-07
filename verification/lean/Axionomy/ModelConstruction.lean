import Axionomy.Account

namespace Axionomy.ModelConstruction

inductive Issue (AccountId RateId : Type) where
  | duplicateAccount (id : AccountId)
  | duplicateRate (id : RateId)
  deriving DecidableEq, Repr

structure Model (AccountId AccountData RateId RateData Invariant : Type) where
  accounts : List (AccountId × AccountData)
  rates : List (RateId × RateData)
  invariants : List Invariant

structure Builder (AccountId AccountData RateId RateData Invariant : Type) extends
    Model AccountId AccountData RateId RateData Invariant where
  issues : List (Issue AccountId RateId)

namespace Builder
variable {AccountId AccountData RateId RateData Invariant : Type}

def empty : Builder AccountId AccountData RateId RateData Invariant := ⟨⟨[], [], []⟩, []⟩

def addAccount [DecidableEq AccountId]
    (builder : Builder AccountId AccountData RateId RateData Invariant)
    (id : AccountId) (value : AccountData) : Builder AccountId AccountData RateId RateData Invariant :=
  if (Account.find? id builder.accounts).isSome then
    { builder with issues := builder.issues ++ [.duplicateAccount id] }
  else { builder with accounts := builder.accounts ++ [(id, value)] }

def addRate [DecidableEq RateId]
    (builder : Builder AccountId AccountData RateId RateData Invariant)
    (id : RateId) (value : RateData) : Builder AccountId AccountData RateId RateData Invariant :=
  if (Account.find? id builder.rates).isSome then
    { builder with issues := builder.issues ++ [.duplicateRate id] }
  else { builder with rates := builder.rates ++ [(id, value)] }

def addInvariant (builder : Builder AccountId AccountData RateId RateData Invariant)
    (value : Invariant) : Builder AccountId AccountData RateId RateData Invariant :=
  { builder with invariants := builder.invariants ++ [value] }

def build (builder : Builder AccountId AccountData RateId RateData Invariant) :
    Except (List (Issue AccountId RateId)) (Model AccountId AccountData RateId RateData Invariant) :=
  if builder.issues.isEmpty then .ok builder.toModel else .error builder.issues

def Valid (builder : Builder AccountId AccountData RateId RateData Invariant) : Prop :=
  (builder.accounts.map Prod.fst).Nodup ∧ (builder.rates.map Prod.fst).Nodup

@[simp] theorem empty_valid : (empty : Builder AccountId AccountData RateId RateData Invariant).Valid := by
  simp [Valid, empty]

theorem build_success_iff (builder : Builder AccountId AccountData RateId RateData Invariant) :
    build builder = .ok builder.toModel ↔ builder.issues = [] := by
  simp [build]

theorem duplicate_account_retained [DecidableEq AccountId]
    (builder : Builder AccountId AccountData RateId RateData Invariant)
    (id : AccountId) (value old : AccountData) (found : Account.find? id builder.accounts = some old) :
    addAccount builder id value = { builder with issues := builder.issues ++ [.duplicateAccount id] } := by
  simp [addAccount, found]

theorem duplicate_rate_retained [DecidableEq RateId]
    (builder : Builder AccountId AccountData RateId RateData Invariant)
    (id : RateId) (value old : RateData) (found : Account.find? id builder.rates = some old) :
    addRate builder id value = { builder with issues := builder.issues ++ [.duplicateRate id] } := by
  simp [addRate, found]

end Builder

theorem find_none_iff {K V : Type} [DecidableEq K] (key : K) (entries : List (K × V)) :
    Account.find? key entries = none ↔ key ∉ entries.map Prod.fst := by
  induction entries with
  | nil => simp [Account.find?]
  | cons entry rest ih =>
    rcases entry with ⟨other, value⟩
    by_cases h : key = other <;> simp [Account.find?, h, ih]

theorem addAccount_valid {AccountId AccountData RateId RateData Invariant : Type} [DecidableEq AccountId]
    (builder : Builder AccountId AccountData RateId RateData Invariant) (valid : builder.Valid)
    (id : AccountId) (value : AccountData) : (builder.addAccount id value).Valid := by
  unfold Builder.addAccount
  cases h : Account.find? id builder.accounts with
  | none =>
    have absent := (find_none_iff id builder.accounts).mp h
    simp only [Builder.Valid, Option.isSome_none, Bool.false_eq_true, ↓reduceIte,
      List.map_append, List.map_cons, List.map_nil, List.nodup_append, List.nodup_cons,
      List.not_mem_nil, not_false_eq_true, List.nodup_nil, and_self, true_and]
    refine ⟨⟨valid.1, ?_⟩, valid.2⟩
    intro a member b hb equal
    have same : b = id := by simpa using hb
    exact absent (same ▸ equal ▸ member)
  | some old => simpa [Builder.Valid] using valid

theorem addRate_valid {AccountId AccountData RateId RateData Invariant : Type} [DecidableEq RateId]
    (builder : Builder AccountId AccountData RateId RateData Invariant) (valid : builder.Valid)
    (id : RateId) (value : RateData) : (builder.addRate id value).Valid := by
  unfold Builder.addRate
  cases h : Account.find? id builder.rates with
  | none =>
    have absent := (find_none_iff id builder.rates).mp h
    simp only [Builder.Valid, Option.isSome_none, Bool.false_eq_true, ↓reduceIte,
      List.map_append, List.map_cons, List.map_nil, List.nodup_append, List.nodup_cons,
      List.not_mem_nil, not_false_eq_true, List.nodup_nil, and_self, true_and]
    refine ⟨valid.1, valid.2, ?_⟩
    intro a member b hb equal
    have same : b = id := by simpa using hb
    exact absent (same ▸ equal ▸ member)
  | some old => simpa [Builder.Valid] using valid

theorem addInvariant_valid {AccountId AccountData RateId RateData Invariant : Type}
    (builder : Builder AccountId AccountData RateId RateData Invariant) (valid : builder.Valid)
    (value : Invariant) : (builder.addInvariant value).Valid := valid

end Axionomy.ModelConstruction
