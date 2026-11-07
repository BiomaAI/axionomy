import Axionomy.Validation
import Axionomy.CollectionLoopRefinement
import Axionomy.RateDiscoveryRefinement

namespace Axionomy.ValidationRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement RateConstructionRefinement

variable {AccountId A RateId Role : Type}

abbrev RuntimeIssue (AccountId A RateId Role : Type) := economy.ApplyError RateId Role AccountId A Std.U64 Std.I128

def encodeIssue : Validation.Issue AccountId RateId Role → RuntimeIssue AccountId A RateId Role
  | .zeroUnits => .ZeroUnits
  | .missingRate id => .MissingRate id
  | .missingBinding role => .MissingBinding role
  | .unknownBinding role => .UnknownBinding role
  | .missingParameter parameter => .MissingParameter parameter
  | .unknownParameter parameter => .UnknownParameter parameter
  | .rolesMustDiffer left right => .RolesMustDiffer left right
  | .missingAccount id => .MissingAccount id

theorem encodeIssue_injective : Function.Injective (encodeIssue (AccountId := AccountId) (A := A) (RateId := RateId) (Role := Role)) := by
  intro left right same
  cases left <;> cases right <;> simp_all [encodeIssue]

theorem equality_call {K : Type} [DecidableEq K] (eqInst : core.cmp.Eq K)
    (lawful : LawfulEquality eqInst) (left right : K) :
    eqInst.partialEqInst.eq left right = .ok (decide (left = right)) := lawful left right

section RoleLookup
variable [DecidableEq Role] (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (hEq : LawfulEquality roleOrd.eqInst)

include hEq in
theorem has_role_body (remaining : List Role) (query : Role) :
    rate.Rate.has_role_loop.body roleClone roleOrd query remaining =
      .ok (match remaining with
        | [] => .done false
        | role :: rest => if role = query then .done true else .cont rest) := by
  cases remaining with
  | nil => simp [rate.Rate.has_role_loop.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next]
  | cons role rest =>
    by_cases same : role = query <;>
      simp [same, rate.Rate.has_role_loop.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
      core.cmp.impls.PartialEqShared.eq, equality_call roleOrd.eqInst hEq]

include hEq in
theorem has_role_loop (remaining : List Role) (query : Role) :
    rate.Rate.has_role_loop roleClone roleOrd remaining query = .ok (decide (query ∈ remaining)) := by
  induction remaining with
  | nil =>
    unfold rate.Rate.has_role_loop
    rw [loop.eq_1]
    simp [has_role_body roleClone roleOrd hEq [] query]
  | cons role rest ih =>
    unfold rate.Rate.has_role_loop
    rw [loop.eq_1]
    simp only [has_role_body roleClone roleOrd hEq (role :: rest) query]
    by_cases same : role = query
    · simp [same]
    · simp only [ite_eq_right same, bind_ok]
      change rate.Rate.has_role_loop roleClone roleOrd rest query = _
      simpa [List.mem_cons, Ne.symm same] using ih

include hEq in
theorem has_role_refines (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
    (rule : rate.Rate Role A Std.U64) (query : Role) :
    rate.Rate.has_role roleClone roleOrd assetClone assetEq assetHash scalar rule query = .ok (decide (query ∈ rule.roles)) := by
  simp [rate.Rate.has_role, SharedABTreeSet.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter,
    has_role_loop roleClone roleOrd hEq]

end RoleLookup

section MissingRoles
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role) (cmp : Role → Role → Ordering)
variable (hClone : LawfulClone roleClone) (hOrd : LawfulOrdering roleOrd cmp)

include hClone hOrd in
theorem missing_role_step (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (role : Role) (rest : List Role) (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length < Usize.max) :
    ∃ next, economy.Economy.validate_structure_loop0.body roleClone roleOrd proposal (role :: rest) issues =
      .ok (.cont (rest, next)) ∧ next.val = issues.val ++
        (Validation.missingBinding (RateId := RateId) (fun key => Evaluation.orderedFind cmp key proposal.bindings) role).toList.map encodeIssue := by
  cases found : Evaluation.orderedFind cmp role proposal.bindings with
  | none =>
    obtain ⟨next, pushed, entries⟩ := ModelConstructionRefinement.push_call issues (.MissingBinding role) capacity
    refine ⟨next, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop0.body,
        alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
        exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.contains_key,
        alloc.collections.btree.map.BTreeMap.get, tree_get_exact roleOrd cmp hOrd,
        found, hClone role, pushed]
    · simpa [Validation.missingBinding, found, encodeIssue] using entries
  | some account =>
    refine ⟨issues, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop0.body,
        alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
        exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.contains_key,
        alloc.collections.btree.map.BTreeMap.get, tree_get_exact roleOrd cmp hOrd, found]
    · simp [Validation.missingBinding, found]

