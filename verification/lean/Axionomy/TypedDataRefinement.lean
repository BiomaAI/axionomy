import Axionomy.GoalRefinement

namespace Axionomy.TypedDataRefinement
open Aeneas Aeneas.Std AccountExtracted AccountRefinement RateConstructionRefinement
variable {AccountId A RateId Role N M : Type}

/-! Generic value transport needs no arithmetic laws. Validation uses an
independent backend predicate law, not an assumption that arbitrary Rust trait
implementations satisfy numeric algebra. -/

def validate (valid : N → Bool) (value : N) : core.result.Result N quantity.QuantityError :=
  if valid value then .Ok value else .Err .Negative

theorem quantity_validation (inst : quantity.QuantityScalar N M) (valid : N → Bool)
    (law : ∀ value, inst.is_nonnegative value = .ok (valid value)) (value : N) :
    quantity.Quantity.try_from_scalar inst value = .ok (validate valid value) := by
  cases h : valid value <;> simp [quantity.Quantity.try_from_scalar, law value, validate, h]

theorem quantity_validation_iff (inst : quantity.QuantityScalar N M) (valid : N → Bool)
    (law : ∀ value, inst.is_nonnegative value = .ok (valid value)) (value : N) :
    quantity.Quantity.try_from_scalar inst value = .ok (.Ok value) ↔ valid value = true := by
  rw [quantity_validation inst valid law]
  cases h : valid value <;> simp [validate, h]

theorem checked_validation (inst : quantity.QuantityScalar N M) (valid : N → Bool)
    (law : ∀ value, inst.is_nonnegative value = .ok (valid value)) (result : Option N) :
    quantity.Quantity.from_checked inst result = .ok (result.filter valid) := by
  cases result with
  | none => rfl
  | some value =>
    simp only [quantity.Quantity.from_checked, quantity_validation inst valid law value, bind_ok]
    cases h : valid value <;> simp [validate, Option.filter, h]

theorem quantity_new (value : Std.U64) : quantity.QuantityU64.new value = .ok value := rfl

theorem quantity_get (value : Std.U64) : quantity.QuantityU64.get value = .ok value := rfl

theorem quantity_u64_roundtrip (value : Std.U64) :
    (do let q ← quantity.QuantityU64.new value; quantity.QuantityU64.get q) = .ok value := by
  simp [quantity.QuantityU64.new, quantity.QuantityU64.get]

theorem quantity_from_u64 (value : Std.U64) :
    quantity.QuantityU64.Insts.CoreConvertFromU64.from value = .ok value := rfl

theorem quantity_from_u32 (value : Std.U32) :
    ∃ result, quantity.QuantityU64.Insts.CoreConvertFromU32.from value = .ok result ∧ result.val = value.val := by
  refine ⟨core.convert.num.FromU64U32.from value, ?_, ?_⟩
  · simp [quantity.QuantityU64.Insts.CoreConvertFromU32.from, quantity.QuantityU64.new, lift]
  · change (value.bv.setWidth 64).toNat = value.bv.toNat
    exact BitVec.toNat_setWidth_of_le (by decide)

theorem quantity_borrow (inst : quantity.QuantityScalar N M) (value : N) :
    quantity.Quantity.as_scalar inst value = .ok value := rfl

theorem quantity_consume (inst : quantity.QuantityScalar N M) (value : N) :
    quantity.Quantity.into_scalar inst value = .ok value := rfl

theorem quantity_default (inst : quantity.QuantityScalar N M) (zero : N)
    (law : inst.num_traitsidentitiesZeroInst.zero = .ok zero) :
    quantity.Quantity.Insts.CoreDefaultDefault.default inst = .ok zero := by
  simp [quantity.Quantity.Insts.CoreDefaultDefault.default, law]

theorem amount_new (asset : A) (value : N) :
    amount.AssetAmount.new asset value = .ok ⟨asset, value⟩ := rfl

theorem amount_from_pair (parts : A × N) :
    amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from parts = .ok ⟨parts.1, parts.2⟩ := by
  cases parts; rfl

