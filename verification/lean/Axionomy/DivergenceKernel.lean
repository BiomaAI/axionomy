import Axionomy.Divergence
import Axionomy.TransactionSafety

/-!
# The kernel's exchanges account for their receipts

An exchange the kernel applies (`Transaction.apply`) changes what the
economy's accounts hold of each asset, all together, by exactly what its
receipt's deltas made of it: each account it touches holds what it held,
less what its delta consumed, plus what its delta produced, and every other
account holds what it held. So the kernel's exchanges meet
`Divergence.Accounts`, and two traces of them replayed from one economy
reach economies whose supplies of each asset differ by exactly what the two
traces' receipts made.

The accounting holds over canonical economies, whose accounts hold each
asset at most once and none at zero, as the kernel's baskets do; every
exchange keeps an economy canonical. It is proved for the exchange as
`Transaction.apply` defines it; `Lifecycle.apply`, which also opens accounts
empty and closes accounts its effects leave empty, is not restated here.
-/

namespace Axionomy.DivergenceKernel
open Transaction Divergence

variable {AccountId A RateId Role : Type}

section Holdings
variable [DecidableEq A]

theorem unit_self (asset : A) (amount : Nat) : unit asset asset amount = amount := by
  simp [unit, Conservation.contribution, Account.find?]

theorem unit_other (asset other : A) (amount : Nat) (different : other ≠ asset) :
    unit asset other amount = 0 := by
  simp [unit, Conservation.contribution, Account.find?, different]

theorem measure_cons (asset key : A) (amount : Nat) (rest : List (A × Nat)) :
    Conservation.measure (unit asset) ((key, amount) :: rest) =
      unit asset key amount + Conservation.measure (unit asset) rest := by
  simp [Conservation.measure]

theorem measure_absent (asset : A) (entries : List (A × Nat))
    (absent : asset ∉ entries.map Prod.fst) : Conservation.measure (unit asset) entries = 0 := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, amount⟩
    simp only [List.map_cons, List.mem_cons, not_or] at absent
    rw [measure_cons, unit_other asset key amount (fun same => absent.1 same.symm), ih absent.2]
    simp

theorem find_upsert_same {Q : Type} (asset : A) (value : Q) (entries : List (A × Q)) :
    Account.find? asset (Account.upsert asset value entries) = some value := by
  induction entries with
  | nil => simp [Account.upsert, Account.find?]
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases same : asset = key
    · subst key; simp [Account.upsert, Account.find?]
    · simp [Account.upsert, Account.find?, same, ih]

theorem find_erase_same {Q : Type} (asset : A) (entries : List (A × Q))
    (unique : (entries.map Prod.fst).Nodup) : Account.find? asset (Account.erase asset entries) = none := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    have parts : key ∉ rest.map Prod.fst ∧ (rest.map Prod.fst).Nodup := by simpa using unique
    by_cases same : asset = key
    · subst key
      simpa [Account.erase] using Account.find_absent asset rest parts.1
    · simp [Account.erase, Account.find?, same, ih parts.2]

theorem balance_store_same (limit : Nat) (entries : List (A × Nat)) (asset : A) (value : Nat)
    (unique : (entries.map Prod.fst).Nodup) :
    Account.balance 0 (Account.store (Account.naturals limit) entries asset value) asset = value := by
  unfold Account.store Account.balance
  by_cases zero : value = 0
  · subst zero
    simp [Account.naturals, find_erase_same asset entries unique]
  · simp [Account.naturals, zero, find_upsert_same]

theorem balance_store_other (limit : Nat) (entries : List (A × Nat)) (asset other : A) (value : Nat)
    (different : other ≠ asset) :
    Account.balance 0 (Account.store (Account.naturals limit) entries asset value) other =
      Account.balance 0 entries other :=
  Account.store_frame (Account.naturals limit) entries asset other value different

