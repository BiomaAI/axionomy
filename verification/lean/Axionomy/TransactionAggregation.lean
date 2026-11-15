import Axionomy.Transaction

namespace Axionomy.TransactionAggregation
open Transaction

variable {AccountId A RateId Role : Type} [DecidableEq A]

def ValidEffect (effect : Effect A) : Prop :=
  BasketOperations.Valid (Account.naturals Quantity.limit) effect.consume ∧
    BasketOperations.Valid (Account.naturals Quantity.limit) effect.produce ∧
    BasketOperations.Valid (Account.naturals Quantity.limit) effect.preserve

omit [DecidableEq A] in
@[simp] theorem empty_valid : ValidEffect ({} : Effect A) := by
  simp [ValidEffect, BasketOperations.Valid]

theorem scaled_valid (target source updated : Balances A) (units : Nat)
    (valid : BasketOperations.Valid (Account.naturals Quantity.limit) target)
    (success : Effects.scaled (Account.naturals Quantity.limit) (Quantity.mul Quantity.limit) target source units = .ok updated) :
    BasketOperations.Valid (Account.naturals Quantity.limit) updated := by
  unfold Effects.scaled at success
  cases scaled : BasketOperations.scale (Account.naturals Quantity.limit) (Quantity.mul Quantity.limit) source units with
  | error asset => simp [scaled] at success
  | ok amounts => exact BasketOperations.credit_valid _ amounts target updated valid (by simpa [scaled] using success)

theorem staticScaled_valid (rate : RateId) (units : Nat) (target updated : Balances A) (source : Option (Balances A))
    (valid : BasketOperations.Valid (Account.naturals Quantity.limit) target)
    (success : staticScaled (AccountId := AccountId) (Role := Role) rate units target source = .ok updated) :
    BasketOperations.Valid (Account.naturals Quantity.limit) updated := by
  cases source with
  | none =>
    have same : target = updated := by simpa [staticScaled] using success
    exact same ▸ valid
  | some amounts =>
    cases merged : Effects.scaled (Account.naturals Quantity.limit) (Quantity.mul Quantity.limit) target amounts units with
    | error asset => simp [staticScaled, rateResult, merged] at success
    | ok result =>
      have same : result = updated := by simpa [staticScaled, rateResult, merged] using success
      exact same ▸ scaled_valid target amounts result units valid merged

theorem staticPreserved_valid (rate : RateId) (target updated : Balances A) (source : Option (Balances A))
    (valid : BasketOperations.Valid (Account.naturals Quantity.limit) target)
    (success : staticPreserved (AccountId := AccountId) (Role := Role) rate target source = .ok updated) :
    BasketOperations.Valid (Account.naturals Quantity.limit) updated := by
  cases source with
  | none =>
    have same : target = updated := by simpa [staticPreserved] using success
    exact same ▸ valid
  | some amounts =>
    cases merged : Account.credit (Account.naturals Quantity.limit) target amounts with
    | error asset => simp [staticPreserved, rateResult, merged] at success
    | ok result =>
      have same : result = updated := by simpa [staticPreserved, rateResult, merged] using success
      exact same ▸ BasketOperations.credit_valid _ amounts target result valid merged

theorem mergeStatic_valid (cmpRole : Role → Role → Ordering) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (role : Role) (initial updated : Effect A) (valid : ValidEffect initial)
    (success : mergeStatic cmpRole proposal rule role initial = .ok updated) : ValidEffect updated := by
  unfold mergeStatic at success
  cases consumed : staticScaled (AccountId := AccountId) (Role := Role) proposal.rate proposal.units initial.consume
      (Evaluation.orderedFind cmpRole role rule.consume) with
  | error error => simp [consumed] at success
  | ok consume =>
    cases produced : staticScaled (AccountId := AccountId) (Role := Role) proposal.rate proposal.units initial.produce
        (Evaluation.orderedFind cmpRole role rule.produce) with
    | error error => simp [consumed, produced] at success
    | ok produce =>
      cases preserved : staticPreserved (AccountId := AccountId) (Role := Role) proposal.rate initial.preserve
          (Evaluation.orderedFind cmpRole role rule.preserve) with
      | error error => simp only [consumed, produced, preserved, checked_bind_ok, checked_bind_error, reduceCtorEq] at success
      | ok preserve =>
        have same : (⟨consume, produce, preserve⟩ : Effect A) = updated := by
          simp only [consumed, produced, preserved, checked_bind_ok] at success
          exact Except.ok.inj success
        rw [← same]
        exact ⟨staticScaled_valid _ _ _ _ _ valid.1 consumed,
          staticScaled_valid _ _ _ _ _ valid.2.1 produced, staticPreserved_valid _ _ _ _ valid.2.2 preserved⟩

