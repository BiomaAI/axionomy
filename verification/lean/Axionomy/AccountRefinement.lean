import Axionomy.AccountMap
import Axionomy.BasketRefinement
import AccountExtraction

namespace Axionomy.AccountRefinement
open Aeneas Aeneas.Std AccountExtracted

abbrev scalar := U64.Insts.Account_extractionQuantityQuantityScalarI128
abbrev LawfulEquality {A : Type} [DecidableEq A] := @BasketRefinement.LawfulEquality A _
def LawfulClone {A : Type} (inst : core.clone.Clone A) : Prop := ∀ value, inst.clone value = .ok value

def difference (left right : Std.U64) : Std.U64 :=
  ⟨BitVec.ofNat 64 (left.val - right.val)⟩

theorem difference_value (left right : Std.U64) :
    (difference left right).val = left.val - right.val := by
  change (left.val - right.val) % 2^64 = left.val - right.val
  exact Nat.mod_eq_of_lt (Nat.lt_of_le_of_lt (Nat.sub_le _ _) left.hBounds)

def arithmetic : Account.Arithmetic Std.U64 :=
  ⟨0#u64, fun value => decide (value = 0#u64), Std.U64.checked_add,
    difference, fun left right => decide (left.val < right.val)⟩

theorem add_exact (left right : Std.U64) :
    (arithmetic.add left right).map UScalar.val =
      Quantity.add Quantity.u64Max left.val right.val := by
  have spec := Std.U64.checked_add_bv_spec left right
  cases h : Std.U64.checked_add left right with
  | none =>
    simp only [h] at spec
    have large : ¬ left.val + right.val ≤ Quantity.u64Max := by
      simpa [Quantity.u64Max, Std.U64.max, Std.U64.numBits] using Nat.not_le.mpr spec
    simp [arithmetic, h, Quantity.add, Quantity.checked, large]
  | some value =>
    simp only [h] at spec
    have bounded : left.val + right.val ≤ Quantity.u64Max := by
      simpa [Quantity.u64Max, Std.U64.max, Std.U64.numBits] using spec.1
    simp [arithmetic, h, Quantity.add, Quantity.checked, bounded, spec.2.1]

@[simp] theorem add_call (left right : Std.U64) :
    quantity.Quantity.checked_add scalar left right = .ok (arithmetic.add left right) := by
  simp only [quantity.Quantity.checked_add,
    U64.Insts.Num_traitsOpsCheckedCheckedAdd.checked_add, bind_ok]
  cases h : Std.U64.checked_add left right <;>
    simp [quantity.Quantity.from_checked, quantity.Quantity.try_from_scalar,
      U64.Insts.Account_extractionQuantityQuantityScalarI128.is_nonnegative, arithmetic, h]

@[simp] theorem sub_call (left right : Std.U64) (enough : right.val ≤ left.val) :
    quantity.Quantity.checked_sub scalar left right = .ok (some (difference left right)) := by
  have spec := Std.U64.checked_sub_bv_spec left right
  have result : Std.U64.checked_sub left right = some (difference left right) := by
    cases h : Std.U64.checked_sub left right with
    | none => simp only [h] at spec; omega
    | some value =>
      simp only [h] at spec
      have equal : value = difference left right := UScalar.eq_of_val_eq (by rw [difference_value]; exact spec.2.1)
      simp [equal]
  simp [quantity.Quantity.checked_sub, U64.Insts.Num_traitsOpsCheckedCheckedSub.checked_sub,
    result, quantity.Quantity.from_checked, quantity.Quantity.try_from_scalar,
    U64.Insts.Account_extractionQuantityQuantityScalarI128.is_nonnegative]

@[simp] theorem zero_call (value : Std.U64) :
    quantity.Quantity.is_zero scalar value = .ok (arithmetic.isZero value) := by
  rfl

theorem equivalent_eq {A : Type} (eqInst : core.cmp.Eq A) :
    equivalent.Equivalent.Blanket.equivalent eqInst (core.borrow.Borrow.Blanket A) =
      eqInst.partialEqInst.eq := by
  funext left right
  simp [equivalent.Equivalent.Blanket.equivalent, core.borrow.Borrow.Blanket.borrow]

