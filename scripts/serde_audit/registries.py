"""Reviewed exceptions and contract registries."""

from __future__ import annotations

from collections.abc import Sequence

from .models import CAPABILITIES, ExceptionEntry


def _exception(
    crate: str,
    path: str,
    type_names: Sequence[str],
    category: str,
    rationale: str,
    allowed_missing: frozenset[str] = frozenset({"Deserialize", "JsonSchema"}),
) -> tuple[ExceptionEntry, ...]:
    return tuple(
        ExceptionEntry(
            crate=crate,
            path=path,
            type_name=type_name,
            category=category,
            rationale=rationale,
            allowed_missing=allowed_missing,
        )
        for type_name in type_names
    )


ONE_WAY_EXCEPTIONS = (
    *_exception(
        "attribution",
        "src/return_contribution.rs",
        (
            "ReturnContributionResult",
            "InstrumentContribution",
            "GroupContribution",
            "FactorContribution",
            "BenchmarkRelativeContribution",
        ),
        "attribution-report",
        "Emitted attribution report rows. These gained `Deserialize` so the "
        "Python `ReturnContributionResult.from_json` round-trip works; they "
        "still carry no `JsonSchema` because the schema registry publishes the "
        "spec side, not the result side.",
        frozenset({"JsonSchema"}),
    ),
    *_exception(
        "models",
        "src/factor/risk/views.rs",
        (
            "PositionEsContributionView",
            "ParametricEsDecompositionView",
        ),
        "binding-view",
        "Round-trippable factor-risk output; no published JSON schema.",
        frozenset({"JsonSchema"}),
    ),
    *_exception(
        "models",
        "src/factor/risk/views.rs",
        (
            "PositionVarContributionView",
            "ParametricVarDecompositionView",
            "PositionBudgetEntryView",
            "RiskBudgetResultView",
        ),
        "binding-view",
        "Binding-oriented factor-risk view with no accepted inbound representation.",
    ),
    *_exception(
        "portfolio",
        "src/factor_model/weight_allocation.rs",
        ("WeightAllocationResult", "StrategyAllocation", "AllocationDiagnostics"),
        "allocation-output",
        "Computed allocation output; callers persist inputs and rerun allocation.",
        # These three gained `Deserialize`, so the exception no longer needs to
        # cover it. Narrowed rather than left broad: a blanket allowance would
        # stop the audit noticing if `Deserialize` were dropped again.
        allowed_missing=frozenset({"JsonSchema"}),
    ),
    *_exception(
        "portfolio",
        "src/scenarios.rs",
        ("ScenarioRevalueView", "ScenarioPnlView"),
        "scenario-view",
        "Scenario output view intentionally replacing persistence-implying Envelope names.",
    ),
    *_exception(
        "portfolio",
        "src/sensitivity/json.rs",
        ("SensitivityMatrixJson", "FactorPnlProfileJson"),
        "binding-view",
        "JSON projection for host bindings, not an inbound Rust contract.",
    ),
    *_exception(
        "statements-analytics",
        "src/analysis/reports.rs",
        ("CreditAssessmentPoint", "CreditAssessment"),
        "analysis-report",
        "Computed statement-analysis report; source statement results are canonical.",
        frozenset({"JsonSchema"}),
    ),
    *_exception(
        "calibration",
        "src/api/validate.rs",
        ("CalibrationValidationReport", "DependencyGraph", "DependencyNode"),
        "validation-report",
        "Transient calibration validation view, regenerated from the input envelope.",
        frozenset({"JsonSchema"}),
    ),
)


def _classification(
    crate: str,
    path: str,
    type_names: Sequence[str],
    category: str,
    rationale: str,
    allowed_missing: frozenset[str] = frozenset({"JsonSchema"}),
) -> tuple[ExceptionEntry, ...]:
    return _exception(
        crate,
        path,
        type_names,
        category,
        rationale,
        allowed_missing,
    )


def _computed_output(
    crate: str,
    path: str,
    type_names: Sequence[str],
) -> tuple[ExceptionEntry, ...]:
    return _classification(
        crate,
        path,
        type_names,
        "non-maintained-serde-output",
        "Computed output supports serde transport but is reproduced from canonical inputs; "
        "it is not a maintained/versioned persistence document.",
    )