include hClone hOrd in
theorem missing_roles_refines (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (roles : List Role) (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length + roles.length ≤ Usize.max) :
    ∃ result, economy.Economy.validate_structure_loop0 roleClone roleOrd roles proposal issues = .ok result ∧
      result.val = issues.val ++ (roles.filterMap (Validation.missingBinding
        (RateId := RateId) (fun key => Evaluation.orderedFind cmp key proposal.bindings))).map encodeIssue := by
  apply CollectionLoopRefinement.collect_refines
    (fun rest accumulated => economy.Economy.validate_structure_loop0.body roleClone roleOrd proposal rest accumulated)
    Eq (fun _ => True) _ encodeIssue
  · intro values
    simp [economy.Economy.validate_structure_loop0.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next]
  · intro value actual same _ rest values bound
    subst actual
    exact missing_role_step roleClone roleOrd cmp hClone hOrd proposal value rest values bound
  · exact CollectionLoopRefinement.equal_lists _
  · simp
  · exact capacity

end MissingRoles
section UnknownRoles
variable [DecidableEq Role]
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A)
variable (hClone : LawfulClone roleClone) (hEq : LawfulEquality roleOrd.eqInst)

include hClone hEq in
theorem unknown_role_step (rule : rate.Rate Role A Std.U64) (role : Role) (rest : List Role)
    (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role)) (capacity : issues.val.length < Usize.max) :
    ∃ next, economy.Economy.validate_structure_loop1.body assetClone assetEq assetHash roleClone roleOrd scalar
      rule (role :: rest) issues = .ok (.cont (rest, next)) ∧ next.val = issues.val ++
      (Validation.unknownBinding (AccountId := AccountId) (RateId := RateId) rule.roles role).toList.map encodeIssue := by
  by_cases known : role ∈ rule.roles
  · refine ⟨issues, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop1.body,
        alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next,
        has_role_refines roleClone roleOrd hEq assetClone assetEq assetHash, known]
    · simp [Validation.unknownBinding, known]
  · obtain ⟨next, pushed, entries⟩ := ModelConstructionRefinement.push_call issues (.UnknownBinding role) capacity
    refine ⟨next, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop1.body,
        alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next,
        has_role_refines roleClone roleOrd hEq assetClone assetEq assetHash, known, hClone role, pushed]
    · simpa [Validation.unknownBinding, known, encodeIssue] using entries

include hClone hEq in
theorem unknown_roles_refines (rule : rate.Rate Role A Std.U64) (roles : List Role)
    (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length + roles.length ≤ Usize.max) :
    ∃ result, economy.Economy.validate_structure_loop1 assetClone assetEq assetHash roleClone roleOrd scalar
      roles issues rule = .ok result ∧ result.val = issues.val ++
      (roles.filterMap (Validation.unknownBinding (AccountId := AccountId) (RateId := RateId) rule.roles)).map encodeIssue := by
  apply CollectionLoopRefinement.collect_refines
    (fun rest accumulated => economy.Economy.validate_structure_loop1.body assetClone assetEq assetHash roleClone roleOrd scalar rule rest accumulated)
    Eq (fun _ => True) _ encodeIssue
  · intro values
    simp [economy.Economy.validate_structure_loop1.body,
      alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next]
  · intro value actual same _ rest values bound
    subst actual
    exact unknown_role_step roleClone roleOrd assetClone assetEq assetHash hClone hEq rule value rest values bound
  · exact CollectionLoopRefinement.equal_lists _
  · simp
  · exact capacity