theorem mergeComputed_valid (direction : Direction) (asset : A) (amount : Nat) (initial updated : Effect A)
    (valid : ValidEffect initial) (success : mergeComputed direction asset amount initial = .ok updated) : ValidEffect updated := by
  cases direction with
  | consume =>
    cases merged : Account.credit (Account.naturals Quantity.limit) initial.consume [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with consume := entries } : Effect A) = updated := by simpa [mergeComputed, merged, Except.map] using success
      rw [← same]
      exact ⟨BasketOperations.credit_valid _ _ _ _ valid.1 merged, valid.2⟩
  | produce =>
    cases merged : Account.credit (Account.naturals Quantity.limit) initial.produce [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with produce := entries } : Effect A) = updated := by simpa [mergeComputed, merged, Except.map] using success
      rw [← same]
      exact ⟨valid.1, BasketOperations.credit_valid _ _ _ _ valid.2.1 merged, valid.2.2⟩
  | preserve =>
    cases merged : Account.credit (Account.naturals Quantity.limit) initial.preserve [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with preserve := entries } : Effect A) = updated := by simpa [mergeComputed, merged, Except.map] using success
      rw [← same]
      exact ⟨valid.1, valid.2.1, BasketOperations.credit_valid _ _ _ _ valid.2.2 merged⟩

theorem old_valid {K V : Type} (cmp : K → K → Ordering) (query : K) (entries : List (K × V))
    (predicate : V → Prop) (fallback : V) (empty : predicate fallback)
    (valid : ∀ entry ∈ entries, predicate entry.2) :
    predicate ((RateConstruction.locate cmp query entries).old.getD fallback) := by
  induction entries with
  | nil => exact empty
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases compared : cmp query key with
    | lt => simpa [RateConstruction.locate, compared] using empty
    | eq => simpa [RateConstruction.locate, compared] using valid (key, value) (by simp)
    | gt => simpa [RateConstruction.locate, compared] using ih (fun item member => valid item (by simp [member]))

theorem put_valid {K V : Type} (cmp : K → K → Ordering)
    (eqLaw : ∀ x y, cmp x y = .eq → x = y) (query : K) (entries : List (K × V))
    (predicate : K → V → Prop) (value : V) (new : predicate query value)
    (valid : ∀ entry ∈ entries, predicate entry.1 entry.2) :
    ∀ entry ∈ (RateConstruction.locate cmp query entries).put value, predicate entry.1 entry.2 := by
  induction entries with
  | nil => simpa [RateConstruction.locate, RateConstruction.Position.put] using new
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    have head := valid (key, old) (by simp)
    have tail := fun item member => valid item (List.mem_cons_of_mem (key, old) member)
    cases compared : cmp query key with
    | lt => simpa [RateConstruction.locate, compared, RateConstruction.Position.put] using And.intro new valid
    | eq =>
      have same := eqLaw query key compared
      subst key
      simpa [RateConstruction.locate, compared, RateConstruction.Position.put] using And.intro new tail
    | gt =>
      simpa [RateConstruction.locate, compared, RateConstruction.Position.put] using And.intro head (ih tail)

def ValidEffects (allowed : AccountId → Prop) (effects : List (AccountId × Effect A)) : Prop :=
  ∀ entry ∈ effects, allowed entry.1 ∧ ValidEffect entry.2

