import Axionomy.KeyRefinement

namespace Axionomy.FingerprintRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement

theorem fingerprint_get (value : economy.StateFingerprint) :
    economy.StateFingerprint.get value = .ok value := rfl

/-- One FNV-1a update, with the production wrapping u64 arithmetic. -/
def step (state : Std.U64) (byte : Std.U8) : Std.U64 :=
  core.num.U64.wrapping_mul (state ^^^ core.convert.num.FromU64U8.from byte) 1099511628211#u64

def run (state : Std.U64) (bytes : List Std.U8) : Std.U64 := bytes.foldl step state

def fnv (state : Nat) (bytes : List Nat) : Nat :=
  bytes.foldl (fun state byte => ((state ^^^ byte) * 1099511628211) % 18446744073709551616) state

theorem step_value (state : Std.U64) (byte : Std.U8) :
    (step state byte).val = ((state.val ^^^ byte.val) * 1099511628211) % 18446744073709551616 := by
  simp [step, U64.size, U64.numBits]

theorem run_value (state : Std.U64) (bytes : List Std.U8) :
    (run state bytes).val = fnv state.val (bytes.map UScalar.val) := by
  induction bytes generalizing state with
  | nil => rfl
  | cons byte rest ih =>
    change (run (step state byte) rest).val = _
    rw [ih, step_value]
    rfl

theorem run_append (state : Std.U64) (left right : List Std.U8) :
    run state (left ++ right) = run (run state left) right := by simp [run, List.foldl_append]

theorem write_loop (remaining : List Std.U8) (iter : core.slice.iter.Iter Std.U8)
    (contents : iter.slice.val.drop iter.i = remaining) (state : economy.FingerprintHasher) :
    economy.FingerprintHasher.Insts.CoreHashHasher.write_loop iter state = .ok ⟨run state.state remaining⟩ := by
  induction remaining generalizing iter state with
  | nil =>
    unfold economy.FingerprintHasher.Insts.CoreHashHasher.write_loop
    rw [loop.eq_1]
    simp [economy.FingerprintHasher.Insts.CoreHashHasher.write_loop.body,
      ConservationRefinement.next_empty iter contents, run]
  | cons byte rest ih =>
    obtain ⟨next, tail⟩ := ConservationRefinement.next_cons iter byte rest contents
    have body : economy.FingerprintHasher.Insts.CoreHashHasher.write_loop.body iter state =
        .ok (.cont ({iter with i := iter.i + 1}, ⟨step state.state byte⟩)) := by
      simp [economy.FingerprintHasher.Insts.CoreHashHasher.write_loop.body, next, lift, step]
    unfold economy.FingerprintHasher.Insts.CoreHashHasher.write_loop
    rw [loop.eq_1]
    simp only
    rw [body, bind_ok]
    exact ih _ tail ⟨step state.state byte⟩

theorem write_bytes (state : economy.FingerprintHasher) (bytes : Slice Std.U8) :
    economy.FingerprintHasher.Insts.CoreHashHasher.write state bytes = .ok ⟨run state.state bytes.val⟩ := by
  simp [economy.FingerprintHasher.Insts.CoreHashHasher.write,
    SharedSlice.Insts.CoreIterTraitsCollectIntoIteratorSharedIter.into_iter,
    write_loop bytes.val ⟨bytes, 0⟩ (by simp) state]

/-- An independent encoding contract for caller-provided identifier hashing.
It applies to the concrete FingerprintHasher, including every emitted byte. -/
def LawfulHash {T : Type} (hashInst : core.hash.Hash T) (encode : T → List Std.U8) : Prop :=
  ∀ value state, hashInst.hash economy.FingerprintHasher.Insts.CoreHashHasher value ⟨state⟩ =
    .ok ⟨run state (encode value)⟩

def numberBytes (value : Std.U64) : List Std.U8 := (core.num.U64.to_le_bytes value).to_slice.val

/-- This uses the reviewed native-64-bit/little-endian, default-Hasher adapter. -/
theorem number_hash : LawfulHash U64.Insts.CoreHashHash numberBytes := by
  intro value state
  exact write_bytes ⟨state⟩ (core.num.U64.to_le_bytes value).to_slice

theorem quantity_hash : LawfulHash (quantity.Quantity.Insts.CoreHashHash U64.Insts.CoreHashHash) numberBytes := by
  intro value state
  simpa [quantity.Quantity.Insts.CoreHashHash.hash] using number_hash value state

def tripleBytes {AccountId A : Type} (accounts : AccountId → List Std.U8) (assets : A → List Std.U8)
    (value : AccountId × A × Std.U64) : List Std.U8 := accounts value.1 ++ assets value.2.1 ++ numberBytes value.2.2

theorem triple_hash {AccountId A : Type} (accountHash : core.hash.Hash AccountId) (assetHash : core.hash.Hash A)
    (accounts : AccountId → List Std.U8) (assets : A → List Std.U8)
    (hAccount : LawfulHash accountHash accounts) (hAsset : LawfulHash assetHash assets) :
    LawfulHash (TupleTBC.Insts.CoreHashHash accountHash assetHash
      (quantity.Quantity.Insts.CoreHashHash U64.Insts.CoreHashHash)) (tripleBytes accounts assets) := by
  intro value state
  simp only [TupleTBC.Insts.CoreHashHash.hash, hAccount value.1, hAsset value.2.1, bind_ok,
    tripleBytes, run_append]
  exact quantity_hash value.2.2 _

