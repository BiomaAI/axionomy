import Axionomy.TransactionDecode
import Axionomy.ValidationCompleteRefinement

namespace Axionomy.ValidationDecode
open Aeneas Aeneas.Std AccountExtracted

variable {AccountId A RateId Role : Type}

theorem expression_parameters_map {Q R : Type} (convert : Q → R) (expression : Evaluation.Expr Role A Q) :
    Discovery.parameters (expression.map convert) = Discovery.parameters expression := by
  induction expression <;> simp_all [Evaluation.Expr.map, Discovery.parameters]

theorem metadata (rule : rate.Rate Role A Std.U64) :
    Transaction.Rule.metadata (TransactionDecode.rule rule) = ValidationCompleteRefinement.metadata rule := by
  simp only [Transaction.Rule.metadata, Transaction.Rule.parameters, TransactionDecode.rule,
    ValidationCompleteRefinement.metadata, RateDiscoveryRefinement.parameterReferences,
    List.flatMap_append, List.flatMap_map, TransactionDecode.computed,
    TransactionDecode.condition, TransactionDecode.expression, expression_parameters_map]
  rfl

theorem missing_parameter_map {Q R : Type} (convert : Q → R) (parameters : List (String × Q)) (key : String) :
    Validation.missingParameter (AccountId := AccountId) (RateId := RateId) (Role := Role)
      (Account.mapEntries convert parameters) key = Validation.missingParameter parameters key := by
  simp only [Validation.missingParameter, ← TransactionDecode.orderedFind_map]
  cases Evaluation.orderedFind compare key parameters <;> rfl

theorem issues_map [DecidableEq AccountId] [DecidableEq Role] {Q R : Type}
    (convert : Q → R) (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (units : Nat) (metadata : Validation.Metadata Role) (bindings : List (Role × AccountId))
    (parameters : List (String × Q)) (present : AccountId → Bool) :
    Validation.issues (RateId := RateId) cmpAccount cmpRole units metadata bindings (Account.mapEntries convert parameters) present =
      Validation.issues cmpAccount cmpRole units metadata bindings parameters present := by
  have parametersEqual : Validation.missingParameter (AccountId := AccountId) (RateId := RateId) (Role := Role)
      (Account.mapEntries convert parameters) = Validation.missingParameter parameters :=
    funext (missing_parameter_map convert parameters)
  simp only [Validation.issues]
  rw [parametersEqual]
  simp only [Account.mapEntries, List.map_map, Function.comp_def]

theorem validation_decode [DecidableEq AccountId] [DecidableEq Role] [DecidableEq RateId]
    (cmpAccount : AccountId → AccountId → Ordering) (cmpRole : Role → Role → Ordering)
    (model : economy.Economy AccountId A RateId Role Std.U64) (proposal : exchange.Exchange RateId Role AccountId Std.U64) :
    (Validation.validate cmpAccount cmpRole ValidationCompleteRefinement.metadata proposal.units.val proposal.rate model.rates
      proposal.bindings proposal.parameters (fun id => (Account.find? id model.accounts).isSome)).map TransactionDecode.rule =
    Validation.validate cmpAccount cmpRole Transaction.Rule.metadata (TransactionDecode.proposal proposal).units
      (TransactionDecode.proposal proposal).rate (TransactionDecode.model model).rates
      (TransactionDecode.proposal proposal).bindings (TransactionDecode.proposal proposal).parameters
      (fun id => (Account.find? id (TransactionDecode.model model).accounts).isSome) := by
  have present (id : AccountId) : (Account.find? id (TransactionDecode.accounts model.accounts)).isSome =
      (Account.find? id model.accounts).isSome := by
    rw [TransactionDecode.accounts, ← Account.map_find]
    cases Account.find? id model.accounts <;> rfl
  simp only [Validation.validate, TransactionDecode.proposal, TransactionDecode.model, ← Account.map_find,
    present]
  cases found : Account.find? proposal.rate model.rates with
  | none => rfl
  | some rule =>
    simp only [Option.map_some, metadata, issues_map]
    cases (Validation.issues (RateId := RateId) cmpAccount cmpRole proposal.units.val
      (ValidationCompleteRefinement.metadata rule) proposal.bindings proposal.parameters
      (fun id => (Account.find? id model.accounts).isSome)).isEmpty <;> rfl

end Axionomy.ValidationDecode
