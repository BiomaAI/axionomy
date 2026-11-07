import Axionomy.ApplyRefinement
import Axionomy.TransactionExecution

namespace Axionomy.ExecutionRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement

variable {AccountId A RateId Role N : Type}

theorem economy_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role) (numberClone : core.clone.Clone N)
    (lawful : LawfulClone accountClone) (model : economy.Economy AccountId A RateId Role N) :
    economy.Economy.Insts.CoreCloneClone.clone accountClone assetClone rateClone roleClone numberClone model = .ok model := by
  have arcLaw : LawfulClone (alloc.sync.Arc.Insts.CoreCloneClone (account.Account A N)
      alloc.alloc.Global.Insts.CoreAllocAllocatorClone) := fun _ => rfl
  simp [economy.Economy.Insts.CoreCloneClone.clone, indexmap.map.IndexMap.Insts.CoreCloneClone.clone,
    clone_exact accountClone _ lawful arcLaw, alloc.sync.Arc.Insts.CoreCloneClone.clone]

theorem proposal_clone (rateClone : core.clone.Clone RateId) (roleClone : core.clone.Clone Role)
    (accountClone : core.clone.Clone AccountId) (numberClone : core.clone.Clone N)
    (hRate : LawfulClone rateClone) (hRole : LawfulClone roleClone)
    (hAccount : LawfulClone accountClone) (hNumber : LawfulClone numberClone)
    (proposal : exchange.Exchange RateId Role AccountId N) :
    exchange.Exchange.Insts.CoreCloneClone.clone rateClone roleClone accountClone numberClone proposal = .ok proposal := by
  have amounts : LawfulClone (quantity.Quantity.Insts.CoreCloneClone numberClone) := by
    intro value
    simp [quantity.Quantity.Insts.CoreCloneClone.clone, hNumber value]
  have strings : LawfulClone alloc.string.String.Insts.CoreCloneClone := fun _ => rfl
  simp [exchange.Exchange.Insts.CoreCloneClone.clone, alloc.collections.btree.map.BTreeMap.Insts.CoreCloneClone.clone,
    hRate proposal.rate, clone_exact roleClone accountClone hRole hAccount,
    clone_exact _ _ strings amounts, quantity.Quantity.Insts.CoreCloneClone.clone, hNumber proposal.units]

theorem trace_new : exchange.Trace.new RateId Role AccountId N = .ok ⟨alloc.vec.Vec.new _⟩ := rfl

theorem trace_default : exchange.Trace.Insts.CoreDefaultDefault.default RateId Role AccountId N =
    .ok ⟨alloc.vec.Vec.new _⟩ := rfl

theorem trace_push (trace : exchange.Trace RateId Role AccountId N)
    (proposal : exchange.Exchange RateId Role AccountId N) (capacity : trace.exchanges.val.length < Usize.max) :
    ∃ result, exchange.Trace.push trace proposal = .ok result ∧
      result.exchanges.val = trace.exchanges.val ++ [proposal] := by
  obtain ⟨grown, called, values⟩ := ModelConstructionRefinement.push_call trace.exchanges proposal capacity
  exact ⟨⟨grown⟩, by simp [exchange.Trace.push, called], values⟩

theorem trace_borrow (trace : exchange.Trace RateId Role AccountId N) :
    exchange.Trace.impl.exchanges trace = .ok (alloc.vec.Vec.deref trace.exchanges) := rfl

theorem trace_consume (trace : exchange.Trace RateId Role AccountId N) :
    exchange.Trace.into_exchanges trace = .ok trace.exchanges := rfl

def ModelFrame (before after : economy.Economy AccountId A RateId Role Std.U64) : Prop :=
  after.rates = before.rates ∧ after.invariants = before.invariants ∧
    after.accounts.map Prod.fst = before.accounts.map Prod.fst

theorem modelFrame_refl (model : economy.Economy AccountId A RateId Role Std.U64) : ModelFrame model model :=
  ⟨rfl, rfl, rfl⟩

theorem modelFrame_trans {before middle after : economy.Economy AccountId A RateId Role Std.U64}
    (first : ModelFrame before middle) (second : ModelFrame middle after) : ModelFrame before after :=
  ⟨second.1.trans first.1, second.2.1.trans first.2.1, second.2.2.trans first.2.2⟩

def simulationOutcome (value : core.result.Result
    (economy.Economy AccountId A RateId Role Std.U64 × exchange.Receipt RateId Role AccountId A Std.U64)
    (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)) :
    Except (Transaction.Failure AccountId A RateId Role)
      (Transaction.Model AccountId A RateId Role × Transaction.Receipt AccountId A RateId Role) :=
  match value with
  | .Ok (model, receipt) => .ok (TransactionDecode.model model, TransactionDecode.receipt receipt)
  | .Err error => .error (TransactionDecode.failure error)