/-- Crediting a canonical basket adds exactly what is credited. -/
theorem credit_exact (limit : Nat) (state amounts updated : List (A × Nat))
    (valid : BasketOperations.Valid (Account.naturals limit) state)
    (success : Account.credit (Account.naturals limit) state amounts = .ok updated) (asset : A) :
    ((Account.balance 0 updated asset : Nat) : Int) =
      Account.balance 0 state asset + Conservation.measure (unit asset) amounts := by
  induction amounts generalizing state with
  | nil =>
    simp only [Account.credit, Except.ok.injEq] at success
    subst success
    simp [Conservation.measure]
  | cons entry rest ih =>
    rcases entry with ⟨key, amount⟩
    simp only [Account.credit] at success
    cases added : (Account.naturals limit).add
        (Account.balance (Account.naturals limit).zero state key) amount with
    | none => simp [added] at success
    | some value =>
      simp only [added] at success
      have sum : value = Account.balance 0 state key + amount :=
        (Quantity.add_exact limit _ _ value added).1
      rw [ih _ (BasketOperations.store_valid _ state key value valid) success, measure_cons]
      by_cases same : asset = key
      · subst key
        rw [balance_store_same limit state asset value valid.1, unit_self]
        omega
      · rw [balance_store_other limit state key asset value same, unit_other asset key amount
          (fun equal => same equal.symm)]
        omega

/-- Debiting a canonical basket by distinct assets it holds enough of takes
exactly what is debited. -/
theorem debit_exact (limit : Nat) (state amounts : List (A × Nat))
    (valid : BasketOperations.Valid (Account.naturals limit) state)
    (unique : (amounts.map Prod.fst).Nodup)
    (enough : Account.shortfall (Account.naturals limit) state amounts = []) (asset : A) :
    ((Account.balance 0 (Account.debit (Account.naturals limit) state amounts) asset : Nat) : Int) =
      Account.balance 0 state asset - Conservation.measure (unit asset) amounts := by
  induction amounts generalizing state with
  | nil => simp [Account.debit, Conservation.measure]
  | cons entry rest ih =>
    rcases entry with ⟨key, amount⟩
    have parts : key ∉ rest.map Prod.fst ∧ (rest.map Prod.fst).Nodup := by simpa using unique
    have each := (Account.shortfall_empty_iff _ state _).mp enough
    have covered : amount ≤ Account.balance 0 state key := by
      have checked := each (key, amount) (by simp)
      simp only [Account.naturals, decide_eq_false_iff_not, Nat.not_lt] at checked
      exact checked
    have stored := BasketOperations.store_valid (Account.naturals limit) state key
      (Account.balance 0 state key - amount) valid
    have restEnough : Account.shortfall (Account.naturals limit)
        (Account.store (Account.naturals limit) state key (Account.balance 0 state key - amount)) rest = [] := by
      apply (Account.shortfall_empty_iff _ _ rest).mpr
      intro item member
      have other : item.1 ≠ key := fun same => parts.1 (same ▸ List.mem_map.mpr ⟨item, member, rfl⟩)
      have untouched := balance_store_other limit state key item.1 (Account.balance 0 state key - amount) other
      have before := each item (List.mem_cons_of_mem _ member)
      simp only [Account.naturals] at untouched before ⊢
      rw [untouched]
      exact before
    show ((Account.balance 0 (Account.debit (Account.naturals limit)
        (Account.store (Account.naturals limit) state key (Account.balance 0 state key - amount)) rest) asset : Nat) : Int) =
      Account.balance 0 state asset - Conservation.measure (unit asset) ((key, amount) :: rest)
    rw [ih _ stored parts.2 restEnough, measure_cons]
    by_cases same : asset = key
    · subst key
      rw [balance_store_same limit state asset _ valid.1, unit_self, measure_absent asset rest parts.1]
      omega
    · rw [balance_store_other limit state key asset _ same, unit_other asset key amount
        (fun equal => same equal.symm)]
      omega

