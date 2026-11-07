import Axionomy.IteratorContract
import Axionomy.ExecutionRefinement

namespace Axionomy.TraceExtensionRefinement
open Aeneas Aeneas.Std AccountExtracted IteratorContract

variable {RateId Role AccountId N Source Iter : Type}
variable (into : core.iter.traits.collect.IntoIterator Source (exchange.Exchange RateId Role AccountId N) Iter)

theorem extend_loop (iter : Iter) (values : List (exchange.Exchange RateId Role AccountId N))
    (yields : Yields into.iteratorInst.next iter values)
    (trace : exchange.Trace RateId Role AccountId N)
    (capacity : trace.exchanges.val.length + values.length ≤ Usize.max) :
    ∃ result, exchange.Trace.extend_loop into iter trace = .ok result ∧
      result.exchanges.val = trace.exchanges.val ++ values := by
  induction yields generalizing trace with
  | done called =>
    refine ⟨trace, ?_, by simp⟩
    unfold exchange.Trace.extend_loop
    rw [loop.eq_1]
    simp [exchange.Trace.extend_loop.body, called]
  | @step state after value rest called tail ih =>
    obtain ⟨grown, pushed, entries⟩ := ModelConstructionRefinement.push_call trace.exchanges value
      (by simp at capacity; omega)
    obtain ⟨result, executed, specified⟩ := ih ⟨grown⟩ (by simp [entries] at *; omega)
    refine ⟨result, ?_, ?_⟩
    · unfold exchange.Trace.extend_loop
      rw [loop.eq_1]
      simp only [exchange.Trace.extend_loop.body, called, bind_ok, Aeneas.Std.uncurry, pushed]
      exact executed
    · simpa [entries, List.append_assoc] using specified

theorem extend_refines (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter)
    (values : List (exchange.Exchange RateId Role AccountId N))
    (yields : Yields into.iteratorInst.next iter values)
    (trace : exchange.Trace RateId Role AccountId N)
    (capacity : trace.exchanges.val.length + values.length ≤ Usize.max) :
    ∃ result, exchange.Trace.extend into trace source = .ok result ∧
      result.exchanges.val = trace.exchanges.val ++ values := by
  obtain ⟨result, called, specified⟩ := extend_loop into iter values yields trace capacity
  exact ⟨result, by simp [exchange.Trace.extend, started, called], specified⟩

theorem extend_loop_failure (error : Error) (iter : Iter)
    (values : List (exchange.Exchange RateId Role AccountId N))
    (fails : FailsAfter into.iteratorInst.next error iter values)
    (trace : exchange.Trace RateId Role AccountId N)
    (capacity : trace.exchanges.val.length + values.length ≤ Usize.max) :
    exchange.Trace.extend_loop into iter trace = .fail error := by
  induction fails generalizing trace with
  | stopped called =>
    unfold exchange.Trace.extend_loop
    rw [loop.eq_1]
    simp [exchange.Trace.extend_loop.body, called]
  | @step state after value rest called tail ih =>
    obtain ⟨grown, pushed, entries⟩ := ModelConstructionRefinement.push_call trace.exchanges value
      (by simp at capacity; omega)
    have executed := ih ⟨grown⟩ (by simp [entries] at *; omega)
    unfold exchange.Trace.extend_loop
    rw [loop.eq_1]
    simp only [exchange.Trace.extend_loop.body, called, bind_ok, Aeneas.Std.uncurry, pushed]
    exact executed

theorem extend_failure (error : Error) (source : Source) (iter : Iter)
    (started : into.into_iter source = .ok iter)
    (values : List (exchange.Exchange RateId Role AccountId N))
    (fails : FailsAfter into.iteratorInst.next error iter values)
    (trace : exchange.Trace RateId Role AccountId N)
    (capacity : trace.exchanges.val.length + values.length ≤ Usize.max) :
    exchange.Trace.extend into trace source = .fail error := by
  simp [exchange.Trace.extend, started, extend_loop_failure into error iter values fails trace capacity]

theorem extend_start_failure (error : Error) (source : Source)
    (failed : into.into_iter source = .fail error)
    (trace : exchange.Trace RateId Role AccountId N) :
    exchange.Trace.extend into trace source = .fail error := by
  simp [exchange.Trace.extend, failed]

end Axionomy.TraceExtensionRefinement
