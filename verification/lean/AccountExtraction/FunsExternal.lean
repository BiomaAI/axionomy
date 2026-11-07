import AccountExtraction.Types
import BasketExtraction.FunsExternal

namespace Axionomy.AccountExtracted
open Aeneas Aeneas.Std
set_option linter.dupNamespace false
universe u v

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

-- Logical BTreeMap cloning visits every key/value and retains ordered entries.
-- Physical tree storage, allocation and clone isolation remain dependency models.
def alloc.collections.btree.map.BTreeMap.Insts.CoreCloneClone.clone {K V A : Type}
    (keyInst : core.clone.Clone K) (valueInst : core.clone.Clone V)
    (_ : core.alloc.AllocatorClone A)
    (entries : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.BTreeMap K V A) :=
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

-- Lookup borrows keys and compares the borrowed domain, as BTreeMap::get does.
def treeGet {K V Q : Type} (borrowKey : K → Result Q) (cmp : Q → Q → Result Ordering)
    (query : Q) : List (K × V) → Result (Option V)
  | [] => .ok none
  | (key, value) :: rest => do
    let key ← borrowKey key
    let ordering ← cmp query key
    match ordering with
    | .lt => .ok none
    | .eq => .ok (some value)
    | .gt => treeGet borrowKey cmp query rest

def alloc.collections.btree.map.BTreeMap.get {K V Alloc Q : Type}
    (_ : core.alloc.AllocatorClone Alloc) (borrowInst : core.borrow.Borrow K Q)
    (_ : core.cmp.Ord K) (ordInst : core.cmp.Ord Q)
    (entries : alloc.collections.btree.map.BTreeMap K V Alloc) (query : Q) : Result (Option V) :=
  treeGet borrowInst.borrow ordInst.cmp query entries

def core.option.Option.ok_or_else {T E F : Type} (inst : core.ops.function.FnOnce F Unit E)
    (choice : Option T) (error : F) : Result (core.result.Result T E) :=
  match choice with
  | some value => .ok (.Ok value)
  | none => do
    let error ← inst.call_once error ()
    .ok (.Err error)

def alloc.sync.Arc.Insts.CoreOpsDerefDeref.deref {T : Type} (_ : Type)
    (value : alloc.sync.Arc T) : Result T := .ok value
def alloc.sync.Arc.Insts.CoreConvertAsRef.as_ref {T : Type} (_ : Type)
    (value : alloc.sync.Arc T) : Result T := .ok value

def indexmap.map.IndexMap.values {K V S : Type} (entries : indexmap.map.IndexMap K V S) :
    Result (indexmap.map.iter.Values K V) := .ok (entries.map Prod.snd)
def indexmap.map.iter.Values.Insts.CoreIterTraitsIteratorIteratorSharedAV.next {K V : Type} :
    indexmap.map.iter.Values K V → Result (Option V × indexmap.map.iter.Values K V)
  | [] => .ok (none, [])
  | value :: rest => .ok (some value, rest)

-- Strings are mathematical UTF-8 strings. Malformed logical str values and
-- unrepresentable lengths fail closed; allocation and physical byte storage remain trusted.
def encodeString (value : String) (bound : value.toByteArray.size ≤ Usize.max) : Str :=
  Slice.from (value.toByteArray.data.toList.map fun byte => (⟨byte.toBitVec⟩ : Std.U8))
    (by simpa using bound)

def decodeString (value : Str) : Result String :=
  let bytes : ByteArray := ⟨(value.val.map fun byte => UInt8.ofBitVec byte.bv).toArray⟩
  match String.fromUTF8? bytes with
  | some text => .ok text
  | none => .fail .panic

def Str.Insts.AllocBorrowToOwnedString.to_owned := decodeString

def alloc.string.String.Insts.CoreCmpPartialEqString.eq (left right : String) : Result Bool :=
  .ok (decide (left = right))
def alloc.string.String.Insts.CoreCmpPartialOrdString.partial_cmp (left right : String) :
    Result (Option Ordering) := .ok (some (compare left right))
def alloc.string.String.Insts.CoreCmpEq.assert_fields_are_eq (_ : String) : Result Unit := .ok ()
def alloc.string.String.Insts.CoreCmpOrd.cmp (left right : String) : Result Ordering := .ok (compare left right)
def alloc.string.String.Insts.CoreCloneClone.clone (value : String) : Result String := .ok value

