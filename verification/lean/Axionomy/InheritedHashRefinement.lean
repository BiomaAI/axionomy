import Axionomy.InheritedOrderRefinement

namespace Axionomy.InheritedHashRefinement
open Aeneas Aeneas.Std AccountExtracted
variable {T N A AccountId H : Type}

/-- Ordered callback protocol with no implicit prefix or final write. -/
def runHashes (hash : T → H → Result H) : List T → H → Result H
  | [], state => .ok state
  | value :: rest, state => do
    let next ← hash value state
    runHashes hash rest next

theorem hash_values (inst : core.hash.Hash T) (hasher : core.hash.Hasher H)
    (values : List T) (state : H) :
    hashValues inst hasher values state = runHashes (inst.hash hasher) values state := by
  induction values generalizing state with
  | nil => rfl
  | cons value rest ih => simp only [hashValues, runHashes, ih]

theorem quantity_hash_slice (inst : core.hash.Hash N) (hasher : core.hash.Hasher H)
    (values : Slice N) (state : H) :
    inherited_quantity_hash_slice inst hasher values state =
      runHashes (inst.hash hasher) values.val state := by
  exact hash_values (quantity.Quantity.Insts.CoreHashHash inst) hasher values.val state

theorem amount_hash_slice (assetHash : core.hash.Hash A) (numberHash : core.hash.Hash N)
    (hasher : core.hash.Hasher H) (values : Slice (amount.AssetAmount A N)) (state : H) :
    inherited_amount_hash_slice assetHash numberHash hasher values state =
      runHashes (fun (value : amount.AssetAmount A N) state => do
        let next ← assetHash.hash hasher value.asset state
        numberHash.hash hasher value.quantity next) values.val state := by
  exact hash_values (amount.AssetAmount.Insts.CoreHashHash assetHash numberHash) hasher values.val state

theorem fingerprint_hash_slice (hasher : core.hash.Hasher H)
    (values : Slice economy.StateFingerprint) (state : H) :
    inherited_fingerprint_hash_slice hasher values state =
      runHashes (derived_fingerprint_hash hasher) values.val state := by
  exact hash_values economy.StateFingerprint.Insts.CoreHashHash hasher values.val state

theorem observation_key_hash_slice (accountHash : core.hash.Hash AccountId)
    (assetHash : core.hash.Hash A) (numberHash : core.hash.Hash N) (hasher : core.hash.Hasher H)
    (values : Slice (economy.ObservationKey AccountId A N)) (state : H) :
    inherited_observation_key_hash_slice accountHash assetHash numberHash hasher values state =
      runHashes (derived_observation_key_hash accountHash assetHash numberHash hasher) values.val state := by
  exact hash_values (economy.ObservationKey.Insts.CoreHashHash accountHash assetHash numberHash)
    hasher values.val state

theorem empty (hash : T → H → Result H) (state : H) :
    runHashes hash [] state = .ok state := rfl

theorem append (hash : T → H → Result H) (left right : List T) (state : H) :
    runHashes hash (left ++ right) state = (do
      let next ← runHashes hash left state
      runHashes hash right next) := by
  induction left generalizing state with
  | nil => simp [runHashes]
  | cons value rest ih => simp [runHashes, ih]

theorem first_failure (hash : T → H → Result H) (value : T) (rest : List T)
    (state : H) (error : Error) (failed : hash value state = .fail error) :
    runHashes hash (value :: rest) state = .fail error := by
  simp [runHashes, failed]

theorem finite_success (hash : T → H → Result H) (step : T → H → H)
    (law : ∀ value state, hash value state = .ok (step value state)) (values : List T) (state : H) :
    runHashes hash values state = .ok (values.foldl (fun state value => step value state) state) := by
  induction values generalizing state with
  | nil => rfl
  | cons value rest ih => simp [runHashes, law, ih]

theorem quantity_finite_success (inst : core.hash.Hash N) (hasher : core.hash.Hasher H)
    (step : N → H → H) (law : ∀ value state, inst.hash hasher value state = .ok (step value state))
    (values : Slice N) (state : H) :
    inherited_quantity_hash_slice inst hasher values state =
      .ok (values.val.foldl (fun state value => step value state) state) := by
  rw [quantity_hash_slice]
  exact finite_success _ step law values.val state

end Axionomy.InheritedHashRefinement
