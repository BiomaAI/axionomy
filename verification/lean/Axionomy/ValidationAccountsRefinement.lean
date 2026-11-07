import Axionomy.ValidationRefinement

namespace Axionomy.ValidationAccountsRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement RateConstructionRefinement
open ValidationRefinement

variable {AccountId A RateId Role : Type}
section MissingAccounts
variable [DecidableEq AccountId]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (roleOrd : core.cmp.Ord Role) (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hClone : LawfulClone accountClone) (hEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hRoleOrd : LawfulOrdering roleOrd cmpRole)

def emitted (accounts : List (AccountId × account.Account A Std.U64))
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (cmpRole : Role → Role → Ordering) : Role → Option AccountId :=
  Validation.missingAccount (fun id => (Account.find? id accounts).isSome)
    (fun role => Evaluation.orderedFind cmpRole role proposal.bindings)

include hClone hEq hAccountOrd hRoleOrd in
theorem accounts_body (accounts : List (AccountId × account.Account A Std.U64))
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (remaining : List Role) (accumulated : List AccountId) :
    economy.Economy.validate_structure_loop5.body accountClone accountEq accountHash accountOrd roleOrd
      accounts proposal remaining accumulated = .ok (match remaining with
        | [] => .done accumulated
        | role :: rest => .cont (rest, match emitted accounts proposal cmpRole role with
          | none => accumulated | some id => RateConstruction.register cmpAccount id accumulated)) := by
  cases remaining with
  | nil => simp [economy.Economy.validate_structure_loop5.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next]
  | cons role rest =>
    cases bound : Evaluation.orderedFind cmpRole role proposal.bindings with
    | none =>
      simp [economy.Economy.validate_structure_loop5.body,
        alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
        exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
        tree_get_exact roleOrd cmpRole hRoleOrd, bound, emitted, Validation.missingAccount]
    | some holder =>
      cases present : Account.find? holder accounts with
      | none =>
        obtain ⟨inserted, called⟩ := register_exact accountOrd cmpAccount hAccountOrd holder accumulated
        simp [economy.Economy.validate_structure_loop5.body,
          alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
          exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
          tree_get_exact roleOrd cmpRole hRoleOrd, bound,
          indexmap.map.IndexMap.contains_key, indexmap.map.IndexMap.get,
          equivalent_eq, get_exact accountEq hEq, present, hClone holder,
          alloc.collections.btree.set.BTreeSet.insert, called, emitted, Validation.missingAccount]
      | some holderState =>
        simp [economy.Economy.validate_structure_loop5.body,
          alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
          exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
          tree_get_exact roleOrd cmpRole hRoleOrd, bound,
          indexmap.map.IndexMap.contains_key, indexmap.map.IndexMap.get,
          equivalent_eq, get_exact accountEq hEq, present, emitted, Validation.missingAccount]

include hClone hEq hAccountOrd hRoleOrd in
theorem accounts_loop (accounts : List (AccountId × account.Account A Std.U64))
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (remaining : List Role) (accumulated : List AccountId) :
    economy.Economy.validate_structure_loop5 accountClone accountEq accountHash accountOrd roleOrd
      remaining accounts proposal accumulated =
      .ok (Discovery.registerAll cmpAccount (remaining.filterMap (emitted accounts proposal cmpRole)) accumulated) := by
  induction remaining generalizing accumulated with
  | nil =>
    unfold economy.Economy.validate_structure_loop5
    rw [loop.eq_1]
    simp only [accounts_body accountClone accountEq accountHash accountOrd roleOrd cmpAccount cmpRole
      hClone hEq hAccountOrd hRoleOrd accounts proposal [] accumulated, bind_ok]
    rfl
  | cons role rest ih =>
    unfold economy.Economy.validate_structure_loop5
    rw [loop.eq_1]
    simp only [accounts_body accountClone accountEq accountHash accountOrd roleOrd cmpAccount cmpRole
      hClone hEq hAccountOrd hRoleOrd accounts proposal (role :: rest) accumulated]
    cases choice : emitted accounts proposal cmpRole role with
    | none =>
      simp only [bind_ok]
      change economy.Economy.validate_structure_loop5 accountClone accountEq accountHash accountOrd roleOrd
        rest accounts proposal accumulated = _
      simpa [List.filterMap_cons, choice] using ih accumulated
    | some id =>
      simp only [bind_ok]
      change economy.Economy.validate_structure_loop5 accountClone accountEq accountHash accountOrd roleOrd
        rest accounts proposal (RateConstruction.register cmpAccount id accumulated) = _
      simpa [List.filterMap_cons, choice, Discovery.registerAll] using ih (RateConstruction.register cmpAccount id accumulated)

end MissingAccounts

theorem append_accounts (accounts : List AccountId) (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length + accounts.length ≤ Usize.max) :
    ∃ result, economy.Economy.validate_structure_loop6 accounts issues = .ok result ∧
      result.val = issues.val ++ (accounts.map fun id => encodeIssue (.missingAccount id)) := by
  have emitted : (accounts.filterMap fun id => some (Validation.Issue.missingAccount (RateId := RateId) (Role := Role) id)).map
      (encodeIssue (A := A)) = accounts.map (fun id => encodeIssue (.missingAccount id)) := by simp
  rw [← emitted]
  apply CollectionLoopRefinement.collect_refines
    (fun rest accumulated => economy.Economy.validate_structure_loop6.body rest accumulated)
    Eq (fun _ => True) (fun id => some (.missingAccount id)) encodeIssue
  · intro values
    simp [economy.Economy.validate_structure_loop6.body,
      alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next]
  · intro value actual same _ rest values bound
    subst actual
    obtain ⟨next, pushed, entries⟩ := ModelConstructionRefinement.push_call values (.MissingAccount value) bound
    refine ⟨next, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop6.body,
        alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next, pushed]
    · simpa [encodeIssue] using entries
  · exact CollectionLoopRefinement.equal_lists _
  · simp
  · exact capacity

end Axionomy.ValidationAccountsRefinement