/-- Preparing a canonical account's update by distinct consumed assets
leaves it holding exactly what it held, less what is consumed, plus what is
produced. -/
theorem prepare_exact (limit : Nat) (state consumed produced updated : List (A × Nat))
    (valid : BasketOperations.Valid (Account.naturals limit) state)
    (unique : (consumed.map Prod.fst).Nodup)
    (success : Account.prepare (Account.naturals limit) state consumed produced = .ok updated) (asset : A) :
    ((Account.balance 0 updated asset : Nat) : Int) =
      Account.balance 0 state asset - Conservation.measure (unit asset) consumed +
        Conservation.measure (unit asset) produced := by
  obtain ⟨enough, credited⟩ := (Account.prepare_success_iff _ state consumed produced updated).mp success
  rw [credit_exact limit _ produced updated (BasketOperations.debit_valid _ consumed state valid) credited asset,
    debit_exact limit state consumed valid unique enough asset]

end Holdings

section Ledger
variable [DecidableEq AccountId] [DecidableEq A]

/-- What `accounts` hold of `asset`, all together. -/
def supply (accounts : List (AccountId × Balances A)) (asset : A) : Int :=
  (accounts.map fun entry => ((Account.balance 0 entry.2 asset : Nat) : Int)).sum

/-- What the account `id` holds of `asset` among `accounts`. -/
def held (accounts : List (AccountId × Balances A)) (id : AccountId) (asset : A) : Int :=
  (Account.balance 0 ((Account.find? id accounts).getD []) asset : Nat)

theorem find_mem {K V : Type} [DecidableEq K] (key : K) (value : V) (entries : List (K × V))
    (found : Account.find? key entries = some value) : (key, value) ∈ entries := by
  induction entries with
  | nil => simp [Account.find?] at found
  | cons entry rest ih =>
    rcases entry with ⟨stored, old⟩
    by_cases same : key = stored
    · subst stored
      simp only [Account.find?, ↓reduceIte, Option.some.injEq] at found
      simp [found]
    · simp only [Account.find?, same, ↓reduceIte] at found
      exact List.mem_cons_of_mem _ (ih found)

theorem replaceExisting_supply (key : AccountId) (value : Balances A)
    (entries updated : List (AccountId × Balances A))
    (replaced : replaceExisting key value entries = some updated) (asset : A) :
    supply updated asset + held entries key asset = supply entries asset + Account.balance 0 value asset := by
  induction entries generalizing updated with
  | nil => simp [replaceExisting] at replaced
  | cons entry rest ih =>
    rcases entry with ⟨stored, old⟩
    by_cases same : key = stored
    · subst stored
      simp only [replaceExisting, ↓reduceIte, Option.some.injEq] at replaced
      subst updated
      simp only [supply, held, Account.find?, ↓reduceIte, Option.getD_some, List.map_cons, List.sum_cons]
      omega
    · cases remaining : replaceExisting key value rest with
      | none => simp [replaceExisting, same, remaining] at replaced
      | some result =>
        simp only [replaceExisting, same, ↓reduceIte, remaining, Option.map_some,
          Option.some.injEq] at replaced
        subst updated
        have tail := ih result remaining
        simp only [supply, held, Account.find?, same, ↓reduceIte, List.map_cons, List.sum_cons] at tail ⊢
        omega

/-- Installing staged balances, each under a distinct account, changes the
supply by what each staged account holds less what it held. -/
theorem install_supply (accounts staged updated : List (AccountId × Balances A))
    (unique : (staged.map Prod.fst).Nodup) (installed : install accounts staged = some updated) (asset : A) :
    supply updated asset + (staged.map fun entry => held accounts entry.1 asset).sum =
      supply accounts asset + (staged.map fun entry => ((Account.balance 0 entry.2 asset : Nat) : Int)).sum := by
  induction staged generalizing accounts with
  | nil =>
    simp only [install, Option.some.injEq] at installed
    subst updated
    simp
  | cons entry rest ih =>
    rcases entry with ⟨id, value⟩
    have parts : id ∉ rest.map Prod.fst ∧ (rest.map Prod.fst).Nodup := by simpa using unique
    cases replaced : replaceExisting id value accounts with
    | none => simp [install, replaced] at installed
    | some middle =>
      have following : install middle rest = some updated := by simpa [install, replaced] using installed
      have tail := ih middle parts.2 following
      have untouched : (rest.map fun entry => held middle entry.1 asset) =
          rest.map fun entry => held accounts entry.1 asset := by
        apply List.map_congr_left
        intro item member
        have other : item.1 ≠ id := fun same => parts.1 (same ▸ List.mem_map.mpr ⟨item, member, rfl⟩)
        simp only [held, replaceExisting_find_other id item.1 value other accounts middle replaced]
      have first := replaceExisting_supply id value accounts middle replaced asset
      rw [untouched] at tail
      simp only [List.map_cons, List.sum_cons]
      omega

