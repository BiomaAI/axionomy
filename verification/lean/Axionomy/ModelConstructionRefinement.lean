import Axionomy.ModelConstruction
import Axionomy.AccountRefinement

namespace Axionomy.ModelConstructionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement ModelConstruction

def decodeIssue {AccountId RateId : Type} : economy.ModelIssue AccountId RateId → Issue AccountId RateId
  | .DuplicateAccount id => .duplicateAccount id
  | .DuplicateRate id => .duplicateRate id

def decode {AccountId A RateId Role : Type}
    (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64) :
    Builder AccountId (account.Account A Std.U64) RateId (rate.Rate Role A Std.U64) (rate.LinearInvariant A) :=
  ⟨⟨builder.accounts, builder.rates, builder.invariants.val⟩, builder.issues.val.map decodeIssue⟩

theorem push_call {T : Type} (values : alloc.vec.Vec T) (value : T)
    (capacity : values.val.length < Usize.max) :
    ∃ result, alloc.vec.Vec.push values value = .ok result ∧ result.val = values.val ++ [value] := by
  have bound : (values.val ++ [value]).length ≤ Usize.max := by simp; omega
  refine ⟨alloc.vec.Vec.from _ bound, ?_, by simp⟩
  have grows : values.val.length + 1 ≤ Usize.max := by omega
  simp [alloc.vec.Vec.push, grows, List.concat_eq_append]

section Construction
variable {AccountId A RateId Role : Type} [DecidableEq AccountId] [DecidableEq RateId]
variable (accountEq : core.cmp.Eq AccountId) (accountHash : core.hash.Hash AccountId)
variable (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId)
variable (hAccount : LawfulEquality accountEq) (hRate : LawfulEquality rateEq)

omit [DecidableEq AccountId] [DecidableEq RateId] in
theorem new_refines :
    ∃ result, model_new A Role accountEq accountHash rateEq rateHash = .ok result ∧
      decode result = Builder.empty := by
  refine ⟨⟨[], [], alloc.vec.Vec.new _, alloc.vec.Vec.new _⟩, ?_, ?_⟩
  · simp [model_new, economy.EconomyBuilder.new, indexmap.map.IndexMapKVRandomState.new]
  · rfl

omit [DecidableEq AccountId] [DecidableEq RateId] in
theorem default_refines :
    ∃ result, model_default A Role accountEq accountHash rateEq rateHash = .ok result ∧
      decode result = Builder.empty := by
  simpa only [model_default, economy.EconomyBuilder.Insts.CoreDefaultDefault.default, model_new]
    using new_refines (A := A) (Role := Role) accountEq accountHash rateEq rateHash

