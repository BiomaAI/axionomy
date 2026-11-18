import Axionomy.Parametric
import Axionomy.DivergenceLifecycle

namespace Axionomy.Parametric

open Pattern Schema Transaction Divergence DivergenceKernel
variable {Role : Type} [DecidableEq Role]

/-- The representation invariants of a parametric economy: canonical account
baskets, unique account identifiers and unique opening roles in each law. -/
def Sound (model : Model Role) : Prop :=
  (∀ entry ∈ model.accounts, BasketOperations.Valid (Account.naturals Quantity.limit) entry.2) ∧
  (model.accounts.map Prod.fst).Nodup ∧ ∀ law ∈ model.rates, law.schema.opens.Nodup

omit [DecidableEq Role] in
private theorem resolved_opens {model : Model Role} {cmpRole : Role → Role → Ordering}
    {id : Written} {rule : Rule Role Written} {fixed : List (Role × Written)}
    (sound : Sound model) (resolved : resolve cmpRole model id = .ok (rule, fixed)) :
    rule.opens.Nodup := by
  obtain ⟨law, args, _, values, found, _, _, _, _, _, built, _⟩ := resolve_allowed resolved
  obtain ⟨candidate, member, matched⟩ := List.exists_of_findSome?_eq_some found
  cases result : «match» candidate.id id with
  | none => simp [result] at matched
  | some _ =>
    simp only [result, Option.map_eq_map, Option.map_some, Option.some.injEq, Prod.mk.injEq] at matched
    have opens := sound.2.2 law (matched.1 ▸ member)
    unfold Schema.instantiate at built
    simp only [bind, Except.bind] at built
    repeat' split at built
    all_goals first | contradiction | (cases built; exact opens)

/-- Each accepted named exchange changes actual supply by exactly its
receipt's net production, and preserves the hypotheses for the next step. -/
theorem apply_supply (cmpRole : Role → Role → Ordering)
    (equal : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model after : Model Role) (proposal : Proposal Role)
    (receipt : Lifecycle.Receipt Written Written Written Role)
    (sound : Sound model) (success : apply cmpRole model proposal = some (.ok receipt, after)) :
    Sound after ∧ ∀ asset, supply after.accounts asset =
      supply model.accounts asset + made asset receipt.deltas := by
  unfold apply at success
  cases resolved : resolve cmpRole model proposal.rate with
  | error => simp only [resolved] at success; exact (head_not_accepted success).elim
  | ok pair =>
    obtain ⟨rule, fixed⟩ := pair
    simp only [resolved] at success
    cases wrong : wrongAccounts cmpRole proposal fixed with
    | cons => simp only [wrong] at success; exact (head_not_accepted success).elim
    | nil =>
      simp only [wrong] at success
      cases applied : Lifecycle.apply Path.compare cmpRole (instanceModel model proposal.rate rule) proposal with
      | none => simp [applied] at success
      | some outcome =>
        obtain ⟨result, changed⟩ := outcome
        simp only [applied, Option.map_some, Option.some.injEq, Prod.mk.injEq] at success
        obtain ⟨rfl, rfl⟩ := success
        have valid : DivergenceLifecycle.Sound (instanceModel model proposal.rate rule) := by
          refine ⟨sound.1, sound.2.1, ?_⟩
          intro entry member
          simp only [instanceModel, List.mem_singleton] at member
          subst entry
          exact resolved_opens sound resolved
        obtain ⟨next, accounted⟩ := DivergenceLifecycle.apply_supply Path.compare cmpRole
          Path.orderLaws equal _ proposal receipt changed valid applied
        exact ⟨⟨next.1, next.2.1, sound.2.2⟩, accounted⟩

/-- A completed replay of named rate instances accounts for the entire
change in holdings, including accounts opened or closed along the way. -/
theorem replay_supply (cmpRole : Role → Role → Ordering)
    (equal : ∀ a b, cmpRole a b = .eq ↔ a = b)
    (model after : Model Role) (actions : List (Proposal Role))
    (receipts : List (Lifecycle.Receipt Written Written Written Role)) (sound : Sound model)
    (success : Execution.replay (apply cmpRole) model actions = some (.ok receipts, after)) :
    Sound after ∧ ∀ asset, supply after.accounts asset = supply model.accounts asset +
      madeBy (fun receipt => receipt.deltas) asset receipts := by
  have ledger : Divergence.Accounts (apply cmpRole) Sound (fun m a => supply m.accounts a)
      (fun receipt => receipt.deltas) :=
    ⟨fun m p r n s h => (apply_supply cmpRole equal m n p r s h).1,
     fun m p r n s h => (apply_supply cmpRole equal m n p r s h).2⟩
  have run := (Execution.replay_success _ _ _ _ _).mp success
  obtain ⟨_, _, history⟩ := Execution.success_history _ actions model after [] receipts run
  exact ⟨history.invariant _ _ ledger.preserves sound,
    Divergence.run_total _ _ _ _ ledger model after actions receipts sound run⟩

end Axionomy.Parametric
