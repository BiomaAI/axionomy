import Axionomy.Lifecycle
import Axionomy.DivergenceKernel
import Axionomy.TransactionReceipts
import Axionomy.ModelConstruction

/-!
# Accounting for opening and closing accounts

Opening contributes empty accounts, the transaction accounts for every change
in holdings, and closing removes only empty committed accounts. Consequently
the full lifecycle transition and replay account for actual per-asset supply.

`Sound` records the representation invariants used here: canonical baskets,
distinct account identifiers, and distinct opening roles in each rate. Rust
enforces the latter two with its account index and rate's `BTreeSet`.
-/

namespace Axionomy.DivergenceLifecycle
open Transaction Lifecycle Divergence DivergenceKernel

variable {AccountId A RateId Role : Type}

def Sound (model : Model AccountId A RateId Role) : Prop :=
  Canonical model ∧ (model.accounts.map Prod.fst).Nodup ∧
    ∀ entry ∈ model.rates, entry.2.opens.Nodup

/-- Construction supplies the account-identity invariant. Baskets and rates
carry their own canonical representation conditions into the built model. -/
theorem built_sound (cmpAccount : AccountId → AccountId → Ordering) (cmpRate : RateId → RateId → Ordering)
    (builder : ModelConstruction.Builder AccountId (Balances A) RateId (Rule Role A) (Invariant A))
    (built : ModelConstruction.Model AccountId (Balances A) RateId (Rule Role A) (Invariant A))
    (valid : builder.Valid)
    (canonical : ∀ entry ∈ builder.accounts, BasketOperations.Valid (Account.naturals Quantity.limit) entry.2)
    (openingRoles : ∀ entry ∈ builder.rates, entry.2.opens.Nodup)
    (success : builder.build cmpAccount cmpRate = .ok built) :
    Sound ⟨built.accounts, built.rates, built.invariants⟩ := by
  unfold ModelConstruction.Builder.build at success
  split at success
  · cases success
    refine ⟨?_, ?_, ?_⟩
    · intro entry member
      exact canonical entry ((Ordered.mem_ordered cmpAccount _ entry).mp member)
    · exact ((Ordered.ordered_perm cmpAccount _).map Prod.fst).nodup_iff.mpr valid.1
    · intro entry member
      exact openingRoles entry ((Ordered.mem_ordered cmpRate _ entry).mp member)
  · cases success

section Opening
variable [DecidableEq AccountId] [DecidableEq Role]
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)

