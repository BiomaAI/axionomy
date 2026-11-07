import Axionomy.DerivedKeyRefinement

namespace Axionomy.DerivedAccountEqualityRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement DerivedValueRefinement
variable {A N M : Type}

/-- Basket equality compares amounts by key, independently of insertion order.
This specification also describes noncanonical logical inputs, without silently
assuming uniqueness or erasing explicit zero entries. -/
def matchesEntries [DecidableEq A] [DecidableEq N] : List (A × N) → List (A × N) → Bool
  | [], _ => true
  | (asset, number) :: rest, other => decide (Account.find? asset other = some number) && matchesEntries rest other

def basketEquality [DecidableEq A] [DecidableEq N] (left right : List (A × N)) : Bool :=
  decide (left.length = right.length) && matchesEntries left right

theorem option_eq [DecidableEq N] (numberEq : core.cmp.PartialEq N N) (numberLaw : LawfulPartialEquality numberEq)
    (left : Option N) (right : N) :
    core.option.Option.Insts.CoreCmpPartialEqOption.eq
      (core.cmp.PartialEqShared (quantity.Quantity.Insts.CoreCmpPartialEqQuantity numberEq)) left (some right) =
      .ok (decide (left = some right)) := by
  simp only [LawfulPartialEquality] at numberLaw
  cases left <;> simp [core.option.Option.Insts.CoreCmpPartialEqOption.eq,
    core.cmp.impls.PartialEqShared.eq, quantity.Quantity.Insts.CoreCmpPartialEqQuantity.eq, numberLaw]

theorem equal_body [DecidableEq A] [DecidableEq N]
    (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (numberEq : core.cmp.PartialEq N N)
    (assetLaw : LawfulEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (remaining right : List (A × N)) :
    basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body assetEq assetHash numberEq ⟨right⟩ remaining =
      .ok (match remaining with
        | [] => .done true
        | (asset, number) :: rest => if Account.find? asset right = some number then .cont rest else .done false) := by
  cases remaining with
  | nil => simp [basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, number⟩
    simp [basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
      indexmap.map.IndexMap.get, equivalent_eq, get_exact assetEq assetLaw, option_eq numberEq numberLaw]
    split <;> rfl

theorem equal_loop [DecidableEq A] [DecidableEq N]
    (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (numberEq : core.cmp.PartialEq N N)
    (assetLaw : LawfulEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (remaining right : List (A × N)) :
    basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop assetEq assetHash numberEq remaining ⟨right⟩ =
      .ok (matchesEntries remaining right) := by
  induction remaining with
  | nil =>
    unfold basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop
    rw [loop.eq_1]
    simp [equal_body assetEq assetHash numberEq assetLaw numberLaw, matchesEntries]
  | cons entry rest ih =>
    rcases entry with ⟨asset, number⟩
    unfold basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop
    rw [loop.eq_1]
    simp only [equal_body assetEq assetHash numberEq assetLaw numberLaw ((asset, number) :: rest) right]
    by_cases agrees : Account.find? asset right = some number
    · simp only [agrees, ↓reduceIte, bind_ok]
      change basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop assetEq assetHash numberEq rest ⟨right⟩ = _
      rw [ih]
      simp [matchesEntries, agrees]
    · simp [agrees, matchesEntries]

theorem basket_eq [DecidableEq A] [DecidableEq N]
    (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (numberEq : core.cmp.PartialEq N N)
    (assetLaw : LawfulEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : basket.Basket A N)
    (leftBound : left.quantities.length < 2^UScalarTy.Usize.numBits)
    (rightBound : right.quantities.length < 2^UScalarTy.Usize.numBits) :
    basket.Basket.Insts.CoreCmpPartialEqBasket.eq assetEq assetHash numberEq left right =
      .ok (basketEquality left.quantities right.quantities) := by
  cases left with | mk left =>
    cases right with | mk right =>
      simp only [basket.Basket.Insts.CoreCmpPartialEqBasket.eq, basket.Basket.len,
        indexmap.map.IndexMap.len, leftBound, rightBound, ↓reduceDIte, bind_ok]
      have lengthEqual : Std.Usize.ofNatCore left.length leftBound = Std.Usize.ofNatCore right.length rightBound ↔
          left.length = right.length := by
        constructor
        · intro same; simpa using congrArg UScalar.val same
        · intro same; apply UScalar.eq_of_val_eq; simpa using same
      by_cases lengths : left.length = right.length
      · simp [basket.Basket.iter, indexmap.map.IndexMap.iter,
          equal_loop assetEq assetHash numberEq assetLaw numberLaw, basketEquality, lengths]
      · simp [lengthEqual, lengths, basketEquality]

theorem account_eq [DecidableEq A] [DecidableEq N]
    (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (numberEq : core.cmp.PartialEq N N)
    (assetLaw : LawfulEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : account.Account A N)
    (leftBound : left.balances.quantities.length < 2^UScalarTy.Usize.numBits)
    (rightBound : right.balances.quantities.length < 2^UScalarTy.Usize.numBits) :
    account.Account.Insts.CoreCmpPartialEqAccount.eq assetEq assetHash numberEq left right =
      .ok (basketEquality left.balances.quantities right.balances.quantities) :=
  basket_eq assetEq assetHash numberEq assetLaw numberLaw left.balances right.balances leftBound rightBound

def errorBound : account.AccountError A N M → Prop
  | .InsufficientBalance missing => missing.quantities.length < 2^UScalarTy.Usize.numBits
  | .Overflow _ => True

def errorEquality [DecidableEq A] [DecidableEq N] : account.AccountError A N M → account.AccountError A N M → Bool
  | .InsufficientBalance left, .InsufficientBalance right => basketEquality left.quantities right.quantities
  | .Overflow left, .Overflow right => decide (left = right)
  | _, _ => false

theorem account_error_eq [DecidableEq A] [DecidableEq N]
    (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (scalar : quantity.QuantityScalar N M)
    (assetLaw : LawfulEquality assetEq) (numberLaw : LawfulPartialEquality scalar.corecmpEqInst.partialEqInst)
    (left right : account.AccountError A N M) (leftBound : errorBound left) (rightBound : errorBound right) :
    derived_account_error_eq assetEq assetHash scalar left right = .ok (errorEquality left right) := by
  cases left with
  | InsufficientBalance left =>
    cases right with
    | InsufficientBalance right =>
      simpa [derived_account_error_eq, account.AccountError.Insts.CoreCmpPartialEqAccountError.eq,
        account.AccountError.read_discriminant, errorEquality] using
        basket_eq assetEq assetHash scalar.corecmpEqInst.partialEqInst assetLaw numberLaw left right leftBound rightBound
    | Overflow _ =>
      simp [derived_account_error_eq, account.AccountError.Insts.CoreCmpPartialEqAccountError.eq,
        account.AccountError.read_discriminant, errorEquality]
  | Overflow left =>
    cases right with
    | InsufficientBalance _ =>
      simp [derived_account_error_eq, account.AccountError.Insts.CoreCmpPartialEqAccountError.eq,
        account.AccountError.read_discriminant, errorEquality]
    | Overflow right =>
      simpa [derived_account_error_eq, account.AccountError.Insts.CoreCmpPartialEqAccountError.eq,
        account.AccountError.read_discriminant, core.cmp.impls.PartialEqShared.eq, errorEquality] using assetLaw left right

end Axionomy.DerivedAccountEqualityRefinement
