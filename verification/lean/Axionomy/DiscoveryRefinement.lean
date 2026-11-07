import Axionomy.Discovery
import Axionomy.EvaluationRefinement
import Axionomy.DiagnosticRefinement
import Axionomy.ModelConstructionRefinement

namespace Axionomy.DiscoveryRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement EvaluationRefinement

section Roles
variable {Role A : Type} (ordInst : core.cmp.Ord Role) (cmp : Role → Role → Ordering)
variable (lawful : RateConstructionRefinement.LawfulOrdering ordInst cmp)

include lawful in
theorem roles_refines (expression : rate.QuantityExpression Role A Std.U64) (accumulated : List Role) :
    rate.QuantityExpression.collect_roles ordInst expression accumulated =
      .ok (Discovery.registerAll cmp (Discovery.roles (decodeExpression expression)) accumulated) := by
  induction expression generalizing accumulated with
  | Constant value => rw [rate.QuantityExpression.collect_roles.eq_def]; rfl
  | Units => rw [rate.QuantityExpression.collect_roles.eq_def]; rfl
  | Parameter parameter => rw [rate.QuantityExpression.collect_roles.eq_def]; rfl
  | Balance role asset =>
    have shared : RateConstructionRefinement.LawfulOrdering (Shared0A.Insts.CoreCmpOrd ordInst) cmp := by
      intro l r
      simpa [Shared0A.Insts.CoreCmpOrd.cmp] using lawful l r
    obtain ⟨inserted, exactInsert⟩ := RateConstructionRefinement.register_exact _ cmp shared role accumulated
    rw [rate.QuantityExpression.collect_roles.eq_def]
    simp [alloc.collections.btree.set.BTreeSet.insert, exactInsert,
      Discovery.registerAll, Discovery.roles, decodeExpression]
  | Add left right ihl ihr | Subtract left right ihl ihr | Multiply left right ihl ihr
  | DivideFloor left right ihl ihr | Minimum left right ihl ihr | Maximum left right ihl ihr =>
    rw [rate.QuantityExpression.collect_roles.eq_def]
    simp [ihl, ihr, decodeExpression, Discovery.roles, Discovery.registerAll_append]

include lawful in
theorem roles_complete (identity : ∀ x y, cmp x y = .eq → x = y)
    (expression : rate.QuantityExpression Role A Std.U64) (accumulated : List Role) :
    ∃ result, rate.QuantityExpression.collect_roles ordInst expression accumulated = .ok result ∧
      ∀ role, role ∈ result ↔ role ∈ Discovery.roles (decodeExpression expression) ∨ role ∈ accumulated := by
  refine ⟨_, roles_refines ordInst cmp lawful expression accumulated, ?_⟩
  exact fun role => Discovery.registerAll_mem cmp identity role _ _

end Roles
theorem assets_refines {Role A : Type} (expression : rate.QuantityExpression Role A Std.U64)
    (accumulated : alloc.vec.Vec A)
    (capacity : accumulated.val.length + (Discovery.assets (decodeExpression expression)).length ≤ Usize.max) :
    ∃ result, rate.QuantityExpression.collect_assets expression accumulated = .ok result ∧
      result.val = accumulated.val ++ Discovery.assets (decodeExpression expression) := by
  induction expression generalizing accumulated with
  | Constant value =>
    refine ⟨accumulated, ?_, by simp [Discovery.assets, decodeExpression]⟩
    simp only [rate.QuantityExpression.collect_assets.eq_def]
  | Units =>
    refine ⟨accumulated, ?_, by simp [Discovery.assets, decodeExpression]⟩
    simp only [rate.QuantityExpression.collect_assets.eq_def]
  | Parameter parameter =>
    refine ⟨accumulated, ?_, by simp [Discovery.assets, decodeExpression]⟩
    simp only [rate.QuantityExpression.collect_assets.eq_def]
  | Balance role asset =>
    obtain ⟨result, pushed, entries⟩ := ModelConstructionRefinement.push_call accumulated asset (by
      simp only [decodeExpression, Discovery.assets, List.length_singleton] at capacity
      omega)
    refine ⟨result, ?_, entries⟩
    rw [rate.QuantityExpression.collect_assets.eq_def]
    exact pushed
  | Add left right ihl ihr | Subtract left right ihl ihr | Multiply left right ihl ihr
  | DivideFloor left right ihl ihr | Minimum left right ihl ihr | Maximum left right ihl ihr =>
    simp only [decodeExpression, Discovery.assets, List.length_append] at capacity
    obtain ⟨middle, leftCall, middleEntries⟩ := ihl accumulated (by omega)
    obtain ⟨result, rightCall, resultEntries⟩ := ihr middle (by rw [middleEntries]; simp only [List.length_append]; omega)
    refine ⟨result, ?_, ?_⟩
    · rw [rate.QuantityExpression.collect_assets.eq_def]
      simp [leftCall, rightCall]
    · simp [resultEntries, middleEntries, decodeExpression, Discovery.assets, List.append_assoc]

