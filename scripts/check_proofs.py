#!/usr/bin/env python3
"""Check coverage, fresh Rust extraction, kernel proofs and Rust conformance.

Source hashes record review freshness. They do NOT establish Rust refinement.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from extract_quantity import check_extraction
from extract_basket import check_extraction as check_basket_extraction
from extract_account import check_extraction as check_account_extraction

ROOT = Path(__file__).resolve().parents[1]


def validate_coverage(root, coverage):
    if coverage.get("version") != 1:
        raise ValueError("unsupported proof coverage version")
    sources = {"Cargo.toml", "Cargo.lock", "verification/extract.rs", "verification/extract_basket.rs",
               "verification/extract_account.rs",
               "verification/patches/aeneas-namespaced-discriminant.patch",
               "verification/aeneas-toolchain.json"} | {
                   str(p.relative_to(root)) for p in (root / "src").rglob("*.rs")}
    recorded = coverage["sources"]
    if sources != set(recorded):
        raise ValueError(f"core source coverage differs: {sorted(sources ^ set(recorded))}")
    for source, expected in recorded.items():
        actual = hashlib.sha256((root / source).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"{source}: proof coverage review is stale (current SHA-256: {actual}). "
                             "Review the specification, proofs, Rust bridge and limitations before updating coverage.json.")
    features = coverage["features"]
    if not features:
        raise ValueError("no proof-bearing core features registered")
    covered = set()
    ids = set()
    theorems = set()
    for feature in features:
        if feature["id"] in ids:
            raise ValueError(f"duplicate feature: {feature['id']}")
        ids.add(feature["id"])
        if feature["status"] not in {"design_proved_rust_tested", "aeneas_refinement_proved"}:
            raise ValueError(f"unsupported verification status: {feature['status']}")
        if feature["status"] == "aeneas_refinement_proved":
            # These are the implemented extraction pipelines. Future
            # claims need their own regeneration and trust-boundary checks.
            pipelines = {
                "invariant_input_validation": ("AccountExtraction.Funs", {
                    "Axionomy.InvariantInput.construction_succeeds_iff",
                    "Axionomy.InvariantInput.zero_cannot_hide_duplicate",
                    "Axionomy.InvariantInput.successful_weights_valid",
                    "Axionomy.InvariantInputRefinement.construct_exact",
                    "Axionomy.InvariantInputRefinement.construct_refines",
                    "Axionomy.InvariantInputRefinement.duplicate_rejected",
                    "Axionomy.InvariantInputRefinement.accepted_fields",
                    "Axionomy.InvariantInputRefinement.accepted_valid",
                }),
                "inherited_traits_boundary": ("AccountExtraction.Funs", {
                    "Axionomy.InheritedCloneRefinement.quantity_clone_from",
                    "Axionomy.InheritedCloneRefinement.amount_clone_from",
                    "Axionomy.InheritedCloneRefinement.expression_clone_from",
                    "Axionomy.InheritedCloneRefinement.computed_amount_clone_from",
                    "Axionomy.InheritedCloneRefinement.condition_clone_from",
                    "Axionomy.InheritedCloneRefinement.comparison_clone_from",
                    "Axionomy.InheritedCloneRefinement.fingerprint_clone_from",
                    "Axionomy.InheritedCloneRefinement.assessment_status_clone_from",
                    "Axionomy.InheritedCloneRefinement.model_issue_clone_from",
                    "Axionomy.InheritedCloneRefinement.model_error_clone_from",
                    "Axionomy.InheritedCloneRefinement.basket_error_clone_from",
                    "Axionomy.InheritedCloneRefinement.rate_error_clone_from",
                    "Axionomy.InheritedCloneRefinement.quantity_error_clone_from",
                    "Axionomy.InheritedCloneRefinement.proposal_clone_from",
                    "Axionomy.InheritedCloneRefinement.trace_clone_from",
                    "Axionomy.InheritedCloneRefinement.observation_key_clone_from",
                    "Axionomy.InheritedCloneRefinement.basket_clone_from",
                    "Axionomy.InheritedCloneRefinement.account_clone_from",
                    "Axionomy.InheritedCloneRefinement.account_error_clone_from",
                    "Axionomy.InheritedCloneRefinement.delta_clone_from",
                    "Axionomy.InheritedCloneRefinement.receipt_clone_from",
                    "Axionomy.InheritedCloneRefinement.goal_clone_from",
                    "Axionomy.InheritedCloneRefinement.rate_clone_from",
                    "Axionomy.InheritedCloneRefinement.invariant_clone_from",
                    "Axionomy.InheritedCloneRefinement.economy_clone_from",
                    "Axionomy.InheritedCloneRefinement.account_assessment_clone_from",
                    "Axionomy.InheritedCloneRefinement.shortfall_clone_from",
                    "Axionomy.InheritedCloneRefinement.assessment_clone_from",
                    "Axionomy.InheritedCloneRefinement.apply_error_clone_from",
                    "Axionomy.InheritedEqualityRefinement.quantity_ne",
                    "Axionomy.InheritedEqualityRefinement.amount_ne",
                    "Axionomy.InheritedEqualityRefinement.expression_ne",
                    "Axionomy.InheritedEqualityRefinement.computed_amount_ne",
                    "Axionomy.InheritedEqualityRefinement.condition_ne",
                    "Axionomy.InheritedEqualityRefinement.comparison_ne",
                    "Axionomy.InheritedEqualityRefinement.fingerprint_ne",
                    "Axionomy.InheritedEqualityRefinement.assessment_status_ne",
                    "Axionomy.InheritedEqualityRefinement.model_issue_ne",
                    "Axionomy.InheritedEqualityRefinement.model_error_ne",
                    "Axionomy.InheritedEqualityRefinement.basket_error_ne",
                    "Axionomy.InheritedEqualityRefinement.rate_error_ne",
                    "Axionomy.InheritedEqualityRefinement.quantity_error_ne",
                    "Axionomy.InheritedEqualityRefinement.proposal_ne",
                    "Axionomy.InheritedEqualityRefinement.trace_ne",
                    "Axionomy.InheritedEqualityRefinement.observation_key_ne",
                    "Axionomy.InheritedEqualityRefinement.account_error_ne",
                    "Axionomy.InheritedEqualityRefinement.basket_ne",
                    "Axionomy.InheritedEqualityRefinement.account_ne",
                    "Axionomy.InheritedHashRefinement.hash_values",
                    "Axionomy.InheritedHashRefinement.quantity_hash_slice",
                    "Axionomy.InheritedHashRefinement.amount_hash_slice",
                    "Axionomy.InheritedHashRefinement.fingerprint_hash_slice",
                    "Axionomy.InheritedHashRefinement.observation_key_hash_slice",
                    "Axionomy.InheritedHashRefinement.empty",
                    "Axionomy.InheritedHashRefinement.append",
                    "Axionomy.InheritedHashRefinement.first_failure",
                    "Axionomy.InheritedHashRefinement.finite_success",
                    "Axionomy.InheritedHashRefinement.quantity_finite_success",
                    "Axionomy.InheritedOrderRefinement.quantity_min_effect",
                    "Axionomy.InheritedOrderRefinement.quantity_max_effect",
                    "Axionomy.InheritedOrderRefinement.quantity_clamp_effect",
                    "Axionomy.InheritedOrderRefinement.quantity_ties",
                    "Axionomy.InheritedOrderRefinement.quantity_incomparable",
                    "Axionomy.InheritedOrderRefinement.quantity_order_failure",
                    "Axionomy.InheritedOrderRefinement.quantity_clamp_invalid",
                    "Axionomy.InheritedOrderRefinement.quantity_clamp_range_failure",
                    "Axionomy.InheritedOrderRefinement.accepts_lt",
                    "Axionomy.InheritedOrderRefinement.accepts_le",
                    "Axionomy.InheritedOrderRefinement.accepts_gt",
                    "Axionomy.InheritedOrderRefinement.quantity_min",
                    "Axionomy.InheritedOrderRefinement.quantity_max",
                    "Axionomy.InheritedOrderRefinement.quantity_clamp",
                    "Axionomy.InheritedOrderRefinement.fingerprint_min_effect",
                    "Axionomy.InheritedOrderRefinement.fingerprint_max_effect",
                    "Axionomy.InheritedOrderRefinement.fingerprint_clamp_effect",
                    "Axionomy.InheritedOrderRefinement.fingerprint_min",
                    "Axionomy.InheritedOrderRefinement.fingerprint_max",
                    "Axionomy.InheritedOrderRefinement.fingerprint_clamp",
                }),
                "derived_traits_boundary": ("AccountExtraction.Funs", {
                    "Axionomy.DerivedAccountEqualityRefinement.option_eq",
                    "Axionomy.DerivedAccountEqualityRefinement.equal_body",
                    "Axionomy.DerivedAccountEqualityRefinement.equal_loop",
                    "Axionomy.DerivedAccountEqualityRefinement.basket_eq",
                    "Axionomy.DerivedAccountEqualityRefinement.account_eq",
                    "Axionomy.DerivedAccountEqualityRefinement.account_error_eq",
                    "Axionomy.DerivedCloneRefinement.basket_clone",
                    "Axionomy.DerivedCloneRefinement.account_clone",
                    "Axionomy.DerivedCloneRefinement.account_error_clone",
                    "Axionomy.DerivedCloneRefinement.delta_clone",
                    "Axionomy.DerivedCloneRefinement.receipt_clone",
                    "Axionomy.DerivedCloneRefinement.goal_clone",
                    "Axionomy.DerivedCloneRefinement.invariant_clone",
                    "Axionomy.DerivedCloneRefinement.economy_clone",
                    "Axionomy.DerivedCloneRefinement.account_assessment_clone",
                    "Axionomy.DerivedCloneRefinement.shortfall_clone",
                    "Axionomy.DerivedCloneRefinement.apply_error_clone",
                    "Axionomy.DerivedCloneRefinement.assessment_clone",
                    "Axionomy.DerivedCloneRefinement.rate_clone",
                    "Axionomy.DerivedCollectionRefinement.equal_entries",
                    "Axionomy.DerivedCollectionRefinement.map_eq",
                    "Axionomy.DerivedCollectionRefinement.zip_eq",
                    "Axionomy.DerivedCollectionRefinement.vec_eq",
                    "Axionomy.DerivedCollectionRefinement.vec_clone",
                    "Axionomy.DerivedCompositeRefinement.proposal_eq",
                    "Axionomy.DerivedCompositeRefinement.proposal_clone",
                    "Axionomy.DerivedCompositeRefinement.trace_eq",
                    "Axionomy.DerivedCompositeRefinement.trace_clone",
                    "Axionomy.DerivedCompositeRefinement.model_error_eq",
                    "Axionomy.DerivedCompositeRefinement.model_error_clone",
                    "Axionomy.DerivedCompositeRefinement.proposal_unequal_rate",
                    "Axionomy.DerivedCompositeRefinement.proposal_rate_comparison_failure",
                    "Axionomy.DerivedExpressionRefinement.clone_expression",
                    "Axionomy.DerivedExpressionRefinement.clone_public",
                    "Axionomy.DerivedExpressionRefinement.equal_expression",
                    "Axionomy.DerivedExpressionRefinement.equal_public",
                    "Axionomy.DerivedKeyRefinement.triple_eq",
                    "Axionomy.DerivedKeyRefinement.observation_key_eq",
                    "Axionomy.DerivedKeyRefinement.observation_key_clone",
                    "Axionomy.DerivedKeyRefinement.triple_unequal_first",
                    "Axionomy.DerivedKeyRefinement.triple_second_failure",
                    "Axionomy.DerivedKeyRefinement.fingerprint_eq",
                    "Axionomy.DerivedKeyRefinement.fingerprint_clone",
                    "Axionomy.DerivedKeyRefinement.fingerprint_cmp",
                    "Axionomy.DerivedKeyRefinement.fingerprint_partial_cmp",
                    "Axionomy.DerivedKeyRefinement.fingerprint_lt",
                    "Axionomy.DerivedKeyRefinement.fingerprint_le",
                    "Axionomy.DerivedKeyRefinement.fingerprint_gt",
                    "Axionomy.DerivedKeyRefinement.fingerprint_ge",
                    "Axionomy.DerivedKeyRefinement.fingerprint_hash",
                    "Axionomy.DerivedKeyRefinement.observation_key_hash_effect",
                    "Axionomy.DerivedKeyRefinement.balance_hash",
                    "Axionomy.DerivedKeyRefinement.observation_key_hash",
                    "Axionomy.DerivedRecordRefinement.comparison_eq",
                    "Axionomy.DerivedRecordRefinement.comparison_clone",
                    "Axionomy.DerivedRecordRefinement.assessment_status_eq",
                    "Axionomy.DerivedRecordRefinement.assessment_status_clone",
                    "Axionomy.DerivedRecordRefinement.quantity_error_eq",
                    "Axionomy.DerivedRecordRefinement.quantity_error_clone",
                    "Axionomy.DerivedRecordRefinement.basket_error_eq",
                    "Axionomy.DerivedRecordRefinement.basket_error_clone",
                    "Axionomy.DerivedRecordRefinement.rate_error_eq",
                    "Axionomy.DerivedRecordRefinement.rate_error_clone",
                    "Axionomy.DerivedRecordRefinement.model_issue_eq",
                    "Axionomy.DerivedRecordRefinement.model_issue_clone",
                    "Axionomy.DerivedRecordRefinement.computed_amount_clone",
                    "Axionomy.DerivedRecordRefinement.computed_amount_eq",
                    "Axionomy.DerivedRecordRefinement.condition_clone",
                    "Axionomy.DerivedRecordRefinement.condition_eq",
                    "Axionomy.DerivedValueRefinement.guard_ok",
                    "Axionomy.DerivedValueRefinement.quantity_eq_effect",
                    "Axionomy.DerivedValueRefinement.quantity_clone_effect",
                    "Axionomy.DerivedValueRefinement.quantity_partial_cmp_effect",
                    "Axionomy.DerivedValueRefinement.quantity_cmp_effect",
                    "Axionomy.DerivedValueRefinement.quantity_lt_effect",
                    "Axionomy.DerivedValueRefinement.quantity_le_effect",
                    "Axionomy.DerivedValueRefinement.quantity_gt_effect",
                    "Axionomy.DerivedValueRefinement.quantity_ge_effect",
                    "Axionomy.DerivedValueRefinement.quantity_hash_effect",
                    "Axionomy.DerivedValueRefinement.amount_eq_effect",
                    "Axionomy.DerivedValueRefinement.amount_eq",
                    "Axionomy.DerivedValueRefinement.amount_clone_effect",
                    "Axionomy.DerivedValueRefinement.amount_clone",
                    "Axionomy.DerivedValueRefinement.amount_hash_effect",
                    "Axionomy.DerivedValueRefinement.quantity_incomparable",
                    "Axionomy.DerivedValueRefinement.quantity_order_failure",
                    "Axionomy.DerivedValueRefinement.amount_unequal_asset",
                    "Axionomy.DerivedValueRefinement.amount_comparison_failure",
                    "Axionomy.DerivedValueRefinement.amount_clone_failure",
                    "Axionomy.DerivedValueRefinement.amount_hash_failure",
                }),
                "convenience_constructor_boundary": ("AccountExtraction.Funs", {
                    "Axionomy.ConvenienceRefinement.unwrap_panics_iff",
                    "Axionomy.ConvenienceRefinement.merge_overflow_naturals",
                    "Axionomy.ConvenienceRefinement.consume_refines",
                    "Axionomy.ConvenienceRefinement.produce_refines",
                    "Axionomy.ConvenienceRefinement.preserve_refines",
                    "Axionomy.ConvenienceRefinement.consume_panics_iff",
                    "Axionomy.ConvenienceRefinement.produce_panics_iff",
                    "Axionomy.ConvenienceRefinement.preserve_panics_iff",
                    "Axionomy.ConvenienceRefinement.consume_panics_naturals",
                    "Axionomy.ConvenienceRefinement.produce_panics_naturals",
                    "Axionomy.ConvenienceRefinement.preserve_panics_naturals",
                    "Axionomy.ConvenienceRefinement.map_identity",
                    "Axionomy.ConvenienceRefinement.array_identity",
                    "Axionomy.ConvenienceRefinement.literal_conversion",
                    "Axionomy.ConvenienceRefinement.literal_refines",
                    "Axionomy.ConvenienceRefinement.literal_panics_iff",
                    "Axionomy.CallbackRefinement.expression_effect",
                    "Axionomy.CallbackRefinement.expression_success",
                    "Axionomy.CallbackRefinement.expression_failure",
                    "Axionomy.CallbackRefinement.proposal_effect",
                    "Axionomy.CallbackRefinement.proposal_success",
                    "Axionomy.CallbackRefinement.proposal_failure",
                    "Axionomy.CallbackRefinement.invariant_effect",
                    "Axionomy.CallbackRefinement.condition_effect",
                    "Axionomy.CallbackRefinement.condition_success",
                    "Axionomy.CallbackRefinement.condition_failure",
                }),
                "core_inspection_boundary": ("AccountExtraction.Funs", {
                    "Axionomy.Inspection.first_none",
                    "Axionomy.Inspection.first_prefix",
                    "Axionomy.Inspection.first_member",
                    "Axionomy.Inspection.zero_removes",
                    "Axionomy.Inspection.other_weight_unchanged",
                    "Axionomy.Inspection.weights_valid",
                    "Axionomy.InspectionLookupRefinement.assessment_account",
                    "Axionomy.InspectionLookupRefinement.assessment_shortfall",
                    "Axionomy.InspectionLookupRefinement.account_first_match",
                    "Axionomy.InspectionLookupRefinement.shortfall_first_match",
                    "Axionomy.InspectionLookupRefinement.account_prefix",
                    "Axionomy.InspectionLookupRefinement.account_comparison_failure",
                    "Axionomy.InspectionLookupRefinement.shortfall_prefix",
                    "Axionomy.InspectionLookupRefinement.shortfall_comparison_failure",
                    "Axionomy.InspectionRefinement.iterator_length",
                    "Axionomy.InspectionRefinement.exact_hint",
                    "Axionomy.InspectionRefinement.account_next",
                    "Axionomy.InspectionRefinement.accounts_yield",
                    "Axionomy.InspectionRefinement.accounts_hint",
                    "Axionomy.InspectionRefinement.weight_next",
                    "Axionomy.InspectionRefinement.weights_yield",
                    "Axionomy.InspectionRefinement.weights_hint",
                    "Axionomy.InspectionRefinement.keys_yield",
                    "Axionomy.InspectionRefinement.invariant_new",
                    "Axionomy.InspectionRefinement.invariant_new_failure",
                    "Axionomy.InspectionRefinement.invariant_name",
                    "Axionomy.InspectionRefinement.invariant_weights",
                    "Axionomy.InspectionRefinement.invariant_weight",
                    "Axionomy.InspectionRefinement.invariant_weight_contract",
                    "Axionomy.InspectionRefinement.rate_roles",
                    "Axionomy.InspectionRefinement.rate_distinct",
                    "Axionomy.InspectionRefinement.rate_computed_consumed",
                    "Axionomy.InspectionRefinement.rate_computed_produced",
                    "Axionomy.InspectionRefinement.rate_computed_preserved",
                    "Axionomy.InspectionRefinement.rate_conditions",
                    "Axionomy.InspectionRefinement.rate_consumed",
                    "Axionomy.InspectionRefinement.rate_produced",
                    "Axionomy.InspectionRefinement.rate_preserved",
                    "Axionomy.InspectionRefinement.model_accounts",
                    "Axionomy.InspectionRefinement.model_rate_ids",
                    "Axionomy.InspectionRefinement.model_invariants",
                    "Axionomy.InspectionRefinement.model_rate",
                    "Axionomy.InspectionIteratorRefinement.slice_yields",
                    "Axionomy.InspectionIteratorRefinement.slice_hint",
                    "Axionomy.InspectionIteratorRefinement.weights_iterator",
                    "Axionomy.InspectionIteratorRefinement.weights_public_hint",
                    "Axionomy.InspectionIteratorRefinement.accounts_iterator",
                    "Axionomy.InspectionIteratorRefinement.accounts_public_next",
                    "Axionomy.InspectionIteratorRefinement.accounts_public_hint",
                    "Axionomy.InspectionIteratorRefinement.rate_ids_iterator",
                    "Axionomy.InspectionIteratorRefinement.rate_ids_public_next",
                    "Axionomy.InspectionIteratorRefinement.rate_ids_public_hint",
                    "Axionomy.InspectionIteratorRefinement.invariants_iterator",
                    "Axionomy.InspectionIteratorRefinement.invariants_public_next",
                    "Axionomy.InspectionIteratorRefinement.invariants_public_hint",
                    "Axionomy.InspectionAssetsRefinement.expression_assets",
                    "Axionomy.InspectionAssetsRefinement.computed_assets",
                    "Axionomy.InspectionCollectionRefinement.rate_assets",
                    "Axionomy.InspectionModelAssetsRefinement.model_assets",
                    "Axionomy.InspectionModelAssetsRefinement.model_assets_contract",
                }),
                "basket_sorted_iteration": ("AccountExtraction.Funs", {
                    "Axionomy.BasketOrdering.permutation",
                    "Axionomy.BasketOrdering.ordered",
                    "Axionomy.BasketOrdering.unique_keys",
                    "Axionomy.BasketOrdering.positive_contents",
                    "Axionomy.BasketOrdering.strictly_ordered",
                    "Axionomy.BasketOrdering.ties_stable",
                    "Axionomy.BasketOrdering.canonical",
                    "Axionomy.BasketSortedRefinement.collect_loop",
                    "Axionomy.BasketSortedRefinement.comparator_law",
                    "Axionomy.BasketSortedRefinement.sorted_refines",
                    "Axionomy.BasketSortedRefinement.sorted_contract",
                    "Axionomy.BasketSortedRefinement.canonical_contract",
                    "Axionomy.BasketSortedRefinement.constructed_sorted",
                }),
                "basket_collection_boundary": ("AccountExtraction.Funs", {
                    "Axionomy.BasketCollection.success_iff",
                    "Axionomy.BasketCollection.preserves_order",
                    "Axionomy.BasketCollection.nonzero_entries",
                    "Axionomy.BasketCollection.unique_entries",
                    "Axionomy.BasketCollection.zero_still_duplicates",
                    "Axionomy.BasketCollection.naturals",
                    "Axionomy.BasketCollectionRefinement.collected_panics_iff",
                    "Axionomy.BasketCollectionRefinement.loop_refines",
                    "Axionomy.BasketCollectionRefinement.reject_prefix",
                    "Axionomy.BasketCollectionRefinement.loop_failure",
                    "Axionomy.BasketCollectionRefinement.entries_refines",
                    "Axionomy.BasketCollectionRefinement.entries_contract",
                    "Axionomy.BasketCollectionRefinement.entries_reject_prefix",
                    "Axionomy.BasketCollectionRefinement.entries_failure",
                    "Axionomy.BasketCollectionRefinement.entries_start_failure",
                    "Axionomy.BasketCollectionRefinement.collect_refines",
                    "Axionomy.BasketCollectionRefinement.collect_duplicate_panics",
                    "Axionomy.BasketCollectionRefinement.collect_panics_iff",
                    "Axionomy.BasketCollectionRefinement.collect_failure",
                    "Axionomy.BasketCollectionRefinement.collect_start_failure",
                    "Axionomy.BasketAmountsRefinement.yields_parts",
                    "Axionomy.BasketAmountsRefinement.prefix_parts",
                    "Axionomy.BasketAmountsRefinement.failure_parts",
                    "Axionomy.BasketAmountsRefinement.amounts_refines",
                    "Axionomy.BasketAmountsRefinement.amounts_reject_prefix",
                    "Axionomy.BasketAmountsRefinement.amounts_failure",
                    "Axionomy.BasketAmountsRefinement.amounts_start_failure",
                    "Axionomy.BasketPublicRefinement.new_empty",
                    "Axionomy.BasketPublicRefinement.default_empty",
                    "Axionomy.BasketPublicRefinement.empty_refines",
                    "Axionomy.BasketPublicRefinement.length_refines",
                    "Axionomy.BasketPublicRefinement.lookup_refines",
                    "Axionomy.BasketPublicRefinement.iteration_refines",
                    "Axionomy.BasketPublicRefinement.array_yields",
                    "Axionomy.BasketPublicRefinement.array_refines",
                    "Axionomy.BasketPublicRefinement.array_panics_iff",
                    "Axionomy.BasketPublicRefinement.u64_zero_law",
                    "Axionomy.BasketPublicRefinement.u64_entries_refines",
                }),
                "iterator_consumers": ("AccountExtraction.Funs", {
                    "Axionomy.IteratorContract.yields_unique",
                    "Axionomy.IteratorContract.vector_yields",
                    "Axionomy.IteratorContract.scripted_yields",
                    "Axionomy.TraceExtensionRefinement.extend_loop",
                    "Axionomy.TraceExtensionRefinement.extend_refines",
                    "Axionomy.TraceExtensionRefinement.extend_loop_failure",
                    "Axionomy.TraceExtensionRefinement.extend_failure",
                    "Axionomy.TraceExtensionRefinement.extend_start_failure",
                    "Axionomy.IteratorViewRefinement.view_loop",
                    "Axionomy.IteratorViewRefinement.view_loop_failure",
                    "Axionomy.IteratorViewRefinement.model_view",
                    "Axionomy.IteratorViewRefinement.model_view_contract",
                    "Axionomy.IteratorViewRefinement.model_view_failure",
                    "Axionomy.IteratorViewRefinement.model_view_start_failure",
                    "Axionomy.IteratorFilteringRefinement.filter_loop",
                    "Axionomy.IteratorFilteringRefinement.filter_loop_failure",
                    "Axionomy.IteratorFilteringRefinement.applicable_refines",
                    "Axionomy.IteratorFilteringRefinement.applicable_failure",
                    "Axionomy.IteratorFilteringRefinement.applicable_start_failure",
                }),
                "typed_data_operations": ("AccountExtraction.Funs", {
                    "Axionomy.TypedDataRefinement.quantity_validation", "Axionomy.TypedDataRefinement.quantity_validation_iff",
                    "Axionomy.TypedDataRefinement.checked_validation", "Axionomy.TypedDataRefinement.quantity_new",
                    "Axionomy.TypedDataRefinement.quantity_get", "Axionomy.TypedDataRefinement.quantity_u64_roundtrip",
                    "Axionomy.TypedDataRefinement.quantity_from_u64", "Axionomy.TypedDataRefinement.quantity_from_u32",
                    "Axionomy.TypedDataRefinement.quantity_borrow", "Axionomy.TypedDataRefinement.quantity_consume",
                    "Axionomy.TypedDataRefinement.quantity_default", "Axionomy.TypedDataRefinement.amount_new",
                    "Axionomy.TypedDataRefinement.amount_from_pair", "Axionomy.TypedDataRefinement.amount_to_pair",
                    "Axionomy.TypedDataRefinement.amount_pair_roundtrip", "Axionomy.TypedDataRefinement.amount_value_roundtrip",
                    "Axionomy.TypedDataRefinement.account_new", "Axionomy.TypedDataRefinement.account_default",
                    "Axionomy.TypedDataRefinement.account_roundtrip", "Axionomy.TypedDataRefinement.account_equality",
                    "Axionomy.TypedDataRefinement.insert_amount", "Axionomy.TypedDataRefinement.proposal_new",
                    "Axionomy.TypedDataRefinement.proposal_bind", "Axionomy.TypedDataRefinement.proposal_parameter",
                    "Axionomy.TypedDataRefinement.assessment_accessors", "Axionomy.TypedDataRefinement.condition_name",
                    "Axionomy.TypedDataRefinement.amount_accessors", "Axionomy.TypedDataRefinement.account_accessors",
                    "Axionomy.TypedDataRefinement.delta_accessors", "Axionomy.TypedDataRefinement.receipt_accessors",
                    "Axionomy.TypedDataRefinement.account_assessment_accessors", "Axionomy.TypedDataRefinement.account_shortfall_accessors",
                    "Axionomy.TypedDataRefinement.model_error_accessors", "Axionomy.TypedDataRefinement.computed_accessors",
                    "Axionomy.TypedDataRefinement.condition_accessors", "Axionomy.TypedDataRefinement.proposal_accessors",
                }),
                "expression_construction": ("AccountExtraction.Funs", {
                    "Axionomy.ExpressionConstructionRefinement.constant", "Axionomy.ExpressionConstructionRefinement.units",
                    "Axionomy.ExpressionConstructionRefinement.balance", "Axionomy.ExpressionConstructionRefinement.parameter",
                    "Axionomy.ExpressionConstructionRefinement.add_operator", "Axionomy.ExpressionConstructionRefinement.construct_refines",
                    "Axionomy.ExpressionConstructionRefinement.decode_encode", "Axionomy.ExpressionConstructionRefinement.evaluate_constructed",
                    "Axionomy.ExpressionConstructionRefinement.plus", "Axionomy.ExpressionConstructionRefinement.subtract",
                    "Axionomy.ExpressionConstructionRefinement.multiply", "Axionomy.ExpressionConstructionRefinement.divide_floor",
                    "Axionomy.ExpressionConstructionRefinement.minimum", "Axionomy.ExpressionConstructionRefinement.maximum",
                }),
                "observations_and_goals": ("AccountExtraction.Funs", {
                    "Axionomy.GoalRefinement.new_goal", "Axionomy.GoalRefinement.default_goal",
                    "Axionomy.GoalRefinement.require_goal", "Axionomy.GoalRefinement.requirements",
                    "Axionomy.GoalRefinement.asset_keys", "Axionomy.GoalRefinement.matches_goal",
                    "Axionomy.ViewConstructionRefinement.model_view", "Axionomy.ObservationRefinement.view_account",
                    "Axionomy.ObservationRefinement.view_balance", "Axionomy.Observations.satisfies_iff",
                    "Axionomy.Observations.goal_monotone", "Axionomy.Observations.noninterference"}),
                "canonical_keys_and_fingerprints": ("AccountExtraction.Funs", {
                    "Axionomy.KeyRefinement.state_key", "Axionomy.KeyRefinement.state_key_contract",
                    "Axionomy.KeyRefinement.observation_key", "Axionomy.KeyRefinement.view_key",
                    "Axionomy.KeyRefinement.view_key_sound", "Axionomy.KeyRefinement.key_visible",
                    "Axionomy.KeyRefinement.key_existing", "Axionomy.KeyRefinement.key_balances",
                    "Axionomy.CanonicalKeys.noninterference", "Axionomy.CanonicalKeys.missing_distinguished",
                    "Axionomy.CanonicalKeys.queries_equal", "Axionomy.CanonicalKeys.reordered",
                    "Axionomy.CanonicalSort.sort_canonical", "Axionomy.CanonicalSort.slice_sort",
                    "Axionomy.KeyOrdering.triples_laws", "Axionomy.FingerprintRefinement.fingerprint_refines",
                    "Axionomy.FingerprintRefinement.run_value", "Axionomy.FingerprintRefinement.number_hash",
                    "Axionomy.FingerprintRefinement.fingerprint_get"}),
                "receipts_and_replay": ("AccountExtraction.Funs", {
                    "Axionomy.ExecutionRefinement.fork_refines",
                    "Axionomy.ExecutionRefinement.simulate_refines",
                    "Axionomy.ExecutionRefinement.replay_refines",
                    "Axionomy.ExecutionRefinement.replayed_refines",
                    "Axionomy.ExecutionRefinement.accepted_replay_contract",
                    "Axionomy.ExecutionRefinement.rejected_replay_prefix",
                    "Axionomy.ExecutionRefinement.replay_loop_refines",
                    "Axionomy.ExecutionRefinement.economy_clone",
                    "Axionomy.ExecutionRefinement.proposal_clone",
                    "Axionomy.TransactionExecution.replay_error_prefix",
                    "Axionomy.TransactionExecution.replay_receipt_order",
                    "Axionomy.TransactionExecution.accepted_changes",
                    "Axionomy.TransactionExecution.apply_conserves",
                    "Axionomy.TransactionExecution.replay_conserves"}),
                "complete_transactions": ("AccountExtraction.Funs", {
                    "Axionomy.AnalysisRefinement.analyze_refines",
                    "Axionomy.AnalysisRefinement.assess_refines",
                    "Axionomy.ApplyRefinement.apply_refines",
                    "Axionomy.ApplyRefinement.accepted_receipt_refines",
                    "Axionomy.Transaction.apply_receipt_changes",
                    "Axionomy.FilteringRefinement.is_applicable_refines",
                    "Axionomy.FilteringRefinement.applicable_refines",
                    "Axionomy.AggregationContract.validated_aggregate",
                    "Axionomy.AggregationContract.ready_preflight",
                    "Axionomy.AggregationContract.ready_prepare",
                    "Axionomy.Transaction.analyze_ready",
                    "Axionomy.OrderedEffects.aggregate_contract",
                    "Axionomy.RateWellFormed.new_registered",
                    "Axionomy.RateWellFormed.consume_registered",
                    "Axionomy.RateWellFormed.produce_registered",
                    "Axionomy.RateWellFormed.preserve_registered",
                    "Axionomy.RateWellFormed.static_consume_registered",
                    "Axionomy.RateWellFormed.static_produce_registered",
                    "Axionomy.RateWellFormed.static_preserve_registered",
                    "Axionomy.RateWellFormed.condition_registered",
                    "Axionomy.RateWellFormed.distinct_registered"}),
                'structural_validation': ("AccountExtraction.Funs", {
                    'Axionomy.ValidationCompleteRefinement.validate_refines',
                    'Axionomy.ValidationCompleteRefinement.accepted_has_accounts',
                    'Axionomy.ValidationCompleteRefinement.accepted_parameters_and_distinct',
                    'Axionomy.ValidationCompleteRefinement.rejected_has_issue'}),
                'model_construction': ("AccountExtraction.Funs", {
                    'Axionomy.ModelConstructionRefinement.new_refines',
                    'Axionomy.ModelConstructionRefinement.default_refines',
                    'Axionomy.ModelConstructionRefinement.account_refines',
                    'Axionomy.ModelConstructionRefinement.rate_refines',
                    'Axionomy.ModelConstructionRefinement.invariant_refines',
                    'Axionomy.ModelConstructionRefinement.new_establishes_valid',
                    'Axionomy.ModelConstructionRefinement.account_preserves_valid',
                    'Axionomy.ModelConstructionRefinement.rate_preserves_valid',
                    'Axionomy.ModelConstructionRefinement.invariant_preserves_valid',
                    'Axionomy.ModelConstructionRefinement.build_refines'}),
                'computed_rate_construction': ("AccountExtraction.Funs", {
                    'Axionomy.RateMetadataRefinement.new_refines',
                    'Axionomy.RateMetadataRefinement.default_refines',
                    'Axionomy.RateMetadataRefinement.consume_refines',
                    'Axionomy.RateMetadataRefinement.produce_refines',
                    'Axionomy.RateMetadataRefinement.preserve_refines',
                    'Axionomy.RateMetadataRefinement.condition_refines',
                    'Axionomy.RateMetadataRefinement.distinct_refines',
                    'Axionomy.RateMetadataRefinement.u64_less_equal_lawful'}),
                'rate_discovery': ("AccountExtraction.Funs", {
                    'Axionomy.RateDiscoveryRefinement.parameters_refines',
                    'Axionomy.RateDiscoveryRefinement.parameters_complete',
                    'Axionomy.RateDiscoveryRefinement.assets_refines'}),
                "snapshot_evaluation": ("AccountExtraction.Funs", {
                    "Axionomy.EvaluationRefinement.evaluate_refines_naturals",
                    "Axionomy.EvaluationRefinement.condition_refines_naturals",
                    "Axionomy.DiagnosticRefinement.message_refines"}),
                "bounded_effect_helpers": ("AccountExtraction.Funs", {
                    "Axionomy.EffectsRefinement.scaled_refines_naturals",
                    "Axionomy.EffectsRefinement.amount_refines_naturals"}),
                "static_rate_construction": ("AccountExtraction.Funs", {
                    f"Axionomy.RateConstructionRefinement.{kind}_{contract}"
                    for kind in ("consume", "produce", "preserve")
                    for contract in ("refines", "registers_and_frames")}),
                "runtime_linear_conservation": ("AccountExtraction.Funs", {
                    "Axionomy.ConservationRefinement.weighted_exact",
                    "Axionomy.ConservationRefinement.signed_add_exact",
                    "Axionomy.ConservationRefinement.conserves_refines_integers",
                    "Axionomy.ConservationRefinement.accepted_conserves",
                    "Axionomy.ConservationRefinement.measure_refines_integers"}),
                "u64_checked_arithmetic": ("QuantityExtraction.Funs", {
                    f"Axionomy.QuantityRefinement.checked_{op}_refines"
                    for op in ("add", "sub", "mul", "div")}),
                "basket_construction": ("BasketExtraction.Funs", {
                    "Axionomy.BasketRefinement.construct_refines",
                    "Axionomy.BasketRefinement.duplicate_rejected",
                    "Axionomy.BasketRefinement.exactEquality_lawful"}),
                "account_updates": ("AccountExtraction.Funs", {
                    "Axionomy.AccountRefinement.deposit_refines_naturals",
                    "Axionomy.AccountRefinement.withdraw_refines_naturals",
                    "Axionomy.AccountRefinement.shortfall_refines",
                    "Axionomy.AccountRefinement.exactClone_lawful"}),
                "account_exchange_preparation": ("AccountExtraction.Funs", {
                    "Axionomy.AccountRefinement.prepare_refines_naturals",
                    "Axionomy.Account.prepare_success_iff",
                    "Axionomy.Account.prepare_insufficient",
                    "Axionomy.Account.prepare_late_overflow"}),
                "basket_operations": ("AccountExtraction.Funs", {
                    "Axionomy.BasketOperationsRefinement.add_refines_naturals",
                    "Axionomy.BasketOperationsRefinement.scale_refines_naturals",
                    "Axionomy.BasketOperationsRefinement.contains_refines",
                    "Axionomy.BasketOperationsRefinement.equal_refines",
                    "Axionomy.BasketOperationsRefinement.equal_iff_permutation"}),
            }
            pipeline = pipelines.get(feature["id"])
            if pipeline is None or feature.get("extracted_module") != pipeline[0]:
                raise ValueError("refinement claim has no supported extraction pipeline")
            required = pipeline[1]
            if not required <= set(feature["theorems"]):
                raise ValueError(f"incomplete implementation refinement contract: {feature['id']}")
        if not feature["sources"] or not set(feature["sources"]) <= sources:
            raise ValueError(f"invalid source mapping: {feature['id']}")
        covered.update(feature["sources"])
        if not feature["theorems"] or not feature["rust_tests"] or not feature["limitations"]:
            raise ValueError(f"incomplete proof contract: {feature['id']}")
        for theorem in feature["theorems"]:
            if not re.fullmatch(r"Axionomy\.[A-Za-z0-9_.]+", theorem):
                raise ValueError(f"invalid theorem name: {theorem}")
            theorems.add(theorem)
        for test in feature["rust_tests"]:
            test_file = (root / test).resolve()
            if not test_file.is_relative_to(root.resolve()) or not test_file.is_file():
                raise ValueError(f"missing Rust evidence: {test}")
    if covered != sources:
        raise ValueError(f"core sources without a proof contract: {sorted(sources - covered)}")
    # Every spec module must be imported, so adding an unbuilt file cannot
    # silently evade the kernel audit. Keep the umbrella imports explicit.
    modules = {"Axionomy." + ".".join(p.relative_to(root / "verification/lean/Axionomy").with_suffix("").parts)
               for p in (root / "verification/lean/Axionomy").rglob("*.lean")}
    umbrella = (root / "verification/lean/Axionomy.lean").read_text()
    imports = set(re.findall(r"^import (Axionomy\.[\w.]+)$", umbrella, re.MULTILINE))
    if imports != modules:
        raise ValueError(f"unbuilt or missing proof modules: {sorted(imports ^ modules)}")
    return sorted(theorems)


def run(*command, cwd=ROOT, capture=False):
    print("+ " + " ".join(map(str, command)), flush=True)
    return subprocess.run(command, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def audit_source(theorems):
    return "import Audit\n#audit_axionomy\n" + "".join(
        f"#require_theorem {theorem}\n" for theorem in theorems)


def main():
    coverage = json.loads((ROOT / "verification/coverage.json").read_text())
    theorems = validate_coverage(ROOT, coverage)
    check_extraction()
    check_basket_extraction()
    check_account_extraction()
    lean_root = ROOT / "verification/lean"
    run("lake", "build", cwd=lean_root)
    run("lake", "env", "leanchecker", "Axionomy", cwd=lean_root)
    with tempfile.TemporaryDirectory(prefix="axionomy-proof-audit-") as scratch:
        audit = Path(scratch) / "CoverageAudit.lean"
        audit.write_text(audit_source(theorems))
        run("lake", "env", "lean", str(audit), cwd=lean_root)
    generated = json.loads(run(str(lean_root / ".lake/build/bin/conformance"), capture=True))
    committed = json.loads((ROOT / "verification/conformance.json").read_text())
    if generated != committed:
        raise ValueError("Lean conformance cases are stale; regenerate them from Conformance.lean and review the diff")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--test", "lean_conformance",
        "--test", "transaction_conformance", "--test", "observation_conformance", "--test", "public_data_conformance", "--test", "iterator_conformance", "--test", "basket_collection_conformance", "--test", "basket_sorted_conformance", "--test", "inspection_conformance", "--test", "convenience_conformance", "--test", "derived_conformance", "--test", "inherited_conformance", "--test", "invariant_input_conformance")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--lib",
        "runtime_")
    run("cargo", "test", "-p", "axionomy", "--all-features", "--doc")
    print(f"Verified {len(theorems)} registered theorems and their axiom dependencies, "
          "fresh Aeneas extraction of u64 arithmetic, basket construction/operations, account updates/staging, model/rate construction and metadata discovery, complete transaction assessment/application and finite-input iterator filtering, fork/simulation/ordered replay, goals/views/canonical keys and scoped fingerprints, typed data APIs, expression construction, generic trace extension and basket collection boundaries including sorted borrowed iteration, generic core inspection and complete asset collection, convenience constructor panics and string-conversion callbacks, generic derived equality/clone/order, inherited clone replacement/inequality/order helpers and scoped hash traversal contracts, snapshot evaluation/conditions, signed conservation and measurement, "
          "and Rust conformance/type checks. Collection storage uses an explicit trusted model. "
          "Other core implementation refinement remains unproved; see verification/README.md.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"proof check failed: {error}", file=sys.stderr)
        sys.exit(1)