theorem get_exact {A Q : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (lawful : LawfulEquality eqInst) (asset : A) (entries : List (A × Q)) :
    mapGet eqInst.partialEqInst.eq asset entries = .ok (Account.find? asset entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases equal : asset = key
    · subst asset; simp [mapGet, lawful key key, Account.find?, ih]
    · simp [mapGet, lawful asset key, Account.find?, ih, equal]

theorem insert_exact {A Q : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (lawful : LawfulEquality eqInst) (asset : A) (value : Q) (entries : List (A × Q)) :
    BasketExtracted.mapInsert eqInst.partialEqInst.eq asset value entries =
      .ok (Account.find? asset entries, Account.upsert asset value entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, old⟩
    by_cases equal : asset = key
    · subst asset; simp [BasketExtracted.mapInsert, lawful key key, Account.find?, Account.upsert, ih]
    · simp [BasketExtracted.mapInsert, lawful asset key, Account.find?, Account.upsert, ih, equal]

theorem remove_exact {A Q : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (lawful : LawfulEquality eqInst) (asset : A) (entries : List (A × Q)) :
    mapRemove eqInst.partialEqInst.eq asset entries =
      .ok (Account.find? asset entries, Account.erase asset entries) := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    by_cases equal : asset = key
    · subst asset; simp [mapRemove, lawful key key, Account.find?, Account.erase, ih]
    · simp [mapRemove, lawful asset key, Account.find?, Account.erase, ih, equal]

@[simp] theorem quantity_call {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (lawful : LawfulEquality eqInst)
    (entries : List (A × Std.U64)) (asset : A) :
    basket.Basket.quantity eqInst hashInst scalar ⟨entries⟩ asset =
      .ok (Account.balance arithmetic.zero entries asset) := by
  simp only [basket.Basket.quantity, indexmap.map.IndexMap.get, equivalent_eq,
    get_exact eqInst lawful, bind_ok]
  cases h : Account.find? asset entries <;>
    simp [core.option.OptionShared0T.cloned, core.option.Option.unwrap_or_default,
      quantity.Quantity.Insts.CoreCloneClone.clone, quantity.Quantity.Insts.CoreDefaultDefault.default,
      U64.Insts.Num_traitsIdentitiesZero.zero, Account.balance, arithmetic, h]

@[simp] theorem store_call {A : Type} [DecidableEq A] (eqInst : core.cmp.Eq A)
    (hashInst : core.hash.Hash A) (lawful : LawfulEquality eqInst)
    (entries : List (A × Std.U64)) (asset : A) (value : Std.U64) :
    basket.Basket.insert eqInst hashInst scalar ⟨entries⟩ asset value =
      .ok (Account.find? asset entries, ⟨Account.store arithmetic entries asset value⟩) := by
  simp only [basket.Basket.insert, zero_call, bind_ok]
  unfold Account.store
  split <;> simp [indexmap.map.IndexMap.shift_remove, indexmap.map.IndexMap.insert,
    equivalent_eq, remove_exact eqInst lawful, insert_exact eqInst lawful]

theorem clone_exact {A Q : Type} (cloneInst : core.clone.Clone A) (valueInst : core.clone.Clone Q)
    (lawful : LawfulClone cloneInst) (valueLawful : LawfulClone valueInst)
    (entries : List (A × Q)) : cloneEntries cloneInst valueInst entries = .ok entries := by
  induction entries with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨key, value⟩
    simp [cloneEntries, lawful key, valueLawful value, ih]

@[simp] theorem clone_call {A : Type} (cloneInst : core.clone.Clone A)
    (lawful : LawfulClone cloneInst) (entries : List (A × Std.U64)) :
    basket.Basket.Insts.CoreCloneClone.clone cloneInst scalar.corecloneCloneInst ⟨entries⟩ = .ok ⟨entries⟩ := by
  simp only [basket.Basket.Insts.CoreCloneClone.clone, indexmap.map.IndexMap.Insts.CoreCloneClone.clone]
  have values : LawfulClone (quantity.Quantity.Insts.CoreCloneClone scalar.corecloneCloneInst) := by
    intro value
    simp [quantity.Quantity.Insts.CoreCloneClone.clone]
  simp [clone_exact cloneInst _ lawful values]

@[simp] theorem lt_call (left right : Std.U64) :
    core.cmp.PartialOrd.lt.trait_default
      (quantity.Quantity.Insts.CoreCmpPartialOrdQuantity core.cmp.PartialOrdU64)
      left right = .ok (arithmetic.lt left right) := by
  simp [core.cmp.PartialOrd.lt.trait_default, core.cmp.PartialOrd.lt.default, core.cmp.PartialOrd.lt_body,
    quantity.Quantity.Insts.CoreCmpPartialOrdQuantity.partial_cmp,
    core.cmp.impls.PartialOrdU64.partial_cmp, arithmetic, Nat.compare_eq_lt]

theorem natural_hom : Account.Hom arithmetic (Account.naturals Quantity.u64Max) (@UScalar.val .U64) where
  zero := rfl
  isZero := by
    intro value
    by_cases zero : value = 0#u64
    · subst value; rfl
    · have nonzero : value.val ≠ 0 := fun h => zero (UScalar.eq_of_val_eq h)
      simp [arithmetic, Account.naturals, zero, nonzero]
  add := add_exact
  sub := difference_value
  lt := by intros; rfl

section Updates
variable {A : Type} [DecidableEq A]
variable (cloneInst : core.clone.Clone A) (eqInst : core.cmp.Eq A) (hashInst : core.hash.Hash A)
variable (hClone : LawfulClone cloneInst) (hEq : LawfulEquality eqInst)

-- The original snapshot is explicitly returned on every overflow branch.
def creditResult (original : List (A × Std.U64)) (result : Except A (List (A × Std.U64))) :
    core.result.Result Unit (account.AccountError A Std.U64 Std.I128) × basket.Basket A Std.U64 :=
  match result with
  | .ok updated => (.Ok (), ⟨updated⟩)
  | .error asset => (.Err (.Overflow asset), ⟨original⟩)

include hClone hEq in
theorem deposit_body (original remaining updated : List (A × Std.U64)) :
    account.Account.deposit_loop.body cloneInst eqInst hashInst scalar ⟨original⟩ remaining ⟨updated⟩ =
      .ok (match remaining with
        | [] => .done (.Ok (), ⟨updated⟩)
        | (asset, amount) :: rest =>
          match arithmetic.add (Account.balance arithmetic.zero updated asset) amount with
          | none => .done (.Err (.Overflow asset), ⟨original⟩)
          | some value => .cont (rest, ⟨Account.store arithmetic updated asset value⟩)) := by
  cases remaining with
  | nil => simp [account.Account.deposit_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, amount⟩
    simp only [account.Account.deposit_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, bind_ok,
      quantity_call eqInst hashInst hEq, add_call]
    cases h : arithmetic.add (Account.balance arithmetic.zero updated asset) amount <;>
      simp [hClone asset, store_call eqInst hashInst hEq, h]

include hClone hEq in
theorem deposit_loop (original remaining updated : List (A × Std.U64)) :
    account.Account.deposit_loop cloneInst eqInst hashInst scalar remaining ⟨original⟩ ⟨updated⟩ =
      .ok (creditResult original (Account.credit arithmetic updated remaining)) := by
  induction remaining generalizing updated with
  | nil =>
    unfold account.Account.deposit_loop
    rw [loop.eq_1]
    simp [deposit_body cloneInst eqInst hashInst hClone hEq, bind_ok, Account.credit, creditResult]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    unfold account.Account.deposit_loop
    rw [loop.eq_1]
    simp only [deposit_body cloneInst eqInst hashInst hClone hEq original ((asset, amount) :: rest) updated]
    cases h : arithmetic.add (Account.balance arithmetic.zero updated asset) amount with
    | none => simp [h, Account.credit, creditResult]
    | some value =>
      simp only [bind_ok]
      change account.Account.deposit_loop cloneInst eqInst hashInst scalar rest ⟨original⟩
        ⟨Account.store arithmetic updated asset value⟩ = _
      rw [ih]
      simp [Account.credit, h]

def encodeError : Account.UpdateError A Std.U64 → account.AccountError A Std.U64 Std.I128
  | .overflow asset => .Overflow asset
  | .insufficient missing => .InsufficientBalance ⟨missing⟩

def encodeOutcome (outcome : Except (Account.UpdateError A Std.U64) Unit × List (A × Std.U64)) :
    core.result.Result Unit (account.AccountError A Std.U64 Std.I128) × account.Account A Std.U64 :=
  (match outcome.1 with | .ok () => .Ok () | .error error => .Err (encodeError error), ⟨⟨outcome.2⟩⟩)

include hClone hEq in
theorem deposit_refines (state amounts : List (A × Std.U64)) :
    AccountExtracted.deposit cloneInst eqInst hashInst ⟨⟨state⟩⟩ ⟨amounts⟩ =
      .ok (encodeOutcome (Account.deposit arithmetic state amounts)) := by
  simp only [AccountExtracted.deposit, account.Account.deposit, clone_call cloneInst hClone,
    basket.Basket.iter, indexmap.map.IndexMap.iter, bind_ok,
    deposit_loop cloneInst eqInst hashInst hClone hEq]
  cases h : Account.credit arithmetic state amounts <;>
    simp [creditResult, encodeOutcome, encodeError, Account.deposit, h]

include hClone hEq in
theorem shortfall_body (state remaining collected : List (A × Std.U64)) :
    basket.Basket.shortfall_loop.body cloneInst eqInst hashInst scalar ⟨state⟩ remaining ⟨collected⟩ =
      .ok (match remaining with
        | [] => .done ⟨collected⟩
        | (asset, amount) :: rest =>
          let available := Account.balance arithmetic.zero state asset
          if arithmetic.lt available amount then
            .cont (rest, ⟨Account.store arithmetic collected asset (difference amount available)⟩)
          else .cont (rest, ⟨collected⟩)) := by
  cases remaining with
  | nil => simp [basket.Basket.shortfall_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]
  | cons entry rest =>
    rcases entry with ⟨asset, amount⟩
    simp only [basket.Basket.shortfall_loop.body,
      indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next, bind_ok,
      quantity_call eqInst hashInst hEq, lt_call]
    cases h : arithmetic.lt (Account.balance arithmetic.zero state asset) amount with
    | false => simp [h]
    | true =>
      have less : (Account.balance arithmetic.zero state asset).val < amount.val := by
        exact of_decide_eq_true h
      simp [h, sub_call amount _ (Nat.le_of_lt less), core.option.Option.expect,
        hClone asset, store_call eqInst hashInst hEq]

include hClone hEq in
theorem shortfall_loop (state remaining collected : List (A × Std.U64))
    (unique : (remaining.map Prod.fst).Nodup)
    (fresh : ∀ asset ∈ remaining.map Prod.fst, asset ∉ collected.map Prod.fst) :
    basket.Basket.shortfall_loop cloneInst eqInst hashInst scalar remaining ⟨state⟩ ⟨collected⟩ =
      .ok ⟨collected ++ Account.shortfall arithmetic state remaining⟩ := by
  induction remaining generalizing collected with
  | nil =>
    unfold basket.Basket.shortfall_loop
    rw [loop.eq_1]
    simp [shortfall_body cloneInst eqInst hashInst hClone hEq, bind_ok, Account.shortfall]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    simp only [List.map_cons, List.nodup_cons] at unique
    have tailFresh : ∀ key ∈ rest.map Prod.fst, key ∉ collected.map Prod.fst := by
      intro key member
      exact fresh key (by simp [member])
    unfold basket.Basket.shortfall_loop
    rw [loop.eq_1]
    simp only [shortfall_body cloneInst eqInst hashInst hClone hEq state ((asset, amount) :: rest) collected]
    cases h : arithmetic.lt (Account.balance arithmetic.zero state asset) amount with
    | false =>
      simp only [Bool.false_eq_true, ↓reduceIte, bind_ok]
      change basket.Basket.shortfall_loop cloneInst eqInst hashInst scalar rest ⟨state⟩ ⟨collected⟩ = _
      rw [ih collected unique.2 tailFresh]
      simp [Account.shortfall, h]
    | true =>
      have less : (Account.balance arithmetic.zero state asset).val < amount.val := of_decide_eq_true h
      have nonzero : arithmetic.isZero (difference amount (Account.balance arithmetic.zero state asset)) = false := by
        simp only [arithmetic, decide_eq_false_iff_not]
        intro equal
        have hv := congrArg UScalar.val equal
        rw [difference_value] at hv
        change amount.val - (Account.balance arithmetic.zero state asset).val = 0 at hv
        omega
      have append : Account.store arithmetic collected asset
          (difference amount (Account.balance arithmetic.zero state asset)) =
          collected ++ [(asset, difference amount (Account.balance arithmetic.zero state asset))] := by
        simp only [Account.store, nonzero, Bool.false_eq_true, ↓reduceIte]
        exact Account.upsert_fresh _ _ _ (fresh asset (by simp))
      have nextFresh : ∀ key ∈ rest.map Prod.fst, key ∉
          (collected ++ [(asset, difference amount (Account.balance arithmetic.zero state asset))]).map Prod.fst := by
        intro key member
        have different : key ≠ asset := by intro equal; subst key; exact unique.1 member
        simp [tailFresh key member, different]
      simp only [↓reduceIte, bind_ok, append]
      change basket.Basket.shortfall_loop cloneInst eqInst hashInst scalar rest ⟨state⟩
        ⟨collected ++ [(asset, difference amount (Account.balance arithmetic.zero state asset))]⟩ = _
      rw [ih _ unique.2 nextFresh]
      simp only [arithmetic] at less
      simp [Account.shortfall, List.append_assoc, arithmetic, less]

include hClone hEq in
theorem shortfall_refines (state required : List (A × Std.U64))
    (unique : (required.map Prod.fst).Nodup) :
    basket.Basket.shortfall cloneInst eqInst hashInst scalar ⟨state⟩ ⟨required⟩ =
      .ok ⟨Account.shortfall arithmetic state required⟩ := by
  simp only [basket.Basket.shortfall, basket.Basket.new, indexmap.map.IndexMapKVRandomState.new,
    basket.Basket.iter, indexmap.map.IndexMap.iter, bind_ok]
  simpa using shortfall_loop cloneInst eqInst hashInst hClone hEq state required [] unique (by simp)

omit [DecidableEq A] in
theorem withdraw_body_nil (state : List (A × Std.U64)) :
    account.Account.withdraw_loop.body cloneInst eqInst hashInst scalar [] ⟨state⟩ = .ok (.done ⟨state⟩) := by
  simp [account.Account.withdraw_loop.body,
    indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next]

include hClone hEq in
theorem withdraw_body_cons (asset : A) (amount : Std.U64) (rest state : List (A × Std.U64))
    (enough : amount.val ≤ (Account.balance arithmetic.zero state asset).val) :
    account.Account.withdraw_loop.body cloneInst eqInst hashInst scalar ((asset, amount) :: rest) ⟨state⟩ =
      .ok (.cont (rest, ⟨Account.store arithmetic state asset
        (difference (Account.balance arithmetic.zero state asset) amount)⟩)) := by
  simp [account.Account.withdraw_loop.body,
    indexmap.map.iter.Iter.Insts.CoreIterTraitsIteratorIteratorPairSharedAKSharedAV.next,
    quantity_call eqInst hashInst hEq, sub_call _ _ enough, core.option.Option.expect,
    hClone asset, store_call eqInst hashInst hEq]

include hClone hEq in
theorem withdraw_loop (remaining state : List (A × Std.U64))
    (unique : (remaining.map Prod.fst).Nodup)
    (enough : ∀ entry ∈ remaining, entry.2.val ≤ (Account.balance arithmetic.zero state entry.1).val) :
    account.Account.withdraw_loop cloneInst eqInst hashInst scalar remaining ⟨state⟩ =
      .ok ⟨Account.debit arithmetic state remaining⟩ := by
  induction remaining generalizing state with
  | nil =>
    unfold account.Account.withdraw_loop
    rw [loop.eq_1]
    simp [withdraw_body_nil cloneInst eqInst hashInst, bind_ok, Account.debit]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    simp only [List.map_cons, List.nodup_cons] at unique
    have first := enough (asset, amount) (by simp)
    have nextEnough : ∀ entry ∈ rest, entry.2.val ≤
        (Account.balance arithmetic.zero (Account.store arithmetic state asset
          (difference (Account.balance arithmetic.zero state asset) amount)) entry.1).val := by
      intro entry member
      have different : entry.1 ≠ asset := by
        intro equal
        exact unique.1 (by rw [← equal]; exact List.mem_map.mpr ⟨entry, member, rfl⟩)
      rw [Account.store_frame arithmetic state asset entry.1 _ different]
      exact enough entry (by simp [member])
    unfold account.Account.withdraw_loop
    rw [loop.eq_1]
    simp only [withdraw_body_cons cloneInst eqInst hashInst hClone hEq asset amount rest state first, bind_ok]
    change account.Account.withdraw_loop cloneInst eqInst hashInst scalar rest
      ⟨Account.store arithmetic state asset (difference (Account.balance arithmetic.zero state asset) amount)⟩ = _
    rw [ih _ unique.2 nextEnough]
    rfl

include hClone hEq in
theorem withdraw_refines (state amounts : List (A × Std.U64))
    (unique : (amounts.map Prod.fst).Nodup) :
    AccountExtracted.withdraw cloneInst eqInst hashInst ⟨⟨state⟩⟩ ⟨amounts⟩ =
      .ok (encodeOutcome (Account.withdraw arithmetic state amounts)) := by
  simp only [AccountExtracted.withdraw, account.Account.withdraw,
    shortfall_refines cloneInst eqInst hashInst hClone hEq state amounts unique,
    basket.Basket.is_empty, indexmap.map.IndexMap.is_empty, bind_ok]
  by_cases empty : Account.shortfall arithmetic state amounts = []
  · have checks := (Account.shortfall_empty_iff arithmetic state amounts).mp empty
    have enough : ∀ entry ∈ amounts, entry.2.val ≤ (Account.balance arithmetic.zero state entry.1).val := by
      intro entry member
      have noLess := checks entry member
      simp only [arithmetic, decide_eq_false_iff_not] at noLess
      exact Nat.le_of_not_gt noLess
    simp [empty, clone_call cloneInst hClone, basket.Basket.iter, indexmap.map.IndexMap.iter,
      withdraw_loop cloneInst eqInst hashInst hClone hEq amounts state unique enough,
      Account.withdraw, encodeOutcome]
  · simp [List.isEmpty_iff, empty, Account.withdraw, encodeOutcome, encodeError]

def decodeOutcome (outcome : core.result.Result Unit (account.AccountError A Std.U64 Std.I128) ×
    account.Account A Std.U64) : Except (Account.UpdateError A Nat) Unit × List (A × Nat) :=
  (match outcome.1 with
    | .Ok () => .ok ()
    | .Err (.Overflow asset) => .error (.overflow asset)
    | .Err (.InsufficientBalance missing) => .error (.insufficient (Account.mapEntries UScalar.val missing.quantities)),
    Account.mapEntries UScalar.val outcome.2.balances.quantities)

omit [DecidableEq A] in
theorem decode_encode (outcome : Except (Account.UpdateError A Std.U64) Unit × List (A × Std.U64)) :
    decodeOutcome (encodeOutcome outcome) = Account.mapOutcome UScalar.val outcome := by
  rcases outcome with ⟨status, state⟩
  cases status with
  | ok result => cases result; rfl
  | error error => cases error <;> rfl

include hClone hEq in
theorem deposit_refines_naturals (state amounts : List (A × Std.U64)) :
    ∃ result, AccountExtracted.deposit cloneInst eqInst hashInst ⟨⟨state⟩⟩ ⟨amounts⟩ = .ok result ∧
      decodeOutcome result = Account.deposit (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val state) (Account.mapEntries UScalar.val amounts) := by
  refine ⟨encodeOutcome (Account.deposit arithmetic state amounts),
    deposit_refines cloneInst eqInst hashInst hClone hEq state amounts, ?_⟩
  rw [decode_encode, Account.map_deposit arithmetic _ _ natural_hom]

include hClone hEq in
theorem withdraw_refines_naturals (state amounts : List (A × Std.U64))
    (unique : (amounts.map Prod.fst).Nodup) :
    ∃ result, AccountExtracted.withdraw cloneInst eqInst hashInst ⟨⟨state⟩⟩ ⟨amounts⟩ = .ok result ∧
      decodeOutcome result = Account.withdraw (Account.naturals Quantity.u64Max)
        (Account.mapEntries UScalar.val state) (Account.mapEntries UScalar.val amounts) := by
  refine ⟨encodeOutcome (Account.withdraw arithmetic state amounts),
    withdraw_refines cloneInst eqInst hashInst hClone hEq state amounts unique, ?_⟩
  rw [decode_encode, Account.map_withdraw arithmetic _ _ natural_hom]

-- Both premises have concrete witnesses, independent of the requested outcome.
def exactClone (A : Type) : core.clone.Clone A :=
  { clone := fun value => .ok value, clone_from := fun _ source => .ok source }
theorem exactClone_lawful (A : Type) : LawfulClone (exactClone A) := by intro value; rfl

end Updates

end Axionomy.AccountRefinement
