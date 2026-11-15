import Axionomy.Path

/-!
# Patterns: paths with variables

A rate may have variables: a pattern is a path in which some segments are
variables, `{name}`, each standing for exactly one segment. A rate's
identifier is a pattern; so is each asset and account it names. An
identifier written in full, such as `stabilize/1/east`, matches a pattern of
as many segments whose literal segments it repeats, `stabilize/{patient}/
{location}`, and gives each variable the segment in its place. Substituting
those values back writes the identifier again.
-/

namespace Axionomy.Pattern

open Path (Segment)

/-- One segment of a pattern: written out, or a variable. -/
inductive Piece where
  | segment (value : Segment)
  | var (name : String)
  deriving DecidableEq, Repr

abbrev Pattern := List Piece

/-- The values a variable may take: whole numbers from `first` to `last`, or
the segments listed. -/
inductive Domain where
  | range (first last : Nat)
  | values (values : List Segment)
  deriving DecidableEq, Repr

def Domain.contains : Domain → Segment → Bool
  | .range first last, .number value => first ≤ value && value ≤ last
  | .range _ _, .word _ => false
  | .values listed, segment => listed.contains segment

/-- A variable and its values. -/
structure Variable where
  name : String
  domain : Domain
  deriving DecidableEq, Repr

/-- Each variable's value. -/
abbrev Arguments := List (String × Segment)

def lookup (arguments : Arguments) (name : String) : Option Segment :=
  (arguments.find? (·.1 == name)).map (·.2)

/-- The variables of a pattern, in order. -/
def variables : Pattern → List String
  | [] => []
  | .segment _ :: rest => variables rest
  | .var name :: rest => name :: variables rest

/-- `pattern` written with each variable's value; none where a variable has
no value. -/
def substitute (arguments : Arguments) : Pattern → Option (List Segment)
  | [] => some []
  | .segment value :: rest => (value :: ·) <$> substitute arguments rest
  | .var name :: rest => do
    let value ← lookup arguments name
    (value :: ·) <$> substitute arguments rest

/-- The values a path gives a pattern's variables, in order: none unless the
path has as many segments and repeats each written one. -/
def «match» : Pattern → List Segment → Option Arguments
  | [], [] => some []
  | .segment expected :: rest, segment :: path =>
    if expected = segment then «match» rest path else none
  | .var name :: rest, segment :: path => ((name, segment) :: ·) <$> «match» rest path
  | _, _ => none

/-- Two patterns no path matches both: of different lengths, or written
differently at a position where both are written out. -/
def disjoint : Pattern → Pattern → Bool
  | [], [] => false
  | [], _ :: _ => true
  | _ :: _, [] => true
  | .segment left :: rest, .segment right :: others => left != right || disjoint rest others
  | _ :: rest, _ :: others => disjoint rest others

/-- A value for a variable a pattern does not have changes nothing it writes. -/
theorem substitute_cons_absent {name : String} {value : Segment} {arguments : Arguments} :
    ∀ {pattern : Pattern}, name ∉ variables pattern →
      substitute ((name, value) :: arguments) pattern = substitute arguments pattern
  | [], _ => rfl
  | .segment _ :: rest, absent => by
    simp [substitute, substitute_cons_absent (pattern := rest) (by simpa [variables] using absent)]
  | .var other :: rest, absent => by
    simp only [variables, List.mem_cons, not_or] at absent
    have apart : (name == other) = false := by simpa using absent.1
    simp [substitute, lookup, List.find?, apart, substitute_cons_absent (pattern := rest) absent.2]

/-- A path matching a pattern no variable of which appears twice is that
pattern written with the values the match gives: writing and reading an
identifier are inverses. -/
theorem match_substitute : ∀ {pattern : Pattern} {path : List Segment} {arguments : Arguments},
    (variables pattern).Nodup → «match» pattern path = some arguments →
      substitute arguments pattern = some path
  | [], [], _, _, matched => by simp [«match»] at matched; subst matched; rfl
  | [], _ :: _, _, _, matched => by simp [«match»] at matched
  | piece :: _, [], _, _, matched => by cases piece <;> simp [«match»] at matched
  | .segment expected :: rest, segment :: path, arguments, nodup, matched => by
    by_cases same : expected = segment
    · simp only [«match», same, ite_true] at matched
      subst same
      simp [substitute, match_substitute (pattern := rest) (by simpa [variables] using nodup) matched]
    · simp [«match», same] at matched
  | .var name :: rest, segment :: path, arguments, nodup, matched => by
    simp only [«match»] at matched
    cases found : «match» rest path with
    | none => simp [found] at matched
    | some others =>
      simp only [found, Option.map_eq_map, Option.map_some] at matched
      cases matched
      simp only [variables, List.nodup_cons] at nodup
      simp [substitute, lookup, substitute_cons_absent nodup.1, match_substitute nodup.2 found]

/-- No path matches two disjoint patterns. -/
theorem disjoint_match {left right : Pattern} (apart : disjoint left right = true)
    (path : List Segment) : «match» left path = none ∨ «match» right path = none := by
  induction left generalizing right path with
  | nil =>
    cases right with
    | nil => simp [disjoint] at apart
    | cons piece rest => cases path <;> cases piece <;> simp [«match»]
  | cons piece rest ih =>
    cases right with
    | nil => cases path <;> cases piece <;> simp [«match»]
    | cons other others =>
      cases path with
      | nil => cases piece <;> simp [«match»]
      | cons segment path =>
        cases piece with
        | segment expected =>
          cases other with
          | segment written =>
            simp only [disjoint, Bool.or_eq_true, bne_iff_ne, ne_eq] at apart
            by_cases left_same : expected = segment
            · by_cases right_same : written = segment
              · subst left_same; subst right_same
                rcases apart with differ | rest_apart
                · exact absurd rfl differ
                · simpa [«match»] using ih rest_apart path
              · right; simp [«match», right_same]
            · left; simp [«match», left_same]
          | var name =>
            simp only [disjoint] at apart
            by_cases left_same : expected = segment
            · rcases ih apart path with none | none
              · left; simp [«match», left_same, none]
              · right; simp [«match», none]
            · left; simp [«match», left_same]
        | var name =>
          cases other with
          | segment written =>
            simp only [disjoint] at apart
            by_cases right_same : written = segment
            · rcases ih apart path with none | none
              · left; simp [«match», none]
              · right; simp [«match», right_same, none]
            · right; simp [«match», right_same]
          | var other =>
            simp only [disjoint] at apart
            rcases ih apart path with none | none
            · left; simp [«match», none]
            · right; simp [«match», none]

end Axionomy.Pattern
