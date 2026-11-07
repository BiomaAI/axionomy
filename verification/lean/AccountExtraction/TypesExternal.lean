import BasketExtraction.TypesExternal

namespace Axionomy.AccountExtracted
-- Same ordered-map abstraction as basket construction; physical storage is trusted.
abbrev std.hash.random.RandomState := Unit
abbrev std.hash.random.DefaultHasher := Unit
abbrev indexmap.map.IndexMap (K V _S : Type) := List (K × V)
abbrev indexmap.map.iter.Iter (K V : Type) := List (K × V)
end Axionomy.AccountExtracted