theorem amount_to_pair (value : amount.AssetAmount A N) :
    PairAQuantity.Insts.CoreConvertFromAssetAmount.from value = .ok (value.asset, value.quantity) := rfl

theorem amount_pair_roundtrip (parts : A × N) :
    (do let value ← amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from parts
        PairAQuantity.Insts.CoreConvertFromAssetAmount.from value) = .ok parts := by
  cases parts
  simp [amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from, amount.AssetAmount.new,
    PairAQuantity.Insts.CoreConvertFromAssetAmount.from, amount.AssetAmount.into_parts]

theorem amount_value_roundtrip (value : amount.AssetAmount A N) :
    (do let parts ← PairAQuantity.Insts.CoreConvertFromAssetAmount.from value
        amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from parts) = .ok value := by
  cases value
  simp [amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from, amount.AssetAmount.new,
    PairAQuantity.Insts.CoreConvertFromAssetAmount.from, amount.AssetAmount.into_parts]

theorem account_new (balances : basket.Basket A N) :
    account.Account.new balances = .ok ⟨balances⟩ := rfl

theorem account_default : account.Account.Insts.CoreDefaultDefault.default A N = .ok ⟨⟨[]⟩⟩ := by
  simp [account.Account.Insts.CoreDefaultDefault.default, basket.Basket.new,
    indexmap.map.IndexMapKVRandomState.new, account.Account.new]

theorem account_roundtrip (balances : basket.Basket A N) :
    (do let holder ← account.Account.Insts.CoreConvertFromBasket.from balances
        account.Account.into_balances holder) = .ok balances := by
  simp [account.Account.Insts.CoreConvertFromBasket.from, account.Account.new, account.Account.into_balances]

