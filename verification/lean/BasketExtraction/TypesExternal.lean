import Aeneas

namespace Axionomy.BasketExtracted

-- Logical storage model. Hash-table layout, allocation, and hash seeds are
-- outside this proof. Keys and values remain separate, arbitrary Lean types.
abbrev std.hash.random.RandomState := Unit
abbrev std.hash.random.DefaultHasher := Unit
abbrev indexmap.map.IndexMap (K V _S : Type) := List (K × V)

end Axionomy.BasketExtracted
