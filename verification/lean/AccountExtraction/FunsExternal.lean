import AccountExtraction.Types
import BasketExtraction.FunsExternal

namespace Axionomy.AccountExtracted
open Aeneas Aeneas.Std
set_option linter.dupNamespace false

-- Widening a u64 to i128 is exact. Formatting and hash internals still fail
-- closed; logical collection operations do not execute those paths.
def widenU64 (value : Std.U64) : Std.I128 :=
  IScalar.ofIntCore (Int.ofNat value.val) (by
    have h := value.hBounds
    simp only [UScalarTy.numBits] at h
    norm_num [IScalarTy.numBits]
    omega)
def I128.Insts.CoreConvertFromU64.from (value : Std.U64) : Result Std.I128 := .ok (widenU64 value)
def I128.Insts.CoreFmtDisplay.fmt (_ : Std.I128) (_ : core.fmt.Formatter) :
    Result (core.result.Result Unit core.fmt.Error × core.fmt.Formatter) := .fail .panic
def U64.Insts.CoreHashHash.hash {H : Type} (_ : core.hash.Hasher H)
    (_ : Std.U64) (_ : H) : Result H := .fail .panic

def U64.Insts.CoreOpsArithAddU64U64.add := @UScalar.add .U64
def U64.Insts.CoreOpsArithSubU64U64.sub := @UScalar.sub .U64
def U64.Insts.CoreOpsArithMulU64U64.mul := @UScalar.mul .U64
def U64.Insts.CoreOpsArithDivU64U64.div := @UScalar.div .U64
def I128.Insts.CoreOpsArithAddI128I128.add := @IScalar.add .I128

def core.option.Option.unwrap_or_default {T : Type} (inst : core.default.Default T) :
    Option T → Result T
  | none => inst.default
  | some value => .ok value

def core.option.OptionShared0T.cloned {T : Type} (inst : core.clone.Clone T) :
    Option T → Result (Option T)
  | none => .ok none
  | some value => do
    let copied ← inst.clone value
    .ok (some copied)

-- Option equality invokes the supplied value comparison, preserving failures.
def core.option.Option.Insts.CoreCmpPartialEqOption.eq {T : Type}
    (inst : core.cmp.PartialEq T T) : Option T → Option T → Result Bool
  | none, none => .ok true
  | some left, some right => inst.eq left right
  | _, _ => .ok false

def core.option.Option.Insts.CoreOpsTry_traitTry.branch {T : Type} :
    Option T → Result (core.ops.control_flow.ControlFlow (Option Never) T)
  | some value => .ok (.Continue value)
  | none => .ok (.Break none)

def core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionNever.from_residual
    (T : Type) : Option Never → Result (Option T)
  | none => .ok none
  | some impossible => nomatch impossible

def std.hash.random.RandomState.Insts.CoreCloneClone.clone
    (state : std.hash.random.RandomState) : Result std.hash.random.RandomState := .ok state

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

def indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next
    {K V : Type} : indexmap.map.iter.Iter K V → Result (Option (K × V) × indexmap.map.iter.Iter K V)
  | [] => .ok (none, [])
  | entry :: rest => .ok (some entry, rest)

def cloneEntries {K V : Type} (keyInst : core.clone.Clone K) (valueInst : core.clone.Clone V) :
    List (K × V) → Result (List (K × V))
  | [] => .ok []
  | (key, value) :: rest => do
    let key ← keyInst.clone key
    let value ← valueInst.clone value
    let rest ← cloneEntries keyInst valueInst rest
    .ok ((key, value) :: rest)

def indexmap.map.IndexMap.Insts.CoreCloneClone.clone {K V S : Type}
    (keyInst : core.clone.Clone K) (valueInst : core.clone.Clone V) (_ : core.clone.Clone S)
    (entries : indexmap.map.IndexMap K V S) : Result (indexmap.map.IndexMap K V S) :=
  cloneEntries keyInst valueInst entries

def indexmap.map.IndexMapKVRandomState.new (K V : Type) :
    Result (indexmap.map.IndexMap K V std.hash.random.RandomState) := .ok []
def indexmap.map.IndexMap.is_empty {K V S : Type}
    (entries : indexmap.map.IndexMap K V S) : Result Bool := .ok entries.isEmpty

-- Length is defined only for lists representable by a Rust collection.
def indexmap.map.IndexMap.len {K V S : Type}
    (entries : indexmap.map.IndexMap K V S) : Result Std.Usize :=
  if bound : entries.length < 2^UScalarTy.Usize.numBits then .ok (Std.Usize.ofNatCore entries.length bound)
  else .fail .panic

def indexmap.map.IndexMap.iter {K V S : Type}
    (entries : indexmap.map.IndexMap K V S) : Result (indexmap.map.iter.Iter K V) := .ok entries

