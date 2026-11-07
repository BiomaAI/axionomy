import Axionomy.DerivedCloneRefinement
import Axionomy.FingerprintRefinement

namespace Axionomy.DerivedKeyRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement DerivedValueRefinement
open DerivedCollectionRefinement
variable {AccountId A N H : Type}

deriving instance DecidableEq for economy.ObservationKey

theorem triple_eq [DecidableEq AccountId] [DecidableEq A] [DecidableEq N]
    (accountEq : core.cmp.PartialEq AccountId AccountId) (assetEq : core.cmp.PartialEq A A)
    (numberEq : core.cmp.PartialEq N N) (accountLaw : LawfulPartialEquality accountEq)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : AccountId × A × N) :
    TupleVUT.Insts.CoreCmpPartialEqTupleVUT.eq accountEq assetEq numberEq left right =
      .ok (decide (left = right)) := by
  simp only [LawfulPartialEquality] at accountLaw assetLaw numberLaw
  rcases left with ⟨account, asset, number⟩
  rcases right with ⟨otherAccount, otherAsset, otherNumber⟩
  simp [TupleVUT.Insts.CoreCmpPartialEqTupleVUT.eq, accountLaw, assetLaw, numberLaw]

theorem observation_key_eq [DecidableEq AccountId] [DecidableEq A] [DecidableEq N]
    (accountEq : core.cmp.PartialEq AccountId AccountId) (assetEq : core.cmp.PartialEq A A)
    (numberEq : core.cmp.PartialEq N N) (accountLaw : LawfulPartialEquality accountEq)
    (assetLaw : LawfulPartialEquality assetEq) (numberLaw : LawfulPartialEquality numberEq)
    (left right : economy.ObservationKey AccountId A N) :
    derived_observation_key_eq accountEq assetEq numberEq left right = .ok (decide (left = right)) := by
  have quantities : LawfulPartialEquality (quantity.Quantity.Insts.CoreCmpPartialEqQuantity numberEq) := numberLaw
  have triples : LawfulPartialEquality (TupleVUT.Insts.CoreCmpPartialEqTupleVUT accountEq assetEq
      (quantity.Quantity.Insts.CoreCmpPartialEqQuantity numberEq)) :=
    triple_eq accountEq assetEq _ accountLaw assetLaw quantities
  cases left; cases right
  simp [derived_observation_key_eq, economy.ObservationKey.Insts.CoreCmpPartialEqObservationKey.eq,
    vec_eq _ accountLaw, vec_eq _ triples, alloc.vec.Vec.eq_iff]

/-- Tuple cloning uses Aeneas's BuiltinClone identity model. The payload clone
laws are independent compatibility requirements for interpreting that model in
Rust; physical allocation, reference counts and destructor effects are not modeled. -/
theorem observation_key_clone (accountClone : core.clone.Clone AccountId) (assetClone : core.clone.Clone A)
    (numberClone : core.clone.Clone N) (accountLaw : LawfulClone accountClone)
    (_assetLaw : LawfulClone assetClone) (_numberLaw : LawfulClone numberClone)
    (value : economy.ObservationKey AccountId A N) :
    derived_observation_key_clone accountClone assetClone numberClone value = .ok value := by
  have triples : LawfulClone (BuiltinClone (AccountId × A × quantity.Quantity N)) := fun _ => rfl
  cases value
  simp [derived_observation_key_clone, economy.ObservationKey.Insts.CoreCloneClone.clone,
    vec_clone _ accountLaw, vec_clone _ triples]

theorem triple_unequal_first (accountEq : core.cmp.PartialEq AccountId AccountId)
    (assetEq : core.cmp.PartialEq A A) (numberEq : core.cmp.PartialEq N N)
    (left right : AccountId × A × N) (different : accountEq.eq left.1 right.1 = .ok false) :
    TupleVUT.Insts.CoreCmpPartialEqTupleVUT.eq accountEq assetEq numberEq left right = .ok false := by
  simp [TupleVUT.Insts.CoreCmpPartialEqTupleVUT.eq, different]