def Strings (logical : List String) (physical : List Str) : Prop :=
  List.Forall₂ (fun text bytes => decodeString bytes = .ok text) logical physical

theorem insert_string (query : String) (encoded : Str) (decoded : decodeString encoded = .ok query)
    (logical : List String) (physical : List Str) (related : Strings logical physical) :
    ∃ inserted result, sortedInsert Str.Insts.CoreCmpOrd.cmp encoded physical = .ok (inserted, result) ∧
      Strings (RateConstruction.register compare query logical) result := by
  induction related with
  | nil => exact ⟨true, [encoded], rfl, .cons decoded .nil⟩
  | @cons text bytes names data head tail ih =>
    cases order : compare query text with
    | lt =>
      refine ⟨true, encoded :: bytes :: data, ?_, ?_⟩
      · simp [sortedInsert, Str.Insts.CoreCmpOrd.cmp, decoded, head, order]
      · simp only [RateConstruction.register, order]
        exact .cons decoded (.cons head tail)
    | eq =>
      refine ⟨false, bytes :: data, ?_, ?_⟩
      · simp [sortedInsert, Str.Insts.CoreCmpOrd.cmp, decoded, head, order]
      · simp only [RateConstruction.register, order]
        exact .cons head tail
    | gt =>
      obtain ⟨inserted, result, called, agreement⟩ := ih
      refine ⟨inserted, bytes :: result, ?_, ?_⟩
      · simp [sortedInsert, Str.Insts.CoreCmpOrd.cmp, decoded, head, order, called]
      · simp only [RateConstruction.register, order]
        exact .cons head agreement

def NamesFit {Role A : Type} (expression : rate.QuantityExpression Role A Std.U64) : Prop :=
  ∀ parameter ∈ Discovery.parameters (decodeExpression expression), parameter.toByteArray.size ≤ Usize.max

theorem parameters_refines {Role A : Type} (expression : rate.QuantityExpression Role A Std.U64)
    (logical : List String) (physical : List Str) (related : Strings logical physical)
    (fits : NamesFit expression) :
    ∃ result, rate.QuantityExpression.collect_parameters expression physical = .ok result ∧
      Strings (Discovery.registerAll compare (Discovery.parameters (decodeExpression expression)) logical) result := by
  induction expression generalizing logical physical with
  | Constant value =>
    refine ⟨physical, ?_, related⟩
    simp only [rate.QuantityExpression.collect_parameters.eq_def]
  | Units =>
    refine ⟨physical, ?_, related⟩
    simp only [rate.QuantityExpression.collect_parameters.eq_def]
  | Balance role asset =>
    refine ⟨physical, ?_, related⟩
    simp only [rate.QuantityExpression.collect_parameters.eq_def]
  | Parameter parameter =>
    have bound := fits parameter (by simp [decodeExpression, Discovery.parameters])
    obtain ⟨inserted, result, called, agreement⟩ := insert_string parameter (encodeString parameter bound)
      (DiagnosticRefinement.decode_encode parameter bound) logical physical related
    refine ⟨result, ?_, agreement⟩
    rw [rate.QuantityExpression.collect_parameters.eq_def]
    simp [DiagnosticRefinement.deref_exact parameter bound, alloc.collections.btree.set.BTreeSet.insert,
      Shared0A.Insts.CoreCmpOrd.cmp, called]
  | Add left right ihl ihr | Subtract left right ihl ihr | Multiply left right ihl ihr
  | DivideFloor left right ihl ihr | Minimum left right ihl ihr | Maximum left right ihl ihr =>
    have fitsLeft : NamesFit left := by
      intro parameter member
      apply fits parameter
      simp only [decodeExpression, Discovery.parameters, List.mem_append]
      exact Or.inl member
    have fitsRight : NamesFit right := by
      intro parameter member
      apply fits parameter
      simp only [decodeExpression, Discovery.parameters, List.mem_append]
      exact Or.inr member
    obtain ⟨middle, leftCall, middleRelated⟩ := ihl logical physical related fitsLeft
    obtain ⟨result, rightCall, resultRelated⟩ := ihr _ middle middleRelated fitsRight
    refine ⟨result, ?_, ?_⟩
    · rw [rate.QuantityExpression.collect_parameters.eq_def]
      simp [leftCall, rightCall]
    · simpa only [decodeExpression, Discovery.parameters, Discovery.registerAll_append] using resultRelated

end Axionomy.DiscoveryRefinement