include hAccount in
omit [DecidableEq RateId] in
theorem account_refines (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (id : AccountId) (state : account.Account A Std.U64)
    (capacity : builder.issues.val.length < Usize.max) :
    ∃ result, model_account accountEq accountHash rateEq rateHash builder id state = .ok result ∧
      decode result = (decode builder).addAccount id state := by
  cases found : Account.find? id builder.accounts with
  | none =>
    have absent := (find_none_iff id builder.accounts).mp found
    refine ⟨{ builder with accounts := builder.accounts ++ [(id, state)] }, ?_, ?_⟩
    · simp [model_account, economy.EconomyBuilder.account, indexmap.map.IndexMap.contains_key,
        indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccount, found,
        indexmap.map.IndexMap.insert, insert_exact accountEq hAccount, Account.upsert_fresh id state _ absent]
    · simp [decode, Builder.addAccount, found]
  | some old =>
    obtain ⟨issues, pushed, entries⟩ := push_call builder.issues (.DuplicateAccount id) capacity
    refine ⟨{ builder with issues := issues }, ?_, ?_⟩
    · simp [model_account, economy.EconomyBuilder.account, indexmap.map.IndexMap.contains_key,
        indexmap.map.IndexMap.get, equivalent_eq, get_exact accountEq hAccount, found, pushed]
    · simp [decode, Builder.addAccount, found, entries, decodeIssue]

include hRate in
omit [DecidableEq AccountId] in
theorem rate_refines (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (id : RateId) (rule : rate.Rate Role A Std.U64)
    (capacity : builder.issues.val.length < Usize.max) :
    ∃ result, model_rate accountEq accountHash rateEq rateHash builder id rule = .ok result ∧
      decode result = (decode builder).addRate id rule := by
  cases found : Account.find? id builder.rates with
  | none =>
    have absent := (find_none_iff id builder.rates).mp found
    refine ⟨{ builder with rates := builder.rates ++ [(id, rule)] }, ?_, ?_⟩
    · simp [model_rate, economy.EconomyBuilder.rate, indexmap.map.IndexMap.contains_key,
        indexmap.map.IndexMap.get, equivalent_eq, get_exact rateEq hRate, found,
        indexmap.map.IndexMap.insert, insert_exact rateEq hRate, Account.upsert_fresh id rule _ absent]
    · simp [decode, Builder.addRate, found]
  | some old =>
    obtain ⟨issues, pushed, entries⟩ := push_call builder.issues (.DuplicateRate id) capacity
    refine ⟨{ builder with issues := issues }, ?_, ?_⟩
    · simp [model_rate, economy.EconomyBuilder.rate, indexmap.map.IndexMap.contains_key,
        indexmap.map.IndexMap.get, equivalent_eq, get_exact rateEq hRate, found, pushed]
    · simp [decode, Builder.addRate, found, entries, decodeIssue]

omit [DecidableEq AccountId] [DecidableEq RateId] in
theorem invariant_refines (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (invariant : rate.LinearInvariant A) (capacity : builder.invariants.val.length < Usize.max) :
    ∃ result, model_invariant accountEq accountHash rateEq rateHash builder invariant = .ok result ∧
      decode result = (decode builder).addInvariant invariant := by
  obtain ⟨invariants, pushed, entries⟩ := push_call builder.invariants invariant capacity
  refine ⟨{ builder with invariants := invariants }, ?_, ?_⟩
  · simp [model_invariant, economy.EconomyBuilder.invariant, pushed]
  · simp [decode, Builder.addInvariant, entries]

include hAccount in
omit [DecidableEq RateId] in
theorem account_preserves_valid (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (valid : (decode builder).Valid) (id : AccountId) (state : account.Account A Std.U64)
    (capacity : builder.issues.val.length < Usize.max) :
    ∃ result, model_account accountEq accountHash rateEq rateHash builder id state = .ok result ∧
      (decode result).Valid := by
  obtain ⟨result, executed, agrees⟩ := account_refines accountEq accountHash rateEq rateHash hAccount
    builder id state capacity
  exact ⟨result, executed, agrees ▸ addAccount_valid (decode builder) valid id state⟩

include hRate in
omit [DecidableEq AccountId] in
theorem rate_preserves_valid (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (valid : (decode builder).Valid) (id : RateId) (rule : rate.Rate Role A Std.U64)
    (capacity : builder.issues.val.length < Usize.max) :
    ∃ result, model_rate accountEq accountHash rateEq rateHash builder id rule = .ok result ∧
      (decode result).Valid := by
  obtain ⟨result, executed, agrees⟩ := rate_refines accountEq accountHash rateEq rateHash hRate
    builder id rule capacity
  exact ⟨result, executed, agrees ▸ addRate_valid (decode builder) valid id rule⟩

include hAccount in
omit [DecidableEq RateId] in
theorem build_body (remaining accumulated : List (AccountId × account.Account A Std.U64)) :
    economy.EconomyBuilder.build_loop.body accountEq accountHash remaining accumulated =
      .ok (match remaining with
        | [] => .done accumulated
        | (id, holder) :: rest => .cont (rest, Account.upsert id holder accumulated)) := by
  cases remaining with
  | nil => simp [economy.EconomyBuilder.build_loop.body,
      indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next]
  | cons entry rest =>
    rcases entry with ⟨id, holder⟩
    simp [economy.EconomyBuilder.build_loop.body,
      indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next,
      alloc.sync.Arc.new, indexmap.map.IndexMap.insert, insert_exact accountEq hAccount]

include hAccount in
omit [DecidableEq RateId] in
theorem build_loop_refines (remaining accumulated : List (AccountId × account.Account A Std.U64))
    (unique : ((accumulated ++ remaining).map Prod.fst).Nodup) :
    economy.EconomyBuilder.build_loop accountEq accountHash remaining accumulated =
      .ok (accumulated ++ remaining) := by
  induction remaining generalizing accumulated with
  | nil =>
    unfold economy.EconomyBuilder.build_loop
    rw [loop.eq_1]
    simp [economy.EconomyBuilder.build_loop.body,
      indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next]
  | cons entry rest ih =>
    rcases entry with ⟨id, holder⟩
    have fresh : id ∉ accumulated.map Prod.fst := by
      have split := List.nodup_append.mp (by simpa using unique)
      intro member
      exact split.2.2 id member id (by simp) rfl
    have nextUnique : (((accumulated ++ [(id, holder)]) ++ rest).map Prod.fst).Nodup := by
      simpa [List.append_assoc] using unique
    unfold economy.EconomyBuilder.build_loop
    rw [loop.eq_1]
    simp only [build_body accountEq accountHash hAccount ((id, holder) :: rest) accumulated,
      Account.upsert_fresh id holder _ fresh, bind_ok]
    change economy.EconomyBuilder.build_loop accountEq accountHash rest
      (accumulated ++ [(id, holder)]) = _
    rw [ih _ nextUnique]
    simp [List.append_assoc]

def decodeBuilt (model : economy.Economy AccountId A RateId Role Std.U64) :
    Model AccountId (account.Account A Std.U64) RateId (rate.Rate Role A Std.U64) (rate.LinearInvariant A) :=
  ⟨model.accounts, model.rates, model.invariants.val⟩

def decodeBuildResult (result : core.result.Result (economy.Economy AccountId A RateId Role Std.U64)
    (economy.ModelBuildError AccountId RateId)) :
    Except (List (Issue AccountId RateId))
      (Model AccountId (account.Account A Std.U64) RateId (rate.Rate Role A Std.U64) (rate.LinearInvariant A)) :=
  match result with
  | .Ok model => .ok (decodeBuilt model)
  | .Err error => .error (error.issues.val.map decodeIssue)

include hAccount in
omit [DecidableEq RateId] in
theorem build_refines (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (valid : (decode builder).Valid) :
    ∃ result, model_build accountEq accountHash rateEq rateHash builder = .ok result ∧
      decodeBuildResult result = (decode builder).build := by
  by_cases clean : builder.issues.val = []
  · refine ⟨.Ok ⟨builder.accounts, builder.rates, builder.invariants⟩, ?_, ?_⟩
    · simp [model_build, economy.EconomyBuilder.build, alloc.vec.Vec.is_empty, clean,
        indexmap.map.IndexMapKVRandomState.new,
        indexmap.map.IndexMap.Insts.CoreIterTraitsCollectIntoIteratorPairIntoIter.into_iter,
        build_loop_refines accountEq accountHash hAccount builder.accounts [] (by simpa [decode] using valid.1),
        alloc.sync.Arc.new]
    · simp [decodeBuildResult, decodeBuilt, decode, Builder.build, clean]
  · refine ⟨.Err ⟨builder.issues⟩, ?_, ?_⟩
    · simp [model_build, economy.EconomyBuilder.build, alloc.vec.Vec.is_empty, clean]
    · simp [decodeBuildResult, decode, Builder.build, clean]

omit [DecidableEq AccountId] [DecidableEq RateId] in
theorem new_establishes_valid :
    ∃ result, model_new A Role accountEq accountHash rateEq rateHash = .ok result ∧ (decode result).Valid := by
  obtain ⟨result, called, decoded⟩ := new_refines (A := A) (Role := Role) accountEq accountHash rateEq rateHash
  refine ⟨result, called, ?_⟩
  rw [decoded]
  exact Builder.empty_valid

omit [DecidableEq AccountId] [DecidableEq RateId] in
theorem invariant_preserves_valid (builder : economy.EconomyBuilder AccountId A RateId Role Std.U64)
    (valid : (decode builder).Valid) (invariant : rate.LinearInvariant A)
    (capacity : builder.invariants.val.length < Usize.max) :
    ∃ result, model_invariant accountEq accountHash rateEq rateHash builder invariant = .ok result ∧
      (decode result).Valid := by
  obtain ⟨result, called, decoded⟩ := invariant_refines accountEq accountHash rateEq rateHash builder invariant capacity
  refine ⟨result, called, ?_⟩
  rw [decoded]
  exact addInvariant_valid (decode builder) valid invariant

end Construction
end Axionomy.ModelConstructionRefinement
