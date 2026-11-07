import QuantityExtraction.Types

-- The pinned generator emits these specialized names for MIR arithmetic;
-- the matching backend exports the generic operations. These are aliases.
namespace Aeneas.Std
abbrev U64.add := @UScalar.add .U64
abbrev U64.sub := @UScalar.sub .U64
abbrev U64.div := @UScalar.div .U64
end Aeneas.Std

/-! Models for the three Rust compiler intrinsics exposed by MIR inlining.
These are part of the declared semantic trust boundary, not extracted Rust.
The overflow arithmetic reuses Aeneas's bitvector models. Extraction is restricted
to u64 instantiations; other types deliberately fail rather than acquire a model.
No axiom or assumption of Axionomy's correctness is introduced. -/

namespace Axionomy.Extracted
open Aeneas Aeneas.Std

noncomputable def u64Intrinsic {T : Type}
    (operation : Std.U64 → Std.U64 → Std.U64 × Bool) (left right : T) :
    Result (T × Bool) := by
  classical
  exact if h : T = Std.U64 then
    let result := operation (cast h left) (cast h right)
    Result.ok (cast h.symm result.1, result.2)
  else Result.fail .panic

def core.intrinsics.cold_path : Result Unit := Result.ok ()

noncomputable def core.intrinsics.add_with_overflow {T : Type}
    (_ : Aeneas.Std.core.marker.Copy T) (left right : T) : Result (T × Bool) :=
  u64Intrinsic Std.U64.overflowing_add left right

noncomputable def core.intrinsics.mul_with_overflow {T : Type}
    (_ : Aeneas.Std.core.marker.Copy T) (left right : T) : Result (T × Bool) :=
  u64Intrinsic Std.U64.overflowing_mul left right

end Axionomy.Extracted
