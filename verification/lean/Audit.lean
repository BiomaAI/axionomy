import Lean
import Axionomy

open Lean Elab Command

/-! Fail closed on unfinished proofs, native-decision trust, or project axioms.
This checks kernel dependencies, including dependencies hidden in helpers. -/
private def allowedAxiom (declName : Name) : Bool :=
  [``propext, ``Classical.choice, ``Quot.sound].contains declName

elab "#audit_axionomy" : command => do
  let mut count : Nat := 0
  for (declName, info) in (← getEnv).constants.toList do
    let components := declName.toString.splitOn "."
    if components.contains "Axionomy" then
      for dependency in ← collectAxioms declName do
        unless allowedAxiom dependency do
          throwError "{declName} depends on forbidden axiom {dependency}"
      if info.isTheorem then count := count + 1
  if count = 0 then throwError "no Axionomy theorems were audited"
  logInfo m!"Audited {count} Axionomy theorems; only standard Lean foundations are allowed."

elab "#require_theorem " declName:ident : command => do
  let constant ← getConstInfo declName.getId
  unless constant.isTheorem do throwError "{declName} is not a theorem"
  unless (`Axionomy).isPrefixOf declName.getId do
    throwError "{declName} is not an Axionomy theorem"
  for dependency in ← collectAxioms declName.getId do
    unless allowedAxiom dependency do
      throwError "{declName} depends on forbidden axiom {dependency}"