theorem triple_second_failure (accountEq : core.cmp.PartialEq AccountId AccountId)
    (assetEq : core.cmp.PartialEq A A) (numberEq : core.cmp.PartialEq N N)
    (left right : AccountId × A × N) (error : Error)
    (same : accountEq.eq left.1 right.1 = .ok true)
    (failed : assetEq.eq left.2.1 right.2.1 = .fail error) :
    TupleVUT.Insts.CoreCmpPartialEqTupleVUT.eq accountEq assetEq numberEq left right = .fail error := by
  simp [TupleVUT.Insts.CoreCmpPartialEqTupleVUT.eq, same, failed]

theorem fingerprint_eq (left right : economy.StateFingerprint) :
    derived_fingerprint_eq left right = .ok (decide (left = right)) := rfl

theorem fingerprint_clone (value : economy.StateFingerprint) : derived_fingerprint_clone value = .ok value := rfl

theorem fingerprint_cmp (left right : economy.StateFingerprint) :
    derived_fingerprint_cmp left right = .ok (compare left.val right.val) := rfl

theorem fingerprint_partial_cmp (left right : economy.StateFingerprint) :
    derived_fingerprint_partial_cmp left right = .ok (some (compare left.val right.val)) := by
  simp [derived_fingerprint_partial_cmp,
    economy.StateFingerprint.Insts.CoreCmpPartialOrdStateFingerprint.partial_cmp,
    economy.StateFingerprint.Insts.CoreCmpOrd.cmp]

theorem fingerprint_lt (left right : economy.StateFingerprint) :
    derived_fingerprint_lt left right = .ok (accepts [.lt] (some (compare left.val right.val))) := by
  simp only [derived_fingerprint_lt, core.cmp.PartialOrd.lt.trait_default,
    core.cmp.PartialOrd.lt.default, core.cmp.PartialOrd.lt_body,
    economy.StateFingerprint.Insts.CoreCmpPartialOrdStateFingerprint.partial_cmp,
    economy.StateFingerprint.Insts.CoreCmpOrd.cmp, bind_ok]
  have compared : core.cmp.impls.OrdU64.cmp left right = compare left.val right.val := rfl
  rw [compared]
  cases compare left.val right.val <;> simp [accepts]

theorem fingerprint_le (left right : economy.StateFingerprint) :
    derived_fingerprint_le left right = .ok (accepts [.lt, .eq] (some (compare left.val right.val))) := by
  simp only [derived_fingerprint_le, core.cmp.PartialOrd.le.trait_default,
    core.cmp.PartialOrd.le.default, core.cmp.PartialOrd.le_body,
    economy.StateFingerprint.Insts.CoreCmpPartialOrdStateFingerprint.partial_cmp,
    economy.StateFingerprint.Insts.CoreCmpOrd.cmp, bind_ok]
  have compared : core.cmp.impls.OrdU64.cmp left right = compare left.val right.val := rfl
  rw [compared]
  cases compare left.val right.val <;> simp [accepts]

theorem fingerprint_gt (left right : economy.StateFingerprint) :
    derived_fingerprint_gt left right = .ok (accepts [.gt] (some (compare left.val right.val))) := by
  simp only [derived_fingerprint_gt, core.cmp.PartialOrd.gt.trait_default,
    core.cmp.PartialOrd.gt.default, core.cmp.PartialOrd.gt_body,
    economy.StateFingerprint.Insts.CoreCmpPartialOrdStateFingerprint.partial_cmp,
    economy.StateFingerprint.Insts.CoreCmpOrd.cmp, bind_ok]
  have compared : core.cmp.impls.OrdU64.cmp left right = compare left.val right.val := rfl
  rw [compared]
  cases compare left.val right.val <;> simp [accepts]

theorem fingerprint_ge (left right : economy.StateFingerprint) :
    derived_fingerprint_ge left right = .ok (accepts [.eq, .gt] (some (compare left.val right.val))) := by
  simp only [derived_fingerprint_ge, core.cmp.PartialOrd.ge.trait_default,
    core.cmp.PartialOrd.ge.default, core.cmp.PartialOrd.ge_body,
    economy.StateFingerprint.Insts.CoreCmpPartialOrdStateFingerprint.partial_cmp,
    economy.StateFingerprint.Insts.CoreCmpOrd.cmp, bind_ok]
  have compared : core.cmp.impls.OrdU64.cmp left right = compare left.val right.val := rfl
  rw [compared]
  cases compare left.val right.val <;> simp [accepts]

