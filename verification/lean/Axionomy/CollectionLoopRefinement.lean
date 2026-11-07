import Axionomy.ModelConstructionRefinement

namespace Axionomy.CollectionLoopRefinement
open Aeneas Aeneas.Std

theorem equal_lists {T : Type} (values : List T) : List.Forall₂ Eq values values := by
  induction values with
  | nil => exact .nil
  | cons value rest ih => exact .cons rfl ih

-- A reusable finite-iteration lemma. Its body obligations are proved from each
-- extracted body; they do not assume that the loop or whole operation succeeds.
theorem collect_refines {T X E F : Type}
    (body : List X → alloc.vec.Vec F → Result (ControlFlow (List X × alloc.vec.Vec F) (alloc.vec.Vec F)))
    (rel : T → X → Prop) (valid : T → Prop) (emit : T → Option E) (encode : E → F)
    (empty : ∀ values, body [] values = .ok (.done values))
    (step : ∀ value actual, rel value actual → valid value → ∀ rest values,
      values.val.length < Usize.max →
      ∃ next, body (actual :: rest) values = .ok (.cont (rest, next)) ∧
        next.val = values.val ++ (emit value).toList.map encode)
    (logical : List T) (physical : List X) (related : List.Forall₂ rel logical physical)
    (validItems : ∀ value ∈ logical, valid value) (values : alloc.vec.Vec F)
    (capacity : values.val.length + logical.length ≤ Usize.max) :
    ∃ result, loop (fun (remaining, accumulated) => body remaining accumulated) (physical, values) = .ok result ∧
      result.val = values.val ++ (logical.filterMap emit).map encode := by
  induction related generalizing values with
  | nil =>
    refine ⟨values, ?_, by simp⟩
    rw [loop.eq_1]
    simp [empty]
  | @cons value actual logical physical relatedHead relatedTail ih =>
    have room : values.val.length < Usize.max := by simp only [List.length_cons] at capacity; omega
    obtain ⟨middle, called, entries⟩ := step value actual relatedHead (validItems value (by simp)) physical values room
    have nextCapacity : middle.val.length + logical.length ≤ Usize.max := by
      rw [entries]
      simp only [List.length_append, List.length_map]
      cases emit value <;> simp at capacity ⊢ <;> omega
    obtain ⟨result, executed, contents⟩ := ih (fun item member => validItems item (by simp [member])) middle nextCapacity
    refine ⟨result, ?_, ?_⟩
    · rw [loop.eq_1]
      simp only [called, bind_ok]
      exact executed
    · rw [contents, entries]
      cases h : emit value <;> simp [h, List.append_assoc]

end Axionomy.CollectionLoopRefinement