def alloc.string.String.Insts.CoreOpsDerefDerefStr.deref (value : String) : Result Str :=
  if bound : value.toByteArray.size ≤ Usize.max then .ok (encodeString value bound)
  else .fail .panic

def alloc.string.String.Insts.CoreConvertFromShared0Str.from := decodeString

def alloc.string.String.push_str (original : String) (suffix : Str) : Result String := do
  let suffix ← decodeString suffix
  let result := original ++ suffix
  if result.toByteArray.size ≤ Usize.max then .ok result else .fail .panic

-- Constructor/owned-iterator models preserve logical values and traversal order.
-- Physical allocation, reference counts and collection destruction stay trusted.
def alloc.sync.Arc.new {T : Type} (value : T) : Result (alloc.sync.Arc T) := .ok value

def indexmap.map.IndexMap.Insts.CoreIterTraitsCollectIntoIteratorPairIntoIter.into_iter {K V S : Type}
    (entries : indexmap.map.IndexMap K V S) : Result (indexmap.map.iter.IntoIter K V) := .ok entries

def indexmap.map.iter.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next {K V : Type} :
    indexmap.map.iter.IntoIter K V → Result (Option (K × V) × indexmap.map.iter.IntoIter K V)
  | [] => .ok (none, [])
  | entry :: rest => .ok (some entry, rest)

def alloc.collections.btree.map.BTreeMapKVGlobal.new (K V : Type) :
    Result (alloc.collections.btree.map.BTreeMap K V alloc.alloc.Global) := .ok []

def alloc.collections.btree.set.BTreeSetTGlobal.new (T : Type) :
    Result (alloc.collections.btree.set.BTreeSet T alloc.alloc.Global) := .ok []

def alloc.collections.btree.set.BTreeSet.Insts.CoreIterTraitsCollectIntoIteratorTIntoIter.into_iter {T A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.set.BTreeSet T A) :
    Result (alloc.collections.btree.set.IntoIter T A) := .ok entries

def alloc.collections.btree.set.IntoIter.Insts.CoreIterTraitsIteratorIterator.next {T A : Type}
    (_ : core.alloc.AllocatorClone A) : alloc.collections.btree.set.IntoIter T A →
      Result (Option T × alloc.collections.btree.set.IntoIter T A)
  | [] => .ok (none, [])
  | value :: rest => .ok (some value, rest)

def SharedAVec.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter {T : Type} (_ : Type)
    (values : alloc.vec.Vec T) : Result (core.slice.iter.Iter T) := core.slice.Slice.iter values.slice

def Shared0A.Insts.CoreCmpOrd.cmp {A : Type} (ordInst : core.cmp.Ord A) :
    A → A → Result Ordering := ordInst.cmp

def Str.Insts.CoreCmpOrd.cmp (left right : Str) : Result Ordering := do
  let left ← decodeString left
  let right ← decodeString right
  .ok (compare left right)

def Str.Insts.CoreCmpPartialOrdStr.partial_cmp (left right : Str) : Result (Option Ordering) := do
  let result ← Str.Insts.CoreCmpOrd.cmp left right
  .ok (some result)

def Str.Insts.CoreCmpPartialEqStr.eq (left right : Str) : Result Bool := do
  let left ← decodeString left
  let right ← decodeString right
  .ok (decide (left = right))

-- Tuple comparison follows the pinned core/tuple.rs lexical_cmp and
-- lexical_partial_cmp macros; equality short-circuits in component order.
def Pair.Insts.CoreCmpOrd.cmp {U : Type u} {T : Type v} (first : core.cmp.Ord U) (second : core.cmp.Ord T)
    (left right : U × T) : Result Ordering := do
  let order ← first.cmp left.1 right.1
  match order with
  | .eq => second.cmp left.2 right.2
  | result => .ok result

def Pair.Insts.CoreCmpPartialOrdPair.partial_cmp {U : Type u} {T : Type v}
    (first : core.cmp.PartialOrd U U) (second : core.cmp.PartialOrd T T)
    (left right : U × T) : Result (Option Ordering) := do
  let order ← first.partial_cmp left.1 right.1
  match order with
  | some .eq => second.partial_cmp left.2 right.2
  | result => .ok result