def replayResult (value : core.result.Result (alloc.vec.Vec (exchange.Receipt RateId Role AccountId A Std.U64))
    (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)) :
    Except (Transaction.Failure AccountId A RateId Role) (List (Transaction.Receipt AccountId A RateId Role)) :=
  match value with
  | .Ok receipts => .ok (receipts.val.map TransactionDecode.receipt)
  | .Err error => .error (TransactionDecode.failure error)

def replayOutcome (value : core.result.Result (alloc.vec.Vec (exchange.Receipt RateId Role AccountId A Std.U64))
    (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128) × economy.Economy AccountId A RateId Role Std.U64) :=
  (replayResult value.1, TransactionDecode.model value.2)

section Execution
variable [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role]
variable (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
variable (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
variable (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
variable (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
variable (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
variable (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
variable (hAccountClone : LawfulClone accountClone) (hAccountEq : LawfulEquality accountEq)
variable (hAccountOrd : LawfulOrdering accountOrd cmpAccount) (hAssetClone : LawfulClone assetClone)
variable (hAssetEq : LawfulEquality assetEq) (hRateClone : LawfulClone rateClone) (hRateEq : LawfulEquality rateEq)
variable (hRoleClone : LawfulClone roleClone) (hRoleEq : LawfulEquality roleOrd.eqInst) (hRoleOrd : LawfulOrdering roleOrd cmpRole)


omit [DecidableEq AccountId] [DecidableEq A] [DecidableEq RateId] [DecidableEq Role] in
include hAccountClone in
theorem fork_refines (model : economy.Economy AccountId A RateId Role Std.U64) :
    economy.Economy.fork accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model = .ok model := by
  exact economy_clone accountClone assetClone rateClone roleClone scalar.corecloneCloneInst hAccountClone model

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem simulate_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (proposal : exchange.Exchange RateId Role AccountId Std.U64)
    (parameterFits : ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ simulated, economy.Economy.simulate accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model proposal = .ok simulated ∧
      Execution.simulated (Transaction.apply cmpAccount cmpRole) (TransactionDecode.model model)
        (TransactionDecode.proposal proposal) = some (simulationOutcome simulated) := by
  obtain ⟨⟨result, after⟩, executed, decoded, frame⟩ := ApplyRefinement.apply_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws model proposal parameterFits rulesReady invariantFits accountCapacity
  have forked := fork_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd hAccountClone model
  cases result with
  | Err error =>
    refine ⟨.Err error, ?_, ?_⟩
    · simp [economy.Economy.simulate, forked, executed, Aeneas.Std.uncurry,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
    · simp [Execution.simulated, decoded, ApplyRefinement.outcome, ApplyRefinement.result, simulationOutcome, Except.map]
  | Ok receipt =>
    refine ⟨.Ok (after, receipt), ?_, ?_⟩
    · simp [economy.Economy.simulate, forked, executed, Aeneas.Std.uncurry, core.result.Result.Insts.CoreOpsTry.branch]
    · simp [Execution.simulated, decoded, ApplyRefinement.outcome, ApplyRefinement.result, simulationOutcome, Except.map]

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem replay_loop_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (remaining : List (exchange.Exchange RateId Role AccountId Std.U64))
    (iter : core.slice.iter.Iter (exchange.Exchange RateId Role AccountId Std.U64))
    (suffix : iter.slice.val.drop iter.i = remaining)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (receipts : alloc.vec.Vec (exchange.Receipt RateId Role AccountId A Std.U64))
    (capacity : receipts.val.length + remaining.length ≤ Usize.max)
    (parameterFits : ∀ proposal ∈ remaining, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ remaining, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ replayed, economy.Economy.replay_loop accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar iter model receipts = .ok replayed ∧
      Execution.run (Transaction.apply cmpAccount cmpRole) (TransactionDecode.model model)
        (remaining.map TransactionDecode.proposal) (receipts.val.map TransactionDecode.receipt) = some (replayOutcome replayed) ∧
      ModelFrame model replayed.2 := by
  induction remaining generalizing iter model receipts with
  | nil =>
    refine ⟨(.Ok receipts, model), ?_, rfl, modelFrame_refl model⟩
    unfold economy.Economy.replay_loop
    rw [loop.eq_1]
    simp [economy.Economy.replay_loop.body, ConservationRefinement.next_empty iter suffix]
  | cons proposal rest ih =>
    obtain ⟨advanced, tail⟩ := ConservationRefinement.next_cons iter proposal rest suffix
    have copied := proposal_clone rateClone roleClone accountClone scalar.corecloneCloneInst
      hRateClone hRoleClone hAccountClone (fun _ => rfl) proposal
    obtain ⟨⟨result, after⟩, executed, decoded, frame⟩ := ApplyRefinement.apply_refines accountClone accountEq accountHash accountOrd
      assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
      hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
      laws model proposal (parameterFits proposal (by simp)) (rulesReady proposal (by simp)) invariantFits accountCapacity
    have preserved : ModelFrame model after := ⟨frame.1, frame.2.1, frame.2.2.1⟩
    cases result with
    | Err error =>
      refine ⟨(.Err error, after), ?_, ?_, preserved⟩
      · unfold economy.Economy.replay_loop
        rw [loop.eq_1]
        simp [economy.Economy.replay_loop.body, advanced, copied, executed, Aeneas.Std.uncurry,
          core.result.Result.Insts.CoreOpsTry.branch,
          core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
      · simp [Execution.run, decoded, ApplyRefinement.outcome, ApplyRefinement.result, replayOutcome, replayResult]
    | Ok receipt =>
      obtain ⟨grown, pushed, grows⟩ := ModelConstructionRefinement.push_call receipts receipt (by simp at capacity; omega)
      have nextRules : ∀ proposal ∈ rest, ∀ rule, Account.find? proposal.rate after.rates = some rule → AnalysisRefinement.RuleReady proposal rule := by
        intro next member rule found
        rw [frame.1] at found
        exact rulesReady next (by simp [member]) rule found
      have nextInvariants : ∀ invariant ∈ after.invariants.val, invariant.name.toByteArray.size ≤ Usize.max := by
        rw [frame.2.1]
        exact invariantFits
      have nextCapacity : after.accounts.length ≤ Usize.max := by
        have same := congrArg List.length frame.2.2.1
        simp only [List.length_map] at same
        exact same.le.trans accountCapacity
      obtain ⟨replayed, continued, mapped, finalFrame⟩ := ih { iter with i := iter.i + 1 } tail after grown
        (by simp [grows] at *; omega) (fun next member => parameterFits next (by simp [member])) nextRules nextInvariants nextCapacity
      refine ⟨replayed, ?_, ?_, modelFrame_trans preserved finalFrame⟩
      · unfold economy.Economy.replay_loop
        rw [loop.eq_1]
        simp only [economy.Economy.replay_loop.body, advanced, copied, executed, bind_ok, Aeneas.Std.uncurry,
          core.result.Result.Insts.CoreOpsTry.branch, pushed]
        exact continued
      · simpa [Execution.run, decoded, ApplyRefinement.outcome, ApplyRefinement.result, grows] using mapped

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem replay_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (trace : exchange.Trace RateId Role AccountId Std.U64)
    (parameterFits : ∀ proposal ∈ trace.exchanges.val, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ trace.exchanges.val, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ replayed, economy.Economy.replay accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model trace = .ok replayed ∧
      Execution.run (Transaction.apply cmpAccount cmpRole) (TransactionDecode.model model)
        (trace.exchanges.val.map TransactionDecode.proposal) [] = some (replayOutcome replayed) ∧
      ModelFrame model replayed.2 := by
  obtain ⟨replayed, called, decoded, frame⟩ := replay_loop_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws trace.exchanges.val ⟨alloc.vec.Vec.deref trace.exchanges, 0⟩ (by simp [alloc.vec.Vec.deref]) model
    (alloc.vec.Vec.new _) (by simp [alloc.vec.Vec.new]) parameterFits rulesReady invariantFits accountCapacity
  refine ⟨replayed, ?_, ?_, frame⟩
  · simpa [economy.Economy.replay, exchange.Trace.impl.exchanges,
      SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter, core.slice.Slice.iter] using called
  · simpa [alloc.vec.Vec.new] using decoded

def branchOutcome (value : core.result.Result (economy.Economy AccountId A RateId Role Std.U64)
    (economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)) :
    Except (Transaction.Failure AccountId A RateId Role) (Transaction.Model AccountId A RateId Role) :=
  match value with
  | .Ok after => .ok (TransactionDecode.model after)
  | .Err error => .error (TransactionDecode.failure error)

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem replayed_refines (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (trace : exchange.Trace RateId Role AccountId Std.U64)
    (parameterFits : ∀ proposal ∈ trace.exchanges.val, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ trace.exchanges.val, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max) :
    ∃ branched, economy.Economy.replayed accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model trace = .ok branched ∧
      Execution.replayed (Transaction.apply cmpAccount cmpRole) (TransactionDecode.model model)
        (trace.exchanges.val.map TransactionDecode.proposal) = some (branchOutcome branched) := by
  obtain ⟨⟨result, after⟩, executed, decoded, frame⟩ := replay_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws model trace parameterFits rulesReady invariantFits accountCapacity
  have forked := fork_refines accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd hAccountClone model
  cases result with
  | Err error =>
    refine ⟨.Err error, ?_, ?_⟩
    · simp [economy.Economy.replayed, forked, executed, Aeneas.Std.uncurry,
        core.result.Result.Insts.CoreOpsTry.branch,
        core.result.Result.Insts.CoreOpsTry_traitFromResidualResult.from_residual, core.convert.FromSame.from]
    · simp [Execution.replayed, decoded, replayOutcome, replayResult, branchOutcome, Except.map]
  | Ok receipts =>
    refine ⟨.Ok after, ?_, ?_⟩
    · simp [economy.Economy.replayed, forked, executed, Aeneas.Std.uncurry, core.result.Result.Insts.CoreOpsTry.branch]
    · simp [Execution.replayed, decoded, replayOutcome, replayResult, branchOutcome, Except.map]

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem accepted_replay_contract (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (trace : exchange.Trace RateId Role AccountId Std.U64)
    (parameterFits : ∀ proposal ∈ trace.exchanges.val, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ trace.exchanges.val, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max)
    (receipts : alloc.vec.Vec (exchange.Receipt RateId Role AccountId A Std.U64))
    (after : economy.Economy AccountId A RateId Role Std.U64)
    (success : economy.Economy.replay accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model trace = .ok (.Ok receipts, after)) :
    ModelFrame model after ∧
      (receipts.val.map TransactionDecode.receipt).map Transaction.Receipt.proposal = trace.exchanges.val.map TransactionDecode.proposal ∧
      TransactionExecution.Changes (TransactionDecode.model model) (trace.exchanges.val.map TransactionDecode.proposal)
        (receipts.val.map TransactionDecode.receipt) (TransactionDecode.model after) ∧
      ∀ receipt ∈ receipts.val.map TransactionDecode.receipt, ∀ invariant ∈ (TransactionDecode.model model).invariants,
        TransactionExecution.ReceiptConserves invariant receipt := by
  obtain ⟨replayed, executed, specified, frame⟩ := replay_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws model trace parameterFits rulesReady invariantFits accountCapacity

  have same : replayed = (.Ok receipts, after) := Result.ok_injective (executed.symm.trans success)
  subst replayed
  simp only [replayOutcome, replayResult] at specified
  obtain ⟨produced, same, history⟩ := Execution.success_history _ _ _ _ _ _ specified
  simp only [List.nil_append] at same
  subst produced
  exact ⟨frame, TransactionExecution.replay_receipt_order cmpAccount cmpRole laws _ _ _ _ specified,
    TransactionExecution.accepted_changes cmpAccount cmpRole laws history,
    (TransactionExecution.accepted_conserves cmpAccount cmpRole history).2⟩

include hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd in
theorem rejected_replay_prefix (laws : OrderedEffects.OrderLaws cmpAccount)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (trace : exchange.Trace RateId Role AccountId Std.U64)
    (parameterFits : ∀ proposal ∈ trace.exchanges.val, ValidationParametersRefinement.ParametersFit proposal.parameters)
    (rulesReady : ∀ proposal ∈ trace.exchanges.val, ∀ rule, Account.find? proposal.rate model.rates = some rule → AnalysisRefinement.RuleReady proposal rule)
    (invariantFits : ∀ invariant ∈ model.invariants.val, invariant.name.toByteArray.size ≤ Usize.max)
    (accountCapacity : model.accounts.length ≤ Usize.max)
    (error : economy.ApplyError RateId Role AccountId A Std.U64 Std.I128)
    (after : economy.Economy AccountId A RateId Role Std.U64)
    (failure : economy.Economy.replay accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
      rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model trace = .ok (.Err error, after)) :
    ∃ leading action trailing receipts,
      trace.exchanges.val.map TransactionDecode.proposal = leading ++ action :: trailing ∧
      Execution.run (Transaction.apply cmpAccount cmpRole) (TransactionDecode.model model) leading [] =
        some (.ok receipts, TransactionDecode.model after) ∧
      Transaction.apply cmpAccount cmpRole (TransactionDecode.model after) action =
        some (.error (TransactionDecode.failure error), TransactionDecode.model after) := by
  obtain ⟨replayed, executed, specified, frame⟩ := replay_refines accountClone accountEq accountHash accountOrd
    assetClone assetEq assetHash assetOrd rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpRole
    hAccountClone hAccountEq hAccountOrd hAssetClone hAssetEq hRateClone hRateEq hRoleClone hRoleEq hRoleOrd
    laws model trace parameterFits rulesReady invariantFits accountCapacity

  have same : replayed = (.Err error, after) := Result.ok_injective (executed.symm.trans failure)
  subst replayed
  exact TransactionExecution.replay_error_prefix cmpAccount cmpRole _ _ _ _
    (by simpa only [replayOutcome, replayResult] using specified)

end Execution
end Axionomy.ExecutionRefinement
