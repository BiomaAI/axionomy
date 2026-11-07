import Axionomy.EvaluationRefinement

namespace Axionomy.DiagnosticRefinement
open Aeneas Aeneas.Std AccountExtracted EvaluationRefinement

-- Relate the explicit UTF-8 storage model to Lean's valid mathematical strings.
theorem decode_bytes (text : String) (value : Str)
    (bytes : (value.val.map fun byte => UInt8.ofBitVec byte.bv).toArray = text.toByteArray.data) :
    decodeString value = .ok text := by
  unfold decodeString
  rw [bytes]
  change (match String.fromUTF8? text.toByteArray with
    | some string => Result.ok string | none => Result.fail Error.panic) = .ok text
  simp [String.fromUTF8?, String.fromUTF8, text.isValidUTF8]

@[simp] theorem decode_encode (text : String) (bound : text.toByteArray.size ≤ Usize.max) :
    decodeString (encodeString text bound) = .ok text := by
  simp [decodeString, encodeString, Slice.from_val, List.map_map, Function.comp_def,
    String.fromUTF8?, String.fromUTF8, text.isValidUTF8]

theorem deref_exact (text : String) (bound : text.toByteArray.size ≤ Usize.max) :
    alloc.string.String.Insts.CoreOpsDerefDerefStr.deref text = .ok (encodeString text bound) := by
  simp only [alloc.string.String.Insts.CoreOpsDerefDerefStr.deref, dite_eq_left bound]

theorem message_refines (error : Evaluation.Error)
    (fits : error.message.toByteArray.size ≤ Usize.max) :
    expression_message (encodeError error) = .ok error.message := by
  cases error with
  | missingParameter parameter =>
    have prefixBytes : decodeString (toStr "missing parameter `") = .ok "missing parameter `" :=
      decode_bytes _ _ (by decide +kernel)
    have suffix : decodeString (toStr "`") = .ok "`" :=
      decode_bytes _ _ (by decide +kernel)
    have hParam : parameter.toByteArray.size ≤ Usize.max := by
      simp only [Evaluation.Error.message, String.size_toByteArray, String.utf8ByteSize_append] at fits ⊢
      omega
    have hPrefix : ("missing parameter `" ++ parameter).toByteArray.size ≤ Usize.max := by
      simp only [Evaluation.Error.message, String.size_toByteArray, String.utf8ByteSize_append] at fits ⊢
      omega
    simp only [Evaluation.Error.message, String.size_toByteArray, String.utf8ByteSize_append] at hPrefix fits
    simp [expression_message, encodeError, economy.ExpressionError.into_message,
      alloc.string.String.Insts.CoreConvertFromShared0Str.from, prefixBytes,
      deref_exact parameter hParam, alloc.string.String.push_str, suffix, hPrefix,
      Evaluation.Error.message, fits]
  | unboundRole =>
    exact decode_bytes _ _ (by decide +kernel)
  | missingAccount =>
    exact decode_bytes _ _ (by decide +kernel)
  | additionOverflow =>
    exact decode_bytes _ _ (by decide +kernel)
  | subtractionUnderflow =>
    exact decode_bytes _ _ (by decide +kernel)
  | multiplicationOverflow =>
    exact decode_bytes _ _ (by decide +kernel)
  | divisionByZero =>
    exact decode_bytes _ _ (by decide +kernel)

end Axionomy.DiagnosticRefinement