theorem hash_values {T : Type} (hashInst : core.hash.Hash T) (encode : T → List Std.U8)
    (lawful : LawfulHash hashInst encode) (values : List T) (state : Std.U64) :
    hashValues hashInst economy.FingerprintHasher.Insts.CoreHashHasher values ⟨state⟩ =
      .ok ⟨run state (values.flatMap encode)⟩ := by
  induction values generalizing state with
  | nil => rfl
  | cons value rest ih => simp [hashValues, lawful value state, ih, run_append]

def countWord (count : Nat) (fits : count ≤ U64.max) : Std.U64 :=
  UScalar.ofNatCore count (by simp only [U64.max_eq] at fits; simp only [UScalarTy.numBits]; omega)

theorem vector_bound {T : Type} (values : alloc.vec.Vec T) : values.val.length ≤ U64.max := by
  have bound := values.property
  rcases Usize.bounds_eq with small | large
  · rw [small] at bound
    simp only [U32.max_eq, U64.max_eq] at *
    omega
  · simpa [large] using bound

def bytes {T : Type} (encode : T → List Std.U8) (values : List T) (fits : values.length ≤ U64.max) : List Std.U8 :=
  numberBytes (countWord values.length fits) ++ values.flatMap encode

theorem vector_hash {T : Type} (hashInst : core.hash.Hash T) (encode : T → List Std.U8)
    (lawful : LawfulHash hashInst encode) (values : alloc.vec.Vec T) (state : Std.U64) :
    alloc.vec.Vec.Insts.CoreHashHash.hash alloc.alloc.Global hashInst economy.FingerprintHasher.Insts.CoreHashHasher values ⟨state⟩ =
      .ok ⟨run state (bytes encode values.val (vector_bound values))⟩ := by
  simp [alloc.vec.Vec.Insts.CoreHashHash.hash, vector_bound values,
    economy.FingerprintHasher.Insts.CoreHashHasher, write_bytes,
    hash_values hashInst encode lawful, bytes, countWord, numberBytes, run_append]

/-- The complete production fingerprint is FNV-1a of the length-prefixed,
canonically sorted typed keys. Identifier encodings are explicit caller laws;
this contract makes no assertion of collision freedom. -/
theorem fingerprint_refines {AccountId A RateId Role : Type}
    (accountClone : core.clone.Clone AccountId) (accountEq : core.cmp.Eq AccountId)
    (accountHash : core.hash.Hash AccountId) (accountOrd : core.cmp.Ord AccountId)
    (assetClone : core.clone.Clone A) (assetEq : core.cmp.Eq A) (assetHash : core.hash.Hash A) (assetOrd : core.cmp.Ord A)
    (rateClone : core.clone.Clone RateId) (rateEq : core.cmp.Eq RateId) (rateHash : core.hash.Hash RateId) (rateOrd : core.cmp.Ord RateId)
    (roleClone : core.clone.Clone Role) (roleOrd : core.cmp.Ord Role)
    (cmpAccount : AccountId → AccountId → Ordering) (cmpAsset : A → A → Ordering)
    (hAccountClone : LawfulClone accountClone) (hAssetClone : LawfulClone assetClone)
    (hAccountOrd : RateConstructionRefinement.LawfulOrdering accountOrd cmpAccount)
    (hAssetOrd : RateConstructionRefinement.LawfulOrdering assetOrd cmpAsset)
    (accountBytes : AccountId → List Std.U8) (assetBytes : A → List Std.U8)
    (hAccountHash : LawfulHash accountHash accountBytes) (hAssetHash : LawfulHash assetHash assetBytes)
    (model : economy.Economy AccountId A RateId Role Std.U64)
    (capacity : (CanonicalKeys.entries (KeyRefinement.world model.accounts)).length ≤ Usize.max) :
    ∃ keys : alloc.vec.Vec (AccountId × A × Std.U64),
      keys.val = CanonicalSort.sort (KeyOrdering.triples cmpAccount cmpAsset) (CanonicalKeys.entries (KeyRefinement.world model.accounts)) ∧
      economy.Economy.state_fingerprint accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
        rateClone rateEq rateHash rateOrd roleClone roleOrd scalar model =
          .ok (run 14695981039346656037#u64 (bytes (tripleBytes accountBytes assetBytes) keys.val (vector_bound keys))) ∧
      (run 14695981039346656037#u64 (bytes (tripleBytes accountBytes assetBytes) keys.val (vector_bound keys))).val =
        fnv 14695981039346656037 ((bytes (tripleBytes accountBytes assetBytes) keys.val (vector_bound keys)).map UScalar.val) := by
  obtain ⟨keys, keyCall, keyValues⟩ := KeyRefinement.state_key accountClone accountEq accountHash accountOrd assetClone assetEq assetHash assetOrd
    rateClone rateEq rateHash rateOrd roleClone roleOrd cmpAccount cmpAsset hAccountClone hAssetClone hAccountOrd hAssetOrd model capacity
  have hashing := vector_hash
    (TupleTBC.Insts.CoreHashHash accountHash assetHash (quantity.Quantity.Insts.CoreHashHash U64.Insts.CoreHashHash))
    (tripleBytes accountBytes assetBytes) (triple_hash accountHash assetHash accountBytes assetBytes hAccountHash hAssetHash) keys 14695981039346656037#u64
  refine ⟨keys, keyValues, ?_, ?_⟩
  · simp [economy.Economy.state_fingerprint, economy.FingerprintHasher.new, keyCall, hashing,
      economy.FingerprintHasher.Insts.CoreHashHasher.finish]
  · exact run_value _ _

end Axionomy.FingerprintRefinement
