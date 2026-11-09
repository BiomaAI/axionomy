import Axionomy.Account

namespace Axionomy.Conservation

/-- Invariant measures are exact integers: a quantity times a signed weight,
summed over any holdings, never overflows. -/
def exactAdd (left right : Int) : Option Int := some (left + right)

theorem exactAdd_exact (left right result : Int) :
    exactAdd left right = some result ↔ result = left + right := by
  simp [exactAdd, eq_comm]

-- The accumulation is generic in how terms combine; the kernel instantiates it
-- with exact addition.
def accumulate {A Q M : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (total : M) : List (A × Q) → Option M
  | [] => some total
  | (asset, amount) :: rest => do
    let contribution ← weighted asset amount
    let total ← add total contribution
    accumulate weighted add total rest

structure Delta (AccountId A Q : Type) where
  account : AccountId
  consumed : List (A × Q)
  produced : List (A × Q)
  preserved : List (A × Q)

def totals {AccountId A Q M : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (consumed produced : M) :
    List (Delta AccountId A Q) → Option (M × M)
  | [] => some (consumed, produced)
  | delta :: rest => do
    let consumed ← accumulate weighted add consumed delta.consumed
    let produced ← accumulate weighted add produced delta.produced
    totals weighted add consumed produced rest

def conserves {AccountId A Q M : Type} [DecidableEq M]
    (weighted : A → Q → Option M) (add : M → M → Option M)
    (zero : M) (deltas : List (Delta AccountId A Q)) : Option Bool :=
  (totals weighted add zero zero deltas).map fun (consumed, produced) => decide (consumed = produced)

def contribution {A : Type} [DecidableEq A] (weights : List (A × Int))
    (asset : A) (amount : Nat) : Int :=
  (amount : Int) * (Account.find? asset weights).getD 0

def measure {A Q : Type} (weighted : A → Q → Int) (entries : List (A × Q)) : Int :=
  (entries.map fun (asset, amount) => weighted asset amount).sum

theorem accumulate_exact {A Q M : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (value : M → Int) (mathematical : A → Q → Int)
    (weightedExact : ∀ a q m, weighted a q = some m → value m = mathematical a q)
    (addExact : ∀ x y z, add x y = some z → value z = value x + value y)
    (entries : List (A × Q)) (initial final : M)
    (accepted : accumulate weighted add initial entries = some final) :
    value final = value initial + measure mathematical entries := by
  induction entries generalizing initial with
  | nil => simp [accumulate] at accepted; subst final; simp [measure]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    cases hw : weighted asset amount with
    | none => simp [accumulate, hw] at accepted
    | some term =>
      cases ha : add initial term with
      | none => simp [accumulate, hw, ha] at accepted
      | some next =>
        simp [accumulate, hw, ha] at accepted
        rw [ih next accepted, addExact initial term next ha, weightedExact asset amount term hw]
        simp [measure, Int.add_assoc]

theorem totals_exact {AccountId A Q M : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (value : M → Int) (mathematical : A → Q → Int)
    (weightedExact : ∀ a q m, weighted a q = some m → value m = mathematical a q)
    (addExact : ∀ x y z, add x y = some z → value z = value x + value y)
    (deltas : List (Delta AccountId A Q)) (consumed produced : M) (result : M × M)
    (accepted : totals weighted add consumed produced deltas = some result) :
    value result.1 = value consumed + (deltas.map fun d => measure mathematical d.consumed).sum ∧
    value result.2 = value produced + (deltas.map fun d => measure mathematical d.produced).sum := by
  induction deltas generalizing consumed produced with
  | nil => simp [totals] at accepted; subst result; simp
  | cons delta rest ih =>
    cases hc : accumulate weighted add consumed delta.consumed with
    | none => simp [totals, hc] at accepted
    | some nextC =>
      cases hp : accumulate weighted add produced delta.produced with
      | none => simp [totals, hc, hp] at accepted
      | some nextP =>
        simp [totals, hc, hp] at accepted
        obtain ⟨left, right⟩ := ih nextC nextP accepted
        have c := accumulate_exact weighted add value mathematical weightedExact addExact _ _ _ hc
        have p := accumulate_exact weighted add value mathematical weightedExact addExact _ _ _ hp
        simp only [List.map_cons, List.sum_cons]
        constructor <;> omega

theorem conservation_sound {AccountId A Q M : Type} [DecidableEq M]
    (weighted : A → Q → Option M) (add : M → M → Option M)
    (value : M → Int) (mathematical : A → Q → Int)
    (weightedExact : ∀ a q m, weighted a q = some m → value m = mathematical a q)
    (addExact : ∀ x y z, add x y = some z → value z = value x + value y)
    (zero : M) (deltas : List (Delta AccountId A Q))
    (accepted : conserves weighted add zero deltas = some true) :
    (deltas.map fun d => measure mathematical d.consumed).sum =
      (deltas.map fun d => measure mathematical d.produced).sum := by
  cases ht : totals weighted add zero zero deltas with
  | none => simp [conserves, ht] at accepted
  | some result =>
    have equal : result.1 = result.2 := by simpa [conserves, ht] using accepted
    obtain ⟨left, right⟩ := totals_exact weighted add value mathematical weightedExact addExact _ _ _ _ ht
    rw [equal] at left
    omega

theorem map_accumulate {A Q M R : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (mappedWeighted : A → Q → Option R)
    (mappedAdd : R → R → Option R) (value : M → R)
    (weightedMap : ∀ a q, (weighted a q).map value = mappedWeighted a q)
    (addMap : ∀ x y, (add x y).map value = mappedAdd (value x) (value y))
    (entries : List (A × Q)) (initial : M) :
    (accumulate weighted add initial entries).map value =
      accumulate mappedWeighted mappedAdd (value initial) entries := by
  induction entries generalizing initial with
  | nil => rfl
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    have hw := weightedMap asset amount
    cases h : weighted asset amount with
    | none => simp only [h, Option.map_none] at hw; simp [accumulate, h, ← hw]
    | some term =>
      simp only [h, Option.map_some] at hw
      have ha := addMap initial term
      cases k : add initial term with
      | none => simp only [k, Option.map_none] at ha; simp [accumulate, h, k, ← hw, ← ha]
      | some next =>
        simp only [k, Option.map_some] at ha
        simpa [accumulate, h, k, ← hw, ← ha] using ih next

theorem map_totals {AccountId A Q M R : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (mappedWeighted : A → Q → Option R)
    (mappedAdd : R → R → Option R) (value : M → R)
    (weightedMap : ∀ a q, (weighted a q).map value = mappedWeighted a q)
    (addMap : ∀ x y, (add x y).map value = mappedAdd (value x) (value y))
    (deltas : List (Delta AccountId A Q)) (consumed produced : M) :
    (totals weighted add consumed produced deltas).map (fun (c, p) => (value c, value p)) =
      totals mappedWeighted mappedAdd (value consumed) (value produced) deltas := by
  induction deltas generalizing consumed produced with
  | nil => rfl
  | cons delta rest ih =>
    have hc := map_accumulate weighted add mappedWeighted mappedAdd value weightedMap addMap delta.consumed consumed
    have hp := map_accumulate weighted add mappedWeighted mappedAdd value weightedMap addMap delta.produced produced
    cases h : accumulate weighted add consumed delta.consumed with
    | none => simp only [h, Option.map_none] at hc; simp [totals, h, ← hc]
    | some nextC =>
      simp only [h, Option.map_some] at hc
      cases k : accumulate weighted add produced delta.produced with
      | none => simp only [k, Option.map_none] at hp; simp [totals, h, k, ← hc, ← hp]
      | some nextP =>
        simp only [k, Option.map_some] at hp
        simpa [totals, h, k, ← hc, ← hp] using ih nextC nextP

theorem accumulate_append {A Q M : Type} (weighted : A → Q → Option M)
    (add : M → M → Option M) (left right : List (A × Q)) (initial : M) :
    accumulate weighted add initial (left ++ right) =
      (accumulate weighted add initial left).bind (fun next => accumulate weighted add next right) := by
  induction left generalizing initial with
  | nil => simp [accumulate]
  | cons entry rest ih =>
    rcases entry with ⟨asset, amount⟩
    cases hw : weighted asset amount with
    | none => simp [accumulate, hw]
    | some term =>
      cases ha : add initial term <;> simp [accumulate, hw, ha, ih]

end Axionomy.Conservation