end UnknownRoles

section DistinctRoles
variable [DecidableEq AccountId]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role) (cmp : Role → Role → Ordering)
variable (hClone : LawfulClone roleClone) (hEq : LawfulEquality accountEq) (hOrd : LawfulOrdering roleOrd cmp)

include hClone hEq hOrd in
theorem distinct_step (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (pair : Role × Role) (rest : List (Role × Role)) (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length < Usize.max) :
    ∃ next, economy.Economy.validate_structure_loop4.body accountClone accountEq roleClone roleOrd proposal
      (pair :: rest) issues = .ok (.cont (rest, next)) ∧ next.val = issues.val ++
      (Validation.distinctIssue (RateId := RateId) (fun role => Evaluation.orderedFind cmp role proposal.bindings) pair).toList.map encodeIssue := by
  rcases pair with ⟨left, right⟩
  cases hl : Evaluation.orderedFind cmp left proposal.bindings with
  | none =>
    refine ⟨issues, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop4.body,
        alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
        exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
        tree_get_exact roleOrd cmp hOrd, hl]
    · simp [Validation.distinctIssue, hl]
  | some leftAccount =>
    cases hr : Evaluation.orderedFind cmp right proposal.bindings with
    | none =>
      refine ⟨issues, ?_, ?_⟩
      · simp [economy.Economy.validate_structure_loop4.body,
          alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
          exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
          tree_get_exact roleOrd cmp hOrd, hl, hr]
      · simp [Validation.distinctIssue, hl, hr]
    | some rightAccount =>
      by_cases same : leftAccount = rightAccount
      · obtain ⟨next, pushed, entries⟩ := ModelConstructionRefinement.push_call issues (.RolesMustDiffer left right) capacity
        refine ⟨next, ?_, ?_⟩
        · simp [economy.Economy.validate_structure_loop4.body,
            alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
            exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
            tree_get_exact roleOrd cmp hOrd, hl, hr, core.cmp.impls.PartialEqShared.eq,
            equality_call accountEq hEq, same, hClone left, hClone right, pushed]
        · simpa [Validation.distinctIssue, hl, hr, same, encodeIssue] using entries
      · refine ⟨issues, ?_, ?_⟩
        · simp [economy.Economy.validate_structure_loop4.body,
            alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next,
            exchange.Exchange.impl.bindings, alloc.collections.btree.map.BTreeMap.get,
            tree_get_exact roleOrd cmp hOrd, hl, hr, core.cmp.impls.PartialEqShared.eq,
            equality_call accountEq hEq, same]
        · simp [Validation.distinctIssue, hl, hr, same]

include hClone hEq hOrd in
theorem distinct_refines (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (pairs : List (Role × Role)) (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length + pairs.length ≤ Usize.max) :
    ∃ result, economy.Economy.validate_structure_loop4 accountClone accountEq roleClone roleOrd pairs proposal issues = .ok result ∧
      result.val = issues.val ++ (pairs.filterMap (Validation.distinctIssue (RateId := RateId)
        (fun role => Evaluation.orderedFind cmp role proposal.bindings))).map encodeIssue := by
  apply CollectionLoopRefinement.collect_refines
    (fun rest accumulated => economy.Economy.validate_structure_loop4.body accountClone accountEq roleClone roleOrd proposal rest accumulated)
    Eq (fun _ => True) _ encodeIssue
  · intro values
    simp [economy.Economy.validate_structure_loop4.body,
      alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next]
  · intro value actual same _ rest values bound
    subst actual
    exact distinct_step accountClone accountEq roleClone roleOrd cmp hClone hEq hOrd proposal value rest values bound
  · exact CollectionLoopRefinement.equal_lists _
  · simp
  · exact capacity

end DistinctRoles

end Axionomy.ValidationRefinement