/-- Scoped to the reviewed native 64-bit little-endian/default write_u64 model. -/
theorem fingerprint_hash (hasher : core.hash.Hasher H) (value : economy.StateFingerprint) (state : H) :
    derived_fingerprint_hash hasher value state = hasher.write state (core.num.U64.to_le_bytes value).to_slice := rfl

/-- Preserve the three independent sequence boundaries in declaration order.
The Vec adapter requires default hash_slice and Hasher prefix behavior; arbitrary
erased overrides are not justified by this theorem. -/
theorem observation_key_hash_effect (accountHash : core.hash.Hash AccountId) (assetHash : core.hash.Hash A)
    (numberHash : core.hash.Hash N) (hasher : core.hash.Hasher H)
    (value : economy.ObservationKey AccountId A N) (state : H) :
    derived_observation_key_hash accountHash assetHash numberHash hasher value state = (do
      let visible ← alloc.vec.Vec.Insts.CoreHashHash.hash alloc.alloc.Global accountHash hasher value.visible_accounts state
      let existing ← alloc.vec.Vec.Insts.CoreHashHash.hash alloc.alloc.Global accountHash hasher value.existing_accounts visible
      alloc.vec.Vec.Insts.CoreHashHash.hash alloc.alloc.Global
        (TupleTBC.Insts.CoreHashHash accountHash assetHash (quantity.Quantity.Insts.CoreHashHash numberHash))
        hasher value.balances existing) := rfl

def balanceBytes (accounts : AccountId → List Std.U8) (assets : A → List Std.U8) (numbers : N → List Std.U8)
    (value : AccountId × A × N) : List Std.U8 := accounts value.1 ++ assets value.2.1 ++ numbers value.2.2

theorem balance_hash (accountHash : core.hash.Hash AccountId) (assetHash : core.hash.Hash A) (numberHash : core.hash.Hash N)
    (accounts : AccountId → List Std.U8) (assets : A → List Std.U8) (numbers : N → List Std.U8)
    (accountLaw : FingerprintRefinement.LawfulHash accountHash accounts)
    (assetLaw : FingerprintRefinement.LawfulHash assetHash assets)
    (numberLaw : FingerprintRefinement.LawfulHash numberHash numbers) :
    FingerprintRefinement.LawfulHash (TupleTBC.Insts.CoreHashHash accountHash assetHash
      (quantity.Quantity.Insts.CoreHashHash numberHash)) (balanceBytes accounts assets numbers) := by
  intro value state
  simp [TupleTBC.Insts.CoreHashHash.hash, quantity.Quantity.Insts.CoreHashHash.hash,
    accountLaw value.1, assetLaw value.2.1, numberLaw value.2.2,
    balanceBytes, FingerprintRefinement.run_append]

/-- All three length-prefixed fields contribute to the hash stream. Numeric
payloads stay generic and need only their independent byte-encoding law. This
uses the native/default-method boundary documented on observation_key_hash_effect. -/
theorem observation_key_hash (accountHash : core.hash.Hash AccountId) (assetHash : core.hash.Hash A)
    (numberHash : core.hash.Hash N) (accounts : AccountId → List Std.U8) (assets : A → List Std.U8)
    (numbers : N → List Std.U8) (accountLaw : FingerprintRefinement.LawfulHash accountHash accounts)
    (assetLaw : FingerprintRefinement.LawfulHash assetHash assets)
    (numberLaw : FingerprintRefinement.LawfulHash numberHash numbers)
    (value : economy.ObservationKey AccountId A N) (state : Std.U64) :
    derived_observation_key_hash accountHash assetHash numberHash economy.FingerprintHasher.Insts.CoreHashHasher value ⟨state⟩ =
      .ok ⟨FingerprintRefinement.run state
        (FingerprintRefinement.bytes accounts value.visible_accounts.val (FingerprintRefinement.vector_bound _) ++
         FingerprintRefinement.bytes accounts value.existing_accounts.val (FingerprintRefinement.vector_bound _) ++
         FingerprintRefinement.bytes (balanceBytes accounts assets numbers) value.balances.val (FingerprintRefinement.vector_bound _))⟩ := by
  have balances := balance_hash accountHash assetHash numberHash accounts assets numbers accountLaw assetLaw numberLaw
  simp [observation_key_hash_effect, FingerprintRefinement.vector_hash _ _ accountLaw,
    FingerprintRefinement.vector_hash _ _ balances, FingerprintRefinement.run_append]

end Axionomy.DerivedKeyRefinement
