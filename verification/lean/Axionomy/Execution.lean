import Axionomy.Transaction

namespace Axionomy.Execution

variable {State Action Error Receipt : Type}

/-- Ordered execution, the building block of `replay`: on failure it retains
the failing step's state, which `replay` discards, and returns no receipts.
An unavailable step denotes a failed safety obligation, not a game error.
Production refinement rules out that case for well-formed bounded inputs. -/
def run (step : State → Action → Option (Except Error Receipt × State)) :
    State → List Action → List Receipt → Option (Except Error (List Receipt) × State)
  | state, [], accumulated => some (.ok accumulated, state)
  | state, action :: rest, accumulated => do
    let (result, next) ← step state action
    match result with
    | .error error => some (.error error, next)
    | .ok receipt => run step next rest (accumulated ++ [receipt])

/-- The same one-step result exposed through an isolated simulation. -/
def simulated (step : State → Action → Option (Except Error Receipt × State))
    (state : State) (action : Action) : Option (Except Error (State × Receipt)) := do
  let (result, next) ← step state action
  pure (result.map (fun receipt => (next, receipt)))

/-- A replay on a branch publishes that branch only if every action succeeds. -/
def replayed (step : State → Action → Option (Except Error Receipt × State))
    (state : State) (actions : List Action) : Option (Except Error State) := do
  let (result, next) ← run step state actions []
  pure (result.map (fun _ => next))

/-- Replay from `origin`, counting accepted actions from `index`: a failure
returns `origin` and the position of the action that failed. -/
def replayFrom (step : State → Action → Option (Except Error Receipt × State)) (origin : State) :
    Nat → State → List Action → List Receipt → Option (Except (Nat × Error) (List Receipt) × State)
  | _, state, [], accumulated => some (.ok accumulated, state)
  | index, state, action :: rest, accumulated => do
    let (result, next) ← step state action
    match result with
    | .error error => some (.error (index, error), origin)
    | .ok receipt => replayFrom step origin (index + 1) next rest (accumulated ++ [receipt])

/-- Replay applies a whole trace or none of it: every receipt and the state the
trace reached, or the starting state and the position, counted from zero, of
the action that failed. -/
def replay (step : State → Action → Option (Except Error Receipt × State))
    (state : State) (actions : List Action) :
    Option (Except (Nat × Error) (List Receipt) × State) :=
  replayFrom step state 0 state actions []

theorem replayFrom_rejection (step : State → Action → Option (Except Error Receipt × State))
    (origin : State) (actions : List Action) (index : Nat) (state after : State)
    (accumulated : List Receipt) (failure : Nat × Error)
    (failed : replayFrom step origin index state actions accumulated = some (.error failure, after)) :
    after = origin := by
  induction actions generalizing index state accumulated with
  | nil => simp [replayFrom] at failed
  | cons action rest ih =>
    cases executed : step state action with
    | none => simp [replayFrom, executed] at failed
    | some value =>
      rcases value with ⟨result, next⟩
      cases result with
      | error error =>
        simp [replayFrom, executed] at failed
        exact failed.2.symm
      | ok receipt =>
        exact ih (index + 1) next (accumulated ++ [receipt]) (by simpa [replayFrom, executed] using failed)

/-- A failed replay changes nothing: it returns the state it started from. -/
theorem replay_rejection_atomic (step : State → Action → Option (Except Error Receipt × State))
    (state after : State) (actions : List Action) (failure : Nat × Error)
    (failed : replay step state actions = some (.error failure, after)) : after = state :=
  replayFrom_rejection step state actions 0 state after [] failure failed

theorem replayFrom_success (step : State → Action → Option (Except Error Receipt × State))
    (origin : State) (actions : List Action) (index : Nat) (state after : State)
    (accumulated receipts : List Receipt) :
    replayFrom step origin index state actions accumulated = some (.ok receipts, after) ↔
      run step state actions accumulated = some (.ok receipts, after) := by
  induction actions generalizing index state accumulated with
  | nil => simp [replayFrom, run]
  | cons action rest ih =>
    cases executed : step state action with
    | none => simp [replayFrom, run, executed]
    | some value =>
      rcases value with ⟨result, next⟩
      cases result with
      | error error => simp [replayFrom, run, executed]
      | ok receipt => simpa [replayFrom, run, executed] using ih (index + 1) next (accumulated ++ [receipt])

/-- A replay that succeeds is ordered execution that succeeds: the same
receipts and the same final state. -/
theorem replay_success (step : State → Action → Option (Except Error Receipt × State))
    (state after : State) (actions : List Action) (receipts : List Receipt) :
    replay step state actions = some (.ok receipts, after) ↔
      run step state actions [] = some (.ok receipts, after) :=
  replayFrom_success step state actions 0 state after [] receipts

theorem replayFrom_error_index (step : State → Action → Option (Except Error Receipt × State))
    (origin : State) (actions : List Action) (index position : Nat) (state after : State)
    (accumulated : List Receipt) (error : Error)
    (failed : replayFrom step origin index state actions accumulated = some (.error (position, error), after)) :
    ∃ leading action trailing receipts middle next,
      actions = leading ++ action :: trailing ∧ position = index + leading.length ∧
      run step state leading accumulated = some (.ok receipts, middle) ∧
      step middle action = some (.error error, next) := by
  induction actions generalizing index state accumulated with
  | nil => simp [replayFrom] at failed
  | cons action rest ih =>
    cases executed : step state action with
    | none => simp [replayFrom, executed] at failed
    | some value =>
      rcases value with ⟨result, next⟩
      cases result with
      | error first =>
        simp [replayFrom, executed] at failed
        obtain ⟨⟨rfl, rfl⟩, -⟩ := failed
        exact ⟨[], action, rest, accumulated, state, next, rfl, by simp, by simp [run], executed⟩
      | ok receipt =>
        obtain ⟨leading, bad, trailing, receipts, middle, after', split, counted, accepted, rejected⟩ :=
          ih (index + 1) next (accumulated ++ [receipt]) (by simpa [replayFrom, executed] using failed)
        exact ⟨action :: leading, bad, trailing, receipts, middle, after', by simp [split],
          by simp [counted]; omega, by simp [run, executed, accepted], rejected⟩