def Pair.Insts.CoreCmpPartialEqPair.eq {U : Type u} {T : Type v}
    (first : core.cmp.PartialEq U U) (second : core.cmp.PartialEq T T)
    (left right : U × T) : Result Bool := do
  let equal ← first.eq left.1 right.1
  if equal then second.eq left.2 right.2 else .ok false

-- These unused methods dispatch new __chaining_* trait hooks in pinned Rust,
-- which this backend dictionary does not expose. Keep them explicitly unsupported.
def Pair.Insts.CoreCmpPartialOrdPair.lt {U : Type u} {T : Type v}
    (_ : core.cmp.PartialOrd U U) (_ : core.cmp.PartialOrd T T)
    (_ _ : U × T) : Result Bool := .fail .panic

def Pair.Insts.CoreCmpPartialOrdPair.le {U : Type u} {T : Type v}
    (_ : core.cmp.PartialOrd U U) (_ : core.cmp.PartialOrd T T)
    (_ _ : U × T) : Result Bool := .fail .panic

def Pair.Insts.CoreCmpPartialOrdPair.ge {U : Type u} {T : Type v}
    (_ : core.cmp.PartialOrd U U) (_ : core.cmp.PartialOrd T T)
    (_ _ : U × T) : Result Bool := .fail .panic

def indexmap.map.IndexMap.contains_key {K V S Q H : Type}
    (hasher : core.hash.BuildHasher S H) (hashInst : core.hash.Hash Q)
    (equiv : equivalent.Equivalent Q K) (entries : indexmap.map.IndexMap K V S) (query : Q) : Result Bool := do
  let value ← indexmap.map.IndexMap.get hasher hashInst equiv entries query
  .ok value.isSome

-- Read-only tree iteration preserves sorted logical key order.
def alloc.collections.btree.map.BTreeMap.keys {K V A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.Keys K V) := .ok (entries.map Prod.fst)

def alloc.collections.btree.map.Keys.Insts.CoreIterTraitsIteratorIteratorSharedAK.next {K V : Type} :
    alloc.collections.btree.map.Keys K V → Result (Option K × alloc.collections.btree.map.Keys K V)
  | [] => .ok (none, [])
  | key :: rest => .ok (some key, rest)

def alloc.collections.btree.set.BTreeSet.iter {T A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.set.BTreeSet T A) :
    Result (alloc.collections.btree.set.Iter T) := .ok entries

def SharedABTreeSet.Insts.CoreIterTraitsCollectIntoIteratorSharedATIter.into_iter {T A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.set.BTreeSet T A) :
    Result (alloc.collections.btree.set.Iter T) := .ok entries

def alloc.collections.btree.set.Iter.Insts.CoreIterTraitsIteratorIteratorSharedAT.next {T : Type} :
    alloc.collections.btree.set.Iter T → Result (Option T × alloc.collections.btree.set.Iter T)
  | [] => .ok (none, [])
  | value :: rest => .ok (some value, rest)

def cloneValues {T : Type} (cloneInst : core.clone.Clone T) : List T → Result (List T)
  | [] => .ok []
  | value :: rest => do
    let copied ← cloneInst.clone value
    let remaining ← cloneValues cloneInst rest
    .ok (copied :: remaining)

def alloc.collections.btree.set.BTreeSet.Insts.CoreCloneClone.clone {T A : Type}
    (cloneInst : core.clone.Clone T) (_ : core.alloc.AllocatorClone A)
    (entries : alloc.collections.btree.set.BTreeSet T A) : Result (alloc.collections.btree.set.BTreeSet T A) :=
  cloneValues cloneInst entries

def Shared0T.Insts.CoreBorrowBorrow.borrow {T : Type} (value : T) : Result T := .ok value

def alloc.string.String.as_str (value : String) : Result Str :=
  alloc.string.String.Insts.CoreOpsDerefDerefStr.deref value

def alloc.string.String.Insts.CoreBorrowBorrowStr.borrow (value : String) : Result Str :=
  alloc.string.String.Insts.CoreOpsDerefDerefStr.deref value