/-- A model each of whose accounts holds a canonical basket: each asset at
most once, and none at zero. -/
def Canonical (model : Model AccountId A RateId Role) : Prop :=
  ∀ entry ∈ model.accounts, BasketOperations.Valid (Account.naturals Quantity.limit) entry.2

/-- What prepared entries change, account by account, is what their
effects' deltas made. -/
theorem prepared_made (accounts : List (AccountId × Balances A))
    (effects : List (AccountId × Effect A)) (staged : List (AccountId × Balances A))
    (matched : Forall₂ (PreparedEntry accounts) effects staged)
    (canonical : ∀ entry ∈ accounts, BasketOperations.Valid (Account.naturals Quantity.limit) entry.2)
    (consumes : ∀ effect ∈ effects, (effect.2.consume.map Prod.fst).Nodup) (asset : A) :
    (staged.map fun entry => ((Account.balance 0 entry.2 asset : Nat) : Int)).sum -
        (staged.map fun entry => held accounts entry.1 asset).sum =
      made asset (effects.map effectDelta) := by
  induction matched with
  | nil => simp [made]
  | @cons effect value effectsRest stagedRest related following ih =>
    obtain ⟨sameKey, before, found, prepared⟩ := related
    have exact := prepare_exact Quantity.limit before effect.2.consume effect.2.produce value.2
      (canonical (effect.1, before) (find_mem effect.1 before accounts found))
      (consumes effect (by simp)) prepared asset
    have rest := ih (fun other member => consumes other (List.mem_cons_of_mem _ member))
    have first : held accounts value.1 asset = Account.balance 0 before asset := by
      simp [held, sameKey, found]
    simp only [made, effectDelta, List.map_cons, List.sum_cons] at rest ⊢
    rw [first]
    omega

theorem replaceExisting_mem {K V : Type} [DecidableEq K] (key : K) (value : V)
    (entries updated : List (K × V)) (replaced : replaceExisting key value entries = some updated) :
    ∀ entry ∈ updated, entry ∈ entries ∨ entry = (key, value) := by
  induction entries generalizing updated with
  | nil => simp [replaceExisting] at replaced
  | cons head rest ih =>
    rcases head with ⟨stored, old⟩
    by_cases same : key = stored
    · subst stored
      simp only [replaceExisting, ↓reduceIte, Option.some.injEq] at replaced
      subst updated
      intro entry member
      rcases List.mem_cons.mp member with equal | later
      · exact Or.inr equal
      · exact Or.inl (List.mem_cons_of_mem _ later)
    · cases remaining : replaceExisting key value rest with
      | none => simp [replaceExisting, same, remaining] at replaced
      | some result =>
        simp only [replaceExisting, same, ↓reduceIte, remaining, Option.map_some,
          Option.some.injEq] at replaced
        subst updated
        intro entry member
        rcases List.mem_cons.mp member with equal | later
        · exact Or.inl (equal ▸ List.mem_cons_self)
        · rcases ih result remaining entry later with old | new
          · exact Or.inl (List.mem_cons_of_mem _ old)
          · exact Or.inr new

theorem install_mem {K V : Type} [DecidableEq K] (accounts staged updated : List (K × V))
    (installed : install accounts staged = some updated) :
    ∀ entry ∈ updated, entry ∈ accounts ∨ entry ∈ staged := by
  induction staged generalizing accounts with
  | nil =>
    simp only [install, Option.some.injEq] at installed
    subst updated
    exact fun entry member => Or.inl member
  | cons head rest ih =>
    rcases head with ⟨id, value⟩
    cases replaced : replaceExisting id value accounts with
    | none => simp [install, replaced] at installed
    | some middle =>
      have following : install middle rest = some updated := by simpa [install, replaced] using installed
      intro entry member
      rcases ih middle following entry member with old | later
      · rcases replaceExisting_mem id value accounts middle replaced entry old with kept | new
        · exact Or.inl kept
        · exact Or.inr (new ▸ List.mem_cons_self)
      · exact Or.inr (List.mem_cons_of_mem _ later)

