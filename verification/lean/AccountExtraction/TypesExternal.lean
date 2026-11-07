import BasketExtraction.TypesExternal

namespace Axionomy.AccountExtracted
-- Same ordered-map abstraction as basket construction; physical storage is trusted.
abbrev std.hash.random.RandomState := Unit
abbrev std.hash.random.DefaultHasher := Unit
abbrev indexmap.map.IndexMap (K V _S : Type) := List (K × V)
abbrev indexmap.map.iter.Iter (K V : Type) := List (K × V)

-- Sorted logical maps/sets; allocator and tree internals are outside the model.
abbrev alloc.collections.btree.map.BTreeMap (K V _A : Type) := List (K × V)
abbrev alloc.collections.btree.set.BTreeSet (T _A : Type) := List T

-- A borrowed entry retains the surrounding map so its backward function can
-- restore precisely that map after the value borrow ends.
structure alloc.collections.btree.map.entry.OccupiedEntry (K V _A : Type) where
  before : List (K × V)
  key : K
  value : V
  after : List (K × V)

structure alloc.collections.btree.map.entry.VacantEntry (K V _A : Type) where
  before : List (K × V)
  key : K
  after : List (K × V)
-- Read-only Arc borrows expose the logical pointee; physical reference counting
-- and allocation are trusted. No mutation or copy-on-write behavior is modeled here.
abbrev alloc.sync.Arc (T : Type) := T
abbrev indexmap.map.iter.Values (_K V : Type) := List V
end Axionomy.AccountExtracted