def alloc.collections.btree.map.BTreeMap.contains_key {K V A Q : Type}
    (allocator : core.alloc.AllocatorClone A) (borrowInst : core.borrow.Borrow K Q)
    (keyOrd : core.cmp.Ord K) (queryOrd : core.cmp.Ord Q)
    (entries : alloc.collections.btree.map.BTreeMap K V A) (query : Q) : Result Bool := do
  let result ← alloc.collections.btree.map.BTreeMap.get allocator borrowInst keyOrd queryOrd entries query
  .ok result.isSome

def alloc.collections.btree.set.BTreeSet.contains {T A Q : Type}
    (allocator : core.alloc.AllocatorClone A) (borrowInst : core.borrow.Borrow T Q)
    (keyOrd : core.cmp.Ord T) (queryOrd : core.cmp.Ord Q)
    (entries : alloc.collections.btree.set.BTreeSet T A) (query : Q) : Result Bool :=
  alloc.collections.btree.map.BTreeMap.contains_key allocator borrowInst keyOrd queryOrd
    (entries.map fun key => (key, ())) query

def core.result.Result.is_err {T E : Type} : core.result.Result T E → Result Bool
  | .Ok _ => .ok false
  | .Err _ => .ok true

def SharedABTreeMap.Insts.CoreIterTraitsCollectIntoIteratorPairSharedAKSharedAVIter.into_iter {K V A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.Iter K V) := .ok entries

def alloc.collections.btree.map.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next {K V : Type} :
    alloc.collections.btree.map.Iter K V → Result (Option (K × V) × alloc.collections.btree.map.Iter K V)
  | [] => .ok (none, [])
  | entry :: rest => .ok (some entry, rest)

def alloc.collections.btree.map.BTreeMap.Insts.CoreIterTraitsCollectIntoIteratorPairIntoIter.into_iter {K V A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.map.BTreeMap K V A) :
    Result (alloc.collections.btree.map.IntoIter K V A) := .ok entries

def alloc.collections.btree.map.IntoIter.Insts.CoreIterTraitsIteratorIteratorPair.next {K V A : Type}
    (_ : core.alloc.AllocatorClone A) : alloc.collections.btree.map.IntoIter K V A →
    Result (Option (K × V) × alloc.collections.btree.map.IntoIter K V A)
  | [] => .ok (none, [])
  | entry :: rest => .ok (some entry, rest)

def alloc.collections.btree.map.BTreeMap.len {K V A : Type}
    (_ : core.alloc.AllocatorClone A) (entries : alloc.collections.btree.map.BTreeMap K V A) : Result Std.Usize :=
  if bound : entries.length < 2^UScalarTy.Usize.numBits then .ok (Std.Usize.ofNatCore entries.length bound)
  else .fail .panic

-- An Arc clone shares the same logical immutable pointee. Reference-counting,
-- physical allocation, aliasing and copy-on-write remain dependency boundaries.
def alloc.sync.Arc.Insts.CoreCloneClone.clone {T A : Type}
    (_ : core.alloc.AllocatorClone A) (value : alloc.sync.Arc T) : Result (alloc.sync.Arc T) := .ok value

-- A mutable map index preserves the stored key and every other entry. Missing
-- keys panic; callers must prove existence rather than assume the borrow succeeds.
def mapIndexMut {K V Q : Type} (eq : Q → K → Result Bool) (query : Q) :
    List (K × V) → Result (V × (V → List (K × V)))
  | [] => .fail .panic
  | (key, stored) :: rest => do
    let equal ← eq query key
    if equal then .ok (stored, fun value => (key, value) :: rest)
    else
      let (selected, restore) ← mapIndexMut eq query rest
      .ok (selected, fun value => (key, stored) :: restore value)

def indexmap.map.IndexMap.Insts.CoreOpsIndexIndexMutShared0QV.index_mut {K V Q S H : Type}
    (_ : core.hash.Hash Q) (eqInst : equivalent.Equivalent Q K) (_ : core.hash.BuildHasher S H)
    (entries : indexmap.map.IndexMap K V S) (query : Q) : Result (V × (V → indexmap.map.IndexMap K V S)) :=
  mapIndexMut eqInst.equivalent query entries

end Axionomy.AccountExtracted