def _in_process_spec(
    crate: str,
    path: str,
    type_names: Sequence[str],
) -> tuple[ExceptionEntry, ...]:
    return _classification(
        crate,
        path,
        type_names,
        "in-process-serde-spec",
        "Operation input is accepted by serde-powered adapters but has no maintained "
        "versioned-document or standalone-schema promise.",
    )


NON_MAINTAINED_SERDE_EXCEPTIONS = (
    *_computed_output("analytics", "src/benchmark.rs", ("BetaResult", "GreeksResult", "MultiFactorResult")),
    *_in_process_spec(
        "attribution",
        "src/return_contribution.rs",
        ("ReturnContributionSpec",),
    ),
    *_computed_output("models", "src/credit/pd/master_scale.rs", ("MasterScaleResult",)),
    *_classification(
        "models",
        "src/credit/registry.rs",
        ("CreditAssumptionRegistry",),
        "internal-registry-document",
        "Embedded credit-assumption registry is loaded through component-specific validation "
        "and is outside the maintained public persistence catalog.",
    ),
    *_computed_output("models", "src/credit/scoring/types.rs", ("ScoringResult",)),
    *_computed_output("core", "src/expr/ast.rs", ("EvaluationResult",)),
    *_in_process_spec("core", "src/market_data/bumps.rs", ("BumpSpec",)),
    *_computed_output(
        "core",
        "src/market_data/term_structures/base_correlation.rs",
        ("ArbitrageCheckResult",),
    ),
    *_computed_output("core", "src/money/fx/types.rs", ("FxRateResult",)),
    *_classification(
        "core",
        "src/rating_scales.rs",
        ("RatingScaleRegistry",),
        "internal-registry-document",
        "Embedded rating-scale registry uses component validation and is explicitly outside "
        "the maintained public persistence catalog.",
    ),
    *_in_process_spec(
        "covenants",
        "src/engine/types.rs",
        ("CovenantSpec",),
    ),
    *_computed_output("margin", "src/regulatory/frtb/types.rs", ("FrtbSbaResult",)),
    *_computed_output("margin", "src/regulatory/sa_ccr/types.rs", ("EadResult",)),
    *_computed_output("models", "src/monte_carlo/results.rs", ("MonteCarloResult",)),
    *_computed_output("portfolio", "src/brinson.rs", ("BrinsonPeriodResult",)),
    *_computed_output("portfolio", "src/excess_return.rs", ("ExcessReturnResult",)),
    *_computed_output("portfolio", "src/factor_brinson.rs", ("FactorBrinsonResult",)),
    *_computed_output("models", "src/factor/risk/budget.rs", ("RiskBudgetResult",)),
    *_in_process_spec(
        "portfolio",
        "src/factor_model/weight_allocation.rs",
        ("WeightAllocationSpec",),
    ),
    *_computed_output("portfolio", "src/factor_model/whatif.rs", ("WhatIfResult", "StressResult")),
    *_computed_output(
        "portfolio",
        "src/fi_attribution.rs",
        ("FiAttributionResult", "FiCarinoLinkedResult"),
    ),
    *_computed_output(
        "portfolio",
        "src/grid_attribution.rs",
        ("GridAttributionResult", "GridCarinoLinkedResult"),
    ),
    *_classification(
        "portfolio",
        "src/margin/results.rs",
        ("PortfolioMarginResult",),
        "non-maintained-serde-output",
        "Portfolio margin aggregate round-trips through its private wire adapter for host "
        "transport, but is not a versioned maintained result document.",
    ),
    *_in_process_spec("portfolio", "src/optimization/helpers.rs", ("PortfolioOptimizationSpec",)),
    *_in_process_spec("portfolio", "src/portfolio.rs", ("PortfolioSpec",)),
    *_in_process_spec("portfolio", "src/position.rs", ("PositionSpec",)),
    *_computed_output("portfolio", "src/replay.rs", ("ReplayResult",)),
    *_classification(
        "scenarios",
        "src/engine/types.rs",
        ("ApplicationEnvelope",),
        "in-process-execution-envelope",
        "Scenario application receipt is an immediate execution handoff, not a maintained "
        "persisted envelope from the contract catalog.",
    ),
    *_computed_output("scenarios", "src/horizon.rs", ("HorizonResult",)),
    *_computed_output("statements", "src/adjustments/types.rs", ("NormalizationResult",)),
    *_in_process_spec(
        "statements",
        "src/checks/suite.rs",
        ("CheckSuiteSpec", "BuiltinCheckSpec", "FormulaCheckSpec"),
    ),
    *_classification(
        "statements",
        "src/registry/schema.rs",
        ("MetricRegistry",),
        "internal-registry-document",
        "Embedded statement-metric registry has registry-specific validation and is outside "
        "the maintained public persistence catalog.",
    ),
    *_computed_output(
        "statements-analytics",
        "src/analysis/comps/scoring.rs",
        ("RelativeValueResult",),
    ),
    *_computed_output(
        "statements-analytics",
        "src/analysis/comps/stats.rs",
        ("RegressionResult",),
    ),
    *_computed_output("statements-analytics", "src/analysis/ecl/cecl.rs", ("CeclResult",)),
    *_computed_output(
        "statements-analytics",
        "src/analysis/ecl/engine.rs",
        ("EclResult", "WeightedEclResult", "ExposureEclResult"),
    ),
    *_computed_output(
        "statements-analytics",
        "src/analysis/ecl/portfolio.rs",
        ("PortfolioEclResult",),
    ),
    *_computed_output(
        "statements-analytics",
        "src/analysis/ecl/staging.rs",
        ("StageResult",),
    ),
    *_classification(
        "statements-analytics",
        "src/analysis/scenarios/types.rs",
        ("ParameterSpec",),
        "in-process-serde-spec",
        "Scenario parameter input is accepted by the analysis engine but is not a maintained versioned document.",
    ),
    *_computed_output(
        "statements-analytics",
        "src/analysis/scenarios/types.rs",
        ("SensitivityResult",),
    ),
    *_in_process_spec(
        "statements-analytics",
        "src/templates/real_estate/mod.rs",
        (
            "RentStepSpec",
            "FreeRentWindowSpec",
            "RenewalSpec",
            "LeaseSpec",
            "ManagementFeeSpec",
        ),
    ),
    *_computed_output(
        "valuations",
        "src/instruments/fixed_income/structured_credit/metrics/risk/oas.rs",
        ("OasResult",),
    ),
)

