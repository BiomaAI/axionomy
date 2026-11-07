import Axionomy.Transaction

namespace Axionomy.TransactionAggregation
open Transaction

variable {AccountId A RateId Role : Type} [DecidableEq A]

def ValidEffect (effect : Effect A) : Prop :=
  BasketOperations.Valid (Account.naturals Quantity.u64Max) effect.consume ∧
    BasketOperations.Valid (Account.naturals Quantity.u64Max) effect.produce ∧
    BasketOperations.Valid (Account.naturals Quantity.u64Max) effect.preserve

omit [DecidableEq A] in
@[simp] theorem empty_valid : ValidEffect ({} : Effect A) := by
  simp [ValidEffect, BasketOperations.Valid]

theorem scaled_valid (target source updated : Balances A) (units : Nat)
    (valid : BasketOperations.Valid (Account.naturals Quantity.u64Max) target)
    (success : Effects.scaled (Account.naturals Quantity.u64Max) (Quantity.mul Quantity.u64Max) target source units = .ok updated) :
    BasketOperations.Valid (Account.naturals Quantity.u64Max) updated := by
  unfold Effects.scaled at success
  cases scaled : BasketOperations.scale (Account.naturals Quantity.u64Max) (Quantity.mul Quantity.u64Max) source units with
  | error asset => simp [scaled] at success
  | ok amounts => exact BasketOperations.credit_valid _ amounts target updated valid (by simpa [scaled] using success)

theorem staticScaled_valid (rate : RateId) (units : Nat) (target updated : Balances A) (source : Option (Balances A))
    (valid : BasketOperations.Valid (Account.naturals Quantity.u64Max) target)
    (success : staticScaled (AccountId := AccountId) (Role := Role) rate units target source = .ok updated) :
    BasketOperations.Valid (Account.naturals Quantity.u64Max) updated := by
  cases source with
  | none =>
    have same : target = updated := by simpa [staticScaled] using success
    exact same ▸ valid
  | some amounts =>
    cases merged : Effects.scaled (Account.naturals Quantity.u64Max) (Quantity.mul Quantity.u64Max) target amounts units with
    | error asset => simp [staticScaled, rateResult, merged] at success
    | ok result =>
      have same : result = updated := by simpa [staticScaled, rateResult, merged] using success
      exact same ▸ scaled_valid target amounts result units valid merged

theorem staticPreserved_valid (rate : RateId) (target updated : Balances A) (source : Option (Balances A))
    (valid : BasketOperations.Valid (Account.naturals Quantity.u64Max) target)
    (success : staticPreserved (AccountId := AccountId) (Role := Role) rate target source = .ok updated) :
    BasketOperations.Valid (Account.naturals Quantity.u64Max) updated := by
  cases source with
  | none =>
    have same : target = updated := by simpa [staticPreserved] using success
    exact same ▸ valid
  | some amounts =>
    cases merged : Account.credit (Account.naturals Quantity.u64Max) target amounts with
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
    cases merged : Account.credit (Account.naturals Quantity.u64Max) initial.consume [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with consume := entries } : Effect A) = updated := by simpa [mergeComputed, merged, Except.map] using success
      rw [← same]
      exact ⟨BasketOperations.credit_valid _ _ _ _ valid.1 merged, valid.2⟩
  | produce =>
    cases merged : Account.credit (Account.naturals Quantity.u64Max) initial.produce [(asset, amount)] with
    | error bad => simp [mergeComputed, merged, Except.map] at success
    | ok entries =>
      have same : ({ initial with produce := entries } : Effect A) = updated := by simpa [mergeComputed, merged, Except.map] using success
      rw [← same]
      exact ⟨valid.1, BasketOperations.credit_valid _ _ _ _ valid.2.1 merged, valid.2.2⟩
  | preserve =>
    cases merged : Account.credit (Account.naturals Quantity.u64Max) initial.preserve [(asset, amount)] with
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
    cases evaluated : Evaluation.eval (Evaluation.naturals Quantity.u64Max) context amount.quantity with
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

end Axionomy.TransactionAggregation
