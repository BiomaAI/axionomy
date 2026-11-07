import Axionomy.ValidationAccountsRefinement

namespace Axionomy.ValidationParametersRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement DiagnosticRefinement
open ValidationRefinement DiscoveryRefinement

variable {AccountId A RateId Role V : Type}

def ParametersFit (entries : List (String × V)) : Prop :=
  ∀ entry ∈ entries, entry.1.toByteArray.size ≤ Usize.max

theorem string_tree_get (query : String) (encoded : Str) (decoded : decodeString encoded = .ok query)
    (entries : List (String × V)) (fits : ParametersFit entries) :
    treeGet alloc.string.String.Insts.CoreBorrowBorrowStr.borrow Str.Insts.CoreCmpOrd.cmp encoded entries =
      .ok (Evaluation.orderedFind compare query entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    have bound := fits (key, value) (by simp)
    have tailFits : ParametersFit rest := fun entry member => fits entry (by simp [member])
    cases order : compare query key <;>
      simp [treeGet, alloc.string.String.Insts.CoreBorrowBorrowStr.borrow, deref_exact key bound,
        Str.Insts.CoreCmpOrd.cmp, decoded, DiagnosticRefinement.decode_encode, order, Evaluation.orderedFind, ih tailFits]

theorem string_set_get (query : String) (encoded : Str) (decoded : decodeString encoded = .ok query)
    (logical : List String) (physical : List Str) (related : Strings logical physical) :
    treeGet (Shared0T.Insts.CoreBorrowBorrow.borrow (T := Str)) Str.Insts.CoreCmpOrd.cmp encoded
      (physical.map fun key => (key, ())) =
      .ok (Evaluation.orderedFind compare query (logical.map fun key => (key, ()))) := by
  induction related with
  | nil => rfl
  | @cons key bytes names data head tail ih =>
    cases order : compare query key <;>
      simp [treeGet, Shared0T.Insts.CoreBorrowBorrow.borrow, Str.Insts.CoreCmpOrd.cmp,
        decoded, head, order, Evaluation.orderedFind, ih]

theorem missing_parameter_step (roleOrd : core.cmp.Ord Role)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (fits : ParametersFit proposal.parameters)
    (parameter : String) (bytes : Str) (decoded : decodeString bytes = .ok parameter) (rest : List Str)
    (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role)) (capacity : issues.val.length < Usize.max) :
    ∃ next, economy.Economy.validate_structure_loop2.body roleOrd proposal (bytes :: rest) issues =
      .ok (.cont (rest, next)) ∧ next.val = issues.val ++
      (Validation.missingParameter (AccountId := AccountId) (RateId := RateId) (Role := Role) proposal.parameters parameter).toList.map encodeIssue := by
  cases found : Evaluation.orderedFind compare parameter proposal.parameters with
  | none =>
    obtain ⟨next, pushed, entries⟩ := ModelConstructionRefinement.push_call issues (.MissingParameter parameter) capacity
    refine ⟨next, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop2.body,
        alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next,
        exchange.Exchange.impl.parameters, alloc.collections.btree.map.BTreeMap.contains_key,
        alloc.collections.btree.map.BTreeMap.get, string_tree_get parameter bytes decoded proposal.parameters fits,
        found, Str.Insts.AllocBorrowToOwnedString.to_owned, decoded, pushed]
    · simpa [Validation.missingParameter, found, encodeIssue] using entries
  | some amount =>
    refine ⟨issues, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop2.body,
        alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next,
        exchange.Exchange.impl.parameters, alloc.collections.btree.map.BTreeMap.contains_key,
        alloc.collections.btree.map.BTreeMap.get, string_tree_get parameter bytes decoded proposal.parameters fits, found]
    · simp [Validation.missingParameter, found]

