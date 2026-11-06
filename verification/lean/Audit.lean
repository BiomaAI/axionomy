import Lean
import Axionomy

open Lean Elab Command

/-! Fail closed on unfinished proofs, native-decision trust, or project axioms.
This checks kernel dependencies, including dependencies hidden in helpers. -/
private def allowedAxiom (name : Name) : Bool :=
  [``propext, ``Classical.choice, ``Quot.sound].contains name

elab "#audit_axionomy" : command => do
  let mut count : Nat := 0
  for (name, info) in (← getEnv).constants.toList do
    let components := name.toString.splitOn "."
    if components.contains "Axionomy" then
      for dependency in ← collectAxioms name do
        unless allowedAxiom dependency do
          throwError "{name} depends on forbidden axiom {dependency}"
      if info.isTheorem then count := count + 1
  if count = 0 then throwError "no Axionomy theorems were audited"
  logInfo m!"Audited {count} Axionomy theorems; only standard Lean foundations are allowed."

elab "#require_theorem " name:ident : command => do
  let constant ← getConstInfo name.getId
  unless constant.isTheorem do throwError "{name} is not a theorem"
  unless (`Axionomy).isPrefixOf name.getId do
    throwError "{name} is not an Axionomy theorem"
  for dependency in ← collectAxioms name.getId do
    unless allowedAxiom dependency do
      throwError "{name} depends on forbidden axiom {dependency}"