def indexmap.map.IndexMap.insert {K V S H : Type}
    (_ : core.hash.Hash K) (eqInst : core.cmp.Eq K) (_ : core.hash.BuildHasher S H)
    (entries : indexmap.map.IndexMap K V S) (key : K) (value : V) :
    Result (Option V × indexmap.map.IndexMap K V S) :=
  BasketExtracted.mapInsert eqInst.partialEqInst.eq key value entries

def mapGet {K V Q : Type} (eq : Q → K → Result Bool) (query : Q) : List (K × V) → Result (Option V)
  | [] => .ok none
  | (key, value) :: rest => do
    let equal ← eq query key
    if equal then .ok (some value) else mapGet eq query rest

def mapRemove {K V Q : Type} (eq : Q → K → Result Bool) (query : Q) :
    List (K × V) → Result (Option V × List (K × V))
  | [] => .ok (none, [])
  | (key, value) :: rest => do
    let equal ← eq query key
    if equal then .ok (some value, rest)
    else
      let (old, updated) ← mapRemove eq query rest
      .ok (old, (key, value) :: updated)

def indexmap.map.IndexMap.get {K V S Q H : Type}
    (_ : core.hash.BuildHasher S H) (_ : core.hash.Hash Q) (eq : equivalent.Equivalent Q K)
    (entries : indexmap.map.IndexMap K V S) (query : Q) : Result (Option V) :=
  mapGet eq.equivalent query entries

def indexmap.map.IndexMap.shift_remove {K V S Q H : Type}
    (_ : core.hash.BuildHasher S H) (_ : core.hash.Hash Q) (eq : equivalent.Equivalent Q K)
    (entries : indexmap.map.IndexMap K V S) (query : Q) : Result (Option V × indexmap.map.IndexMap K V S) :=
  mapRemove eq.equivalent query entries

def treePrepend {K V Alloc : Type} (entry : K × V) :
    alloc.collections.btree.map.entry.Entry K V Alloc → alloc.collections.btree.map.entry.Entry K V Alloc
  | .Vacant position => .Vacant { position with before := entry :: position.before }
  | .Occupied position => .Occupied { position with before := entry :: position.before }

def treeLocate {K V Alloc : Type} (cmp : K → K → Result Ordering) (query : K) :
    List (K × V) → Result (alloc.collections.btree.map.entry.Entry K V Alloc)
  | [] => .ok (.Vacant ⟨[], query, []⟩)
  | (key, value) :: rest => do
    let order ← cmp query key
    match order with
    | .lt => .ok (.Vacant ⟨[], query, (key, value) :: rest⟩)
    | .eq => .ok (.Occupied ⟨[], key, value, rest⟩)
    | .gt =>
      let position ← treeLocate cmp query rest
      .ok (treePrepend (key, value) position)

def treeRestore {K V Alloc : Type} : alloc.collections.btree.map.entry.Entry K V Alloc → List (K × V)
  | .Vacant position => position.before ++ position.after
  | .Occupied position => position.before ++ (position.key, position.value) :: position.after

def alloc.collections.btree.map.BTreeMap.entry {K V Alloc : Type}
    (_ : core.alloc.AllocatorClone Alloc) (ordInst : core.cmp.Ord K)
    (entries : alloc.collections.btree.map.BTreeMap K V Alloc) (key : K) :
    Result (alloc.collections.btree.map.entry.Entry K V Alloc ×
      (alloc.collections.btree.map.entry.Entry K V Alloc → alloc.collections.btree.map.BTreeMap K V Alloc)) := do
  let position ← treeLocate ordInst.cmp key entries
  .ok (position, treeRestore)

def alloc.collections.btree.map.entry.Entry.or_default {K V Alloc : Type}
    (_ : core.cmp.Ord K) (defaultInst : core.default.Default V) (_ : core.alloc.AllocatorClone Alloc)
    (position : alloc.collections.btree.map.entry.Entry K V Alloc) :
    Result (V × (V → alloc.collections.btree.map.entry.Entry K V Alloc)) := do
  match position with
  | .Occupied entry => .ok (entry.value, fun value => .Occupied { entry with value })
  | .Vacant entry =>
    let value ← defaultInst.default
    .ok (value, fun updated => .Occupied ⟨entry.before, entry.key, updated, entry.after⟩)

def sortedInsert {K : Type} (cmp : K → K → Result Ordering) (value : K) :
    List K → Result (Bool × List K)
  | [] => .ok (true, [value])
  | key :: rest => do
    let order ← cmp value key
    match order with
    | .lt => .ok (true, value :: key :: rest)
    | .eq => .ok (false, key :: rest)
    | .gt =>
      let (inserted, updated) ← sortedInsert cmp value rest
      .ok (inserted, key :: updated)

def alloc.collections.btree.set.BTreeSet.insert {K Alloc : Type}
    (_ : core.alloc.AllocatorClone Alloc) (ordInst : core.cmp.Ord K)
    (entries : alloc.collections.btree.set.BTreeSet K Alloc) (value : K) :
    Result (Bool × alloc.collections.btree.set.BTreeSet K Alloc) :=
  sortedInsert ordInst.cmp value entries

end Axionomy.AccountExtracted