/-- The position a failed replay names splits its trace: every action before
it applies in order, and the action at it fails against the state they
reached. -/
theorem replay_error_index (step : State → Action → Option (Except Error Receipt × State))
    (state after : State) (actions : List Action) (position : Nat) (error : Error)
    (failed : replay step state actions = some (.error (position, error), after)) :
    ∃ leading action trailing receipts middle next,
      actions = leading ++ action :: trailing ∧ position = leading.length ∧
      run step state leading [] = some (.ok receipts, middle) ∧
      step middle action = some (.error error, next) := by
  obtain ⟨leading, action, trailing, receipts, middle, next, split, counted, accepted, rejected⟩ :=
    replayFrom_error_index step state actions 0 position state after [] error failed
  exact ⟨leading, action, trailing, receipts, middle, next, split, by simpa using counted, accepted, rejected⟩

theorem append (step : State → Action → Option (Except Error Receipt × State))
    (left right : List Action) (state : State) (accumulated : List Receipt) :
    run step state (left ++ right) accumulated = (do
      let (result, next) ← run step state left accumulated
      match result with
      | .error error => some (.error error, next)
      | .ok receipts => run step next right receipts) := by
  induction left generalizing state accumulated with
  | nil => simp [run]
  | cons action rest ih =>
    simp only [List.cons_append, run]
    cases step state action with
    | none => rfl
    | some value =>
      rcases value with ⟨result, next⟩
      cases result <;> simp [ih]

/-- A failing replay has an all-successful prefix followed by the reported
failure. Later actions do not participate in the resulting state. -/
theorem error_prefix (step : State → Action → Option (Except Error Receipt × State))
    (actions : List Action) (state finalState : State) (accumulated : List Receipt) (error : Error)
    (failed : run step state actions accumulated = some (.error error, finalState)) :
    ∃ leading action trailing receipts middle,
      actions = leading ++ action :: trailing ∧
      run step state leading accumulated = some (.ok receipts, middle) ∧
      step middle action = some (.error error, finalState) := by
  induction actions generalizing state accumulated with
  | nil => simp [run] at failed
  | cons action rest ih =>
    cases executed : step state action with
    | none => simp [run, executed] at failed
    | some value =>
      rcases value with ⟨result, next⟩
      cases result with
      | error first =>
        simp [run, executed] at failed
        rcases failed with ⟨rfl, rfl⟩
        exact ⟨[], action, rest, accumulated, state, rfl, rfl, executed⟩
      | ok receipt =>
        have continued : run step next rest (accumulated ++ [receipt]) = some (.error error, finalState) := by
          simpa [run, executed] using failed
        obtain ⟨leading, bad, trailing, receipts, middle, split, accepted, rejected⟩ := ih next _ continued
        exact ⟨action :: leading, bad, trailing, receipts, middle, by simp [split],
          by simp [run, executed, accepted], rejected⟩

/-- The exact ordered successful steps, including their intermediate states. -/
inductive Accepted (step : State → Action → Option (Except Error Receipt × State)) :
    State → List Action → List Receipt → State → Prop
  | nil (state) : Accepted step state [] [] state
  | cons {state next finalState action receipt actions receipts}
      (executed : step state action = some (.ok receipt, next))
      (following : Accepted step next actions receipts finalState) :
      Accepted step state (action :: actions) (receipt :: receipts) finalState

theorem success_history (step : State → Action → Option (Except Error Receipt × State))
    (actions : List Action) (state finalState : State) (accumulated receipts : List Receipt)
    (success : run step state actions accumulated = some (.ok receipts, finalState)) :
    ∃ produced, receipts = accumulated ++ produced ∧ Accepted step state actions produced finalState := by
  induction actions generalizing state accumulated with
  | nil =>
    simp [run] at success
    rcases success with ⟨rfl, rfl⟩
    exact ⟨[], by simp, .nil _⟩
  | cons action rest ih =>
    cases executed : step state action with
    | none => simp [run, executed] at success
    | some value =>
      rcases value with ⟨result, next⟩
      cases result with
      | error error => simp [run, executed] at success
      | ok receipt =>
        obtain ⟨produced, receiptsEqual, history⟩ := ih next (accumulated ++ [receipt]) (by simpa [run, executed] using success)
        exact ⟨receipt :: produced, by simpa [List.append_assoc] using receiptsEqual, .cons executed history⟩

theorem Accepted.receipt_order (step : State → Action → Option (Except Error Receipt × State))
    (original : Receipt → Action)
    (stepOrder : ∀ state action receipt after, step state action = some (.ok receipt, after) → original receipt = action)
    {actions receipts state finalState} (history : Accepted step state actions receipts finalState) :
    receipts.map original = actions := by
  induction history with
  | nil => rfl
  | cons executed following ih => simp [stepOrder _ _ _ _ executed, ih]

theorem Accepted.invariant (step : State → Action → Option (Except Error Receipt × State))
    (property : State → Prop)
    (preserves : ∀ state action receipt after, property state → step state action = some (.ok receipt, after) → property after)
    {actions receipts state finalState} (history : Accepted step state actions receipts finalState)
    (initial : property state) : property finalState := by
  induction history with
  | nil => exact initial
  | cons executed following ih => exact ih (preserves _ _ _ _ initial executed)

end Axionomy.Execution
