import BasketExtraction.Types

namespace Axionomy.BasketExtracted
open Aeneas Aeneas.Std
set_option linter.dupNamespace false

-- Primitive addition is carried by Zero's parent trait, but never called by
-- construction. This is the same checked primitive used by the quantity proof.
def U64.Insts.CoreOpsArithAddU64U64.add := @UScalar.add .U64

-- The logical map does not inspect hash seeds or call these operations. Fail
-- closed if extracted code ever starts observing hash internals. No theorem
-- claims to model DefaultHasher or prove the physical IndexMap implementation.
def std.hash.random.DefaultHasher.Insts.CoreHashHasher.finish
    (_ : std.hash.random.DefaultHasher) : Result Std.U64 := .fail .panic
def std.hash.random.DefaultHasher.Insts.CoreHashHasher.write
    (_ : std.hash.random.DefaultHasher) (_ : Slice Std.U8) :
    Result std.hash.random.DefaultHasher := .fail .panic
def std.hash.random.RandomState.Insts.CoreHashBuildHasherDefaultHasher.build_hasher
    (_ : std.hash.random.RandomState) : Result std.hash.random.DefaultHasher := .fail .panic

def equivalent.Equivalent.Blanket.equivalent {Q K : Type}
    (eqInst : core.cmp.Eq Q) (borrowInst : core.borrow.Borrow K Q)
    (query : Q) (key : K) : Result Bool := do
  let borrowed ← borrowInst.borrow key
  eqInst.partialEqInst.eq query borrowed

def indexmap.map.IndexMapKVRandomState.new (K V : Type) :
    Result (indexmap.map.IndexMap K V std.hash.random.RandomState) := .ok []

def mapContains {K V Q : Type} (eq : Q → K → Result Bool)
    (query : Q) : List (K × V) → Result Bool
  | [] => .ok false
  | (key, _) :: rest => do
    let equal ← eq query key
    if equal then .ok true else mapContains eq query rest

def mapInsert {K V : Type} (eq : K → K → Result Bool)
    (key : K) (value : V) : List (K × V) → Result (Option V × List (K × V))
  | [] => .ok (none, [(key, value)])
  | (oldKey, oldValue) :: rest => do
    let equal ← eq key oldKey
    if equal then .ok (some oldValue, (oldKey, value) :: rest)
    else
      let (previous, updated) ← mapInsert eq key value rest
      .ok (previous, (oldKey, oldValue) :: updated)

def indexmap.map.IndexMap.contains_key {K V S Q H : Type}
    (_ : core.hash.BuildHasher S H) (_ : core.hash.Hash Q)
    (equiv : equivalent.Equivalent Q K)
    (entries : indexmap.map.IndexMap K V S) (query : Q) : Result Bool :=
  mapContains equiv.equivalent query entries

def indexmap.map.IndexMap.insert {K V S H : Type}
    (_ : core.hash.Hash K) (eqInst : core.cmp.Eq K) (_ : core.hash.BuildHasher S H)
    (entries : indexmap.map.IndexMap K V S) (key : K) (value : V) :
    Result (Option V × indexmap.map.IndexMap K V S) :=
  mapInsert eqInst.partialEqInst.eq key value entries

-- Model the shared-reference retain adapter by invoking its EXTRACTED predicate
-- in insertion order. It preserves values and propagates callback failure.
def retainValues {K V F : Type} (keep : F → V → Result (Bool × F))
    (state : F) : List (K × V) → Result (List (K × V))
  | [] => .ok []
  | (key, value) :: rest => do
    let (retain, state) ← keep state value
    let tail ← retainValues keep state rest
    .ok (if retain then (key, value) :: tail else tail)

def basket.retain_quantities {A N F : Type}
    (predicate : core.ops.function.FnMut F (quantity.Quantity N) Bool)
    (entries : indexmap.map.IndexMap A (quantity.Quantity N) std.hash.random.RandomState)
    (state : F) :
    Result (indexmap.map.IndexMap A (quantity.Quantity N) std.hash.random.RandomState) :=
  retainValues predicate.call_mut state entries

end Axionomy.BasketExtracted