theorem staged_valid (accounts : List (AccountId × Balances A))
    (effects : List (AccountId × Effect A)) (staged : List (AccountId × Balances A))
    (matched : Forall₂ (PreparedEntry accounts) effects staged)
    (canonical : ∀ entry ∈ accounts, BasketOperations.Valid (Account.naturals Quantity.limit) entry.2) :
    ∀ entry ∈ staged, BasketOperations.Valid (Account.naturals Quantity.limit) entry.2 := by
  induction matched with
  | nil => simp
  | @cons effect value effectsRest stagedRest related following ih =>
    obtain ⟨_, before, found, prepared⟩ := related
    intro entry member
    rcases List.mem_cons.mp member with same | later
    · subst same
      exact BasketOperations.prepare_valid _ before effect.2.consume effect.2.produce entry.2
        (canonical (effect.1, before) (find_mem effect.1 before accounts found)) prepared
    · exact ih entry later

end Ledger

section Effects
variable [DecidableEq AccountId] [DecidableEq A]
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)

omit [DecidableEq AccountId] in
theorem staticEffects_bound (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (remaining : List Role) (initial result : List (AccountId × Effect A))
    (success : staticEffects cmpAccount cmpRole proposal rule remaining initial = .ok result) :
    ∀ role ∈ remaining, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ True := by
  induction remaining generalizing initial with
  | nil => simp
  | cons role rest ih =>
    cases binding : Evaluation.orderedFind cmpRole role proposal.bindings with
    | none => simp [staticEffects, binding, require] at success
    | some id =>
      cases merged : mergeStatic cmpRole proposal rule role
          ((RateConstruction.locate cmpAccount id initial).old.getD {}) with
      | error error => simp [staticEffects, binding, require, merged] at success
      | ok updated =>
        intro candidate member
        rcases List.mem_cons.mp member with same | later
        · subst same
          exact ⟨id, binding, trivial⟩
        · exact ih ((RateConstruction.locate cmpAccount id initial).put updated)
            (by simpa only [staticEffects, binding, require, checked_bind_ok, merged] using success)
            candidate later

omit [DecidableEq AccountId] in
theorem computedEffects_bound (proposal : Proposal AccountId RateId Role)
    (context : Evaluation.Context Role A Nat) (direction : Direction)
    (remaining : List (Computed Role A)) (initial result : List (AccountId × Effect A))
    (success : computedEffects cmpAccount cmpRole proposal context direction remaining initial = .ok result) :
    ∀ amount ∈ remaining, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ True := by
  induction remaining generalizing initial with
  | nil => simp
  | cons amount rest ih =>
    cases evaluated : Evaluation.eval (Evaluation.naturals Quantity.limit) context amount.quantity with
    | error error => simp [computedEffects, evaluated] at success
    | ok value =>
      cases binding : Evaluation.orderedFind cmpRole amount.role proposal.bindings with
      | none => simp [computedEffects, evaluated, binding, require] at success
      | some id =>
        cases merged : mergeComputed direction amount.asset value
            ((RateConstruction.locate cmpAccount id initial).old.getD {}) with
        | error error => simp [computedEffects, evaluated, binding, require, merged, rateResult] at success
        | ok updated =>
          intro candidate member
          rcases List.mem_cons.mp member with same | later
          · subst same
            exact ⟨id, binding, trivial⟩
          · exact ih ((RateConstruction.locate cmpAccount id initial).put updated)
              (by simpa only [computedEffects, evaluated, binding, require, checked_bind_ok, merged,
                rateResult] using success)
              candidate later

omit [DecidableEq AccountId] in
/-- Every account's consumption an exchange aggregates names each asset
once. -/
theorem aggregate_consumes (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (context : Evaluation.Context Role A Nat)
    (effects : List (AccountId × Effect A))
    (success : aggregate cmpAccount cmpRole proposal rule context = .ok effects) :
    ∀ effect ∈ effects, (effect.2.consume.map Prod.fst).Nodup := by
  cases initial : staticEffects cmpAccount cmpRole proposal rule rule.roles [] with
  | error error => simp [aggregate, initial] at success
  | ok first =>
    have channels : computedChannels cmpAccount cmpRole proposal rule context first = .ok effects := by
      simpa [aggregate, initial] using success
    have boundStatic := staticEffects_bound cmpAccount cmpRole proposal rule rule.roles [] first initial
    have boundComputed : ∀ amount ∈ rule.computedConsume ++ rule.computedProduce ++ rule.computedPreserve,
        ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ True := by
      cases consumed : computedEffects cmpAccount cmpRole proposal context .consume rule.computedConsume first with
      | error error => simp [computedChannels, consumed] at channels
      | ok consume =>
        cases produced : computedEffects cmpAccount cmpRole proposal context .produce rule.computedProduce consume with
        | error error => simp [computedChannels, consumed, produced] at channels
        | ok produce =>
          have preserved : computedEffects cmpAccount cmpRole proposal context .preserve rule.computedPreserve
              produce = .ok effects := by
            simpa [computedChannels, consumed, produced] using channels
          intro amount member
          simp only [List.mem_append] at member
          rcases member with (fromConsume | fromProduce) | fromPreserve
          · exact computedEffects_bound cmpAccount cmpRole proposal context _ _ _ _ consumed amount fromConsume
          · exact computedEffects_bound cmpAccount cmpRole proposal context _ _ _ _ produced amount fromProduce
          · exact computedEffects_bound cmpAccount cmpRole proposal context _ _ _ _ preserved amount fromPreserve
    have valid := TransactionAggregation.aggregate_valid cmpAccount cmpRole eqLaw (fun _ => True)
      proposal rule context effects boundStatic boundComputed success
    exact fun effect member => (valid effect member).2.1.1

/-- Every delta of an applicable analysis consumes each asset once. -/
theorem stages_consumes (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (analysis : Analysis AccountId A RateId Role)
    (success : stages cmpAccount cmpRole model proposal rule = .ok analysis)
    (entries : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (assessed : analysis.assessment = .applicable entries deltas) :
    ∀ delta ∈ deltas, (delta.consumed.map Prod.fst).Nodup := by
  let context := Evaluation.fromState cmpRole 0 proposal.units proposal.parameters proposal.bindings model.accounts
  unfold stages at success
  change (do
    liftIssue (checkConditions proposal.rate context rule.conditions)
    let effects ← aggregate cmpAccount cmpRole proposal rule context
    let availability ← preflight proposal.rate model.accounts effects ⟨[], []⟩
    if availability.shortfalls.isEmpty then
      let prepared ← prepare model.accounts effects ⟨[], []⟩
      checkInvariants model.accounts prepared model.invariants
      return (⟨.applicable availability.accounts prepared.deltas, some prepared.accounts⟩ : Analysis AccountId A RateId Role)
    else return (⟨.infeasible availability.accounts availability.shortfalls, none⟩ : Analysis AccountId A RateId Role)) = .ok analysis at success
  cases conditions : liftIssue (AccountId := AccountId) (A := A) (checkConditions proposal.rate context rule.conditions) with
  | error error => simp only [conditions, checked_bind_error, reduceCtorEq] at success
  | ok value =>
    simp only [conditions, checked_bind_ok] at success
    cases aggregated : aggregate cmpAccount cmpRole proposal rule context with
    | error error => simp only [aggregated, checked_bind_error, reduceCtorEq] at success
    | ok effects =>
      simp only [aggregated, checked_bind_ok] at success
      cases available : preflight (Role := Role) proposal.rate model.accounts effects ⟨[], []⟩ with
      | error error => simp only [available, checked_bind_error, reduceCtorEq] at success
      | ok availability =>
        simp only [available, checked_bind_ok] at success
        by_cases feasible : availability.shortfalls.isEmpty = true
        · simp only [feasible, ite_true] at success
          cases preparation : prepare (RateId := RateId) (Role := Role) model.accounts effects ⟨[], []⟩ with
          | error error => simp only [preparation, checked_bind_error, reduceCtorEq] at success
          | ok prepared =>
            simp only [preparation, checked_bind_ok] at success
            cases invariants : checkInvariants (RateId := RateId) (Role := Role) model.accounts prepared model.invariants with
            | error error => simp only [invariants, checked_bind_error, reduceCtorEq] at success
            | ok checked =>
              simp only [invariants, checked_bind_ok] at success
              have same : (⟨.applicable availability.accounts prepared.deltas, some prepared.accounts⟩ :
                  Analysis AccountId A RateId Role) = analysis := Except.ok.inj success
              rw [← same] at assessed
              simp only [Assessment.applicable.injEq] at assessed
              rw [← assessed.2, (preparation_contract model.accounts effects prepared preparation).2.1]
              intro delta member
              obtain ⟨effect, effectMember, rfl⟩ := List.mem_map.mp member
              exact aggregate_consumes cmpAccount cmpRole eqLaw proposal rule context effects aggregated
                effect effectMember
        · simp only [feasible] at success
          have same : (⟨.infeasible availability.accounts availability.shortfalls, none⟩ :
              Analysis AccountId A RateId Role) = analysis := Except.ok.inj success
          rw [← same] at assessed
          simp at assessed

theorem analyze_consumes [DecidableEq RateId] [DecidableEq Role]
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (analysis : Analysis AccountId A RateId Role)
    (success : analyze cmpAccount cmpRole model proposal = some analysis)
    (entries : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (assessed : analysis.assessment = .applicable entries deltas) :
    ∀ delta ∈ deltas, (delta.consumed.map Prod.fst).Nodup := by
  unfold analyze at success
  cases validation : Validation.validate cmpAccount cmpRole Rule.metadata proposal.units proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome) with
  | error issues =>
    have same : invalid (issues.map (Failure.structural (A := A))) = analysis := by
      simpa only [validation, Option.some.injEq] using success
    rw [← same] at assessed
    simp [invalid] at assessed
  | ok rule =>
    cases execution : stages cmpAccount cmpRole model proposal rule with
    | ok result =>
      have same : result = analysis := by
        simpa only [validation, execution, resolveStages, Option.some.injEq] using success
      exact stages_consumes cmpAccount cmpRole eqLaw model proposal rule result execution entries deltas
        (same ▸ assessed)
    | error fault =>
      cases fault with
      | panic => simp [validation, execution, resolveStages] at success
      | issue error =>
        have same : invalid [error] = analysis := by
          simpa only [validation, execution, resolveStages, Option.some.injEq] using success
        rw [← same] at assessed
        simp [invalid] at assessed

end Effects

section Exchanges
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)

/-- An exchange the kernel applies to a canonical economy changes what its
accounts hold of each asset, all together, by exactly what its receipt's
deltas made of it, and leaves the economy canonical. -/
theorem apply_supply (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (receipt : Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (canonical : Canonical model)
    (success : apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    Canonical after ∧
      ∀ asset, supply after.accounts asset = supply model.accounts asset + made asset receipt.deltas := by
  cases analyzed : analyze cmpAccount cmpRole model proposal with
  | none => simp [apply, analyzed] at success
  | some analysis =>
    have ready := analyze_ready cmpAccount cmpRole laws model proposal analysis analyzed
    have consumes := analyze_consumes cmpAccount cmpRole laws.equal model proposal analysis analyzed
    cases assessed : analysis.assessment with
    | invalid issues =>
      cases first : issues.head? <;> simp [apply, analyzed, applyAnalysis, assessed, first] at success
    | infeasible accounts missing => simp [apply, analyzed, applyAnalysis, assessed] at success
    | applicable entries deltas =>
      simp only [AnalysisReady, assessed] at ready
      obtain ⟨effects, staged, preparation, exactDeltas, matched, unique, _⟩ := ready
      cases installed : install model.accounts staged with
      | none => simp [apply, analyzed, applyAnalysis, assessed, preparation, installed] at success
      | some committed =>
        have same : (⟨proposal, deltas⟩ : Receipt AccountId A RateId Role) = receipt ∧
            { model with accounts := committed } = after := by
          simpa [apply, analyzed, applyAnalysis, assessed, preparation, installed] using success
        rcases same with ⟨rfl, rfl⟩
        refine ⟨?_, fun asset => ?_⟩
        · intro entry member
          rcases install_mem model.accounts staged committed installed entry member with old | new
          · exact canonical entry old
          · exact staged_valid model.accounts effects staged matched canonical entry new
        · have total := install_supply model.accounts staged committed unique installed asset
          have changed := prepared_made model.accounts effects staged matched canonical
            (fun effect member => consumes entries deltas assessed (effectDelta effect)
              (exactDeltas ▸ List.mem_map_of_mem member)) asset
          show supply committed asset = supply model.accounts asset + made asset deltas
          rw [exactDeltas]
          omega

/-- The kernel's exchanges account for their receipts, over canonical
economies. -/
theorem apply_accounts (laws : OrderedEffects.OrderLaws cmpAccount) :
    Divergence.Accounts (apply cmpAccount cmpRole) (Canonical (AccountId := AccountId) (A := A) (RateId := RateId) (Role := Role))
      (fun model asset => supply model.accounts asset) Receipt.deltas where
  preserves := fun model proposal receipt after canonical success =>
    (apply_supply cmpAccount cmpRole laws model proposal receipt after canonical success).1
  exact := fun model proposal receipt after canonical success =>
    (apply_supply cmpAccount cmpRole laws model proposal receipt after canonical success).2

/-- Two traces of exchanges the kernel replays from one canonical economy
reach economies whose accounts, all together, hold of each asset amounts
that differ by exactly what the two traces' receipts made of it: no
difference arises that an exchange did not make. -/
theorem replay_difference_made (laws : OrderedEffects.OrderLaws cmpAccount)
    (model left right : Model AccountId A RateId Role) (lefts rights : List (Proposal AccountId RateId Role))
    (leftReceipts rightReceipts : List (Receipt AccountId A RateId Role)) (canonical : Canonical model)
    (leftRun : Execution.run (apply cmpAccount cmpRole) model lefts [] = some (.ok leftReceipts, left))
    (rightRun : Execution.run (apply cmpAccount cmpRole) model rights [] = some (.ok rightReceipts, right))
    (asset : A) :
    supply left.accounts asset - supply right.accounts asset =
      madeBy Receipt.deltas asset leftReceipts - madeBy Receipt.deltas asset rightReceipts :=
  Divergence.difference_made _ _ _ _ (apply_accounts cmpAccount cmpRole laws) model left right lefts rights
    leftReceipts rightReceipts canonical leftRun rightRun asset

/-- Two traces the kernel replays from one canonical economy that share
their first exchanges, as two histories do up to the last point both pass
through, differ in what they hold of each asset by exactly what the
exchanges after the shared ones made, each side's. -/
theorem replay_shared_steps_make_no_difference (laws : OrderedEffects.OrderLaws cmpAccount)
    (model left right : Model AccountId A RateId Role)
    (shared lefts rights : List (Proposal AccountId RateId Role))
    (leftReceipts rightReceipts : List (Receipt AccountId A RateId Role)) (canonical : Canonical model)
    (leftRun : Execution.run (apply cmpAccount cmpRole) model (shared ++ lefts) [] = some (.ok leftReceipts, left))
    (rightRun : Execution.run (apply cmpAccount cmpRole) model (shared ++ rights) [] =
      some (.ok rightReceipts, right)) :
    ∃ common leftAfter rightAfter,
      leftReceipts = common ++ leftAfter ∧ rightReceipts = common ++ rightAfter ∧
      ∀ asset, supply left.accounts asset - supply right.accounts asset =
        madeBy Receipt.deltas asset leftAfter - madeBy Receipt.deltas asset rightAfter :=
  Divergence.shared_steps_make_no_difference _ _ _ _ (apply_accounts cmpAccount cmpRole laws) model left right
    shared lefts rights leftReceipts rightReceipts canonical leftRun rightRun

end Exchanges

end Axionomy.DivergenceKernel
