#!/usr/bin/env python3
"""Extract the production account updates and check its storage boundary."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from setup_aeneas import PIN, ROOT, verify_tools
from extract_quantity import LEAN, WORK

GENERATED = {
    "Types.lean": "AccountExtraction/Types.lean",
    "Funs.lean": "AccountExtraction/Funs.lean",
    "AccountExtraction.lean": "AccountExtraction.lean",
}
TEMPLATES = {
    "TypesExternal_Template.lean": "d78d125816508ceeb1f3e19fdbad34361a8bc3f878f639958d8abe34f386c57c",
    "FunsExternal_Template.lean": "8fd02491d90d463616d2188f5946fed128f59fa012a78b5da78d6966a2857700",
}

# The pinned backend's Iterator dictionary omits size_hint. Its supported
# filtering mode removes that dictionary field, while preserving concrete
# methods and their callers for extraction and proof. Generic calls through the
# absent field cannot typecheck in Lean. Hash also omits hash_slice, but the
# four core wrappers call its separately modeled std default directly, with
# concrete extracted Hash implementations. They do not dispatch through an
# erased payload hash_slice override. Accept only the two exact diagnostics;
# every other warning/error remains fatal. Concrete iterator definitions are
# mandatory below, so filtering cannot silently replace their implementation.
FILTERED_TRAIT_WARNING = (
    "When retrieving the builtin information for trait decl "
    "'core::iter::traits::iterator::Iterator', could not find the information "
    "for item 'size_hint'. The model defined in the Lean library seems to be "
    "missing the corresponding field."
)
FILTERED_HASH_WARNING = (
    "When retrieving the builtin information for trait decl "
    "'core::hash::Hash', could not find the information for item 'hash_slice'. "
    "The model defined in the Lean library seems to be missing the corresponding field."
)

INSPECTION_DEFINITIONS = (
    "economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.next",
    "economy.AccountEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAccountIdSharedAAccount.size_hint",
    "rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.next",
    "rate.WeightEntries.Insts.CoreIterTraitsIteratorIteratorPairSharedAAI64.size_hint",
    *[f"economy.ExchangeAssessment.{method}{suffix}" for method in ("account", "shortfall")
      for suffix in ("", "_loop", "_loop.body")],
    *[f"rate.Rate.{method}" for method in ("impl.roles", "distinct_roles", "computed_consumed",
      "computed_produced", "computed_preserved", "impl.conditions", "consumed", "produced", "preserved")],
    *[f"rate.LinearInvariant.{method}" for method in ("new", "weight", "impl.name", "impl.weights")],
    *[f"economy.Economy.{method}" for method in ("impl.accounts", "rate_ids", "impl.invariants", "rate", "asset_keys")],
    *[f"{method}{suffix}" for method in ("economy.collect_basket_assets", "economy.collect_invariant_assets",
      "rate.Rate.collect_optional_assets") for suffix in ("", "_loop", "_loop.body")],
    "rate.Rate.collect_assets",
    *[f"{method}_loop{index}{suffix}" for method, count in (("rate.Rate.collect_assets", 2),
      ("economy.Economy.asset_keys", 3)) for index in range(count) for suffix in ("", ".body")],
)

CONVENIENCE_DEFINITIONS = (
    "rate.Rate.impl.consume", "rate.Rate.impl.produce", "rate.Rate.impl.preserve",
    "rate.basket", "rate.basket.closure.Insts.CoreOpsFunctionFnMutTuplePairAU64PairAQuantityU64.call_mut",
    "exchange.Exchange.parameter", "rate.QuantityExpression.parameter", "rate.Rate.condition",
)

INVARIANT_INPUT_DEFINITIONS = (
    "rate.LinearInvariant.from_serialized_weights",
    *[f"rate.LinearInvariant.from_serialized_weights_{loop}{suffix}"
      for loop in ("loop0", "loop0_loop0") for suffix in ("", ".body")],
)

INVARIANT_INPUT_CALL = '''let data = InvariantData::<A>::deserialize(deserializer)?;
        Self::from_serialized_weights(data.name, data.weights).map_err(|_| {
            serde::de::Error::custom("linear invariant contains a duplicate asset weight")
        })'''

# Derived methods must be production definitions, not merely forwarding roots
# that could resolve to an external stub or a copied specification.
DERIVED_DEFINITIONS = (
    *[f"{typ}.Insts.CoreCloneClone.clone" for typ in (
        "quantity.Quantity", "amount.AssetAmount", "rate.QuantityExpression",
        "rate.ComputedAmount", "rate.RateCondition", "rate.QuantityComparison", "quantity.Rounding",
        "economy.StateFingerprint", "economy.AssessmentStatus", "economy.ModelIssue",
        "economy.ModelBuildError", "basket.BasketError", "rate.RateError", "quantity.QuantityError",
        "exchange.Exchange", "exchange.Trace", "economy.ObservationKey", "basket.Basket",
        "account.Account", "account.AccountError", "exchange.AccountDelta", "exchange.Receipt",
        "economy.Goal", "rate.Rate", "rate.LinearInvariant", "economy.Economy",
        "economy.AccountAssessment", "economy.AccountShortfall", "economy.ExchangeAssessment",
        "economy.ApplyError")],
    *[f"{typ}.Insts.CoreCmpPartialEq{typ.split('.')[-1]}.eq" for typ in (
        "quantity.Quantity", "amount.AssetAmount", "rate.QuantityExpression",
        "rate.ComputedAmount", "rate.RateCondition", "rate.QuantityComparison", "quantity.Rounding",
        "economy.StateFingerprint", "economy.AssessmentStatus", "economy.ModelIssue",
        "economy.ModelBuildError", "basket.BasketError", "rate.RateError", "quantity.QuantityError",
        "exchange.Exchange", "exchange.Trace", "economy.ObservationKey", "account.AccountError")],
    *[f"{typ}.Insts.CoreCmpOrd.cmp" for typ in ("quantity.Quantity", "economy.StateFingerprint")],
    *[f"{typ}.Insts.CoreCmpPartialOrd{typ.split('.')[-1]}.partial_cmp"
      for typ in ("quantity.Quantity", "economy.StateFingerprint")],
    *[f"{typ}.Insts.CoreHashHash.hash" for typ in (
        "quantity.Quantity", "amount.AssetAmount", "economy.StateFingerprint", "economy.ObservationKey")],
)


def validate_extraction_diagnostics(output):
    plain = re.sub(r"\x1b\[[0-9;]*m", "", output)
    diagnostics = re.findall(r"\[(Warn|Error)\s*\]\s*([^\n]*)", plain)
    if sorted(diagnostics) != sorted([("Warn", FILTERED_TRAIT_WARNING), ("Warn", FILTERED_HASH_WARNING)]):
        raise ValueError("unexpected account extraction diagnostics; only the reviewed Iterator.size_hint and Hash.hash_slice warnings are permitted")

# Keep critical handoffs explicit alongside the complete caller extraction.
# These syntactic guards supplement, rather than replace, the Lean refinements.
PREPARATION_CALL = """let holder = match self
                .accounts
                .get(&account_id)
                .expect("bound accounts were checked")
                .prepare_exchange(&effect.consume, &effect.produce)
            {
                Ok(holder) => holder,
                Err(error) => {
                    return Err(map_account_error(account_id.clone(), error));
                }
            };"""

VALIDATION_CALL = """let rule = match self.validate_structure(proposal) {
            Ok(rule) => rule,
            Err(issues) => return invalid_analysis(issues),
        };"""

ENTRY_CALLS = {
    "invariant_from_serialized_weights": "LinearInvariant::from_serialized_weights(name,entries)",
    "inherited_quantity_clone_from": "target.clone_from(source)",
    "inherited_amount_clone_from": "target.clone_from(source)",
    "inherited_expression_clone_from": "target.clone_from(source)",
    "inherited_computed_amount_clone_from": "target.clone_from(source)",
    "inherited_condition_clone_from": "target.clone_from(source)",
    "inherited_comparison_clone_from": "target.clone_from(source)",
    "inherited_rounding_clone_from": "target.clone_from(source)",
    "inherited_fingerprint_clone_from": "target.clone_from(source)",
    "inherited_assessment_status_clone_from": "target.clone_from(source)",
    "inherited_model_issue_clone_from": "target.clone_from(source)",
    "inherited_model_error_clone_from": "target.clone_from(source)",
    "inherited_basket_error_clone_from": "target.clone_from(source)",
    "inherited_rate_error_clone_from": "target.clone_from(source)",
    "inherited_quantity_error_clone_from": "target.clone_from(source)",
    "inherited_proposal_clone_from": "target.clone_from(source)",
    "inherited_trace_clone_from": "target.clone_from(source)",
    "inherited_observation_key_clone_from": "target.clone_from(source)",
    "inherited_basket_clone_from": "target.clone_from(source)",
    "inherited_account_clone_from": "target.clone_from(source)",
    "inherited_account_error_clone_from": "target.clone_from(source)",
    "inherited_delta_clone_from": "target.clone_from(source)",
    "inherited_receipt_clone_from": "target.clone_from(source)",
    "inherited_goal_clone_from": "target.clone_from(source)",
    "inherited_rate_clone_from": "target.clone_from(source)",
    "inherited_invariant_clone_from": "target.clone_from(source)",
    "inherited_economy_clone_from": "target.clone_from(source)",
    "inherited_account_assessment_clone_from": "target.clone_from(source)",
    "inherited_shortfall_clone_from": "target.clone_from(source)",
    "inherited_assessment_clone_from": "target.clone_from(source)",
    "inherited_apply_error_clone_from": "target.clone_from(source)",
    "inherited_quantity_ne": "left!=right",
    "inherited_amount_ne": "left!=right",
    "inherited_expression_ne": "left!=right",
    "inherited_computed_amount_ne": "left!=right",
    "inherited_condition_ne": "left!=right",
    "inherited_comparison_ne": "left!=right",
    "inherited_rounding_ne": "left!=right",
    "inherited_fingerprint_ne": "left!=right",
    "inherited_assessment_status_ne": "left!=right",
    "inherited_model_issue_ne": "left!=right",
    "inherited_model_error_ne": "left!=right",
    "inherited_basket_error_ne": "left!=right",
    "inherited_rate_error_ne": "left!=right",
    "inherited_quantity_error_ne": "left!=right",
    "inherited_proposal_ne": "left!=right",
    "inherited_trace_ne": "left!=right",
    "inherited_observation_key_ne": "left!=right",
    "inherited_account_error_ne": "left!=right",
    "inherited_basket_ne": "left!=right",
    "inherited_account_ne": "left!=right",
    "inherited_quantity_min": "left.min(right)",
    "inherited_quantity_max": "left.max(right)",
    "inherited_quantity_clamp": "value.clamp(lower,upper)",
    "inherited_fingerprint_min": "left.min(right)",
    "inherited_fingerprint_max": "left.max(right)",
    "inherited_fingerprint_clamp": "value.clamp(lower,upper)",
    "inherited_quantity_hash_slice": "std::hash::Hash::hash_slice(values,state)",
    "inherited_amount_hash_slice": "std::hash::Hash::hash_slice(values,state)",
    "inherited_fingerprint_hash_slice": "std::hash::Hash::hash_slice(values,state)",
    "inherited_observation_key_hash_slice": "std::hash::Hash::hash_slice(values,state)",
    "derived_basket_clone": "value.clone()",
    "derived_account_clone": "value.clone()",
    "derived_account_error_clone": "value.clone()",
    "derived_delta_clone": "value.clone()",
    "derived_receipt_clone": "value.clone()",
    "derived_goal_clone": "value.clone()",
    "derived_rate_clone": "value.clone()",
    "derived_invariant_clone": "value.clone()",
    "derived_economy_clone": "value.clone()",
    "derived_account_assessment_clone": "value.clone()",
    "derived_shortfall_clone": "value.clone()",
    "derived_assessment_clone": "value.clone()",
    "derived_apply_error_clone": "value.clone()",
    "derived_account_error_eq": "left==right",
    "clone_value": "value.clone()",
    "derived_quantity_eq": "left==right",
    "derived_quantity_clone": "value.clone()",
    "derived_amount_eq": "left==right",
    "derived_amount_clone": "value.clone()",
    "derived_expression_eq": "left==right",
    "derived_expression_clone": "clone_value(value)",
    "derived_computed_amount_eq": "left==right",
    "derived_computed_amount_clone": "value.clone()",
    "derived_condition_eq": "left==right",
    "derived_condition_clone": "value.clone()",
    "derived_comparison_eq": "left==right",
    "derived_comparison_clone": "value.clone()",
    "derived_rounding_eq": "left==right",
    "derived_rounding_clone": "value.clone()",
    "derived_fingerprint_eq": "left==right",
    "derived_fingerprint_clone": "value.clone()",
    "derived_assessment_status_eq": "left==right",
    "derived_assessment_status_clone": "value.clone()",
    "derived_model_issue_eq": "left==right",
    "derived_model_issue_clone": "value.clone()",
    "derived_model_error_eq": "left==right",
    "derived_model_error_clone": "value.clone()",
    "derived_basket_error_eq": "left==right",
    "derived_basket_error_clone": "value.clone()",
    "derived_rate_error_eq": "left==right",
    "derived_rate_error_clone": "value.clone()",
    "derived_quantity_error_eq": "left==right",
    "derived_quantity_error_clone": "value.clone()",
    "derived_proposal_eq": "left==right",
    "derived_proposal_clone": "value.clone()",
    "derived_trace_eq": "left==right",
    "derived_trace_clone": "value.clone()",
    "derived_observation_key_eq": "left==right",
    "derived_observation_key_clone": "value.clone()",
    "derived_quantity_partial_cmp": "left.partial_cmp(right)",
    "derived_quantity_lt": "left.lt(right)",
    "derived_quantity_le": "left.le(right)",
    "derived_quantity_gt": "left.gt(right)",
    "derived_quantity_ge": "left.ge(right)",
    "derived_quantity_cmp": "left.cmp(right)",
    "derived_fingerprint_partial_cmp": "left.partial_cmp(right)",
    "derived_fingerprint_lt": "left.lt(right)",
    "derived_fingerprint_le": "left.le(right)",
    "derived_fingerprint_gt": "left.gt(right)",
    "derived_fingerprint_ge": "left.ge(right)",
    "derived_fingerprint_cmp": "left.cmp(right)",
    "derived_quantity_hash": "std::hash::Hash::hash(value,state)",
    "derived_amount_hash": "std::hash::Hash::hash(value,state)",
    "derived_fingerprint_hash": "std::hash::Hash::hash(value,state)",
    "derived_observation_key_hash": "std::hash::Hash::hash(value,state)",
    "rate_consume_or_panic": "rule.consume(role,assets)",
    "rate_produce_or_panic": "rule.produce(role,assets)",
    "rate_preserve_or_panic": "rule.preserve(role,assets)",
    "basket_literal": "rate::basket(entries)",
    "proposal_parameter_with": "proposal.parameter(name,value)",
    "expression_parameter_with": "QuantityExpression::parameter(name)",
    "rate_condition_with": "rule.condition(label,left,comparison,right)",
    "inspect_model_accounts_next": "model.accounts().next()",
    "inspect_model_accounts_hint": "model.accounts().size_hint()",
    "inspect_model_rate_ids_next": "model.rate_ids().next()",
    "inspect_model_rate_ids_hint": "model.rate_ids().size_hint()",
    "inspect_model_invariants_next": "model.invariants().next()",
    "inspect_model_invariants_hint": "model.invariants().size_hint()",
    "invariant_weights_hint": "value.weights().size_hint()",
    'assessment_account': 'value.account(id)',
    'assessment_shortfall': 'value.shortfall(id)',
    'inspect_rate_roles': 'rule.roles()',
    'inspect_rate_distinct_roles': 'rule.distinct_roles()',
    'inspect_rate_computed_consumed': 'rule.computed_consumed()',
    'inspect_rate_computed_produced': 'rule.computed_produced()',
    'inspect_rate_computed_preserved': 'rule.computed_preserved()',
    'inspect_rate_conditions': 'rule.conditions()',
    'inspect_rate_consumed': 'rule.consumed(role)',
    'inspect_rate_produced': 'rule.produced(role)',
    'inspect_rate_preserved': 'rule.preserved(role)',
    'invariant_new': 'LinearInvariant::new(name)',
    'invariant_weight': 'value.weight(asset,weight)',
    'invariant_name': 'value.name()',
    'invariant_weights': 'value.weights()',
    'inspect_model_accounts': 'model.accounts()',
    'inspect_model_rate_ids': 'model.rate_ids()',
    'inspect_model_invariants': 'model.invariants()',
    'inspect_model_asset_keys': 'model.asset_keys()',
    'inspect_model_rate': 'model.rate(id)',
    "basket_try_entries": "Basket::try_from_entries(entries)",
    "basket_try_amounts": "Basket::try_from_amounts(amounts)",
    "basket_collect": "Basket::from_iter(entries)",
    "basket_from_array": "Basket::from(entries)",
    "basket_new": "Basket::new()",
    "basket_default": "Basket::default()",
    "basket_len": "state.len()",
    "basket_empty": "state.is_empty()",
    "basket_entries": "state.iter()",
    "basket_sorted_entries": "state.iter_sorted()",
    "basket_get": "state.get(asset)",
    "amount_new": "AssetAmount::new(asset,value)",
    "amount_asset": "value.asset()",
    "amount_quantity": "value.quantity()",
    "amount_parts": "value.into_parts()",
    "amount_from_pair": "AssetAmount::from(parts)",
    "amount_to_pair": "value.into()",
    "quantity_new": "Quantity::new(value)",
    "quantity_get": "value.get()",
    "quantity_from_u64": "Quantity::from(value)",
    "quantity_from_u32": "Quantity::from(value)",
    "quantity_validate": "Quantity::try_from_scalar(value)",
    "quantity_borrow": "value.as_scalar()",
    "quantity_consume": "value.into_scalar()",
    "account_new": "Account::new(assets)",
    "account_default": "Account::default()",
    "account_from_basket": "Account::from(assets)",
    "account_into_basket": "holder.into_balances()",
    "account_borrow_basket": "holder.balances()",
    "account_equal": "left==right",
    "proposal_new": "Exchange::new(rate_id,units)",
    "proposal_bind": "proposal.bind(role,holder)",
    "proposal_parameter": "proposal.parameter(name,value)",
    "proposal_rate": "proposal.rate()",
    "proposal_bindings": "proposal.bindings()",
    "proposal_units": "proposal.units()",
    "proposal_parameters": "proposal.parameters()",
    "delta_account": "value.account()",
    "delta_consumed": "value.consumed()",
    "delta_produced": "value.produced()",
    "delta_preserved": "value.preserved()",
    "receipt_exchange": "value.exchange()",
    "receipt_deltas": "value.deltas()",
    "account_assessment_account": "value.account()",
    "account_assessment_available": "value.available()",
    "account_assessment_required": "value.required()",
    "account_assessment_consumed": "value.consumed()",
    "account_assessment_produced": "value.produced()",
    "account_assessment_preserved": "value.preserved()",
    "account_shortfall_account": "value.account()",
    "account_shortfall_missing": "value.missing()",
    "assessment_status": "value.status()",
    "assessment_accounts": "value.accounts()",
    "assessment_shortfalls": "value.shortfalls()",
    "assessment_projected_deltas": "value.projected_deltas()",
    "assessment_issues": "value.issues()",
    "computed_role": "value.role()",
    "computed_asset": "value.asset()",
    "computed_quantity": "value.quantity()",
    "condition_name": "value.name()",
    "condition_left": "value.left()",
    "condition_comparison": "value.comparison()",
    "condition_right": "value.right()",
    "model_build_error_issues": "value.issues()",
    "expression_constant": "QuantityExpression::constant(value)",
    "expression_units": "QuantityExpression::units()",
    "expression_balance": "QuantityExpression::balance(role,asset)",
    "expression_parameter": "QuantityExpression::parameter(name)",
    "expression_plus": "QuantityExpression::plus(left,right)",
    "expression_subtract": "QuantityExpression::subtract(left,right)",
    "expression_multiply": "QuantityExpression::multiply(left,right)",
    "expression_mul_div": "QuantityExpression::mul_div(left,right,denominator,rounding)",
    "expression_divide": "QuantityExpression::divide(numerator,denominator,rounding)",
    "quantity_mul_div": "left.checked_mul_div(right,denominator,rounding)",
    "expression_minimum": "QuantityExpression::minimum(left,right)",
    "expression_maximum": "QuantityExpression::maximum(left,right)",
    "expression_add": "left+right",
    "basket_insert_amount": "state.insert_amount(value)",

    "goal_new": "economy::Goal::new()",
    "goal_default": "economy::Goal::default()",
    "goal_require": "goal.require(id,assets)",
    "goal_requirements": "goal.requirements()",
    "goal_assets": "goal.asset_keys()",
    "inspect_model_account": "model.account(id)",
    "model_balance": "model.balance(id,asset)",
    "model_matches": "model.matches(goal)",
    "model_view": "model.view(visible)",
    "model_state_key": "model.state_key()",
    "model_fingerprint": "model.state_fingerprint()",
    "view_account": "view.account(id)",
    "view_balance": "view.balance(id,asset)",
    "view_key": "view.observation_key()",
    "key_visible": "key.visible_accounts()",
    "key_existing": "key.existing_accounts()",
    "key_balances": "key.balances()",
    "fingerprint_get": "value.get()",
    "model_fork": "model.fork()",
    "model_simulate": "model.simulate(proposal)",
    "model_replay": "model.replay(trace)",
    "model_replayed": "model.replayed(trace)",
    "trace_new": "Trace::new()",
    "trace_default": "Trace::default()",
    "trace_push": "trace.push(proposal)",
    "trace_extend": "trace.extend(exchanges)",
    "trace_exchanges": "trace.exchanges()",
    "trace_into_exchanges": "trace.into_exchanges()",
    "model_is_applicable": "model.is_applicable(proposal)",
    "model_applicable": "model.applicable(candidates)",
    "assess_model_proposal": "model.assess(proposal)",
    "apply_model_proposal": "model.apply(proposal)",
    "validate_model_proposal": "model.validate_structure(proposal)",
    "model_new": "economy::EconomyBuilder::new()",
    "model_default": "economy::EconomyBuilder::default()",
    "model_account": "builder.account(id,state)",
    "model_rate": "builder.rate(id,rule)",
    "model_invariant": "builder.invariant(invariant)",
    "model_build": "builder.build()",
    "rate_new": "Rate::new()",
    "rate_default": "Rate::default()",
    "rate_distinct": "rule.distinct(left,right)",
    "rate_computed_consume": "rule.consume_computed(role,asset,expression)",
    "rate_computed_produce": "rule.produce_computed(role,asset,expression)",
    "rate_computed_preserve": "rule.preserve_computed(role,asset,expression)",
    "rate_condition": "rule.condition(label,left,comparison,right)",
    "rate_parameters": "rule.parameter_names()",
    "rate_assets": "rule.computed_asset_keys()",
    "deposit": "state.deposit(assets)",
    "withdraw": "state.withdraw(assets)",
    "prepare_exchange": "state.prepare_exchange(consumed,produced)",
    "basket_add": "state.checked_add(assets)",
    "basket_scale": "state.checked_scale(units)",
    "basket_contains": "state.contains(required)",
    "basket_equal": "left==right",
    "weighted": "value.checked_weighted(coefficient)",
    "expression_message": "error.into_message()",
    "evaluate": "economy::evaluate_expression(expression,exchange,accounts)",
    "evaluate_condition": "economy::evaluate_condition(condition,exchange,accounts)",
    "merge_effects": "economy::merge_scaled(target,source,units)",
    "merge_amount": "economy::merge_quantity(target,asset,amount)",
    "invariant_measure": "invariant.measure(accounts)",
    "invariant_conserves": "invariant.conserves(deltas)",
    "rate_consume": "rule.try_consume(role,assets)",
    "rate_produce": "rule.try_produce(role,assets)",
    "rate_preserve": "rule.try_preserve(role,assets)",
}


PUBLIC_DEFINITIONS = (
    "amount.AssetAmount.new", "amount.AssetAmount.impl.asset", "amount.AssetAmount.impl.quantity",
    "amount.AssetAmount.into_parts", "amount.AssetAmount.Insts.CoreConvertFromPairAQuantity.from",
    "PairAQuantity.Insts.CoreConvertFromAssetAmount.from", "basket.Basket.insert_amount",
    "quantity.QuantityU64.new", "quantity.QuantityU64.get", "quantity.Quantity.into_scalar",
    "quantity.QuantityU64.Insts.CoreConvertFromU32.from", "quantity.QuantityU64.Insts.CoreConvertFromU64.from",
    "account.Account.new", "account.Account.into_balances", "account.Account.Insts.CoreDefaultDefault.default",
    "account.Account.Insts.CoreConvertFromBasket.from", "account.Account.Insts.CoreCmpPartialEqAccount.eq",
    "exchange.Exchange.new", "exchange.Exchange.bind", "exchange.Exchange.parameter",
    "exchange.Receipt.exchange", "exchange.Receipt.impl.deltas", "exchange.AccountDelta.impl.account",
    "exchange.AccountDelta.impl.preserved", "economy.ModelBuildError.impl.issues",
    *[f"economy.AccountAssessment.impl.{field}" for field in
      ("account", "available", "required", "consumed", "produced", "preserved")],
    "economy.AccountShortfall.impl.account", "economy.AccountShortfall.impl.missing",
    *[f"economy.ExchangeAssessment.{method}" for method in
      ("status", "accounts", "shortfalls", "projected_deltas", "issues")],
    "rate.QuantityExpressionRoleAU64.constant",
    *[f"rate.QuantityExpression.{method}" for method in
      ("units", "balance", "parameter", "plus", "subtract", "multiply", "mul_div", "minimum", "maximum")],
    "rate.QuantityExpressionRoleAU64.divide",
    "rate.QuantityExpression.Insts.CoreOpsArithAddQuantityExpressionQuantityExpression.add",
    *[f"rate.ComputedAmount.impl.{field}" for field in ("role", "asset", "quantity")],
    *[f"rate.RateCondition.impl.{field}" for field in ("name", "left", "comparison", "right")],
)


def validate_bridge(root=ROOT):
    """Require the harness to forward to the unchanged production methods."""
    basket = (root / "src/basket.rs").read_text()
    retention = re.findall(r"\bfn retain_quantities\b[^{}]*\{([^{}]*)\}", basket)
    if len(retention) != 1 or re.sub(r"\s+", "", retention[0]) != "quantities.retain(|_,quantity|keep(quantity));":
        raise ValueError("basket retention adapter changed; review its storage boundary")
    harness = (root / "verification/extract_account.rs").read_text()
    paths = re.findall(r'#\[path\s*=\s*"([^"]+)"\]', harness)
    if sorted(paths) != ["../src/account.rs", "../src/amount.rs", "../src/basket.rs", "../src/economy.rs", "../src/exchange.rs",
                         "../src/quantity.rs", "../src/rate.rs"]:
        raise ValueError("account extraction must include the exact production source files")
    for operation, call in ENTRY_CALLS.items():
        matches = re.findall(r"\bfn " + operation + r"\b[^{}]*\{([^{}]*)\}", harness)
        if len(matches) != 1 or re.sub(r"\s+", "", matches[0]) != call:
            raise ValueError(f"account extraction entry point is not a forwarding call: {operation}")
    economy = (root / "src/economy.rs").read_text()
    rate = (root / "src/rate.rs").read_text()
    if re.sub(r"\s+", "", rate).count(re.sub(r"\s+", "", INVARIANT_INPUT_CALL)) != 1:
        raise ValueError("invariant deserialization handoff changed; review the production connection")
    if re.sub(r"\s+", "", economy).count(re.sub(r"\s+", "", PREPARATION_CALL)) != 1:
        raise ValueError("economy preparation handoff changed; review the production connection")
    if re.sub(r"\s+", "", economy).count(re.sub(r"\s+", "", VALIDATION_CALL)) != 1:
        raise ValueError("economy validation handoff changed; review the production connection")


def validate_fingerprint_target(cfg):
    if not {'target_pointer_width="64"', 'target_endian="little"'} <= set(cfg.splitlines()):
        raise ValueError("fingerprint proof adapter requires a 64-bit little-endian extraction target")


def validate_generated(directory):
    names = {p.name for p in directory.iterdir()}
    if names != set(GENERATED) | set(TEMPLATES):
        raise ValueError(f"unexpected account extraction files: {sorted(names)}")
    for name, expected in TEMPLATES.items():
        if hashlib.sha256((directory / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"account external model boundary changed: {name}; review its semantics")
    for name in GENERATED:
        if re.search(r"\b(sorry|admit|axiom)\b", (directory / name).read_text()):
            raise ValueError(f"unfinished or axiomatic account extraction: {name}")
    functions = (directory / "Funs.lean").read_text()
    for entry in (*ENTRY_CALLS, *PUBLIC_DEFINITIONS, *INSPECTION_DEFINITIONS, *CONVENIENCE_DEFINITIONS,
                  *DERIVED_DEFINITIONS, *INVARIANT_INPUT_DEFINITIONS, "account.Account.prepare_exchange",
                  "account.Account.Insts.CoreCloneClone.clone",
                  "basket.construct_entries", "basket.construct_entries_loop.body",
                  "basket.Basket.try_from_entries", "basket.Basket.try_from_amounts",
                  "basket.AmountEntries.Insts.CoreIterTraitsIteratorIteratorPairAQuantity.next",
                  "basket.Basket.Insts.CoreIterTraitsCollectFromIteratorPairAQuantity.from_iter",
                  "basket.Basket.Insts.CoreConvertFromArrayPairAQuantityLEN.from",
                  "basket.Basket.get", "basket.Basket.iter", "basket.Basket.new", "basket.Basket.len",
                  "basket.Basket.is_empty", "basket.Basket.Insts.CoreDefaultDefault.default",
                  "basket.sorted_entries", "basket.Basket.iter_sorted",
                  "account.Account.deposit", "account.Account.withdraw",
                  "account.Account.deposit_loop.body", "account.Account.withdraw_loop.body",
                  "basket.Basket.shortfall", "basket.Basket.shortfall_loop.body",
                  "basket.Basket.insert", "basket.Basket.checked_add", "basket.Basket.checked_add_loop.body",
                  "basket.Basket.checked_scale", "basket.Basket.checked_scale_loop.body", "basket.Basket.contains",
                  "basket.Basket.Insts.CoreCmpPartialEqBasket.eq", "basket.Basket.Insts.CoreCmpPartialEqBasket.eq_loop.body",
                  "quantity.Quantity.checked_add", "quantity.Quantity.checked_sub", "quantity.Quantity.checked_mul",
                  "economy.EconomyBuilder.new", "economy.EconomyBuilder.account", "economy.EconomyBuilder.rate",
                  "economy.EconomyBuilder.invariant", "economy.EconomyBuilder.build", "economy.EconomyBuilder.build_loop.body",
                  "economy.Economy.validate_structure",
                  *[f"economy.Economy.validate_structure_loop{i}.body" for i in range(7)],
                  "economy.Economy.analyze", "economy.Economy.assess", "economy.Economy.apply",
                  "economy.Economy.fork", "economy.Economy.simulate", "economy.Economy.replay",
                  "economy.Economy.replay_loop.body", "economy.Economy.replayed",
                  "economy.Economy.Insts.CoreCloneClone.clone", "exchange.Exchange.Insts.CoreCloneClone.clone",
                  "exchange.Trace.new", "exchange.Trace.push", "exchange.Trace.impl.exchanges", "exchange.Trace.into_exchanges",
                  "exchange.Trace.extend", "exchange.Trace.extend_loop.body",
                  "economy.Goal.new", "economy.Goal.require", "economy.Goal.requirements",
                  "economy.Goal.collect_assets_loop.body", "economy.Goal.asset_keys_loop.body", "economy.Goal.asset_keys",
                  "economy.Economy.account", "economy.Economy.balance", "economy.Economy.matches", "economy.Economy.matches_loop.body",
                  "economy.Economy.view", "economy.Economy.view_loop.body", "economy.Economy.state_key",
                  "economy.Economy.state_key_loop0.body", "economy.Economy.state_key_loop0_loop0.body",
                  "economy.Economy.observation_key_for", "economy.Economy.observation_key_for_loop0.body",
                  "economy.Economy.observation_key_for_loop0_loop0.body", "economy.EconomicView.account",
                  "economy.EconomicView.balance", "economy.EconomicView.observation_key",
                  "economy.ObservationKey.impl.visible_accounts", "economy.ObservationKey.impl.existing_accounts",
                  "economy.ObservationKey.impl.balances", "economy.Economy.state_fingerprint", "economy.StateFingerprint.get",
                  "economy.FingerprintHasher.new", "economy.FingerprintHasher.Insts.CoreHashHasher.write",
                  "economy.FingerprintHasher.Insts.CoreHashHasher.write_loop.body", "economy.FingerprintHasher.Insts.CoreHashHasher.finish",
                  "economy.Economy.is_applicable", "economy.Economy.applicable", "economy.ExchangeAssessment.is_applicable",
                  "exchange.Receipt.new", "economy.Effect.merge_computed", "economy.relevant_balances",
                  *[f"economy.Economy.{stage}{suffix}" for stage in (
                      "merge_computed_effects", "aggregate_effects", "check_conditions", "preflight_effects",
                      "prepare_effects", "accounts_with_prepared", "check_invariants", "apply", "applicable")
                    for suffix in ("_loop", "_loop.body")],
                  *[f"economy.Economy.{stage}" for stage in (
                      "merge_computed_effects", "aggregate_effects", "check_conditions", "preflight_effects",
                      "prepare_effects", "accounts_with_prepared", "check_invariants")],
                  "rate.Rate.has_role", "rate.Rate.has_role_loop.body",
                  "rate.Rate.new", "rate.Rate.impl.distinct", "rate.Rate.register_expression_roles_loop.body",
                  "rate.Rate.consume_computed", "rate.Rate.produce_computed", "rate.Rate.preserve_computed",
                  "rate.Rate.condition", "rate.Rate.condition_loop.body",
                  "rate.QuantityExpression.collect_roles", "rate.QuantityExpression.collect_parameters", "rate.QuantityExpression.collect_assets",
                  "rate.ComputedAmount.collect_parameters_loop.body", "rate.ComputedAmount.collect_assets_loop.body",
                  "rate.Rate.parameter_names", "rate.Rate.parameter_names_loop.body",
                  "rate.Rate.computed_asset_keys", "rate.Rate.computed_asset_keys_loop.body",
                  "rate.merge", "rate.Rate.try_consume", "rate.Rate.try_produce", "rate.Rate.try_preserve",
                  "rate.LinearInvariant.checked_add_basket_measure_loop.body",
                  "rate.LinearInvariant.conserves_loop.body", "rate.LinearInvariant.conserves",
                  "rate.LinearInvariant.measure_loop.body", "rate.LinearInvariant.measure",
                  "economy.evaluate_expression", "economy.evaluate_binary", "economy.evaluate_condition",
                  "economy.merge_scaled", "economy.merge_quantity", "economy.ExpressionError.into_message",
                  "U64.Insts.Account_extractionQuantityQuantityScalarI128.checked_weighted"):
        if not re.search(r"^def\s+" + re.escape(entry) + r"\s", functions, re.MULTILINE):
            raise ValueError(f"missing account production definition: {entry}")


def compare_generated(directory, lean=LEAN, write=False):
    for name, relative in GENERATED.items():
        actual = (directory / name).read_bytes()
        target = lean / relative
        if write:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(actual)
        elif not target.is_file() or actual != target.read_bytes():
            raise ValueError(f"stale account model: {relative}; run python3 scripts/extract_account.py --write and review/reprove")


def check_extraction(write=False):
    validate_bridge()
    aeneas, charon = verify_tools()
    WORK.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(WORK / "cargo-target")
    environment["CARGO_INCREMENTAL"] = "0"
    toolchain = "+" + PIN["charon"]["rust_toolchain"]
    rustc = subprocess.check_output(["rustc", toolchain, "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    # The reviewed integer/length-prefix Hash adapters model native 64-bit
    # little-endian writes. Make the extraction target explicit and fail closed.
    environment["CARGO_BUILD_TARGET"] = host
    cfg = subprocess.check_output(["rustc", toolchain, "--print", "cfg", "--target", host], text=True)
    validate_fingerprint_target(cfg)
    subprocess.run(["cargo", toolchain, "clean", "--target", host, "-p", "axionomy"],
                   cwd=ROOT, env=environment, check=True)
    with tempfile.TemporaryDirectory(prefix="account-extraction-", dir=WORK) as scratch:
        scratch = Path(scratch)
        llbc = scratch / "account_extraction.llbc"
        generated = scratch / "generated"
        generated.mkdir()
        command = [str(charon), "cargo", "--preset", "aeneas", "--monomorphize-mut",
                   "--resugar-drops", "--rustc-arg=-Zinline-mir=no",
                   *[arg for entry in ENTRY_CALLS for arg in
                     ("--start-from-if-exists", "account_extraction::" + entry)],
                   "--mir", "promoted", "--sysroot", "default",
                   "--include", "account_extraction::account", "--include", "account_extraction::basket",
                   "--include", "account_extraction::quantity", "--include", "account_extraction::amount",
                   "--include", "account_extraction::rate", "--include", "account_extraction::exchange",
                   "--include", "account_extraction::economy",
                   "--include", "num_traits", "--opaque", "indexmap",
                   "--opaque", "account_extraction::basket::retain_quantities",
                   "--dest-file", str(llbc), "--", "--locked", "-p", "axionomy",
                   "--example", "account_extraction", "--no-default-features"]
        print("Extracting the production account updates...", flush=True)
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
        if not llbc.is_file():
            raise ValueError("Charon did not emit fresh account LLBC")
        extraction = subprocess.run([str(aeneas), "-filter-trait-methods", "-backend", "lean", "-namespace", "Axionomy.AccountExtracted",
                        "-dest", str(generated), "-use-lean-modules", "false", "-split-files",
                        "-gen-lib-entry", "-no-progress-bar", str(llbc)],
                       cwd=ROOT, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(extraction.stdout, end="", flush=True)
        extraction.check_returncode()
        validate_extraction_diagnostics(extraction.stdout)
        validate_generated(generated)
        compare_generated(generated, write=write)
    print("Account extraction updated; review and recheck proofs." if write else
          "Production account extraction matches the committed model.", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    try:
        check_extraction(write=args.write)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Account extraction failed: {error}", file=sys.stderr)
        sys.exit(1)