RESULT_ALIAS_EXCEPTIONS = tuple(
    entry
    for crate, path in (
        ("analytics", "src/correlation/mod.rs"),
        ("core", "src/error/mod.rs"),
        ("models", "src/correlation/error.rs"),
        ("portfolio", "src/error.rs"),
        ("scenarios", "src/error.rs"),
        ("statements", "src/error.rs"),
        ("valuations", "src/error.rs"),
    )
    for entry in _classification(
        crate,
        path,
        ("Result",),
        "generic-error-result-alias",
        "Generic alias to std::result::Result for crate error propagation; it is a type "
        "constructor, not a serializable DTO or persistence contract.",
        frozenset(CAPABILITIES),
    )
)


def _runtime_exception(
    crate: str,
    path: str,
    type_names: Sequence[str],
    category: str = "runtime-result",
) -> tuple[ExceptionEntry, ...]:
    return _exception(
        crate,
        path,
        type_names,
        category,
        "In-memory algorithm state or output; never accepted as a persisted wire contract.",
        frozenset(CAPABILITIES),
    )


# Runtime Result names are individually reviewed because their suffix alone cannot
# distinguish persisted DTOs from in-memory solver or algorithm state.
RUNTIME_RESULT_EXCEPTIONS = (
    *_runtime_exception(
        "models",
        "src/monte_carlo/greeks/gbm_european.rs",
        ("GbmEuropeanFdSpec",),
        "runtime-spec",
    ),
    *_runtime_exception("core", "src/math/linalg.rs", ("LedoitWolfResult",)),
    *_runtime_exception(
        "statements",
        "src/capital_structure/waterfall/mod.rs",
        ("WaterfallPeriodResult",),
    ),
    *_exception(
        "statements-analytics",
        "src/analysis/valuation/corporate.rs",
        ("CorporateValuationResult", "DcfSensitivityResult"),
        "runtime-result",
        "Round-trippable valuation output; no published JSON schema.",
        frozenset({"JsonSchema"}),
    ),
    *_exception(
        "statements-analytics",
        "src/analysis/valuation/lbo.rs",
        ("LboResult",),
        "runtime-result",
        "Round-trippable valuation output; no published JSON schema.",
        frozenset({"JsonSchema"}),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/credit_derivatives/cds_index/types.rs",
        ("ConstituentResult", "IndexResult", "IndexParSpreadResult"),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/bond/pricing/engine/merton_mc/types.rs",
        ("MertonMcResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/bond/pricing/ytm_solver.rs",
        ("YtmPricingSpec",),
        "runtime-spec",
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/cmo/waterfall.rs",
        ("CmoWaterfallPeriodResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/dollar_roll/carry.rs",
        ("CarryResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/revolving_credit/pricing/results.rs",
        ("PathResult", "EnhancedMonteCarloResult"),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/structured_credit/types/pool.rs",
        ("ConcentrationCheckResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/tba/allocation.rs",
        ("AllocationResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/rates/cms_swap/types.rs",
        ("FundingLegSpec",),
        "runtime-spec",
    ),
    *_runtime_exception("valuations", "src/metrics/risk/var_calculator.rs", ("VarResult",)),
    *_runtime_exception(
        "models",
        "src/trees/short_rate_tree/tree.rs",
        ("TreeCalibrationResult",),
    ),
)

REVIEWED_EXCEPTIONS = (
    *ONE_WAY_EXCEPTIONS,
    *NON_MAINTAINED_SERDE_EXCEPTIONS,
    *RESULT_ALIAS_EXCEPTIONS,
    *RUNTIME_RESULT_EXCEPTIONS,
)
MAINTAINED_CONTRACTS = frozenset({
    ("valuations", "src/instruments/json_loader.rs", "InstrumentEnvelope"),
    ("calibration", "src/api/schema.rs", "CalibrationEnvelope"),
    ("calibration", "src/api/schema.rs", "CalibrationResultEnvelope"),
    ("core", "src/market_data/context/state_serde.rs", "MarketContextState"),
    ("statements", "src/types/model.rs", "FinancialModelSpec"),
    ("scenarios", "src/envelope.rs", "ScenarioEnvelope"),
    ("factor-model", "src/envelope.rs", "FactorModelConfigEnvelope"),
    ("factor-model", "src/credit/hierarchy.rs", "CreditFactorModel"),
    ("portfolio", "src/materialization/envelope.rs", "PortfolioMaterializationEnvelope"),
    ("valuations", "src/results/valuation_result.rs", "ValuationResult"),
    ("statements", "src/evaluator/results.rs", "StatementResult"),
    ("portfolio", "src/results.rs", "PortfolioResult"),
    ("portfolio", "src/optimization/result.rs", "PortfolioOptimizationResult"),
})

# Required public binding outputs are not maintained persistence contracts:
# they need no version marker, strict loader, or contract-matrix entry. This
# registry keeps their public reachability and effective output schema
# fail-closed when module/re-export resolution changes.
REQUIRED_PUBLIC_TYPES = {
    (
        "valuations",
        "src/instruments/common_impl/cashflow_export.rs",
        "InstrumentCashflowEnvelope",
    ): CAPABILITIES,
}

MAINTAINED_REQUIRED_CAPABILITIES = {
    identity: (
        frozenset({"Serialize", "JsonSchema"})
        if identity
        == (
            "portfolio",
            "src/optimization/result.rs",
            "PortfolioOptimizationResult",
        )
        else CAPABILITIES
    )
    for identity in MAINTAINED_CONTRACTS
}

MAINTAINED_ONE_WAY_OUTPUTS = frozenset({
    (
        "portfolio",
        "src/optimization/result.rs",
        "PortfolioOptimizationResult",
    ),
})

ONE_WAY_OUTPUT_IDENTITIES = frozenset({
    *((entry.crate, entry.path, entry.type_name) for entry in ONE_WAY_EXCEPTIONS),
    *MAINTAINED_ONE_WAY_OUTPUTS,
})