omit [DecidableEq AccountId] [DecidableEq Role] in
private theorem orderedFind_mem
    (equal : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (role : Role) (account : AccountId) (bindings : List (Role × AccountId))
    (found : Evaluation.orderedFind cmpRole role bindings = some account) :
    (role, account) ∈ bindings := by
  induction bindings with
  | nil => simp [Evaluation.orderedFind] at found
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    cases compared : cmpRole role key with
    | lt => simp [Evaluation.orderedFind, compared] at found
    | eq =>
      have same := (equal role key).mp compared
      subst key
      simp [Evaluation.orderedFind, compared] at found
      simp [found]
    | gt =>
      exact List.mem_cons_of_mem _ (ih (by simpa [Evaluation.orderedFind, compared] using found))

theorem opening_distinct (equal : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model : Model AccountId A RateId Role) (rule : Rule Role A)
    (proposal : Proposal AccountId RateId Role) (distinct : rule.opens.Nodup)
    (quiet : openingIssues cmpRole model rule proposal = []) :
    (boundTo cmpRole rule.opens proposal).Nodup := by
  have injective : ∀ left ∈ rule.opens, ∀ right ∈ rule.opens, ∀ account,
      Evaluation.orderedFind cmpRole left proposal.bindings = some account →
      Evaluation.orderedFind cmpRole right proposal.bindings = some account → left = right := by
    intro left hl right _ account fl fr
    have silent := List.filterMap_eq_nil_iff.mp quiet left hl
    have member := orderedFind_mem cmpRole equal right account proposal.bindings fr
    by_cases different : left = right
    · exact different
    have shared : proposal.bindings.any
        (fun binding => decide (binding.1 ≠ left ∧ binding.2 = account)) = true :=
      List.any_eq_true.mpr ⟨(right, account), member, by simp [Ne.symm different]⟩
    simp only [fl, shared, ↓reduceIte] at silent
    split at silent <;> simp at silent
  unfold boundTo
  generalize rolesEq : rule.opens = roles at *
  clear rolesEq quiet
  induction roles with
  | nil => simp
  | cons role rest ih =>
    have parts := List.nodup_cons.mp distinct
    have tail := ih parts.2 (fun l hl r hr a fl fr =>
      injective l (by simp [hl]) r (by simp [hr]) a fl fr)
    cases found : Evaluation.orderedFind cmpRole role proposal.bindings with
    | none => simpa [found] using tail
    | some account =>
      simp only [List.filterMap_cons, found, List.nodup_cons]
      refine ⟨?_, tail⟩
      intro member
      obtain ⟨other, ho, fo⟩ := List.mem_filterMap.mp member
      have same := injective role (by simp) other (by simp [ho]) account found fo
      exact parts.1 (same ▸ ho)

theorem opened_sound (equal : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model : Model AccountId A RateId Role) (rule : Rule Role A)
    (proposal : Proposal AccountId RateId Role) (sound : Sound model)
    (distinct : rule.opens.Nodup) (quiet : openingIssues cmpRole model rule proposal = []) :
    Sound (opened cmpAccount model (boundTo cmpRole rule.opens proposal)) := by
  refine ⟨?_, ?_, sound.2.2⟩
  · intro entry member
    have member := (Ordered.mem_ordered cmpAccount _ entry).mp member
    rcases List.mem_append.mp member with old | fresh
    · exact sound.1 entry old
    · obtain ⟨account, _, rfl⟩ := List.mem_map.mp fresh
      simp [BasketOperations.Valid]
  · apply ((Ordered.ordered_perm cmpAccount _).map Prod.fst).nodup_iff.mpr
    simp only [List.map_append, List.map_map, Function.comp_def, List.map_id']
    apply List.nodup_append.mpr
    refine ⟨sound.2.1, opening_distinct cmpRole equal model rule proposal distinct quiet, ?_⟩
    intro old ho fresh hf same
    have absent := (ModelConstruction.find_none_iff fresh model.accounts).mp
      (opening_absent cmpRole model rule proposal quiet fresh hf)
    exact absent (same ▸ ho)

end Opening

section Ledger
variable [DecidableEq AccountId] [DecidableEq A]

omit [DecidableEq AccountId] in
private theorem supply_perm (asset : A) {left right : List (AccountId × Balances A)}
    (perm : left.Perm right) : supply left asset = supply right asset := by
  induction perm with
  | nil => rfl
  | cons _ _ ih => simp only [supply, List.map_cons, List.sum_cons] at *; omega
  | swap => simp only [supply, List.map_cons, List.sum_cons]; omega
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

omit [DecidableEq AccountId] in
theorem opened_supply (cmpAccount : AccountId → AccountId → Ordering)
    (model : Model AccountId A RateId Role) (fresh : List AccountId) (asset : A) :
    supply (opened cmpAccount model fresh).accounts asset = supply model.accounts asset := by
  change supply (Ordered.ordered cmpAccount _) asset = _
  rw [supply_perm asset (Ordered.ordered_perm cmpAccount _)]
  simp [supply, List.map_append, List.sum_append, Function.comp_def, Account.balance, Account.find?,
    List.map_const', List.sum_replicate_int]

omit [DecidableEq A] in
theorem installed_remaining (accounts staged committed : List (AccountId × Balances A))
    (unique : (staged.map Prod.fst).Nodup) (installed : install accounts staged = some committed)
    (account : AccountId) :
    (Account.find? account committed).getD [] = remaining accounts staged account := by
  cases found : Account.find? account staged with
  | none =>
    rw [install_find_other account accounts staged committed
      ((ModelConstruction.find_none_iff account staged).mp found) installed]
    simp [remaining, found]
  | some balances =>
    have committedFind := install_find_staged accounts staged committed unique installed
      (account, balances) (find_mem account balances staged found)
    simp [remaining, found, committedFind]

/-- Emptiness checked at lookup is emptiness of every actual removed entry,
because committed account identifiers are distinct. -/
theorem close_supply (accounts : List (AccountId × Balances A)) (closed : List AccountId)
    (unique : (accounts.map Prod.fst).Nodup)
    (empty : ∀ account ∈ closed, (Account.find? account accounts).getD [] = []) (asset : A) :
    supply (accounts.filter fun entry => decide (entry.1 ∉ closed)) asset = supply accounts asset := by
  have removed : ∀ entry ∈ accounts, entry.1 ∈ closed → entry.2 = [] := by
    intro entry member closing
    have found := (BasketOperations.find_some_iff accounts unique entry.1 entry.2).mpr member
    simpa [found] using empty entry.1 closing
  clear unique empty
  induction accounts with
  | nil => rfl
  | cons entry rest ih =>
    have tail := ih (fun e he => removed e (List.mem_cons_of_mem _ he))
    by_cases closing : entry.1 ∈ closed
    · have zero := removed entry (by simp) closing
      simpa [supply, closing, zero, Account.balance, Account.find?] using tail
    · simpa [supply, closing] using congrArg
        (fun total => ((Account.balance 0 entry.2 asset : Nat) : Int) + total) tail

end Ledger

section Exchanges
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)

omit [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
private theorem checkClosing_original (accounts : List (AccountId × Balances A))
    (closed : List AccountId) (inner analysis : Analysis AccountId A RateId Role)
    (entries : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (checked : checkClosing accounts closed inner = analysis)
    (assessed : analysis.assessment = .applicable entries deltas) : inner = analysis := by
  subst analysis
  obtain ⟨assessment, prepared⟩ := inner
  cases assessment <;> cases prepared <;> simp only [checkClosing] at assessed ⊢ <;> try rfl
  split at *
  · rfl
  · simp [invalid] at assessed

private theorem applicable_inner (model : Model AccountId A RateId Role)
    (proposal : Proposal AccountId RateId Role) (analysis : Analysis AccountId A RateId Role)
    (rule : Rule Role A) (analyzed : Lifecycle.analyze cmpAccount cmpRole model proposal = some analysis)
    (found : Account.find? proposal.rate model.rates = some rule)
    (entries : List (AccountAssessment AccountId A)) (deltas : List (Delta AccountId A))
    (assessed : analysis.assessment = .applicable entries deltas) :
    Transaction.analyze cmpAccount cmpRole
      (opened cmpAccount model (boundTo cmpRole rule.opens proposal)) proposal = some analysis := by
  have quiet := applicable_opening cmpAccount cmpRole model proposal analysis rule analyzed found entries deltas assessed
  simp only [Lifecycle.analyze, found, quiet] at analyzed
  cases inner : Transaction.analyze cmpAccount cmpRole
      (opened cmpAccount model (boundTo cmpRole rule.opens proposal)) proposal with
  | none => simp [inner] at analyzed
  | some result =>
    simp only [inner, Option.map_some, Option.some.injEq] at analyzed
    rw [checkClosing_original _ _ result analysis entries deltas analyzed assessed]

/-- Every successful lifecycle step preserves valid state and changes actual
per-asset supply by exactly what its receipt produced less what it consumed. -/
theorem apply_supply (laws : OrderedEffects.OrderLaws cmpAccount)
    (roleEqual : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model : Model AccountId A RateId Role) (proposal : Proposal AccountId RateId Role)
    (receipt : Lifecycle.Receipt AccountId A RateId Role) (after : Model AccountId A RateId Role)
    (sound : Sound model)
    (success : Lifecycle.apply cmpAccount cmpRole model proposal = some (.ok receipt, after)) :
    Sound after ∧
      ∀ asset, supply after.accounts asset = supply model.accounts asset + made asset receipt.deltas := by
  cases analyzed : Lifecycle.analyze cmpAccount cmpRole model proposal with
  | none => simp [Lifecycle.apply, analyzed] at success
  | some analysis =>
    cases assessed : analysis.assessment with
    | invalid issues => cases first : issues.head? <;> simp [Lifecycle.apply, analyzed, assessed, first] at success
    | infeasible entries missing => simp [Lifecycle.apply, analyzed, assessed] at success
    | applicable entries deltas =>
      cases found : Account.find? proposal.rate model.rates with
      | none => simp [Lifecycle.apply, analyzed, assessed, found] at success
      | some rule =>
        have quiet := applicable_opening cmpAccount cmpRole model proposal analysis rule analyzed found entries deltas assessed
        have extendedSound := opened_sound cmpAccount cmpRole roleEqual model rule proposal sound
          (sound.2.2 (proposal.rate, rule) (find_mem proposal.rate rule model.rates found)) quiet
        have inner := applicable_inner cmpAccount cmpRole model proposal analysis rule analyzed found entries deltas assessed
        have ready := analyze_ready cmpAccount cmpRole laws _ proposal analysis inner
        simp only [AnalysisReady, assessed] at ready
        obtain ⟨effects, staged, prepared, _, _, unique, _⟩ := ready
        cases installed : install (opened cmpAccount model (boundTo cmpRole rule.opens proposal)).accounts staged with
        | none => simp [Lifecycle.apply, analyzed, assessed, found, prepared, installed] at success
        | some committed =>
          simp only [Lifecycle.apply, analyzed, assessed, found, prepared, installed, Option.bind_eq_bind,
            Option.bind_some, Option.some.injEq, Prod.mk.injEq, Except.ok.injEq] at success
          obtain ⟨rfl, rfl⟩ := success
          have transaction : Transaction.apply cmpAccount cmpRole
              (opened cmpAccount model (boundTo cmpRole rule.opens proposal)) proposal =
              some (.ok ⟨proposal, deltas⟩,
                { opened cmpAccount model (boundTo cmpRole rule.opens proposal) with accounts := committed }) := by
            simp [Transaction.apply, inner, applyAnalysis, assessed, prepared, installed]
          have accounted := DivergenceKernel.apply_supply cmpAccount cmpRole laws _ proposal _ _ extendedSound.1 transaction
          have committedUnique : (committed.map Prod.fst).Nodup := by
            rw [install_keys _ staged committed installed]
            exact extendedSound.2.1
          have empty := applicable_closes_empty cmpAccount cmpRole model proposal analysis rule analyzed found
            entries deltas assessed staged prepared
          have closing := close_supply committed (boundTo cmpRole rule.closes proposal) committedUnique
            (fun account member =>
              (installed_remaining _ staged committed unique installed account).trans (empty account member))
          refine ⟨⟨?_, ?_, sound.2.2⟩, fun asset => ?_⟩
          · intro entry member
            exact accounted.1 entry (List.mem_filter.mp member).1
          · exact committedUnique.sublist (List.filter_sublist.map Prod.fst)
          · change supply (committed.filter _) asset = supply model.accounts asset + made asset deltas
            rw [closing asset, accounted.2 asset, opened_supply]

/-- The full lifecycle step satisfies the generic accounting contract. -/
theorem apply_accounts (laws : OrderedEffects.OrderLaws cmpAccount)
    (roleEqual : ∀ a b, cmpRole a b = .eq ↔ a = b) :
    Divergence.Accounts (Lifecycle.apply cmpAccount cmpRole)
      (Sound (AccountId := AccountId) (A := A) (RateId := RateId) (Role := Role))
      (fun model asset => supply model.accounts asset) (fun receipt => receipt.deltas) where
  preserves := fun model proposal receipt after sound success =>
    (apply_supply cmpAccount cmpRole laws roleEqual model proposal receipt after sound success).1
  exact := fun model proposal receipt after sound success =>
    (apply_supply cmpAccount cmpRole laws roleEqual model proposal receipt after sound success).2

/-- Successful public replay preserves valid state and accounts for its
entire change in supply, including every opening, closing and reopening. -/
theorem replay_supply (laws : OrderedEffects.OrderLaws cmpAccount)
    (roleEqual : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model after : Model AccountId A RateId Role) (actions : List (Proposal AccountId RateId Role))
    (receipts : List (Lifecycle.Receipt AccountId A RateId Role)) (sound : Sound model)
    (success : Execution.replay (Lifecycle.apply cmpAccount cmpRole) model actions = some (.ok receipts, after)) :
    Sound after ∧ ∀ asset, supply after.accounts asset = supply model.accounts asset +
      madeBy (fun receipt => receipt.deltas) asset receipts := by
  have run := (Execution.replay_success _ _ _ _ _).mp success
  have accounts := apply_accounts (A := A) (RateId := RateId) cmpAccount cmpRole laws roleEqual
  obtain ⟨_, _, history⟩ := Execution.success_history _ actions model after [] receipts run
  exact ⟨history.invariant _ _ accounts.preserves sound,
    Divergence.run_total _ _ _ _ accounts model after actions receipts sound run⟩

/-- Two successful public replays differ in supply by exactly the difference
in what their receipts made, even if their account memberships differ. -/
theorem replay_difference_made (laws : OrderedEffects.OrderLaws cmpAccount)
    (roleEqual : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model left right : Model AccountId A RateId Role) (lefts rights : List (Proposal AccountId RateId Role))
    (leftReceipts rightReceipts : List (Lifecycle.Receipt AccountId A RateId Role)) (sound : Sound model)
    (leftRun : Execution.replay (Lifecycle.apply cmpAccount cmpRole) model lefts = some (.ok leftReceipts, left))
    (rightRun : Execution.replay (Lifecycle.apply cmpAccount cmpRole) model rights = some (.ok rightReceipts, right))
    (asset : A) :
    supply left.accounts asset - supply right.accounts asset =
      madeBy (fun receipt => receipt.deltas) asset leftReceipts - madeBy (fun receipt => receipt.deltas) asset rightReceipts :=
  Divergence.difference_made _ _ _ _ (apply_accounts cmpAccount cmpRole laws roleEqual) model left right lefts rights
    leftReceipts rightReceipts sound ((Execution.replay_success _ _ _ _ _).mp leftRun)
    ((Execution.replay_success _ _ _ _ _).mp rightRun) asset

/-- Shared lifecycle steps make no difference between the branches that
follow them: all supply differences are accounted for by the suffixes. -/
theorem replay_shared_steps_make_no_difference (laws : OrderedEffects.OrderLaws cmpAccount)
    (roleEqual : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model left right : Model AccountId A RateId Role)
    (shared lefts rights : List (Proposal AccountId RateId Role))
    (leftReceipts rightReceipts : List (Lifecycle.Receipt AccountId A RateId Role)) (sound : Sound model)
    (leftRun : Execution.replay (Lifecycle.apply cmpAccount cmpRole) model (shared ++ lefts) = some (.ok leftReceipts, left))
    (rightRun : Execution.replay (Lifecycle.apply cmpAccount cmpRole) model (shared ++ rights) = some (.ok rightReceipts, right)) :
    ∃ common leftAfter rightAfter,
      leftReceipts = common ++ leftAfter ∧ rightReceipts = common ++ rightAfter ∧
      ∀ asset, supply left.accounts asset - supply right.accounts asset =
        madeBy (fun receipt => receipt.deltas) asset leftAfter - madeBy (fun receipt => receipt.deltas) asset rightAfter :=
  Divergence.shared_steps_make_no_difference _ _ _ _ (apply_accounts cmpAccount cmpRole laws roleEqual) model left right
    shared lefts rights leftReceipts rightReceipts sound ((Execution.replay_success _ _ _ _ _).mp leftRun)
    ((Execution.replay_success _ _ _ _ _).mp rightRun)

end Exchanges

end Axionomy.DivergenceLifecycle
