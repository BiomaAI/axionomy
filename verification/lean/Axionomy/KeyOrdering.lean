import Axionomy.CanonicalSort

namespace Axionomy.KeyOrdering
open Aeneas Aeneas.Std AccountExtracted RateConstructionRefinement

variable {K V : Type}

def lex (first : K → K → Ordering) (second : V → V → Ordering) (left right : K × V) : Ordering :=
  match first left.1 right.1 with
  | .eq => second left.2 right.2
  | order => order

theorem lex_lt (first : K → K → Ordering) (second : V → V → Ordering)
    (laws : OrderedEffects.OrderLaws first) (left right : K × V) :
    lex first second left right = .lt ↔
      first left.1 right.1 = .lt ∨ left.1 = right.1 ∧ second left.2 right.2 = .lt := by
  by_cases same : left.1 = right.1
  · simp [lex, same, laws.reflexive]
  · have notEqual : first left.1 right.1 ≠ .eq := fun equal => same (laws.equal _ _ equal)
    cases compared : first left.1 right.1 <;> simp_all [lex]

theorem lex_laws (first : K → K → Ordering) (second : V → V → Ordering)
    (firstLaws : OrderedEffects.OrderLaws first) (secondLaws : OrderedEffects.OrderLaws second) :
    OrderedEffects.OrderLaws (lex first second) where
  reflexive := by intro x; simp [lex, firstLaws.reflexive, secondLaws.reflexive]
  equal := by
    intro x y same
    cases compared : first x.1 y.1 with
    | lt => simp [lex, compared] at same
    | gt => simp [lex, compared] at same
    | eq =>
      have heads := firstLaws.equal _ _ compared
      have tails := secondLaws.equal x.2 y.2 (by simpa [lex, compared] using same)
      exact Prod.ext heads tails
  transitive := by
    intro x y z earlier later
    rw [lex_lt first second firstLaws] at earlier later ⊢
    rcases earlier with less | ⟨equal, less⟩ <;> rcases later with more | ⟨same, more⟩
    · exact Or.inl (firstLaws.transitive _ _ _ less more)
    · exact Or.inl (same ▸ less)
    · exact Or.inl (equal ▸ more)
    · exact Or.inr ⟨equal.trans same, secondLaws.transitive _ _ _ less more⟩
  greater := by
    intro x y more
    cases compared : first x.1 y.1 with
    | lt => simp [lex, compared] at more
    | gt => simp [lex, firstLaws.greater _ _ compared]
    | eq =>
      have same := firstLaws.equal _ _ compared
      have tails : second x.2 y.2 = .gt := by simpa [lex, compared] using more
      simp [lex, ← same, firstLaws.reflexive, secondLaws.greater _ _ tails]

def number (left right : Std.U64) : Ordering := compare left.val right.val

theorem number_laws : OrderedEffects.OrderLaws number where
  reflexive := fun _ => OrderedEffects.naturalOrder.reflexive _
  equal := fun _ _ same => UScalar.eq_of_val_eq (OrderedEffects.naturalOrder.equal _ _ same)
  transitive := fun _ _ _ => OrderedEffects.naturalOrder.transitive _ _ _
  greater := fun _ _ => OrderedEffects.naturalOrder.greater _ _

def triples {AccountId A : Type} (accounts : AccountId → AccountId → Ordering) (assets : A → A → Ordering) :=
  lex accounts (lex assets number)

theorem triples_laws {AccountId A : Type}
    (accounts : AccountId → AccountId → Ordering) (assets : A → A → Ordering)
    (accountLaws : OrderedEffects.OrderLaws accounts) (assetLaws : OrderedEffects.OrderLaws assets) :
    OrderedEffects.OrderLaws (triples accounts assets) :=
  lex_laws accounts (lex assets number) accountLaws (lex_laws assets number assetLaws number_laws)

theorem triples_ordering {AccountId A : Type} (accountOrd : core.cmp.Ord AccountId) (assetOrd : core.cmp.Ord A)
    (accounts : AccountId → AccountId → Ordering) (assets : A → A → Ordering)
    (hAccount : LawfulOrdering accountOrd accounts) (hAsset : LawfulOrdering assetOrd assets) :
    LawfulOrdering (TupleVUT.Insts.CoreCmpOrd accountOrd assetOrd (quantity.Quantity.Insts.CoreCmpOrd core.cmp.OrdU64))
      (triples accounts assets) := by
  intro left right
  cases accountCmp : accounts left.1 right.1 <;> cases assetCmp : assets left.2.1 right.2.1 <;>
    simp [TupleVUT.Insts.CoreCmpOrd.cmp, hAccount left.1 right.1, hAsset left.2.1 right.2.1,
      quantity.Quantity.Insts.CoreCmpOrd.cmp, triples, lex, number, accountCmp, assetCmp]

end Axionomy.KeyOrdering