theorem account_equality [DecidableEq A] (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
    (law : LawfulEquality eqInst) (left right : account.Account A Std.U64)
    (leftBound : left.balances.quantities.length < 2^UScalarTy.Usize.numBits)
    (rightBound : right.balances.quantities.length < 2^UScalarTy.Usize.numBits) :
    account.Account.Insts.CoreCmpPartialEqAccount.eq eqInst hashInst core.cmp.PartialEqU64 left right =
      .ok (BasketOperations.equal left.balances.quantities right.balances.quantities) :=
  BasketOperationsRefinement.equal_refines eqInst hashInst law _ _ leftBound rightBound

theorem insert_amount [DecidableEq A] (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
    (law : LawfulEquality eqInst) (entries : List (A × Std.U64)) (value : amount.AssetAmount A Std.U64) :
    basket.Basket.insert_amount eqInst hashInst scalar ⟨entries⟩ value =
      .ok (Account.find? value.asset entries, ⟨Account.store arithmetic entries value.asset value.quantity⟩) := by
  simp [basket.Basket.insert_amount, amount.AssetAmount.into_parts, store_call eqInst hashInst law]

/-- Map insertion retains the stored key and replaces its value. -/
theorem tree_insert {K V : Type} (ordInst : core.cmp.Ord K) (cmp : K → K → Ordering)
    (law : LawfulOrdering ordInst cmp) (equal : ∀ x y, cmp x y = .eq → x = y)
    (entries : List (K × V)) (key : K) (value : V) :
    alloc.collections.btree.map.BTreeMap.insert alloc.alloc.Global.Insts.CoreAllocAllocatorClone ordInst entries key value =
      .ok ((RateConstruction.locate cmp key entries).old, (RateConstruction.locate cmp key entries).put value) := by
  have same := RateConstruction.locate_key cmp equal key entries
  simp only [alloc.collections.btree.map.BTreeMap.insert, locate_exact ordInst cmp law, bind_ok]
  cases found : (RateConstruction.locate cmp key entries).old <;>
    simp [encodePosition, found, RateConstruction.Position.put, same]

theorem proposal_new (ordInst : core.cmp.Ord Role) (rateId : RateId) (units : N) :
    exchange.Exchange.new AccountId ordInst rateId units = .ok ⟨rateId, [], units, []⟩ := by
  simp [exchange.Exchange.new, alloc.collections.btree.map.BTreeMapKVGlobal.new]

theorem proposal_bind (ordInst : core.cmp.Ord Role) (cmp : Role → Role → Ordering)
    (law : LawfulOrdering ordInst cmp) (equal : ∀ x y, cmp x y = .eq → x = y)
    (proposal : exchange.Exchange RateId Role AccountId N) (role : Role) (holder : AccountId) :
    exchange.Exchange.bind ordInst proposal role holder =
      .ok { proposal with bindings := (RateConstruction.locate cmp role proposal.bindings).put holder } := by
  simp [exchange.Exchange.bind, tree_insert ordInst cmp law equal]

theorem proposal_parameter {T : Type} (ordInst : core.cmp.Ord Role) (conversion : core.convert.Into T String)
    (encode : T → String) (law : ∀ value, conversion.into value = .ok (encode value))
    (proposal : exchange.Exchange RateId Role AccountId N) (key : T) (value : N) :
    exchange.Exchange.parameter ordInst conversion proposal key value =
      .ok { proposal with parameters := (RateConstruction.locate compare (encode key) proposal.parameters).put value } := by
  have stringLaw : LawfulOrdering alloc.string.String.Insts.CoreCmpOrd compare := fun _ _ => rfl
  have equal : ∀ x y : String, compare x y = .eq → x = y := by
    intro x y same
    exact Std.LawfulEqOrd.eq_of_compare same
  simp [exchange.Exchange.parameter, law key, tree_insert _ _ stringLaw equal]

def assessmentStatus : economy.ExchangeAssessment AccountId A RateId Role N M → economy.AssessmentStatus
  | .Applicable _ _ => .Applicable
  | .Infeasible _ _ => .Infeasible
  | .Invalid _ => .Invalid

def assessmentAccounts : economy.ExchangeAssessment AccountId A RateId Role N M → List (economy.AccountAssessment AccountId A N)
  | .Applicable values _ | .Infeasible values _ => values.val
  | .Invalid _ => []

def assessmentShortfalls : economy.ExchangeAssessment AccountId A RateId Role N M → List (economy.AccountShortfall AccountId A N)
  | .Infeasible _ values => values.val
  | _ => []

def assessmentDeltas : economy.ExchangeAssessment AccountId A RateId Role N M → Option (List (exchange.AccountDelta AccountId A N))
  | .Applicable _ values => some values.val
  | _ => none

def assessmentIssues : economy.ExchangeAssessment AccountId A RateId Role N M → List (economy.ApplyError RateId Role AccountId A N M)
  | .Invalid values => values.val
  | _ => []

theorem result_map_ok {X Y : Type} (f : X → Y) (value : X) :
    f <$> (Result.ok value) = Result.ok (f value) := by
  change Aeneas.Std.bind (.ok value) (fun x => .ok (f x)) = _
  rw [bind_ok]

theorem assessment_accessors (inst : quantity.QuantityScalar N M)
    (value : economy.ExchangeAssessment AccountId A RateId Role N M) :
    economy.ExchangeAssessment.status inst value = .ok (assessmentStatus value) ∧
    (do let values ← economy.ExchangeAssessment.accounts inst value; pure values.val) = .ok (assessmentAccounts value) ∧
    (do let values ← economy.ExchangeAssessment.shortfalls inst value; pure values.val) = .ok (assessmentShortfalls value) ∧
    (do let values ← economy.ExchangeAssessment.projected_deltas inst value; pure (values.map Slice.val)) = .ok (assessmentDeltas value) ∧
    (do let values ← economy.ExchangeAssessment.issues inst value; pure values.val) = .ok (assessmentIssues value) := by
  cases value <;> simp [economy.ExchangeAssessment.status, economy.ExchangeAssessment.accounts,
    economy.ExchangeAssessment.shortfalls, economy.ExchangeAssessment.projected_deltas, economy.ExchangeAssessment.issues,
    assessmentStatus, assessmentAccounts, assessmentShortfalls, assessmentDeltas, assessmentIssues,
    alloc.vec.Vec.deref, Array.to_slice, Std.Array.empty, result_map_ok]

theorem condition_name (value : rate.RateCondition Role A N) (fits : value.name.toByteArray.size ≤ Usize.max) :
    rate.RateCondition.impl.name value = .ok (encodeString value.name fits) := by
  exact DiagnosticRefinement.deref_exact value.name fits

theorem amount_accessors (value : amount.AssetAmount A N) :
    amount.AssetAmount.impl.asset value = .ok value.asset ∧
    amount.AssetAmount.impl.quantity value = .ok value.quantity ∧
    amount.AssetAmount.into_parts value = .ok (value.asset, value.quantity) := ⟨rfl, rfl, rfl⟩

theorem account_accessors (value : account.Account A N) :
    account.Account.impl.balances value = .ok value.balances ∧
    account.Account.into_balances value = .ok value.balances := ⟨rfl, rfl⟩

theorem delta_accessors (value : exchange.AccountDelta AccountId A N) :
    exchange.AccountDelta.impl.account value = .ok value.account ∧
    exchange.AccountDelta.impl.consumed value = .ok value.consumed ∧
    exchange.AccountDelta.impl.produced value = .ok value.produced ∧
    exchange.AccountDelta.impl.preserved value = .ok value.preserved := ⟨rfl, rfl, rfl, rfl⟩

theorem receipt_accessors (value : exchange.Receipt RateId Role AccountId A N) :
    exchange.Receipt.exchange value = .ok value.proposal ∧
    exchange.Receipt.impl.deltas value = .ok (alloc.vec.Vec.deref value.deltas) := ⟨rfl, rfl⟩

theorem account_assessment_accessors (value : economy.AccountAssessment AccountId A N) :
    economy.AccountAssessment.impl.account value = .ok value.account ∧
    economy.AccountAssessment.impl.available value = .ok value.available ∧
    economy.AccountAssessment.impl.required value = .ok value.required ∧
    economy.AccountAssessment.impl.consumed value = .ok value.consumed ∧
    economy.AccountAssessment.impl.produced value = .ok value.produced ∧
    economy.AccountAssessment.impl.preserved value = .ok value.preserved := ⟨rfl, rfl, rfl, rfl, rfl, rfl⟩

theorem account_shortfall_accessors (value : economy.AccountShortfall AccountId A N) :
    economy.AccountShortfall.impl.account value = .ok value.account ∧
    economy.AccountShortfall.impl.missing value = .ok value.missing := ⟨rfl, rfl⟩

theorem model_error_accessors (value : economy.ModelBuildError AccountId RateId) :
    economy.ModelBuildError.impl.issues value = .ok (alloc.vec.Vec.deref value.issues) := rfl

theorem computed_accessors (value : rate.ComputedAmount Role A N) :
    rate.ComputedAmount.impl.role value = .ok value.role ∧
    rate.ComputedAmount.impl.asset value = .ok value.asset ∧
    rate.ComputedAmount.impl.quantity value = .ok value.quantity := ⟨rfl, rfl, rfl⟩

theorem condition_accessors (value : rate.RateCondition Role A N) :
    rate.RateCondition.impl.left value = .ok value.left ∧
    rate.RateCondition.impl.comparison value = .ok value.comparison ∧
    rate.RateCondition.impl.right value = .ok value.right := ⟨rfl, rfl, rfl⟩

theorem proposal_accessors (ordInst : core.cmp.Ord Role) (value : exchange.Exchange RateId Role AccountId N) :
    exchange.Exchange.impl.rate ordInst value = .ok value.rate ∧
    exchange.Exchange.impl.bindings ordInst value = .ok value.bindings ∧
    exchange.Exchange.impl.units ordInst value = .ok value.units ∧
    exchange.Exchange.impl.parameters ordInst value = .ok value.parameters := ⟨rfl, rfl, rfl, rfl⟩

end Axionomy.TypedDataRefinement