theorem staticEffects_valid (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (allowed : AccountId → Prop)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (remaining : List Role)
    (initial result : List (AccountId × Effect A))
    (bound : ∀ role ∈ remaining, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ allowed id)
    (valid : ValidEffects allowed initial)
    (success : staticEffects cmpAccount cmpRole proposal rule remaining initial = .ok result) : ValidEffects allowed result := by
  induction remaining generalizing initial with
  | nil =>
    have same : initial = result := Except.ok.inj success
    exact same ▸ valid
  | cons role rest ih =>
    obtain ⟨id, binding, present⟩ := bound role (by simp)
    let position := RateConstruction.locate cmpAccount id initial
    have old := old_valid cmpAccount id initial ValidEffect {} empty_valid (fun entry member => (valid entry member).2)
    cases merged : mergeStatic cmpRole proposal rule role (position.old.getD {}) with
    | error error => simp [staticEffects, binding, require, merged, position] at success
    | ok updated =>
      have next := put_valid cmpAccount eqLaw id initial (fun id effect => allowed id ∧ ValidEffect effect) updated
        ⟨present, mergeStatic_valid cmpRole proposal rule role _ updated old merged⟩ valid
      apply ih (position.put updated) (fun value member => bound value (by simp [member])) next
      dsimp only [position] at merged
      simpa only [staticEffects, binding, require, checked_bind_ok, merged] using success

theorem computedEffects_valid (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (allowed : AccountId → Prop)
    (proposal : Proposal AccountId RateId Role) (context : Evaluation.Context Role A Nat) (direction : Direction)
    (remaining : List (Computed Role A)) (initial result : List (AccountId × Effect A))
    (bound : ∀ amount ∈ remaining, ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ allowed id)
    (valid : ValidEffects allowed initial)
    (success : computedEffects cmpAccount cmpRole proposal context direction remaining initial = .ok result) : ValidEffects allowed result := by
  induction remaining generalizing initial with
  | nil =>
    have same : initial = result := Except.ok.inj success
    exact same ▸ valid
  | cons amount rest ih =>
    cases evaluated : Evaluation.eval (Evaluation.naturals Quantity.limit) context amount.quantity with
    | error error => simp [computedEffects, evaluated] at success
    | ok value =>
      obtain ⟨id, binding, present⟩ := bound amount (by simp)
      let position := RateConstruction.locate cmpAccount id initial
      have old := old_valid cmpAccount id initial ValidEffect {} empty_valid (fun entry member => (valid entry member).2)
      cases merged : mergeComputed direction amount.asset value (position.old.getD {}) with
      | error error => simp [computedEffects, evaluated, binding, require, merged, position, rateResult] at success
      | ok updated =>
        have next := put_valid cmpAccount eqLaw id initial (fun id effect => allowed id ∧ ValidEffect effect) updated
          ⟨present, mergeComputed_valid direction amount.asset value _ updated old merged⟩ valid
        apply ih (position.put updated) (fun item member => bound item (by simp [member])) next
        dsimp only [position] at merged
        simpa only [computedEffects, evaluated, binding, require, checked_bind_ok, merged, rateResult] using success

theorem computedChannels_valid (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (allowed : AccountId → Prop)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (context : Evaluation.Context Role A Nat)
    (initial result : List (AccountId × Effect A))
    (bound : ∀ amount ∈ rule.computedConsume ++ rule.computedProduce ++ rule.computedPreserve,
      ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ allowed id)
    (valid : ValidEffects allowed initial)
    (success : computedChannels cmpAccount cmpRole proposal rule context initial = .ok result) : ValidEffects allowed result := by
  cases consumed : computedEffects cmpAccount cmpRole proposal context .consume rule.computedConsume initial with
  | error error => simp [computedChannels, consumed] at success
  | ok consume =>
    have consumeValid := computedEffects_valid cmpAccount cmpRole eqLaw allowed proposal context .consume rule.computedConsume
      initial consume (fun amount member => bound amount (by simp [member])) valid consumed
    cases produced : computedEffects cmpAccount cmpRole proposal context .produce rule.computedProduce consume with
    | error error => simp [computedChannels, consumed, produced] at success
    | ok produce =>
      have produceValid := computedEffects_valid cmpAccount cmpRole eqLaw allowed proposal context .produce rule.computedProduce
        consume produce (fun amount member => bound amount (by simp [member])) consumeValid produced
      exact computedEffects_valid cmpAccount cmpRole eqLaw allowed proposal context .preserve rule.computedPreserve
        produce result (fun amount member => bound amount (by simp [member])) produceValid
        (by simpa [computedChannels, consumed, produced] using success)

theorem aggregate_valid (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (allowed : AccountId → Prop)
    (proposal : Proposal AccountId RateId Role) (rule : Rule Role A) (context : Evaluation.Context Role A Nat)
    (result : List (AccountId × Effect A))
    (boundStatic : ∀ role ∈ rule.roles, ∃ id, Evaluation.orderedFind cmpRole role proposal.bindings = some id ∧ allowed id)
    (boundComputed : ∀ amount ∈ rule.computedConsume ++ rule.computedProduce ++ rule.computedPreserve,
      ∃ id, Evaluation.orderedFind cmpRole amount.role proposal.bindings = some id ∧ allowed id)
    (success : aggregate cmpAccount cmpRole proposal rule context = .ok result) : ValidEffects allowed result := by
  cases aggregated : staticEffects cmpAccount cmpRole proposal rule rule.roles [] with
  | error error => simp [aggregate, aggregated] at success
  | ok initial =>
    have valid := staticEffects_valid cmpAccount cmpRole eqLaw allowed proposal rule rule.roles [] initial boundStatic (by simp [ValidEffects]) aggregated
    exact computedChannels_valid cmpAccount cmpRole eqLaw allowed proposal rule context initial result boundComputed valid
      (by simpa [aggregate, aggregated] using success)

/-! ## What an exchange takes from an account

Aggregation only ever adds to an account's effect: crediting a role's static
entries or a computed amount into it never lowers what it consumes or
preserves of any asset. So the effect an account ends with consumes and
preserves at least what each role bound to it does. -/

/-- `after` consumes and preserves, of each asset, at least what `before`
does. -/
def Grows (before after : Effect A) : Prop :=
  ∀ asset, Account.balance 0 before.consume asset ≤ Account.balance 0 after.consume asset ∧
    Account.balance 0 before.preserve asset ≤ Account.balance 0 after.preserve asset

/-- An effect grows itself. -/
theorem Grows.refl (effect : Effect A) : Grows effect effect :=
  fun _ => ⟨Nat.le_refl _, Nat.le_refl _⟩

/-- Growing twice grows. -/
theorem Grows.trans {first second third : Effect A} (left : Grows first second)
    (right : Grows second third) : Grows first third :=
  fun asset => ⟨Nat.le_trans (left asset).1 (right asset).1, Nat.le_trans (left asset).2 (right asset).2⟩

/-- Every account `before` has an effect for has one in `after` that grows
it. -/
def Retains (before after : List (AccountId × Effect A)) : Prop :=
  ∀ entry ∈ before, ∃ effect, (entry.1, effect) ∈ after ∧ Grows entry.2 effect

/-- Effects retain themselves. -/
theorem Retains.refl (effects : List (AccountId × Effect A)) : Retains effects effects :=
  fun entry member => ⟨entry.2, member, Grows.refl _⟩

/-- Retaining twice retains. -/
theorem Retains.trans {first second third : List (AccountId × Effect A)} (left : Retains first second)
    (right : Retains second third) : Retains first third := by
  intro entry member
  obtain ⟨middle, inSecond, grown⟩ := left entry member
  obtain ⟨last, inThird, further⟩ := right (entry.1, middle) inSecond
  exact ⟨last, inThird, grown.trans further⟩

/-- `consumed` and `preserved` hold, of each asset, at least what `role`'s
static entries take from its account: each asset it consumes, times the
units, and each it preserves. -/
def Holds (cmpRole : Role → Role → Ordering) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (role : Role) (consumed preserved : Balances A) : Prop :=
  (∀ asset amount, (asset, amount) ∈ (Evaluation.orderedFind cmpRole role rule.consume).getD [] →
    amount * proposal.units ≤ Account.balance 0 consumed asset) ∧
  (∀ asset amount, (asset, amount) ∈ (Evaluation.orderedFind cmpRole role rule.preserve).getD [] →
    amount ≤ Account.balance 0 preserved asset)

/-- Holding more of each asset still holds what a role takes. -/
theorem Holds.mono {cmpRole : Role → Role → Ordering} {proposal : Proposal AccountId RateId Role}
    {rule : Rule Role A} {role : Role} {consumed preserved consumed' preserved' : Balances A}
    (holds : Holds cmpRole proposal rule role consumed preserved)
    (moreConsumed : ∀ asset, Account.balance 0 consumed asset ≤ Account.balance 0 consumed' asset)
    (morePreserved : ∀ asset, Account.balance 0 preserved asset ≤ Account.balance 0 preserved' asset) :
    Holds cmpRole proposal rule role consumed' preserved' :=
  ⟨fun asset amount member => Nat.le_trans (holds.1 asset amount member) (moreConsumed asset),
    fun asset amount member => Nat.le_trans (holds.2 asset amount member) (morePreserved asset)⟩

/-- Crediting a role's entries, scaled by the units, into what an effect
consumes only grows it, and reaches each entry times the units when the
entries name each asset once. -/
theorem staticScaled_takes (rate : RateId) (units : Nat) (target updated : Balances A)
    (source : Option (Balances A)) (unique : (target.map Prod.fst).Nodup)
    (success : staticScaled (AccountId := AccountId) (Role := Role) rate units target source = .ok updated) :
    (∀ asset, Account.balance 0 target asset ≤ Account.balance 0 updated asset) ∧
      (∀ entries, source = some entries → (entries.map Prod.fst).Nodup →
        ∀ asset amount, (asset, amount) ∈ entries → amount * units ≤ Account.balance 0 updated asset) := by
  cases source with
  | none =>
    have same : target = updated := by simpa [staticScaled] using success
    subst same
    exact ⟨fun _ => Nat.le_refl _, fun _ absurd => by simp at absurd⟩
  | some entries =>
    cases scaled : BasketOperations.scale (Account.naturals Quantity.limit) (Quantity.mul Quantity.limit) entries units with
    | error asset => simp [staticScaled, rateResult, Effects.scaled, scaled] at success
    | ok amounts =>
      have credited : Account.credit (Account.naturals Quantity.limit) target amounts = .ok updated := by
        cases result : Account.credit (Account.naturals Quantity.limit) target amounts <;>
          simp_all [staticScaled, rateResult, Effects.scaled]
      have totals := (Account.credit_total Quantity.limit unique credited).1
      refine ⟨fun asset => by rw [totals asset]; exact Nat.le_add_right _ _, ?_⟩
      intro written same distinct asset amount member
      simp only [Option.some.injEq] at same
      subst same
      rw [totals asset, ← BasketOperations.scale_entries Quantity.limit units distinct scaled asset amount member]
      exact Nat.le_trans (Account.balance_le_total amounts asset) (Nat.le_add_left _ _)

/-- Crediting a role's preserved entries into what an effect preserves only
grows it, and reaches each entry. -/
theorem staticPreserved_takes (rate : RateId) (target updated : Balances A) (source : Option (Balances A))
    (unique : (target.map Prod.fst).Nodup)
    (success : staticPreserved (AccountId := AccountId) (Role := Role) rate target source = .ok updated) :
    (∀ asset, Account.balance 0 target asset ≤ Account.balance 0 updated asset) ∧
      ∀ asset amount, (asset, amount) ∈ source.getD [] → amount ≤ Account.balance 0 updated asset := by
  cases source with
  | none =>
    have same : target = updated := by simpa [staticPreserved] using success
    subst same
    exact ⟨fun _ => Nat.le_refl _, fun _ _ absurd => by simp at absurd⟩
  | some entries =>
    have credited : Account.credit (Account.naturals Quantity.limit) target entries = .ok updated := by
      cases result : Account.credit (Account.naturals Quantity.limit) target entries <;>
        simp_all [staticPreserved, rateResult]
    have totals := (Account.credit_total Quantity.limit unique credited).1
    refine ⟨fun asset => by rw [totals asset]; exact Nat.le_add_right _ _, ?_⟩
    intro asset amount member
    rw [totals asset]
    exact Nat.le_trans (Account.le_total member) (Nat.le_add_left _ _)

/-- Merging a role's static entries into an effect credits them into what
it consumes and what it preserves. -/
theorem mergeStatic_parts (cmpRole : Role → Role → Ordering) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (role : Role) (initial updated : Effect A)
    (success : mergeStatic cmpRole proposal rule role initial = .ok updated) :
    staticScaled (AccountId := AccountId) (Role := Role) proposal.rate proposal.units initial.consume
        (Evaluation.orderedFind cmpRole role rule.consume) = .ok updated.consume ∧
      staticPreserved (AccountId := AccountId) (Role := Role) proposal.rate initial.preserve
        (Evaluation.orderedFind cmpRole role rule.preserve) = .ok updated.preserve := by
  unfold mergeStatic at success
  cases consumed : staticScaled (AccountId := AccountId) (Role := Role) proposal.rate proposal.units initial.consume
      (Evaluation.orderedFind cmpRole role rule.consume) with
  | error error => simp [consumed] at success
  | ok consume =>
    cases produced : staticScaled (AccountId := AccountId) (Role := Role) proposal.rate proposal.units initial.produce
        (Evaluation.orderedFind cmpRole role rule.produce) with
    | error error => simp [consumed, produced] at success
    | ok produce =>
      cases preserved : staticPreserved (AccountId := AccountId) (Role := Role) proposal.rate initial.preserve
          (Evaluation.orderedFind cmpRole role rule.preserve) with
      | error error => simp only [consumed, produced, preserved, checked_bind_ok, checked_bind_error, reduceCtorEq] at success
      | ok preserve =>
        have same : (⟨consume, produce, preserve⟩ : Effect A) = updated := by
          simp only [consumed, produced, preserved, checked_bind_ok] at success
          exact Except.ok.inj success
        subst same
        exact ⟨rfl, rfl⟩

/-- Merging a role's static entries into an effect only grows it. -/
theorem mergeStatic_grows (cmpRole : Role → Role → Ordering) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (role : Role) (initial updated : Effect A) (valid : ValidEffect initial)
    (success : mergeStatic cmpRole proposal rule role initial = .ok updated) : Grows initial updated := by
  have parts := mergeStatic_parts cmpRole proposal rule role initial updated success
  exact fun asset => ⟨(staticScaled_takes _ _ _ _ _ valid.1.1 parts.1).1 asset,
    (staticPreserved_takes _ _ _ _ valid.2.2.1 parts.2).1 asset⟩

/-- Merging a role's static entries into an effect makes it take at least
what the role takes, when each role's consumed entries name each asset
once. -/
theorem mergeStatic_holds (cmpRole : Role → Role → Ordering) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (role : Role) (initial updated : Effect A) (valid : ValidEffect initial)
    (unique : ∀ entry ∈ rule.consume, (entry.2.map Prod.fst).Nodup)
    (success : mergeStatic cmpRole proposal rule role initial = .ok updated) :
    Holds cmpRole proposal rule role updated.consume updated.preserve := by
  have parts := mergeStatic_parts cmpRole proposal rule role initial updated success
  refine ⟨?_, (staticPreserved_takes _ _ _ _ valid.2.2.1 parts.2).2⟩
  intro asset amount member
  cases found : Evaluation.orderedFind cmpRole role rule.consume with
  | none => simp [found] at member
  | some entries =>
    simp only [found, Option.getD_some] at member
    obtain ⟨key, listed⟩ := Evaluation.orderedFind_mem found
    exact (staticScaled_takes _ _ _ _ _ valid.1.1 parts.1).2 entries found (unique (key, entries) listed)
      asset amount member

/-- Merging a computed amount into an effect only grows it. -/
theorem mergeComputed_grows (direction : Direction) (asset : A) (amount : Nat) (initial updated : Effect A)
    (valid : ValidEffect initial) (success : mergeComputed direction asset amount initial = .ok updated) :
    Grows initial updated := by
  cases direction with
  | consume =>
    cases merged : Account.credit (Account.naturals Quantity.limit) initial.consume [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with consume := entries } : Effect A) = updated := by
        simpa [mergeComputed, merged, Except.map] using success
      subst same
      have totals := (Account.credit_total Quantity.limit valid.1.1 merged).1
      exact fun other => ⟨by rw [totals other]; exact Nat.le_add_right _ _, Nat.le_refl _⟩
  | produce =>
    cases merged : Account.credit (Account.naturals Quantity.limit) initial.produce [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with produce := entries } : Effect A) = updated := by
        simpa [mergeComputed, merged, Except.map] using success
      subst same
      exact fun _ => ⟨Nat.le_refl _, Nat.le_refl _⟩
  | preserve =>
    cases merged : Account.credit (Account.naturals Quantity.limit) initial.preserve [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with preserve := entries } : Effect A) = updated := by
        simpa [mergeComputed, merged, Except.map] using success
      subst same
      have totals := (Account.credit_total Quantity.limit valid.2.2.1 merged).1
      exact fun other => ⟨Nat.le_refl _, by rw [totals other]; exact Nat.le_add_right _ _⟩

omit [DecidableEq A] in
/-- Putting a value at a key keeps every entry but the one the key locates,
whose key then holds the value. -/
theorem put_keeps {K V : Type} (cmp : K → K → Ordering) (query : K) (entries : List (K × V)) (value : V) :
    ∀ entry ∈ entries, entry ∈ (RateConstruction.locate cmp query entries).put value ∨
      ((RateConstruction.locate cmp query entries).old = some entry.2 ∧
        (entry.1, value) ∈ (RateConstruction.locate cmp query entries).put value) := by
  induction entries with
  | nil => simp
  | cons head rest ih =>
    rcases head with ⟨key, old⟩
    intro entry member
    cases compared : cmp query key with
    | lt =>
      refine Or.inl ?_
      simp only [RateConstruction.locate, compared, RateConstruction.Position.put, List.nil_append]
      exact List.mem_cons_of_mem _ member
    | eq =>
      rcases List.mem_cons.mp member with same | later
      · subst same
        exact Or.inr (by simp [RateConstruction.locate, compared, RateConstruction.Position.put])
      · exact Or.inl (by simp [RateConstruction.locate, compared, RateConstruction.Position.put, later])
    | gt =>
      rcases List.mem_cons.mp member with same | later
      · subst same
        exact Or.inl (by simp [RateConstruction.locate, compared, RateConstruction.Position.put])
      · rcases ih entry later with kept | ⟨found, placed⟩
        · refine Or.inl ?_
          simp only [RateConstruction.Position.put] at kept
          simp only [RateConstruction.locate, compared, RateConstruction.Position.put, List.cons_append]
          exact List.mem_cons_of_mem _ kept
        · refine Or.inr ⟨by simpa [RateConstruction.locate, compared] using found, ?_⟩
          simp only [RateConstruction.Position.put] at placed
          simp only [RateConstruction.locate, compared, RateConstruction.Position.put, List.cons_append]
          exact List.mem_cons_of_mem _ placed

/-- Putting, at an account, an effect that grows the one it had keeps or
grows every account's effect. -/
theorem put_retains (cmp : AccountId → AccountId → Ordering) (query : AccountId)
    (entries : List (AccountId × Effect A)) (value : Effect A)
    (grows : Grows ((RateConstruction.locate cmp query entries).old.getD {}) value) :
    Retains entries ((RateConstruction.locate cmp query entries).put value) := by
  intro entry member
  rcases put_keeps cmp query entries value entry member with kept | ⟨found, placed⟩
  · exact ⟨entry.2, kept, Grows.refl _⟩
  · rw [found, Option.getD_some] at grows
    exact ⟨value, placed, grows⟩

/-- Static effects only grow the effects they start from, and keep every
effect canonical. -/
theorem staticEffects_retains (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (remaining : List Role) (initial result : List (AccountId × Effect A))
    (valid : ∀ entry ∈ initial, ValidEffect entry.2)
    (success : staticEffects cmpAccount cmpRole proposal rule remaining initial = .ok result) :
    (∀ entry ∈ result, ValidEffect entry.2) ∧ Retains initial result := by
  induction remaining generalizing initial with
  | nil =>
    have same : initial = result := Except.ok.inj success
    subst same
    exact ⟨valid, Retains.refl _⟩
  | cons role rest ih =>
    cases binding : Evaluation.orderedFind cmpRole role proposal.bindings with
    | none => simp [staticEffects, binding, require] at success
    | some id =>
      let position := RateConstruction.locate cmpAccount id initial
      have old := old_valid cmpAccount id initial ValidEffect {} empty_valid (fun entry member => valid entry member)
      cases merged : mergeStatic cmpRole proposal rule role (position.old.getD {}) with
      | error error => simp [staticEffects, binding, require, merged, position] at success
      | ok updated =>
        have next := put_valid cmpAccount eqLaw id initial (fun _ effect => ValidEffect effect) updated
          (mergeStatic_valid cmpRole proposal rule role _ updated old merged) valid
        have kept := put_retains cmpAccount id initial updated
          (mergeStatic_grows cmpRole proposal rule role _ updated old merged)
        dsimp only [position] at merged
        have later := ih (position.put updated) next
          (by simpa only [staticEffects, binding, require, checked_bind_ok, merged] using success)
        exact ⟨later.1, kept.trans later.2⟩

/-- Computed effects only grow the effects they start from, and keep every
effect canonical. -/
theorem computedEffects_retains (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (proposal : Proposal AccountId RateId Role)
    (context : Evaluation.Context Role A Nat) (direction : Direction)
    (remaining : List (Computed Role A)) (initial result : List (AccountId × Effect A))
    (valid : ∀ entry ∈ initial, ValidEffect entry.2)
    (success : computedEffects cmpAccount cmpRole proposal context direction remaining initial = .ok result) :
    (∀ entry ∈ result, ValidEffect entry.2) ∧ Retains initial result := by
  induction remaining generalizing initial with
  | nil =>
    have same : initial = result := Except.ok.inj success
    subst same
    exact ⟨valid, Retains.refl _⟩
  | cons amount rest ih =>
    cases evaluated : Evaluation.eval (Evaluation.naturals Quantity.limit) context amount.quantity with
    | error error => simp [computedEffects, evaluated] at success
    | ok value =>
      cases binding : Evaluation.orderedFind cmpRole amount.role proposal.bindings with
      | none => simp [computedEffects, evaluated, binding, require] at success
      | some id =>
        let position := RateConstruction.locate cmpAccount id initial
        have old := old_valid cmpAccount id initial ValidEffect {} empty_valid (fun entry member => valid entry member)
        cases merged : mergeComputed direction amount.asset value (position.old.getD {}) with
        | error error => simp [computedEffects, evaluated, binding, require, merged, position, rateResult] at success
        | ok updated =>
          have next := put_valid cmpAccount eqLaw id initial (fun _ effect => ValidEffect effect) updated
            (mergeComputed_valid direction amount.asset value _ updated old merged) valid
          have kept := put_retains cmpAccount id initial updated
            (mergeComputed_grows direction amount.asset value _ updated old merged)
          dsimp only [position] at merged
          have later := ih (position.put updated) next
            (by simpa only [computedEffects, evaluated, binding, require, checked_bind_ok, merged, rateResult]
              using success)
          exact ⟨later.1, kept.trans later.2⟩

/-- Every computed channel only grows the effects it starts from, and keeps
every effect canonical. -/
theorem computedChannels_retains (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (context : Evaluation.Context Role A Nat) (initial result : List (AccountId × Effect A))
    (valid : ∀ entry ∈ initial, ValidEffect entry.2)
    (success : computedChannels cmpAccount cmpRole proposal rule context initial = .ok result) :
    (∀ entry ∈ result, ValidEffect entry.2) ∧ Retains initial result := by
  cases consumed : computedEffects cmpAccount cmpRole proposal context .consume rule.computedConsume initial with
  | error error => simp [computedChannels, consumed] at success
  | ok consume =>
    have first := computedEffects_retains cmpAccount cmpRole eqLaw proposal context .consume
      rule.computedConsume initial consume valid consumed
    cases produced : computedEffects cmpAccount cmpRole proposal context .produce rule.computedProduce consume with
    | error error => simp [computedChannels, consumed, produced] at success
    | ok produce =>
      have second := computedEffects_retains cmpAccount cmpRole eqLaw proposal context .produce
        rule.computedProduce consume produce first.1 produced
      have third := computedEffects_retains cmpAccount cmpRole eqLaw proposal context .preserve
        rule.computedPreserve produce result second.1 (by simpa [computedChannels, consumed, produced] using success)
      exact ⟨third.1, first.2.trans (second.2.trans third.2)⟩

/-- After the static effects, the account each of `remaining`'s roles is
bound to has an effect taking at least what the role takes. -/
theorem staticEffects_holds (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (proposal : Proposal AccountId RateId Role) (rule : Rule Role A)
    (unique : ∀ entry ∈ rule.consume, (entry.2.map Prod.fst).Nodup)
    (remaining : List Role) (initial result : List (AccountId × Effect A))
    (valid : ∀ entry ∈ initial, ValidEffect entry.2)
    (success : staticEffects cmpAccount cmpRole proposal rule remaining initial = .ok result) :
    ∀ role ∈ remaining, ∀ account, Evaluation.orderedFind cmpRole role proposal.bindings = some account →
      ∃ effect, (account, effect) ∈ result ∧ Holds cmpRole proposal rule role effect.consume effect.preserve := by
  induction remaining generalizing initial with
  | nil => simp
  | cons first rest ih =>
    cases binding : Evaluation.orderedFind cmpRole first proposal.bindings with
    | none => simp [staticEffects, binding, require] at success
    | some id =>
      let position := RateConstruction.locate cmpAccount id initial
      have old := old_valid cmpAccount id initial ValidEffect {} empty_valid (fun entry member => valid entry member)
      cases merged : mergeStatic cmpRole proposal rule first (position.old.getD {}) with
      | error error => simp [staticEffects, binding, require, merged, position] at success
      | ok updated =>
        have next := put_valid cmpAccount eqLaw id initial (fun _ effect => ValidEffect effect) updated
          (mergeStatic_valid cmpRole proposal rule first _ updated old merged) valid
        have holds := mergeStatic_holds cmpRole proposal rule first _ updated old unique merged
        dsimp only [position] at merged
        have following : staticEffects cmpAccount cmpRole proposal rule rest (position.put updated) = .ok result := by
          simpa only [staticEffects, binding, require, checked_bind_ok, merged] using success
        intro role member account bound
        rcases List.mem_cons.mp member with same | later
        · subst same
          rw [binding, Option.some.injEq] at bound
          subst bound
          have placed : (id, updated) ∈ position.put updated := by
            have key := RateConstruction.locate_key cmpAccount eqLaw id initial
            simp only [position, RateConstruction.Position.put] at key ⊢
            rw [key]
            exact List.mem_append_right _ (List.mem_cons_self ..)
          obtain ⟨effect, present, grown⟩ :=
            (staticEffects_retains cmpAccount cmpRole eqLaw proposal rule rest _ result next following).2
              (id, updated) placed
          exact ⟨effect, present, holds.mono (fun asset => (grown asset).1) (fun asset => (grown asset).2)⟩
        · exact ih (position.put updated) next following role later account bound

/-- The effects an exchange aggregates are canonical, and take, from the
account each role of its rate is bound to, at least what the role takes. -/
theorem aggregate_holds (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (eqLaw : ∀ x y, cmpAccount x y = .eq → x = y) (proposal : Proposal AccountId RateId Role)
    (rule : Rule Role A) (unique : ∀ entry ∈ rule.consume, (entry.2.map Prod.fst).Nodup)
    (context : Evaluation.Context Role A Nat) (result : List (AccountId × Effect A))
    (success : aggregate cmpAccount cmpRole proposal rule context = .ok result) :
    (∀ entry ∈ result, ValidEffect entry.2) ∧
      ∀ role ∈ rule.roles, ∀ account, Evaluation.orderedFind cmpRole role proposal.bindings = some account →
        ∃ effect, (account, effect) ∈ result ∧ Holds cmpRole proposal rule role effect.consume effect.preserve := by
  cases aggregated : staticEffects cmpAccount cmpRole proposal rule rule.roles [] with
  | error error => simp [aggregate, aggregated] at success
  | ok initial =>
    have computed : computedChannels cmpAccount cmpRole proposal rule context initial = .ok result := by
      simpa [aggregate, aggregated] using success
    have static := staticEffects_retains cmpAccount cmpRole eqLaw proposal rule rule.roles [] initial
      (by simp) aggregated
    have channels := computedChannels_retains cmpAccount cmpRole eqLaw proposal rule context initial result
      static.1 computed
    refine ⟨channels.1, ?_⟩
    intro role member account bound
    obtain ⟨effect, present, holds⟩ := staticEffects_holds cmpAccount cmpRole eqLaw proposal rule unique
      rule.roles [] initial (by simp) aggregated role member account bound
    obtain ⟨last, inResult, grown⟩ := channels.2 (account, effect) present
    exact ⟨last, inResult, holds.mono (fun asset => (grown asset).1) (fun asset => (grown asset).2)⟩

end Axionomy.TransactionAggregation