theorem missing_parameters_refines (roleOrd : core.cmp.Ord Role)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64) (fits : ParametersFit proposal.parameters)
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length + logical.length ≤ Usize.max) :
    ∃ result, economy.Economy.validate_structure_loop2 roleOrd physical proposal issues = .ok result ∧
      result.val = issues.val ++ (logical.filterMap (Validation.missingParameter
        (AccountId := AccountId) (RateId := RateId) (Role := Role) proposal.parameters)).map encodeIssue := by
  apply CollectionLoopRefinement.collect_refines
    (fun rest accumulated => economy.Economy.validate_structure_loop2.body roleOrd proposal rest accumulated)
    (fun text bytes => decodeString bytes = .ok text) (fun _ => True) _ encodeIssue
  · intro values
    simp [economy.Economy.validate_structure_loop2.body,
      alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next]
  · intro text bytes decoded _ rest values bound
    exact missing_parameter_step roleOrd proposal fits text bytes decoded rest values bound
  · exact related
  · simp
  · exact capacity

theorem unknown_parameter_step (logical : List String) (physical : List Str) (related : Strings logical physical)
    (parameter : String) (fit : parameter.toByteArray.size ≤ Usize.max) (rest : List String)
    (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role)) (capacity : issues.val.length < Usize.max) :
    ∃ next, economy.Economy.validate_structure_loop3.body physical (parameter :: rest) issues =
      .ok (.cont (rest, next)) ∧ next.val = issues.val ++
      (Validation.unknownParameter (AccountId := AccountId) (RateId := RateId) (Role := Role) logical parameter).toList.map encodeIssue := by
  have lookup := string_set_get parameter (encodeString parameter fit) (DiagnosticRefinement.decode_encode parameter fit) logical physical related
  cases found : Evaluation.orderedFind compare parameter (logical.map fun key => (key, ())) with
  | none =>
    obtain ⟨next, pushed, entries⟩ := ModelConstructionRefinement.push_call issues (.UnknownParameter parameter) capacity
    refine ⟨next, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop3.body,
        alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next,
        alloc.string.String.as_str, deref_exact parameter fit,
        alloc.collections.btree.set.BTreeSet.contains, alloc.collections.btree.map.BTreeMap.contains_key,
        alloc.collections.btree.map.BTreeMap.get, lookup, found,
        alloc.string.String.Insts.CoreCloneClone.clone, pushed]
    · simpa [Validation.unknownParameter, Validation.containsName, found, encodeIssue] using entries
  | some value =>
    refine ⟨issues, ?_, ?_⟩
    · simp [economy.Economy.validate_structure_loop3.body,
        alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next,
        alloc.string.String.as_str, deref_exact parameter fit,
        alloc.collections.btree.set.BTreeSet.contains, alloc.collections.btree.map.BTreeMap.contains_key,
        alloc.collections.btree.map.BTreeMap.get, lookup, found]
    · simp [Validation.unknownParameter, Validation.containsName, found]

theorem unknown_parameters_refines (logical : List String) (physical : List Str) (related : Strings logical physical)
    (parameters : List String) (fits : ∀ parameter ∈ parameters, parameter.toByteArray.size ≤ Usize.max)
    (issues : alloc.vec.Vec (RuntimeIssue AccountId A RateId Role))
    (capacity : issues.val.length + parameters.length ≤ Usize.max) :
    ∃ result, economy.Economy.validate_structure_loop3 parameters issues physical = .ok result ∧
      result.val = issues.val ++ (parameters.filterMap (Validation.unknownParameter
        (AccountId := AccountId) (RateId := RateId) (Role := Role) logical)).map encodeIssue := by
  apply CollectionLoopRefinement.collect_refines
    (fun rest accumulated => economy.Economy.validate_structure_loop3.body physical rest accumulated)
    Eq (fun parameter => parameter.toByteArray.size ≤ Usize.max) _ encodeIssue
  · intro values
    simp [economy.Economy.validate_structure_loop3.body,
      alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next]
  · intro value actual same fit rest values bound
    subst actual
    exact unknown_parameter_step logical physical related value fit rest values bound
  · exact CollectionLoopRefinement.equal_lists _
  · exact fits
  · exact capacity

end Axionomy.ValidationParametersRefinement
