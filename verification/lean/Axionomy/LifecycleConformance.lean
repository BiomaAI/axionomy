import Axionomy.TransactionConformance
import Axionomy.DivergenceLifecycle

namespace LifecycleConformance
open Lean Axionomy TransactionConformance

/-- Compare actual supply and receipt changes on two branches of one economy.
Failed public replay contributes no receipts and returns the starting supply. -/
def scenario (expression : Evaluation.Expr Nat Nat Nat → Json) (label : String)
    (before : List (Nat × List (Nat × Nat))) (rule : Rule) (shared lefts rights : List Proposal)
    (assets : List Nat) : Json :=
  let model : Model := ⟨Ordered.ordered compare (before.map fun (id, entries) => (id, Basket.canonical entries)),
    [(7, normalize rule)], []⟩
  let left := shared ++ lefts
  let right := shared ++ rights
  let expected := match Execution.replay (Lifecycle.apply compare compare) model left,
      Execution.replay (Lifecycle.apply compare compare) model right with
    | some (lr, la), some (rr, ra) =>
      let receipts := fun result => match result with | .ok rs => rs | .error _ => []
      let change := fun asset rs => Divergence.madeBy (fun r : Lifecycle.Receipt Nat Nat Nat Nat => r.deltas) asset rs
      toJson (assets.map fun asset => Json.mkObj [
        ("asset", toJson asset),
        ("before", toJson (toString (DivergenceKernel.supply model.accounts asset))),
        ("left", toJson (toString (DivergenceKernel.supply la.accounts asset))),
        ("right", toJson (toString (DivergenceKernel.supply ra.accounts asset))),
        ("leftMade", toJson (toString (change asset (receipts lr)))),
        ("rightMade", toJson (toString (change asset (receipts rr)))),
        ("difference", toJson (toString (DivergenceKernel.supply la.accounts asset - DivergenceKernel.supply ra.accounts asset)))])
    | _, _ => tagged "panic"
  Json.mkObj [("left", replayScenario expression label before rule left),
    ("right", replayScenario expression label before rule right),
    ("shared", toJson shared.length), ("assets", toJson assets), ("expected", expected)]

def fixtures (expression : Evaluation.Expr Nat Nat Nat → Json) : Array Json := Id.run do
  let before := [(9, [(7, 11)]), (0, [(0, 2)]), (1, [(1, 3)])]
  let migrate : Rule := { blank with
    consume := [(0, [(0, 2)]), (1, [(1, 3)])],
    produce := [(2, [(0, 2)]), (3, [(1, 3)])], opens := [2, 3], closes := [0, 1] }
  let move := fun a b c d : Nat => (⟨7, [(0, a), (1, b), (2, c), (3, d)], 1, []⟩ : Proposal)
  let first := move 0 1 5 6
  let back := move 5 6 0 1
  let onward := move 5 6 7 8
  let add := fun label rule shared left right => scenario expression label before rule shared left right [0, 1, 7, 99]
  let mut cases := #[
    add "multiple-open-close-equal-supply" migrate [] [first] [],
    add "shared-lifecycle-reopen-or-onward" migrate [first] [back] [onward],
    add "lifecycle-failure-rolls-back-prefix" migrate [] [first, first] [first],
    add "lifecycle-closing-rejection" { migrate with consume := [(0, [(0, 1)]), (1, [(1, 3)])] } [] [first] []]
  -- Both branches open fresh accounts from one shared opening. Receipts account
  -- for nonzero supply differences as well as pure migrations.
  let mint : Rule := { blank with produce := [(0, [(0, 2), (1, 3)])], opens := [0] }
  let create := fun id units => (⟨7, [(0, id)], units, []⟩ : Proposal)
  cases := cases.push (add "shared-opening-different-production" mint [create 5 1] [create 6 2] [create 7 3])
  let burn : Rule := { blank with consume := [(0, [(0, 2)])], closes := [0] }
  cases := cases.push (add "closing-negative-supply-change" burn [] [⟨7, [(0, 0)], 1, []⟩] [])
  let transient : Rule := { blank with opens := [0], closes := [0] }
  cases := cases.push (add "transient-open-close-repeated" transient [] [create 5 1, create 5 1] [])
  return cases

end LifecycleConformance
