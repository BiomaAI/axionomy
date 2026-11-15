import Axionomy.Ordered

/-!
# Paths, in their natural order

Every identifier is written as a path: segments separated by `/`, each a
number written in decimal or a word. Paths are ordered as the kernel orders
them: segment by segment, numbers by value and before words, words by
their characters, and a path before the longer paths it starts, so
`job/2` comes before `job/10`. A name compared as text, such as an
exchange's parameter, is ordered as the path it reads as, then by its
characters, so two names that read as one path but are written apart
(`07` and `7`) still differ.
-/

namespace Axionomy.Path

open Ordered (OrderLaws)

variable {α : Type}

/-- `cmp` lifted to lists, element by element, a list before the longer
lists it starts. -/
def lex (cmp : α → α → Ordering) : List α → List α → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | a :: as, b :: bs =>
    match cmp a b with
    | .eq => lex cmp as bs
    | .lt => .lt
    | .gt => .gt

theorem lex_laws {cmp : α → α → Ordering} (laws : OrderLaws cmp) : OrderLaws (lex cmp) where
  reflexive xs := by
    induction xs with
    | nil => rfl
    | cons a as ih => simp [lex, laws.reflexive a, ih]
  equal xs := by
    induction xs with
    | nil => intro ys h; cases ys <;> simp_all [lex]
    | cons a as ih =>
      intro ys h
      cases ys with
      | nil => simp [lex] at h
      | cons b bs =>
        cases hab : cmp a b <;> simp [lex, hab] at h
        rw [laws.equal a b hab, ih bs h]
  transitive xs := by
    induction xs with
    | nil =>
      intro ys zs xy yz
      cases ys <;> cases zs <;> simp_all [lex]
    | cons a as ih =>
      intro ys zs xy yz
      cases ys with
      | nil => simp [lex] at xy
      | cons b bs =>
        cases zs with
        | nil => simp [lex] at yz
        | cons c cs =>
          cases hab : cmp a b <;> cases hbc : cmp b c <;> simp [lex, hab, hbc] at xy yz ⊢
          · simp [laws.transitive a b c hab hbc]
          · rw [← laws.equal b c hbc]; simp [hab]
          · rw [laws.equal a b hab]; simp [hbc]
          · have hac : cmp a c = .eq := by
              rw [laws.equal a b hab, laws.equal b c hbc]; exact laws.reflexive c
            simp [hac]
            exact ih bs cs xy yz
  greater xs := by
    induction xs with
    | nil => intro ys h; cases ys <;> simp [lex] at h
    | cons a as ih =>
      intro ys h
      cases ys with
      | nil => rfl
      | cons b bs =>
        cases hab : cmp a b <;> simp [lex, hab] at h ⊢
        · have hba : cmp b a = .eq := by
            rw [laws.equal a b hab]; exact laws.reflexive b
          simp [hba]
          exact ih bs h
        · simp [laws.greater a b hab]

/-- One segment of a path: a number, written in decimal without leading
zeros, or a word, its characters. -/
inductive Segment where
  | number (value : Nat)
  | word (chars : List Char)
  deriving DecidableEq, Repr

/-- Characters by their codes, as bytes compare in the kernel's paths. -/
def compareChar (left right : Char) : Ordering :=
  compare left.toNat right.toNat

theorem compareChar_laws : OrderLaws compareChar where
  reflexive c := Ordered.naturalOrder.reflexive c.toNat
  equal _ _ h := Char.toNat_inj.mp (Ordered.naturalOrder.equal _ _ h)
  transitive c d e := Ordered.naturalOrder.transitive c.toNat d.toNat e.toNat
  greater c d := Ordered.naturalOrder.greater c.toNat d.toNat

/-- Numbers by value and before words; words character by character. -/
def Segment.compare : Segment → Segment → Ordering
  | .number left, .number right => Ord.compare left right
  | .number _, .word _ => .lt
  | .word _, .number _ => .gt
  | .word left, .word right => lex compareChar left right

theorem Segment.compare_laws : OrderLaws Segment.compare where
  reflexive
    | .number value => Ordered.naturalOrder.reflexive value
    | .word chars => (lex_laws compareChar_laws).reflexive chars
  equal
    | .number a, .number b, h => by rw [Ordered.naturalOrder.equal a b h]
    | .word a, .word b, h => by rw [(lex_laws compareChar_laws).equal a b h]
    | .number _, .word _, h => by simp [Segment.compare] at h
    | .word _, .number _, h => by simp [Segment.compare] at h
  transitive
    | .number a, .number b, .number c, ab, bc => Ordered.naturalOrder.transitive a b c ab bc
    | .word a, .word b, .word c, ab, bc => (lex_laws compareChar_laws).transitive a b c ab bc
    | .number _, .number _, .word _, _, _ => rfl
    | .number _, .word _, .word _, _, _ => rfl
    | .number _, .word _, .number _, _, bc => by simp [Segment.compare] at bc
    | .word _, .number _, _, ab, _ => by simp [Segment.compare] at ab
    | .word _, .word _, .number _, _, bc => by simp [Segment.compare] at bc
  greater
    | .number a, .number b, h => Ordered.naturalOrder.greater a b h
    | .word a, .word b, h => (lex_laws compareChar_laws).greater a b h
    | .word _, .number _, _ => rfl
    | .number _, .word _, h => by simp [Segment.compare] at h

/-- Paths in their natural order. -/
def compare : List Segment → List Segment → Ordering :=
  lex Segment.compare

theorem orderLaws : OrderLaws compare :=
  lex_laws Segment.compare_laws

theorem lawful : Ordered.Lawful compare :=
  orderLaws.lawful

/-- A segment written as text: a number where it is all digits, else a word. -/
def segmentOf (text : String) : Segment :=
  if !text.isEmpty && text.all Char.isDigit then .number text.toNat! else .word text.toList

/-- The path a text reads as, its segments between `/`. -/
def ofText (text : String) : List Segment :=
  (text.splitOn "/").map segmentOf

/-- Names compared as the paths they read as, then by their characters. -/
def compareText (left right : String) : Ordering :=
  match compare (ofText left) (ofText right) with
  | .eq => lex compareChar left.toList right.toList
  | .lt => .lt
  | .gt => .gt

theorem compareText_laws : OrderLaws compareText where
  reflexive text := by
    simp [compareText, orderLaws.reflexive, (lex_laws compareChar_laws).reflexive]
  equal left right h := by
    unfold compareText at h
    cases hpath : compare (ofText left) (ofText right) <;> simp [hpath] at h
    exact String.toList_inj.mp ((lex_laws compareChar_laws).equal _ _ h)
  transitive a b c ab bc := by
    unfold compareText at ab bc ⊢
    cases hab : compare (ofText a) (ofText b) <;> cases hbc : compare (ofText b) (ofText c) <;>
      simp [hab, hbc] at ab bc ⊢
    · simp [orderLaws.transitive _ _ _ hab hbc]
    · rw [← orderLaws.equal _ _ hbc]; simp [hab]
    · rw [orderLaws.equal _ _ hab]; simp [hbc]
    · rw [orderLaws.equal _ _ hab, orderLaws.equal _ _ hbc, orderLaws.reflexive]
      exact (lex_laws compareChar_laws).transitive _ _ _ ab bc
  greater a b h := by
    unfold compareText at h ⊢
    cases hab : compare (ofText a) (ofText b) <;> simp [hab] at h
    · rw [orderLaws.equal _ _ hab, orderLaws.reflexive]
      exact (lex_laws compareChar_laws).greater _ _ h
    · simp [orderLaws.greater _ _ hab]

end Axionomy.Path
